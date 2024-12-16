---
layout: default
title: Rust
parent: Language Support
has_children: true
nav_order: 1
---

# Rust Support
{: .fs-9 }

OpenTelemetry exporter for AWS Lambda that writes telemetry data to stdout in OTLP format.
{: .fs-6 .fw-300 }

## Quick Links
{: .text-delta }

[![Crates.io](https://img.shields.io/crates/v/otlp-stdout-client.svg)](https://crates.io/crates/otlp-stdout-client)
[![docs.rs](https://docs.rs/otlp-stdout-client/badge.svg)](https://docs.rs/otlp-stdout-client)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

- [Source Code](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/otlp-stdout-client)
- [API Documentation](https://docs.rs/otlp-stdout-client)
- [Examples](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/otlp-stdout-client/examples)
- [Change Log](https://github.com/dev7a/serverless-otlp-forwarder/blob/main/packages/rust/otlp-stdout-client/CHANGELOG.md)

## Installation
{: .text-delta }

Install the required dependencies using `cargo add`:

```bash
cargo add otlp-stdout-client
cargo add opentelemetry --features trace
cargo add opentelemetry-sdk --features trace,rt-tokio
cargo add opentelemetry-otlp --features http-proto,trace
```

{: .note }
This will automatically add the latest compatible versions of each crate to your `Cargo.toml`.

## Basic Usage
{: .text-delta }

The following example shows a simple AWS Lambda function that:
- Handles API Gateway proxy requests
- Creates and configures an OpenTelemetry tracer with `StdoutClient` to send OTLP data to stdout
- Creates a span for each request
- Returns a "Hello!" message

{: .note }
The key integration is using `StdoutClient::default()` with the OTLP exporter, which redirects all telemetry data to stdout instead of making HTTP calls.

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

## Configuration
{: .text-delta }

Configuration is handled through environment variables:

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | Protocol for OTLP data (`http/protobuf` or `http/json`) | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | Compression type (`gzip` or `none`) | `none` |
| `OTEL_SERVICE_NAME` | Name of your service | Falls back to `AWS_LAMBDA_FUNCTION_NAME` or "unknown-service" |

## Best Practices
{: .text-delta }

{: .info }
- Always call `force_flush()` on the trace provider before your Lambda function exits to ensure all telemetry is written to stdout
- Consider enabling compression if you're generating large amounts of telemetry data
- Set `OTEL_SERVICE_NAME` to easily identify your service in the telemetry data

## Troubleshooting
{: .text-delta }

{: .warning }
Common issues and solutions:

1. **No Data in Logs**
   - Verify `force_flush()` is called before the function exits
   - Check that the `StdoutClient` is properly configured in the exporter
   - Ensure spans are being created and closed properly

2. **JSON Parsing Errors**
   - Verify the correct protocol is set in `OTEL_EXPORTER_OTLP_PROTOCOL`
   - Check for valid JSON in your attributes and events
