//! Lambda LightWeight HTTP Router (lambda-lw-http-router) is a lightweight routing library for AWS Lambda HTTP events.
//!
//! It provides a simple and efficient way to define routes and handlers for AWS Lambda functions
//! that process HTTP events from API Gateway, Application Load Balancer, and WebSocket APIs.
//!
//! # Features
//!
//! * Support for multiple AWS event types (API Gateway v2, v1, ALB, WebSocket)
//! * Path parameter extraction
//! * Type-safe route handlers
//! * Compile-time route registration
//! * Minimal runtime overhead
//!
//! # Quick Start
//!
//! ```rust
//! # use lambda_lw_http_router::{define_router, route};
//! # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
//! # use serde_json::{json, Value};
//! # use lambda_runtime::Error;
//! #
//! // Define your application state
//! #[derive(Clone)]
//! struct AppState {
//!     // your state fields here
//! }
//!
//! // Set up the router
//! define_router!(event = ApiGatewayV2httpRequest, state = AppState);
//!
//! // Define a route handler
//! #[route(path = "/hello/{name}")]
//! async fn handle_hello(ctx: RouteContext) -> Result<Value, Error> {
//!     let name = ctx.params.get("name").map(|s| s.as_str()).unwrap_or("World");
//!     Ok(json!({
//!         "message": format!("Hello, {}!", name)
//!     }))
//! }
//!
//! # fn main() {}
//! ```
//!
//! # Examples
//!
//! Basic usage with default module name:
//!
//! ```rust
//! # use lambda_lw_http_router::define_router;
//! # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
//! # use serde_json::{json, Value};
//! # use lambda_runtime::Error;
//! #
//! #[derive(Clone)]
//! struct AppState {
//!     // your state fields here
//! }
//!
//! define_router!(event = ApiGatewayV2httpRequest, state = AppState);
//!
//! // This creates a module with the following types:
//! // Router - Router<AppState, ApiGatewayV2httpRequest>
//! // RouterBuilder - RouterBuilder<AppState, ApiGatewayV2httpRequest>
//! // RouteContext - RouteContext<AppState, ApiGatewayV2httpRequest>
//! # fn main() {}
//! ```
//!
//! Custom module name for better readability or multiple routers:
//!
//! ```rust
//! # use lambda_lw_http_router::define_router;
//! # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
//! # use aws_lambda_events::alb::AlbTargetGroupRequest;
//! # use serde_json::{json, Value};
//! # use lambda_runtime::Error;
//! #
//! #[derive(Clone)]
//! struct AppState {
//!     // your state fields here
//! }
//!
//! // Define an API Gateway router
//! define_router!(event = ApiGatewayV2httpRequest, module = api_router, state = AppState);
//!
//! // Define an ALB router in the same application
//! define_router!(event = AlbTargetGroupRequest, module = alb_router, state = AppState);
//!
//! // Now you can use specific types for each router:
//! // api_router::Router
//! // api_router::RouterBuilder
//! // api_router::RouteContext
//! //
//! // alb_router::Router
//! // alb_router::RouterBuilder
//! // alb_router::RouteContext
//! # fn main() {}
//! ```
//!
//! # Usage with Route Handlers
//!
//! The module name defined here should match the `module` parameter in your route handlers:
//!
//! ```rust
//! # use lambda_lw_http_router::{define_router, route};
//! # use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
//! # use serde_json::{json, Value};
//! # use lambda_runtime::Error;
//! # #[derive(Clone)]
//! # struct AppState { }
//! #
//! define_router!(event = ApiGatewayV2httpRequest, module = api_router, state = AppState);
//!
//! #[route(path = "/hello", module = "api_router")]
//! async fn handle_hello(ctx: api_router::RouteContext) -> Result<Value, Error> {
//!     Ok(json!({ "message": "Hello, World!" }))
//! }
//! # fn main() {}
//! ```

pub use lambda_lw_http_router_core::*;
pub use lambda_lw_http_router_macro::route;

