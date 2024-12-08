//! # Tracing Example
//!
//! This example demonstrates how to set up OpenTelemetry tracing and metrics using
//! the `lambda-otel-utils` library with stdout output.
//!
//! ## Structure
//!
//! The example shows:
//! 1. Setting up a tracer provider with stdout output
//! 2. Setting up a meter provider with stdout output
//! 3. Creating and instrumenting a basic span with attributes
//! 4. Adding metrics with custom attributes
//!
//! ## Running the Example
//!
//! ```sh
//! RUST_LOG=info OTEL_SERVICE_NAME=example cargo run --example tracing_example
//! ```
//!
//! ## Expected Output
//!
//! The example will output two JSON objects to stdout (because we use `with_stdout_client()`):
//!
//! 1. A traces payload containing:
//!    ```json
//!    {
//!      "resourceSpans": [{
//!        "resource": { "attributes": [{"key": "service.name", "value": "example"}] },
//!        "scopeSpans": [{
//!          "spans": [{
//!            "attributes": [
//!              {"key": "work_done", "value": "true"},
//!              // ... other span attributes
//!            ]
//!          }]
//!        }]
//!      }]
//!    }
//!    ```
//!
//! 2. A metrics payload containing:
//!    ```json
//!    {
//!      "resourceMetrics": [{
//!        "scopeMetrics": [{
//!          "metrics": [{
//!            "name": "operations",
//!            "sum": {
//!              "dataPoints": [{
//!                "attributes": [
//!                  {"key": "operation", "value": "add_item"},
//!                  {"key": "unit", "value": "count"}
//!                ],
//!                "asInt": 1
//!              }]
//!            }
//!          }]
//!        }]
//!      }]
//!    }
//!    ```

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
    // Check if RUST_LOG is set appropriately
    match std::env::var("RUST_LOG") {
        Ok(level) => {
            if !level.to_lowercase().contains("info") {
                println!("Warning: RUST_LOG is set to '{}' but should contain 'info' to see the example output.", level);
                println!("Try running with: RUST_LOG=info cargo run --example tracing_example");
            }
        }
        Err(_) => {
            println!(
                "Warning: RUST_LOG environment variable is not set. No output will be visible."
            );
            println!("Try running with: RUST_LOG=info cargo run --example tracing_example");
        }
    }

    // Initialize tracer provider
    let tracer_provider = HttpTracerProviderBuilder::default()
        .with_stdout_client()
        .with_default_text_map_propagator()
        .with_batch_exporter()
        .enable_global(true)
        .build()?;

    // Initialize meter provider
    let meter_provider = HttpMeterProviderBuilder::default()
        .with_stdout_client()
        .with_meter_name("my-service")
        .with_export_interval(Duration::from_secs(1))
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
