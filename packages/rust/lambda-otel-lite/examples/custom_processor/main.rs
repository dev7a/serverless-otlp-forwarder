use aws_lambda_events::event::apigw::{ApiGatewayV2httpRequest, ApiGatewayV2httpResponse};
use lambda_otel_lite::telemetry::{init_telemetry, TelemetryConfig};
use lambda_otel_lite::{create_traced_handler, LambdaSpanProcessor};
use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry::Context;
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{Span, SpanData, SpanProcessor},
    Resource,
};
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// A custom span processor that implements the "wide events" pattern.
///
/// This processor accumulates attributes from all spans within a trace and attaches
/// them to the root span, creating a comprehensive view of the entire request flow.
/// This is useful for:
/// - Creating rich root spans with context from the entire trace tree
/// - Centralized logging where one span contains all relevant information
/// - Simplified querying in observability systems
///
/// ## How it works:
/// 1. **Buffer Phase**: All completed spans are held in memory (not forwarded yet)
/// 2. **Detection Phase**: Root spans are identified (spans with no parent)
/// 3. **Aggregation Phase**: When root span completes, collect all attributes from all spans
/// 4. **Enrichment Phase**: Replace root span's attributes with the aggregated set
/// 5. **Forward Phase**: Send the enriched root span + all child spans to next processor
#[derive(Debug)]
pub struct WideEventsSpanProcessor {
    /// The next processor in the chain (typically LambdaSpanProcessor -> Exporter)
    next: Box<dyn SpanProcessor>,
    /// Buffer to hold spans until the trace is complete, keyed by trace ID
    traces: Arc<Mutex<HashMap<TraceId, TraceData>>>,
}

#[derive(Debug, Default)]
struct TraceData {
    /// All spans that belong to this trace, held until root span completes
    spans: Vec<SpanData>,
    /// The span ID of the root span (the one with no parent), if we've seen it
    root_span_id: Option<SpanId>,
}

