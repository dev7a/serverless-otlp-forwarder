use lambda_otel_lite::{create_traced_handler, init_telemetry, TelemetryConfig};
use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
use serde_json::{json, Value};
use tracing_opentelemetry::OpenTelemetrySpanExt;

const DEFAULT_DEPTH: u64 = 2;
const DEFAULT_ITERATIONS: u64 = 4;

/// Recursively create a tree of spans to measure OpenTelemetry overhead.
///
/// Args:
///     depth: Current depth level (decrements towards 0)
///     iterations: Number of spans to create at each level
fn process_level(depth: u64, iterations: u64) {
    if depth == 0 {
        return;
    }

    for i in 0..iterations {
        let span_name = format!("operation_depth_{}_iter_{}", depth, i);
        let span = tracing::info_span!("span", otel.name = span_name);

        let _enter = span.enter();

        let current_span = tracing::Span::current();
        current_span.set_attribute("depth", depth as i64);
        current_span.set_attribute("iteration", i as i64);
        current_span.set_attribute("payload", "x".repeat(256));
        // Recursive call to create the tree
        process_level(depth - 1, iterations);
    }
}

/// Lambda handler that creates a tree of spans based on input parameters.
///
/// This handler generates a tree of OpenTelemetry spans to measure overhead
/// and performance characteristics. It also handles trace context propagation
/// from the incoming event JSON.
///
/// Args:
///     event: Lambda event containing depth and iterations as JSON properties
///     context: Lambda context (not used)
///
/// Returns:
///     JSON response containing the benchmark parameters and completion status
async fn handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let benchmark_event = event.payload;

    // Extract depth with fallback to 3
    let depth = benchmark_event
        .get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_DEPTH);

    // Extract iterations with fallback to 4
    let iterations = benchmark_event
        .get("iterations")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_ITERATIONS);

    // Create the tree of spans
    process_level(depth, iterations);

    // Return a successful response with benchmark parameters
    Ok(json!({
        "message": "Benchmark complete",
        "depth": depth,
        "iterations": iterations
    }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize telemetry with default config
    let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;

    // Create the traced handler with the benchmark name
    let handler = create_traced_handler("benchmark-execution", completion_handler, handler);

    // Use it directly with the runtime
    Runtime::new(service_fn(handler)).run().await
}
