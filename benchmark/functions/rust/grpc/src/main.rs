use lambda_runtime::{
    layers::{OpenTelemetryFaasTrigger, OpenTelemetryLayer as OtelLayer},
    tracing::Span, service_fn, Error, LambdaEvent, Runtime
};
use opentelemetry::{global, propagation::TextMapPropagator, trace::TracerProvider};
use opentelemetry_aws::trace::XrayPropagator;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig, Protocol};
use opentelemetry_sdk::{runtime, trace::{self, SdkTracerProvider}, propagation::TraceContextPropagator};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::prelude::*;

/// Recursively create a tree of spans to measure OpenTelemetry overhead.
///
/// Args:
///     depth: Current depth level (decrements towards 0)
///     iterations: Number of spans to create at each level
#[instrument(skip_all, level = "info")]
fn process_level(depth: u32, iterations: u32) {
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
        
        // Recursive call to create the tree
        process_level(depth - 1, iterations);
        
        info!("process-level-complete");
    }
}

/// Extract trace context from event headers if present
fn extract_context(event: &Value) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    
    // Try to extract headers from API Gateway or ALB event format
    if let Some(headers_obj) = event.get("headers").and_then(|h| h.as_object()) {
        for (key, value) in headers_obj {
            if let Some(val_str) = value.as_str() {
                headers.insert(key.to_lowercase(), val_str.to_string());
            }
        }
    }
        
    // If trace ID still not found in headers, check environment variable
    if !headers.contains_key("x-amzn-trace-id") {
        if let Ok(trace_id) = std::env::var("_X_AMZN_TRACE_ID") {
            if !trace_id.is_empty() {
                headers.insert("x-amzn-trace-id".to_string(), trace_id);
            }
        }
    }
    
    
    headers
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
async fn handler(
    event: LambdaEvent<Value>,
) -> Result<Value, Error> {
    // Extract trace context from headers
    let headers = extract_context(&event.payload);
    
    // Create a HashMap-based extractor for the propagator
    struct Extractor<'a>(&'a HashMap<String, String>);
    
    impl<'a> opentelemetry::propagation::Extractor for Extractor<'a> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).map(|v| v.as_str())
        }
        
        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|k| k.as_str()).collect()
        }
    }
    
    // First try to extract context using the propagator
    let span = Span::current();
    
    global::get_text_map_propagator(|propagator| {
        // Extract and set parent context
        let parent_context = propagator.extract(&Extractor(&headers));
        span.set_parent(parent_context);
    });
    
    // Set span kind to SERVER like in the example
    span.record("otel.kind", "SERVER");

    let benchmark_event = event.payload;
    
    // Extract depth with fallback to 3
    let depth = benchmark_event.get("depth")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(3);
    
    // Extract iterations with fallback to 4
    let iterations = benchmark_event.get("iterations")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(4);

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
    // Configure propagators - first set up a combined propagator for both W3C and X-Ray
    let w3c_propagator = TraceContextPropagator::new();
    let xray_propagator = XrayPropagator::default();
    
    // Use a composite propagator that tries W3C first, then X-Ray
    global::set_text_map_propagator(opentelemetry::propagation::TextMapCompositePropagator::new(vec![
        Box::new(w3c_propagator),
        Box::new(xray_propagator),
    ]));
    
    // Set up OpenTelemetry OTLP exporter for HTTP
    let batch_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()?;
    
    // Set up the tracer provider with the OTLP exporter
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(batch_exporter)
        .build();
    
    // Set up link between OpenTelemetry and tracing crate
    tracing_subscriber::registry()
        .with(tracing_opentelemetry::OpenTelemetryLayer::new(
            tracer_provider.tracer("benchmark-execution"),
        ))
        .init();
    
    // Initialize the Lambda runtime and add OpenTelemetry tracing
    let runtime = Runtime::new(service_fn(handler)).layer(
        // Create a tracing span for each Lambda invocation
        OtelLayer::new(|| {
            // Make sure that the trace is exported before the Lambda runtime is frozen
            let _ = tracer_provider.force_flush();
        })
        // Set the "faas.trigger" attribute of the span
        .with_trigger(OpenTelemetryFaasTrigger::Http),
    );
    
    runtime.run().await
}
