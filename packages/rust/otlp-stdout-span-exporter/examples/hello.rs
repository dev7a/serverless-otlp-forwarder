use opentelemetry::trace::{Tracer, TracerProvider};
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Set the service name environment variable
    std::env::set_var("OTEL_SERVICE_NAME", "hello-world");
    // Create a new stdout exporter
    let exporter = OtlpStdoutSpanExporter::new();

    // Create a new tracer provider with the exporter
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(Resource::builder().build())
        .build();

    // Create a tracer
    let tracer = provider.tracer("hello-world");

    // Create nested spans
    tracer.in_span("parent", |_cx| {
        println!("Starting parent operation");
        thread::sleep(Duration::from_millis(100));

        tracer.in_span("child1", |_cx| {
            println!("In child1");
            thread::sleep(Duration::from_millis(50));
        });

        tracer.in_span("child2", |_cx| {
            println!("In child2");
            thread::sleep(Duration::from_millis(50));
        });

        println!("Finishing parent operation");
    });

    // Shut down the provider
    let _ = provider.shutdown();
}
