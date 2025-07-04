use aws_lambda_events::event::apigw::{ApiGatewayV2httpRequest, ApiGatewayV2httpResponse};
use lambda_otel_lite::telemetry::{init_telemetry, TelemetryConfig};
use lambda_otel_lite::{create_traced_handler, LambdaSpanProcessor};
use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry::{Context, KeyValue};
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

/// A custom span processor that aggregates attributes from all spans in a trace
/// and attaches them to the root span.
#[derive(Debug)]
pub struct WideEventsSpanProcessor {
    next: Box<dyn SpanProcessor>,
    traces: Arc<Mutex<HashMap<TraceId, TraceData>>>,
}

#[derive(Debug, Default)]
struct TraceData {
    spans: Vec<SpanData>,
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
        self.next.on_start(span, cx);
    }

    fn on_end(&self, span: SpanData) {
        let trace_id = span.span_context.trace_id();
        let is_root = span.parent_span_id == SpanId::INVALID;
        let mut traces = self.traces.lock().unwrap();

        let trace_data = traces.entry(trace_id).or_default();
        trace_data.spans.push(span);

        if is_root {
            trace_data.root_span_id = Some(trace_data.spans.last().unwrap().span_context.span_id());
        }

        // If the root span has ended and all spans for the trace are collected
        if let Some(root_span_id) = trace_data.root_span_id {
            if trace_data
                .spans
                .iter()
                .any(|s| s.span_context.span_id() == root_span_id)
            {
                let mut trace_data = traces.remove(&trace_id).unwrap();
                let mut root_span_idx = None;
                let mut all_attributes = HashSet::new();

                for (i, s) in trace_data.spans.iter().enumerate() {
                    if s.span_context.span_id() == root_span_id {
                        root_span_idx = Some(i);
                    }
                    for kv in &s.attributes {
                        all_attributes.insert(kv.clone());
                    }
                }

                if let Some(idx) = root_span_idx {
                    let mut root_span = trace_data.spans.remove(idx);
                    root_span.attributes = all_attributes.into_iter().collect();
                    self.next.on_end(root_span);
                }

                for s in trace_data.spans {
                    self.next.on_end(s);
                }
            }
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        let mut traces = self.traces.lock().unwrap();
        for (_, trace_data) in traces.drain() {
            for span in trace_data.spans {
                self.next.on_end(span);
            }
        }
        self.next.force_flush()
    }

    fn shutdown(&self) -> OTelSdkResult {
        self.force_flush()?;
        self.next.shutdown()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.force_flush()?;
        self.next.shutdown_with_timeout(timeout)
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.next.set_resource(resource);
    }
}

// Simple nested function that creates its own span. The attributes are recorded also on the root span.
#[instrument(fields(
    nested_span_attr="nested_value",
    wide_event=1234
))]
async fn nested_function() -> Result<String, Error> {
    Ok("success".to_string())
}

async fn handler(
    _event: LambdaEvent<ApiGatewayV2httpRequest>,
) -> Result<ApiGatewayV2httpResponse, Error> {
    // Use tracing span for easier attribute setting
    let span = tracing::Span::current();
    span.set_attribute("handler_span_attr", "handler_value");

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
    let aggregating_processor =
        WideEventsSpanProcessor::new(Box::new(lambda_processor));

    let config = TelemetryConfig::builder()
        .with_span_processor(aggregating_processor)
        .build();

    let (_, completion_handler) = init_telemetry(config).await?;

    let handler = create_traced_handler("custom-processor-handler", completion_handler, handler);

    Runtime::new(service_fn(handler)).run().await
} 