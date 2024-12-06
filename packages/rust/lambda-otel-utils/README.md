# lambda-otel-utils

`lambda-otel-utils` is a Rust library that simplifies the integration of OpenTelemetry tracing and metrics with AWS Lambda functions. It provides utilities for setting up and configuring OpenTelemetry in serverless environments, making it easier to implement distributed tracing and metrics collection in your Lambda-based applications.

## Features

- Easy setup of OpenTelemetry TracerProvider and MeterProvider for AWS Lambda
- Customizable tracing and metrics configuration
- Compatible with the `lambda_runtime` crate
- Support for outputting to stdout using the `otlp-stdout-client`
- Environment variable configuration support
- AWS Lambda resource detection and attribute injection
- Flexible subscriber configuration with JSON formatting options

## Installation

Add the following to your `Cargo.toml` or run `cargo add lambda-otel-utils`:

```toml
[dependencies]
lambda-otel-utils = "0.2.0" 
```

## Usage

### Basic Setup with Tracing and Metrics

```rust, no_run
use lambda_otel_utils::{
    HttpMeterProviderBuilder, HttpTracerProviderBuilder, OpenTelemetrySubscriberBuilder,
};
use std::error::Error as StdError;
use std::time::Duration;
use tracing::{info, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[tracing::instrument]
fn do_work() {
    let span = tracing::Span::current();
    span.set_attribute("work_done", "true");

    // Add metrics with attributes
    info!(
        monotonic_counter.operations = 1_u64,
        operation = "add_item",
        unit = "count",
        "Operation completed successfully"
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError + Send + Sync + 'static>> {
    // Initialize tracer provider
    let tracer_provider = HttpTracerProviderBuilder::default()
        .with_stdout_client()
        .with_default_text_map_propagator()
        .with_simple_exporter()
        .enable_global(true)
        .build()?;

    // Initialize meter provider
    let meter_provider = HttpMeterProviderBuilder::default()
        .with_stdout_client()
        .with_meter_name("my-service")
        .with_export_interval(Duration::from_secs(30))
        .build()?;

    // Keep references for shutdown
    let tracer_provider_ref = tracer_provider.clone();
    let meter_provider_ref = meter_provider.clone();

    // Initialize the OpenTelemetry subscriber
    OpenTelemetrySubscriberBuilder::new()
        .with_env_filter(true)
        .with_tracer_provider(tracer_provider)
        .with_meter_provider(meter_provider)
        .with_service_name("my-service")
        .init()?;

    // Example instrumentation
    let span = info_span!(
        "test_span",
        service.name = "test-service",
        operation = "test-operation"
    );
    span.in_scope(|| {
        span.set_attribute("test_attribute", "test_value");
        span.set_attribute("another_attribute", "another_value");
        do_work();
    });

    // flush and shutdown
    for result in tracer_provider_ref.force_flush() {
        if let Err(e) = result {
            eprintln!("Error flushing tracer provider: {}", e);
        }
    }
    if let Err(e) = meter_provider_ref.force_flush() {
        eprintln!("Error flushing meter provider: {}", e);
    }
    shutdown_providers(&tracer_provider_ref, &meter_provider_ref)
}

fn shutdown_providers(
    tracer_provider: &opentelemetry_sdk::trace::TracerProvider,
    meter_provider: &opentelemetry_sdk::metrics::SdkMeterProvider,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    tracer_provider.shutdown()?;
    meter_provider.shutdown()?;
    Ok(())
}
```

## Main Components

### HttpTracerProviderBuilder

The `HttpTracerProviderBuilder` allows you to configure and build a custom TracerProvider tailored for Lambda environments. Features include:

- Stdout client support for Lambda environments
- Configurable text map propagators (including XRay support)
- Custom ID generators with XRay support
- Simple and batch exporter options
- Global provider installation option

```rust, ignore
use lambda_otel_utils::HttpTracerProviderBuilder;

let tracer_provider = HttpTracerProviderBuilder::default()
    .with_stdout_client()
    .with_xray_text_map_propagator()
    .with_xray_id_generator()
    .with_simple_exporter()
    .enable_global(true)
    .build()?;
```

### HttpMeterProviderBuilder

The `HttpMeterProviderBuilder` provides configuration options for metrics collection:

- Customizable export intervals
- Stdout client support
- Meter naming
- Export timeout configuration
- Global provider installation

```rust, ignore
use lambda_otel_utils::HttpMeterProviderBuilder;
use std::time::Duration;

let meter_provider = HttpMeterProviderBuilder::default()
    .with_stdout_client()
    .with_meter_name("my-service")
    .with_export_interval(Duration::from_secs(30))
    .build()?;
```

### OpenTelemetrySubscriberBuilder

A flexible builder for configuring tracing subscribers with OpenTelemetry support:

- Environment filter support (RUST_LOG)
- JSON formatting options
- Combined tracing and metrics setup
- Service name configuration

```rust, ignore
use lambda_otel_utils::OpenTelemetrySubscriberBuilder;

OpenTelemetrySubscriberBuilder::new()
    .with_tracer_provider(tracer_provider)
    .with_meter_provider(meter_provider)
    .with_service_name("my-service")
    .with_env_filter(true)
    .with_json_format(true)
    .init()?;
```


## Environment Variables

The crate respects several environment variables for configuration:

- `OTEL_SERVICE_NAME`: Sets the service name for telemetry data
- `AWS_LAMBDA_FUNCTION_NAME`: Fallback for service name if OTEL_SERVICE_NAME is not set
- `OTEL_EXPORTER_OTLP_PROTOCOL`: Configures the OTLP protocol ("http/protobuf" or "http/json")
- `LAMBDA_OTEL_SPAN_PROCESSOR`: Selects the span processor type ("simple" or "batch")
- `RUST_LOG`: Controls logging levels when environment filter is enabled

## Resource Detection

The crate automatically detects and includes AWS Lambda resource attributes in your telemetry data, including:

- Function name
- Function version
- Execution environment
- Memory limits
- Region information

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under MIT. See the LICENSE file for details.
