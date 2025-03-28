use opentelemetry::global;
use opentelemetry::trace::{Tracer, get_active_span};
use opentelemetry::KeyValue;
use opentelemetry_sdk::trace::SdkTracerProvider;
use otlp_stdout_span_exporter::{OtlpStdoutSpanExporter, LogLevel};
use std::collections::HashMap;

fn init_tracer() -> SdkTracerProvider {
    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("test".to_string(), "test".to_string());
    
    // Create exporter with the Debug log level and a file output path
    // You can also use environment variables:
    // OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL=debug
    // OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_PATH=file:///path/to/output.jsonl
    let exporter = OtlpStdoutSpanExporter::builder()
        .headers(headers)
        .level(LogLevel::Debug)
        .output_path("file:///tmp/otlp-spans.jsonl".to_string())
        .build();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());
    provider
}

#[tokio::main]
async fn main() {
    println!("Writing spans to /tmp/otlp-spans.jsonl with DEBUG level");
    
    let provider = init_tracer();
    let tracer = global::tracer("example/simple");
    tracer.in_span("parent-operation", |_cx| {
        get_active_span(|span| {
            span.add_event("Doing work".to_string(), vec![KeyValue::new("work", true)]);
        });

        // Create nested spans
        tracer.in_span("child-operation", |_cx| {
            get_active_span(|span| {
                span.add_event("Not doing work".to_string(), vec![KeyValue::new("work", false)]);
            });
            });
    });

    if let Err(err) = provider.force_flush() {
        println!("Error flushing provider: {:?}", err);
    }
    
    println!("Spans have been written to /tmp/otlp-spans.jsonl");
}
