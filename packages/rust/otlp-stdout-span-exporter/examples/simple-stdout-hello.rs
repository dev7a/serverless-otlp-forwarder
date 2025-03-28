use opentelemetry::global;
use opentelemetry::trace::{Tracer, get_active_span};
use opentelemetry::KeyValue;
use opentelemetry_sdk::trace::SdkTracerProvider;
use otlp_stdout_span_exporter::{OtlpStdoutSpanExporter, LogLevel};
use std::collections::HashMap;

fn init_tracer() -> SdkTracerProvider {
    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("test".to_string(), "test".to_string());
    
    let exporter = OtlpStdoutSpanExporter::builder()
        .headers(headers)
        .level(LogLevel::Debug)
        .build();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());
    provider
}

#[tokio::main]
async fn main() {
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
}
