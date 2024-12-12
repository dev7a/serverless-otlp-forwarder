---
layout: default
title: Rust
parent: Language Support
nav_order: 1
---

# Rust Support

The Rust implementation provides a high-performance, type-safe integration with OpenTelemetry through the `otlp-stdout-client` crate.

## Quick Links
- [Source Code](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/rust/otlp-stdout-client)
- [Documentation](https://docs.rs/otlp-stdout-client)
- [Crate](https://crates.io/crates/otlp-stdout-client)
- [Examples](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/rust/otlp-stdout-client/examples)

## Features
- Full async support with Tokio
- Comprehensive error handling
- Efficient binary serialization
- AWS Lambda context integration
- Batch processing support

## Example Usage

```rust
use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::Value;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use otlp_stdout_client::StdoutClient;

async fn function_handler(_event: LambdaEvent<ApiGatewayProxyRequest>) -> Result<Value, Error> {
    Ok(serde_json::json!({"message": "Hello from Lambda!"}))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracer provider with the new builder pattern
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_http_client(StdoutClient::default())
        .build()?;
    
    let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();

    opentelemetry::global::set_tracer_provider(tracer_provider.clone());
    
    // Create a service with a tracing layer
    let service = tower::ServiceBuilder::new()
        .layer(opentelemetry_aws::lambda::LambdaLayer::new())
        .service(service_fn(function_handler));

    // Run the Lambda runtime
    lambda_runtime::run(service).await?;

    // Ensure traces are flushed
    tracer_provider.force_flush();
    Ok(())
}
```

## Configuration

The Rust implementation can be configured through environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |

## Best Practices

1. **Error Handling**
   - Use the `?` operator for proper error propagation
   - Implement custom error types if needed
   - Record errors in spans

2. **Resource Management**
   - Always call `force_flush()` before exit
   - Use structured concurrency with `tokio::spawn`
   - Implement proper cancellation

3. **Performance**
   - Use batch processing when possible
   - Enable compression for large payloads
   - Configure appropriate timeouts

4. **Context Propagation**
   - Use the `LambdaLayer` for automatic context
   - Implement custom propagators if needed
   - Maintain trace context across async boundaries 