impl WideEventsSpanProcessor {
    pub fn new(next: Box<dyn SpanProcessor>) -> Self {
        Self {
            next,
            traces: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl SpanProcessor for WideEventsSpanProcessor {
    fn on_start(&self, span: &mut Span, cx: &Context) {
        // Pass through to next processor - we don't need to do anything when spans start.
        // All the work happens in on_end() when spans complete.
        self.next.on_start(span, cx);
    }

    /// **THE CORE OF THE WIDE EVENTS PATTERN**
    ///
    /// When a span ends, we don't immediately forward it to the next processor.
    /// Instead, we buffer it until the entire trace is complete, then process all spans together.
    ///
    /// ## Step-by-step process:
    /// 1. **Store the span**: Add this completed span to our trace buffer
    /// 2. **Check if root**: If this span has no parent, mark it as the root span
    /// 3. **Check for completion**: If we now have the root span, trigger aggregation
    /// 4. **Aggregate attributes**: Collect attributes from ALL spans in the trace
    /// 5. **Enrich root span**: Replace root span's attributes with the aggregated set
    /// 6. **Forward everything**: Send enriched root + all child spans to next processor
    /// 7. **Clean up**: Remove this trace from our buffer (it's done)
    fn on_end(&self, span: SpanData) {
        // Step 1: Identify this span and its trace
        let trace_id = span.span_context.trace_id();
        let is_root = span.parent_span_id == SpanId::INVALID; // Root = no parent
        let mut traces = self.traces.lock().unwrap();

        // Step 2: Add this span to the buffer for its trace
        let trace_data = traces.entry(trace_id).or_default();
        trace_data.spans.push(span);

        // Step 3: If this is the root span, remember its ID
        if is_root {
            trace_data.root_span_id = Some(trace_data.spans.last().unwrap().span_context.span_id());
        }

        // Step 4: Check if we can now process this trace (do we have the root span?)
        if let Some(root_span_id) = trace_data.root_span_id {
            // Check if the root span has actually ended (is in our buffer)
            if trace_data
                .spans
                .iter()
                .any(|s| s.span_context.span_id() == root_span_id)
            {
                // Step 5: THE ROOT SPAN IS COMPLETE! Time to aggregate and forward.

                // Remove this trace from buffer (we're done with it)
                let mut trace_data = traces.remove(&trace_id).unwrap();
                let mut root_span_idx = None;
                let mut all_attributes = HashSet::new();

                // Step 6: Collect attributes from ALL spans in this trace
                for (i, s) in trace_data.spans.iter().enumerate() {
                    if s.span_context.span_id() == root_span_id {
                        root_span_idx = Some(i); // Found the root span position
                    }
                    // Collect attributes from this span (root or child)
                    for kv in &s.attributes {
                        all_attributes.insert(kv.clone());
                    }
                }

                // Step 7: Enrich the root span with aggregated attributes
                if let Some(idx) = root_span_idx {
                    let mut root_span = trace_data.spans.remove(idx);
                    // CRITICAL: Replace root span's attributes with ALL attributes from trace
                    root_span.attributes = all_attributes.into_iter().collect();

                    // Forward the enriched root span first
                    self.next.on_end(root_span);
                }

                // Step 8: Forward all child spans (unchanged)
                for s in trace_data.spans {
                    self.next.on_end(s);
                }
            }
        }
        // If we get here, the trace isn't complete yet - just wait for more spans
    }

    fn force_flush(&self) -> OTelSdkResult {
        // Emergency flush: send any incomplete traces as-is (without aggregation)
        // This happens during shutdown or when explicitly requested
        let mut traces = self.traces.lock().unwrap();
        for (_, trace_data) in traces.drain() {
            for span in trace_data.spans {
                // Send spans without modification - we don't have complete trace
                self.next.on_end(span);
            }
        }
        self.next.force_flush()
    }

    fn shutdown(&self) -> OTelSdkResult {
        // Clean shutdown: flush any remaining traces, then shutdown next processor
        self.force_flush()?;
        self.next.shutdown()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        // Shutdown with timeout: same as shutdown but with time limit
        self.force_flush()?;
        self.next.shutdown_with_timeout(timeout)
    }

    fn set_resource(&mut self, resource: &Resource) {
        // Pass resource configuration to next processor
        self.next.set_resource(resource);
    }
}

// Simple nested function that creates its own span. The attributes are recorded also on the root span.
#[instrument(fields(nested_tracing_span_attr_in_function,))]
async fn nested_function() -> Result<String, Error> {
    let span = tracing::Span::current();

    // using the tracing api
    span.record(
        "nested_tracing_span_attr_in_function",
        "nested_tracing_value_in_function",
    );

    // using the OpenTelemetrySpanExt trait
    span.set_attribute(
        "nested_otel_span_attr_in_function",
        "nested_otel_value_in_function",
    );

    Ok("success".to_string())
}

async fn handler(
    _event: LambdaEvent<ApiGatewayV2httpRequest>,
) -> Result<ApiGatewayV2httpResponse, Error> {
    // Use tracing span for easier attribute setting
    let span = tracing::Span::current();
    span.record("handler_span_attr", "handler_value");

    // Call nested function (it will automatically create a child span due to #[instrument])
    let _result = nested_function().await?;

    Ok(ApiGatewayV2httpResponse {
        status_code: 200,
        body: Some("Hello from custom processor!".into()),
        ..Default::default()
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let exporter = OtlpStdoutSpanExporter::default();
    let lambda_processor = LambdaSpanProcessor::builder().exporter(exporter).build();
    let aggregating_processor = WideEventsSpanProcessor::new(Box::new(lambda_processor));

    let config = TelemetryConfig::builder()
        .with_span_processor(aggregating_processor)
        .build();

    let (_, completion_handler) = init_telemetry(config).await?;

    let handler = create_traced_handler("custom-processor-handler", completion_handler, handler);

    Runtime::new(service_fn(handler)).run().await
}
