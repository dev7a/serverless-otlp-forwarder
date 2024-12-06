#![allow(clippy::type_complexity)]

//! Core functionality for the lambda-lw-http-router crate.
//!
//! **Note**: This is an implementation crate for [lambda-lw-http-router](https://crates.io/crates/lambda-lw-http-router)
//! and is not meant to be used directly. Please use the main crate instead.
//!
//! The functionality in this crate is re-exported by the main crate, and using it directly
//! may lead to version conflicts or other issues. Additionally, this crate's API is not
//! guaranteed to be stable between minor versions.
//!
//! # Usage
//!
//! Instead of using this crate directly, use the main crate:
//!
//! ```toml
//! [dependencies]
//! lambda-lw-http-router = "0.1"
//! ```
//!
//! See the [lambda-lw-http-router documentation](https://docs.rs/lambda-lw-http-router)
//! for more information on how to use the router.

pub use ctor;
mod routable_http_event;
mod route_context;
mod router;
pub use routable_http_event::RoutableHttpEvent;
pub use route_context::RouteContext;
pub use router::{register_route, Router, RouterBuilder};

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lambda_events::apigw::ApiGatewayProxyRequest;
    use aws_lambda_events::http::Method;
    use lambda_runtime::LambdaEvent;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Test event struct that implements RoutableHttpEvent
    #[derive(Clone)]
    struct TestHttpEvent {
        path: String,
        method: String,
    }

    impl RoutableHttpEvent for TestHttpEvent {
        fn path(&self) -> Option<String> {
            Some(self.path.clone())
        }

        fn http_method(&self) -> String {
            self.method.clone()
        }
    }

    /// Simple state struct for testing
    #[derive(Clone)]
    struct TestState {}

    #[tokio::test]
    async fn test_path_parameter_extraction() {
        let mut router = Router::<TestState, TestHttpEvent>::new();

        // Register a route with path parameters
        router.register_route("GET", "/users/{id}/posts/{post_id}", |ctx| async move {
            Ok(json!({
                "user_id": ctx.params.get("id"),
                "post_id": ctx.params.get("post_id"),
            }))
        });

        // Create a test event
        let event = TestHttpEvent {
            path: "/users/123/posts/456".to_string(),
            method: "GET".to_string(),
        };
        let lambda_context = lambda_runtime::Context::default();
        let lambda_event = LambdaEvent::new(event, lambda_context);

        // Handle the request
        let result = router
            .handle_request(lambda_event, Arc::new(TestState {}))
            .await
            .unwrap();

        // Verify the extracted parameters
        assert_eq!(result["user_id"], "123");
        assert_eq!(result["post_id"], "456");
    }

    #[tokio::test]
    async fn test_greedy_path_parameter() {
        let mut router = Router::<TestState, TestHttpEvent>::new();

        // Register a route with a greedy path parameter
        router.register_route("GET", "/files/{path+}", |ctx| async move {
            Ok(json!({
                "path": ctx.params.get("path"),
            }))
        });

        // Create a test event with a nested path
        let event = TestHttpEvent {
            path: "/files/documents/2024/report.pdf".to_string(),
            method: "GET".to_string(),
        };
        let lambda_context = lambda_runtime::Context::default();
        let lambda_event = LambdaEvent::new(event, lambda_context);

        // Handle the request
        let result = router
            .handle_request(lambda_event, Arc::new(TestState {}))
            .await
            .unwrap();

        // Verify the extracted parameter captures the full path
        assert_eq!(result["path"], "documents/2024/report.pdf");
    }

    #[tokio::test]
    async fn test_no_match_returns_404() {
        let router = Router::<TestState, TestHttpEvent>::new();

        // Create a test event with a path that doesn't match any routes
        let event = TestHttpEvent {
            path: "/nonexistent".to_string(),
            method: "GET".to_string(),
        };
        let lambda_context = lambda_runtime::Context::default();
        let lambda_event = LambdaEvent::new(event, lambda_context);

        // Handle the request
        let result = router
            .handle_request(lambda_event, Arc::new(TestState {}))
            .await
            .unwrap();

        // Verify we get a 404 response
        assert_eq!(result["statusCode"], 404);
    }

    #[tokio::test]
    async fn test_apigw_resource_path_parameters() {
        let mut router = Router::<TestState, ApiGatewayProxyRequest>::new();

        router.register_route("GET", "/users/{id}/posts/{post_id}", |ctx| async move {
            Ok(json!({
                "params": ctx.params,
            }))
        });

        let mut path_parameters = HashMap::new();
        path_parameters.insert("id".to_string(), "123".to_string());
        path_parameters.insert("post_id".to_string(), "456".to_string());

        let event = ApiGatewayProxyRequest {
            path: Some("/users/123/posts/456".to_string()),
            http_method: Method::GET,
            resource: Some("/users/{id}/posts/{post_id}".to_string()),
            path_parameters,
            ..Default::default()
        };

        let lambda_context = lambda_runtime::Context::default();
        let lambda_event = LambdaEvent::new(event, lambda_context);

        let result = router
            .handle_request(lambda_event, Arc::new(TestState {}))
            .await
            .unwrap();

        assert_eq!(result["params"]["id"], "123");
        assert_eq!(result["params"]["post_id"], "456");
    }

    #[tokio::test]
    async fn test_method_matching_with_apigw() {
        let mut router = Router::<TestState, ApiGatewayProxyRequest>::new();

        // Register both GET and POST handlers for the same path
        router.register_route("GET", "/quotes", |_| async move {
            Ok(json!({ "method": "GET" }))
        });

        router.register_route("POST", "/quotes", |_| async move {
            Ok(json!({ "method": "POST" }))
        });

        // Create a POST request
        let post_event = ApiGatewayProxyRequest {
            path: Some("/quotes".to_string()),
            http_method: Method::POST,
            resource: Some("/quotes".to_string()),
            path_parameters: HashMap::new(),
            ..Default::default()
        };

        let lambda_context = lambda_runtime::Context::default();
        let lambda_event = LambdaEvent::new(post_event, lambda_context);

        // Handle the POST request
        let result = router
            .handle_request(lambda_event, Arc::new(TestState {}))
            .await
            .unwrap();
        assert_eq!(
            result["method"], "POST",
            "POST request should be handled by POST handler"
        );

        // Create a GET request to the same path
        let get_event = ApiGatewayProxyRequest {
            path: Some("/quotes".to_string()),
            http_method: Method::GET,
            resource: Some("/quotes".to_string()),
            path_parameters: HashMap::new(),
            ..Default::default()
        };

        let lambda_context = lambda_runtime::Context::default();
        let lambda_event = LambdaEvent::new(get_event, lambda_context);

        // Handle the GET request
        let result = router
            .handle_request(lambda_event, Arc::new(TestState {}))
            .await
            .unwrap();
        assert_eq!(
            result["method"], "GET",
            "GET request should be handled by GET handler"
        );
    }
}
