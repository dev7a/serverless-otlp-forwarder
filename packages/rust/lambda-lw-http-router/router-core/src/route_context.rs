use lambda_runtime::tracing::Span;
use opentelemetry::{Key as OtelKey, Value as OtelValue};
use std::collections::HashMap;
use std::sync::Arc;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Context passed to route handlers containing request information and application state.
///
/// This struct provides access to request details, path parameters, application state,
/// and Lambda execution context. It is passed to every route handler and provides
/// methods for accessing OpenTelemetry span attributes.
///
/// # Examples
///
/// ```rust
/// use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
/// use lambda_lw_http_router_core::{RouteContext, Router};
/// use serde_json::Value;
/// use lambda_runtime::Error;
/// use serde_json::json;
///
/// async fn handle_user(ctx: RouteContext<(), ApiGatewayV2httpRequest>) -> Result<Value, Error> {
///     // Get path parameters
///     let user_id = ctx.get_param("id").unwrap_or_else(||"default".to_string());
///     
///     // Access request details
///     let method = ctx.method();
///     let path = ctx.path();
///     
///     // Set span attributes
///     ctx.set_otel_attribute("user.id", user_id.clone());
///     
///     Ok(json!({ "id": user_id }))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RouteContext<State: Clone, E> {
    /// The full request path
    pub path: String,
    /// The HTTP method (GET, POST, etc.)
    pub method: String,
    /// Path parameters extracted from the URL (e.g., {id} -> "123")
    pub params: HashMap<String, String>,
    /// Application state shared across all requests
    pub state: Arc<State>,
    /// The original Lambda event
    pub event: E,
    /// Lambda execution context
    pub lambda_context: lambda_runtime::Context,
    /// The route template pattern (e.g., "/quote/{id}")
    pub route_pattern: String,
}

impl<State: Clone, E> RouteContext<State, E> {
    /// Returns the full request path of the current request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     let path = ctx.path();
    ///     assert_eq!(path, "/users/123/profile");
    /// }
    /// ```
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the HTTP method of the current request (e.g., "GET", "POST").
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     let method = ctx.method();
    ///     assert_eq!(method, "POST");
    /// }
    /// ```
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns a reference to the shared application state.
    ///
    /// The state is shared across all request handlers and can be used to store
    /// application-wide data like database connections or configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// #[derive(Clone)]
    /// struct AppState {
    ///     api_key: String,
    /// }
    ///
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     let api_key = &ctx.state().api_key;
    ///     // Use the API key for authentication
    /// }
    /// ```
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Returns a reference to the original Lambda event.
    ///
    /// This provides access to the raw event data from AWS Lambda, which can be useful
    /// for accessing event-specific fields not exposed through the router interface.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     let raw_event = ctx.event();
    ///     if let Some(body) = &raw_event.body {
    ///         // Process the raw request body
    ///     }
    /// }
    /// ```
    pub fn event(&self) -> &E {
        &self.event
    }

    /// Returns a reference to the Lambda execution context.
    ///
    /// The Lambda context contains metadata about the current execution environment,
    /// such as the request ID, function name, and remaining execution time.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     let request_id = &ctx.lambda_context().request_id;
    ///     let deadline = ctx.lambda_context().deadline;
    /// }
    /// ```
    pub fn lambda_context(&self) -> &lambda_runtime::Context {
        &self.lambda_context
    }

    /// Returns the route pattern that matched this request.
    ///
    /// The route pattern is the original path template with parameter placeholders,
    /// such as "/users/{id}/profile".
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     let pattern = ctx.route_pattern();
    ///     assert_eq!(pattern, "/users/{id}/profile");
    /// }
    /// ```
    pub fn route_pattern(&self) -> &str {
        &self.route_pattern
    }

    /// Returns a path parameter by name, if it exists.
    ///
    /// Path parameters are extracted from the URL based on the route pattern.
    /// For example, if the route pattern is "/users/{id}" and the URL is "/users/123",
    /// then `get_param("id")` will return `Some("123")`.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the path parameter to retrieve
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # use serde_json::json;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn get_user(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) -> serde_json::Value {
    ///     let user_id = ctx.get_param("id").unwrap_or_default();
    ///     json!({ "id": user_id })
    /// }
    /// ```
    pub fn get_param(&self, name: &str) -> Option<String> {
        self.params.get(name).cloned()
    }

    /// Returns a path parameter by name, or a default value if it doesn't exist.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # use serde_json::json;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn get_user(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) -> serde_json::Value {
    ///     let user_id = ctx.get_param_or("id", "default");
    ///     json!({ "id": user_id })
    /// }
    /// ```
    pub fn get_param_or(&self, name: &str, default: &str) -> String {
        self.get_param(name).unwrap_or_else(|| default.to_string())
    }

    /// Returns a path parameter by name, or an empty string if it doesn't exist.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # use serde_json::json;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn get_user(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) -> serde_json::Value {
    ///     let user_id = ctx.get_param_or_empty("id");
    ///     json!({ "id": user_id })
    /// }
    /// ```
    pub fn get_param_or_empty(&self, name: &str) -> String {
        self.get_param_or(name, "")
    }

    /// Returns a reference to all path parameters.
    ///
    /// This method returns a HashMap containing all path parameters extracted from
    /// the URL based on the route pattern.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # use serde_json::json;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) -> serde_json::Value {
    ///     let params = ctx.params();
    ///     json!({
    ///         "user_id": params.get("user_id"),
    ///         "post_id": params.get("post_id")
    ///     })
    /// }
    /// ```
    pub fn params(&self) -> &HashMap<String, String> {
        &self.params
    }

    /// Sets a single attribute on the current OpenTelemetry span.
    ///
    /// This method allows you to add custom attributes to the current span for
    /// better observability and tracing.
    ///
    /// # Arguments
    ///
    /// * `key` - The attribute key
    /// * `value` - The attribute value (supports strings, numbers, and booleans)
    ///
    /// # Returns
    ///
    /// Returns a reference to self for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     ctx.set_otel_attribute("user.id", "123")
    ///        .set_otel_attribute("request.size", 1024)
    ///        .set_otel_attribute("cache.hit", true);
    /// }
    /// ```
    pub fn set_otel_attribute(
        &self,
        key: impl Into<OtelKey>,
        value: impl Into<OtelValue>,
    ) -> &Self {
        let span = Span::current();
        span.set_attribute(key, value);
        self
    }

    /// Sets the OpenTelemetry span kind for the current span.
    ///
    /// The span kind describes the relationship between the span and its parent.
    /// Common values include:
    /// - "SERVER" for server-side request handling
    /// - "CLIENT" for outbound requests
    /// - "PRODUCER" for message publishing
    /// - "CONSUMER" for message processing
    /// - "INTERNAL" for internal operations
    ///
    /// # Arguments
    ///
    /// * `kind` - The span kind to set
    ///
    /// # Returns
    ///
    /// Returns a reference to self for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use lambda_lw_http_router_core::{RouteContext, Router};
    /// # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
    /// # #[derive(Clone)]
    /// # struct AppState {}
    /// async fn handler(ctx: RouteContext<AppState, ApiGatewayV2httpRequest>) {
    ///     ctx.set_otel_span_kind("SERVER")
    ///        .set_otel_attribute("request.id", "abc-123");
    /// }
    /// ```
    pub fn set_otel_span_kind(&self, kind: &str) -> &Self {
        let span = Span::current();
        span.record("otel.kind", kind);
        self
    }
}
