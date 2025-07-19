use lambda_otel_lite::{init_telemetry, TelemetryConfig};
use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_sdk::trace::Sampler;
use serde_json::Value;

async fn handler(_event: LambdaEvent<Value>) -> Result<Value, Error> {
    // Get the global tracer
    let tracer = opentelemetry::global::tracer("sampler-example");

    // Create some spans to demonstrate sampling
    tracer.in_span("normal-operation", |cx| {
        cx.span()
            .set_attribute(opentelemetry::KeyValue::new("operation.type", "normal"));
        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(10));
    });

    tracer.in_span("error-operation", |cx| {
        cx.span()
            .set_attribute(opentelemetry::KeyValue::new("operation.type", "error"));
        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(10));
    });

    Ok(serde_json::json!({
        "message": "Sampler example completed",
        "sampled_spans": "Check your telemetry backend to see which spans were sampled"
    }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize telemetry with custom sampler
    let config = TelemetryConfig::builder()
        // Example 1: Sample all traces
        .with_sampler(Sampler::AlwaysOn)
        // Example 2: Sample 50% of traces
        // .with_sampler(Sampler::TraceIdRatioBased(0.5))
        // Example 3: Sample no traces
        // .with_sampler(Sampler::AlwaysOff)
        // Example 4: Parent-based sampling
        // .with_sampler(Sampler::ParentBased(Box::new(Sampler::AlwaysOn)))
        .build();

    let (_, completion_handler) = init_telemetry(config).await?;

    let handler = service_fn(handler);

    // Run the Lambda runtime
    Runtime::new(handler).run().await?;

    // Complete telemetry processing
    completion_handler.complete();

    Ok(())
}
