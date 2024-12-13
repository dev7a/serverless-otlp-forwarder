---
layout: default
title: Rust
parent: Language Support
nav_order: 1
---

# Rust Support
{: .fs-9 }

High-performance, type-safe integration with OpenTelemetry through the `otlp-stdout-client` crate.
{: .fs-6 .fw-300 }

## Quick Links
{: .text-delta }

[![Crates.io](https://img.shields.io/crates/v/otlp-stdout-client.svg)](https://crates.io/crates/otlp-stdout-client)
[![docs.rs](https://docs.rs/otlp-stdout-client/badge.svg)](https://docs.rs/otlp-stdout-client)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

- [Source Code](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/rust/otlp-stdout-client)
- [API Documentation](https://docs.rs/otlp-stdout-client)
- [Examples](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/rust/otlp-stdout-client/examples)
- [Change Log](https://github.com/dev7a/lambda-otlp-forwarder/blob/main/packages/rust/otlp-stdout-client/CHANGELOG.md)

## Installation
{: .text-delta }

Add to your `Cargo.toml`:

```toml
[dependencies]
otlp-stdout-client = "0.2"
opentelemetry = { version = "0.20", features = ["trace"] }
opentelemetry_sdk = { version = "0.20", features = ["trace", "rt-tokio"] }
opentelemetry-otlp = { version = "0.13", features = ["http-proto", "trace"] }
```

## Basic Usage
{: .text-delta }

```rust
use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use opentelemetry::trace::TraceError;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use otlp_stdout_client::StdoutClient;

async fn init_tracer() -> Result<opentelemetry_sdk::trace::TracerProvider, TraceError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_http_client(StdoutClient::default())
        .build()?;
    
    Ok(opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let provider = init_tracer().await?;
    opentelemetry::global::set_tracer_provider(provider.clone());
    
    let handler = service_fn(|event: LambdaEvent<ApiGatewayProxyRequest>| async {
        let tracer = opentelemetry::global::tracer("lambda-handler");
        let span = tracer.start("process-request");
        let _guard = span.enter();
        
        // Your handler logic here
        Ok::<_, Error>(serde_json::json!({ "message": "Hello!" }))
    });

    lambda_runtime::run(handler).await?;
    provider.force_flush();
    Ok(())
}
```

## Advanced Features
{: .text-delta }

### Custom Configuration
{: .text-delta }

```rust
use otlp_stdout_client::{StdoutClient, Config};

let client = StdoutClient::new(Config {
    compression: Some("gzip".to_string()),
    protocol: Some("http/protobuf".to_string()),
    ..Default::default()
});
```

### Error Handling
{: .text-delta }

```rust
use opentelemetry::trace::{Status, StatusCode};

#[tracing::instrument]
async fn handle_request() -> Result<(), Error> {
    let tracer = opentelemetry::global::tracer("error-handler");
    let span = tracer.start("process");
    
    match do_something_risky().await {
        Ok(result) => {
            span.set_status(Status::Ok);
            Ok(result)
        }
        Err(e) => {
            span.set_status(Status::error(e.to_string()));
            span.record_error(&e);
            Err(e)
        }
    }
}
```

### Batch Processing
{: .text-delta }

```rust
use opentelemetry_sdk::trace::BatchConfig;

let batch_config = BatchConfig::default()
    .with_max_queue_size(8192)
    .with_scheduled_delay(std::time::Duration::from_secs(5));

let provider = opentelemetry_sdk::trace::TracerProvider::builder()
    .with_batch_exporter(exporter, batch_config)
    .build();
```

## Best Practices
{: .text-delta }

### Resource Management
{: .text-delta }

{: .info }
- Always call `force_flush()` before exit
- Use structured concurrency with `tokio::spawn`
- Implement proper cancellation
- Clean up resources in drop implementations

### Performance
{: .text-delta }

{: .info }
- Use batch processing when possible
- Enable compression for large payloads
- Configure appropriate timeouts
- Profile memory usage regularly

### Error Handling
{: .text-delta }

{: .info }
- Use the `?` operator for proper error propagation
- Implement custom error types if needed
- Record errors in spans
- Add context to errors with `wrap_err()`

### Context Propagation
{: .text-delta }

{: .info }
- Use the `LambdaLayer` for automatic context
- Implement custom propagators if needed
- Maintain trace context across async boundaries
- Handle baggage appropriately

## Environment Variables
{: .text-delta }

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |
| `OTEL_EXPORTER_OTLP_TIMEOUT` | Export timeout in milliseconds | `10000` |

## Troubleshooting
{: .text-delta }

{: .warning }
Common issues and solutions:

1. **Missing Spans**
   - Check if `force_flush()` is called
   - Verify span is not dropped early
   - Ensure proper context propagation

2. **Performance Issues**
   - Enable batch processing
   - Adjust batch configuration
   - Profile memory usage

3. **Build Errors**
   - Check feature flags
   - Verify dependency versions
   - Enable appropriate features

## Examples
{: .text-delta }

### API Gateway Integration
{: .text-delta }

```rust
use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use opentelemetry::trace::{Span, Tracer};
use opentelemetry_aws::lambda::LambdaLayer;
use tower::ServiceBuilder;

async fn function_handler(
    event: LambdaEvent<ApiGatewayProxyRequest>,
) -> Result<serde_json::Value, Error> {
    let tracer = opentelemetry::global::tracer("api-handler");
    let span = tracer.start("process-request");
    
    span.set_attribute(opentelemetry::KeyValue::new(
        "http.method",
        event.payload.http_method.clone(),
    ));
    
    let _guard = span.enter();
    // Your handler logic here
    Ok(serde_json::json!({ "message": "Success" }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let provider = init_tracer().await?;
    opentelemetry::global::set_tracer_provider(provider.clone());
    
    let service = ServiceBuilder::new()
        .layer(LambdaLayer::new())
        .service(service_fn(function_handler));

    lambda_runtime::run(service).await?;
    provider.force_flush();
    Ok(())
}
```

## Next Steps
{: .text-delta }

- [Configure Processors](../concepts/processors)
- [Performance Tuning](../advanced/performance)
- [Monitoring Setup](../deployment/monitoring)