/// Defines a router module with the necessary type aliases for your Lambda application.
///
/// This macro creates a module containing type aliases for the router components,
/// making them easily accessible throughout your application. It's typically used
/// at the beginning of your Lambda function to set up the routing infrastructure.
///
/// # Type Aliases
///
/// The macro creates the following type aliases:
/// * `Event` - The Lambda HTTP event type (e.g., ApiGatewayV2httpRequest)
/// * `Router` - The router instance type for your application
/// * `RouterBuilder` - The builder type for constructing routers
/// * `RouteContext` - The context type passed to route handlers
///
/// # Arguments
///
/// * `event` - The Lambda HTTP event type (required). Supported types:
///   * `ApiGatewayV2httpRequest` - API Gateway HTTP API v2
///   * `ApiGatewayProxyRequest` - API Gateway REST API v1
///   * `AlbTargetGroupRequest` - Application Load Balancer
///   * `ApiGatewayWebsocketProxyRequest` - API Gateway WebSocket
/// * `module` - The module name (optional, defaults to an internal name)
/// * `state` - The state type for the router
///
/// # Examples
///
/// Basic usage with default module name:
/// ```rust,no_run
/// use lambda_lw_http_router::define_router;
/// use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
/// use serde_json::{json, Value};
/// use lambda_runtime::Error;
///
/// // Define your application state
/// #[derive(Clone)]
/// struct AppState {
///     // your state fields here
/// }
///
/// define_router!(event = ApiGatewayV2httpRequest, state = AppState);
///
/// // This creates a module with the following types:
/// // Router - Router<AppState, ApiGatewayV2httpRequest>
/// // RouterBuilder - RouterBuilder<AppState, ApiGatewayV2httpRequest>
/// // RouteContext - RouteContext<AppState, ApiGatewayV2httpRequest>
/// # fn main() {}
/// ```
///
/// Custom module name for better readability or multiple routers:
/// ```rust,no_run
/// use lambda_lw_http_router::define_router;
/// use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
/// use aws_lambda_events::alb::AlbTargetGroupRequest;
/// use serde_json::{json, Value};
/// use lambda_runtime::Error;
///
/// // Define your application state
/// #[derive(Clone)]
/// struct AppState {
///     // your state fields here
/// }
///
/// // Define an API Gateway router
/// define_router!(event = ApiGatewayV2httpRequest, module = api_router, state = AppState);
///
/// // Define an ALB router in the same application
/// define_router!(event = AlbTargetGroupRequest, module = alb_router, state = AppState);
///
/// // Now you can use specific types for each router:
/// // api_router::Router
/// // api_router::RouterBuilder
/// // api_router::RouteContext
/// //
/// // alb_router::Router
/// // alb_router::RouterBuilder
/// // alb_router::RouteContext
/// # fn main() {}
/// ```
///
/// # Usage with Route Handlers
///
/// The module name defined here should match the `module` parameter in your route handlers:
///
/// ```rust, no_run
/// use lambda_lw_http_router::{define_router, route};
/// use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
/// use serde_json::{json, Value};
/// use lambda_runtime::Error;
///
/// // Define your application state
/// #[derive(Clone)]
/// struct AppState {
///     // your state fields here
/// }
///
/// define_router!(event = ApiGatewayV2httpRequest, module = api_router, state = AppState);
///
/// #[route(path = "/hello", module = "api_router")]
/// async fn handle_hello(ctx: api_router::RouteContext) -> Result<Value, Error> {
///     let name = ctx.params.get("name").map(|s| s.as_str()).unwrap_or("World");
///     Ok(json!({ "message": format!("Hello, {}!", name) }))
/// }
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! define_router {
    (event = $event_type:ty, module = $module:ident, state = $state_type:ty) => {
        pub mod $module {
            use super::*;

            pub type Event = $event_type;
            pub type State = $state_type;
            pub type Router = ::lambda_lw_http_router::Router<State, Event>;
            pub type RouterBuilder = ::lambda_lw_http_router::RouterBuilder<State, Event>;
            pub type RouteContext = ::lambda_lw_http_router::RouteContext<State, Event>;
        }
        pub use $module::*;
    };

    (event = $event_type:ty, state = $state_type:ty) => {
        mod __lambda_lw_http_router_core_default_router {
            use super::*;

            pub type Event = $event_type;
            pub type State = $state_type;
            pub type Router = ::lambda_lw_http_router::Router<State, Event>;
            pub type RouterBuilder = ::lambda_lw_http_router::RouterBuilder<State, Event>;
            pub type RouteContext = ::lambda_lw_http_router::RouteContext<State, Event>;
        }
        pub use __lambda_lw_http_router_core_default_router::*;
    };
}

#[cfg(doctest)]
extern crate doc_comment;

#[cfg(doctest)]
use doc_comment::doctest;

#[cfg(doctest)]
doctest!("../README.md", readme);
