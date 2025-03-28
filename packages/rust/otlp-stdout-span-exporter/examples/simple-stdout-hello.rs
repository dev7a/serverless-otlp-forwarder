use opentelemetry::global;
use opentelemetry::trace::{get_active_span, Tracer};
use opentelemetry::KeyValue;
use opentelemetry_sdk::trace::SdkTracerProvider;
use otlp_stdout_span_exporter::{LogLevel, OtlpStdoutSpanExporter};
use std::collections::HashMap;

fn init_tracer() -> SdkTracerProvider {
    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("test".to_string(), "test".to_string());

    // Create exporter with the Debug log level and named pipe output
    // You can also use environment variables:
    // OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL=debug
    // OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE=pipe
    let exporter = OtlpStdoutSpanExporter::builder()
        .headers(headers)
        .level(LogLevel::Debug)
        .pipe(true) // Will write to /tmp/otlp-stdout-span-exporter.pipe
        .build();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());
    provider
}

#[tokio::main]
async fn main() {
    eprintln!("Writing spans to /tmp/otlp-stdout-span-exporter.pipe with DEBUG level");
    eprintln!("Note: Make sure the named pipe exists (create with `mkfifo /tmp/otlp-stdout-span-exporter.pipe`)");

    let provider = init_tracer();
    let tracer = global::tracer("example/simple");
    tracer.in_span("parent-operation", |_cx| {
        get_active_span(|span| {
            span.add_event("Doing work".to_string(), vec![KeyValue::new("work", true)]);
        });

        // Create nested spans
        tracer.in_span("child-operation", |_cx| {
            get_active_span(|span| {
                span.add_event(
                    "Not doing work".to_string(),
                    vec![KeyValue::new("work", false)],
                );
            });
        });
    });

    if let Err(err) = provider.force_flush() {
        eprintln!("Error flushing provider: {:?}", err);
    }

    eprintln!("Spans have been written to /tmp/otlp-stdout-span-exporter.pipe");
}
