---
layout: default
title: Other Crates
parent: Rust
grand_parent: Language Support
nav_order: 2
has_children: false
---

# Other Crates
{: .fs-9 }

Additional experimental crates for AWS Lambda and OpenTelemetry integration.
{: .fs-6 .fw-300 }

{: .warning }
> These crates are experimental and under active development. APIs and features may change.

## Overview
{: .text-delta }

Due to the early stage of Rust support for OpenTelemetry in AWS Lambda, we've developed several utility crates to fill gaps and provide better integration. These crates are part of the Serverless OTLP Forwarder project but can be used independently.

## Available Crates
{: .text-delta }

### lambda-lw-http-router
{: .text-delta }

[![Crates.io](https://img.shields.io/crates/v/lambda-lw-http-router.svg)](https://crates.io/crates/lambda-lw-http-router)
[![docs.rs](https://docs.rs/lambda-lw-http-router/badge.svg)](https://docs.rs/lambda-lw-http-router)

A lightweight, type-safe HTTP router for AWS Lambda functions with built-in OpenTelemetry support.

Key features:
- Zero runtime overhead with compile-time route registration
- Type-safe route handlers and application state
- Path parameter extraction
- Support for API Gateway (v1/v2) and ALB events
- Automatic OpenTelemetry span creation and attribute injection

[GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/lambda-lw-http-router) |
[Documentation](https://docs.rs/lambda-lw-http-router) |
[Crates.io](https://crates.io/crates/lambda-lw-http-router)

### otlp-sigv4-client
{: .text-delta }

[![Crates.io](https://img.shields.io/crates/v/otlp-sigv4-client.svg)](https://crates.io/crates/otlp-sigv4-client)
[![docs.rs](https://docs.rs/otlp-sigv4-client/badge.svg)](https://docs.rs/otlp-sigv4-client)

An HTTP client for OpenTelemetry that adds AWS SigV4 authentication support.

Key features:
- AWS SigV4 authentication for OTLP endpoints
- Compatible with AWS X-Ray and Application Signals
- Support for both reqwest and hyper HTTP clients
- Automatic credential resolution from environment

[GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/otlp-sigv4-client) |
[Documentation](https://docs.rs/otlp-sigv4-client) |
[Crates.io](https://crates.io/crates/otlp-sigv4-client)

### lambda-otel-utils
{: .text-delta }

[![Crates.io](https://img.shields.io/crates/v/lambda-otel-utils.svg)](https://crates.io/crates/lambda-otel-utils)
[![docs.rs](https://docs.rs/lambda-otel-utils/badge.svg)](https://docs.rs/lambda-otel-utils)

Utilities for integrating OpenTelemetry with AWS Lambda functions.

Key features:
- Easy setup of TracerProvider and MeterProvider
- AWS Lambda resource detection
- Environment variable configuration
- Flexible subscriber configuration
- JSON formatting support

[GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/lambda-otel-utils) |
[Documentation](https://docs.rs/lambda-otel-utils) |
[Crates.io](https://crates.io/crates/lambda-otel-utils)

## Usage Example
{: .text-delta }

Here's a simple example using these crates together:

```rust
use lambda_otel_utils::{HttpTracerProviderBuilder, OpenTelemetrySubscriberBuilder};
use lambda_lw_http_router::{define_router, route};
use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
use serde_json::{json, Value};
use lambda_runtime::{service_fn, Error, LambdaEvent};

// Define application state
#[derive(Clone)]
struct AppState {}

// Set up the router
define_router!(event = ApiGatewayV2httpRequest, state = AppState);

// Define a route handler
#[route(path = "/hello/{name}")]
async fn handle_hello(ctx: RouteContext) -> Result<Value, Error> {
    // The span is automatically created with the route path
    ctx.set_otel_attribute("user.name", ctx.params.get("name").unwrap());
    Ok(json!({ "message": format!("Hello, {}!", ctx.params.get("name").unwrap()) }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize OpenTelemetry with AWS Lambda detection
    let tracer_provider = HttpTracerProviderBuilder::default()
        .with_stdout_client()
        .build()?;

    OpenTelemetrySubscriberBuilder::new()
        .with_tracer_provider(tracer_provider)
        .with_env_filter(true)
        .with_json_format(true)
        .init()?;

    // Set up the router
    let state = Arc::new(AppState {});
    let router = Arc::new(RouterBuilder::from_registry().build());
    
    lambda_runtime::run(service_fn(|event: LambdaEvent<ApiGatewayV2httpRequest>| {
        let router = Arc::clone(&router);
        let state = Arc::clone(&state);
        async move { router.handle_request(event, state).await }
    })).await
}
```

## Contributing
{: .text-delta }

These crates are part of the Serverless OTLP Forwarder project. Contributions are welcome! Please feel free to submit issues or pull requests to the [main repository](https://github.com/dev7a/serverless-otlp-forwarder). 