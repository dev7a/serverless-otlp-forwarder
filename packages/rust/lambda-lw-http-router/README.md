# Lambda LightWeight HTTP Router

A lightweight, type-safe HTTP router for AWS Lambda functions with support for API Gateway, Application Load Balancer, and WebSocket APIs.

## Features

- 🚀 Zero runtime overhead with compile-time route registration
- 🔒 Type-safe route handlers and application state
- 🎯 Path parameter extraction
- 🔄 Support for multiple AWS event types:
  - API Gateway HTTP API (v2)
  - API Gateway REST API (v1)
  - Application Load Balancer
- 📊 OpenTelemetry integration for tracing
- 🏗️ Builder pattern for easy router construction
- 🧩 Modular design with separate core and macro crates

## Installation

Run `cargo add lambda-lw-http-router` to add the crate to your project or add the following to your `Cargo.toml`:

```toml
[dependencies]
lambda-lw-http-router = "0.1.1"
```

## Quick Start

```rust, no_run
use lambda_lw_http_router::{define_router, route};
use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
use serde_json::{json, Value};
use lambda_runtime::{service_fn, Error, LambdaEvent};
use std::sync::Arc;


// Define your application state
#[derive(Clone)]
struct AppState {
    // your state fields here
}

// Set up the router
define_router!(event = ApiGatewayV2httpRequest, state = AppState);

// Define route handlers
#[route(path = "/hello/{name}")]
async fn handle_hello(ctx: RouteContext) -> Result<Value, Error> {
    let name = ctx.params.get("name").map(|s| s.as_str()).unwrap_or("World");
    Ok(json!({
        "message": format!("Hello, {}!", name)
    }))
}

// Lambda function entry point
#[tokio::main]
async fn main() -> Result<(), Error> {
    let state = Arc::new(AppState {});
    let router = Arc::new(RouterBuilder::from_registry().build());
    
    let lambda = move |event: LambdaEvent<ApiGatewayV2httpRequest>| {
        let state = Arc::clone(&state);
        let router = Arc::clone(&router);
        async move { router.handle_request(event, state).await }
    };

    lambda_runtime::run(service_fn(lambda)).await
}
```

### OpenTelemetry Integration

The router automatically integrates with OpenTelemetry for tracing, adding semantic http attributes to the span 
and setting the span name to the route path. 
It also support setting additional attributes to the spanas shown in the following example:

```rust,ignore
#[route(path = "/users/{id}")]
async fn get_user(ctx: RouteContext) -> Result<Value, Error> {
    // Span name will be "GET /users/{id}"
    ctx.set_otel_attribute("user.id", ctx.params.get("id").unwrap());
    // ...
}
```

### Path Parameters

Support for various path parameter patterns:

```rust,ignore
// Basic parameters
#[route(path = "/users/{id}")]
async fn get_user(ctx: RouteContext) -> Result<Value, Error> {
    let user_id = ctx.params.get("id")?
    // ...
}

// Multi-segment parameters
#[route(path = "/files/{path+}")]  // Matches /files/docs/2024/report.pdf
async fn get_file(ctx: RouteContext) -> Result<Value, Error> {
    let path = ctx.params.get("path")?;
    // ...
}

// Multiple parameters
#[route(path = "/users/{user_id}/posts/{post_id}")]
async fn get_post_for_user(ctx: RouteContext) -> Result<Value, Error> {
    let user_id = ctx.params.get("user_id")?;
    let post_id = ctx.params.get("post_id")?;
    // ...
}
```

## API Documentation

For detailed API documentation, run:

```bash
cargo doc --open
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
