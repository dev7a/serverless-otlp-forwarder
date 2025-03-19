use opentelemetry::global;
use opentelemetry::trace::Tracer;
use opentelemetry_sdk::trace::SdkTracerProvider;
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;

fn init_tracer() -> SdkTracerProvider {
    let exporter = OtlpStdoutSpanExporter::default();
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
        println!("Doing work...");

        // Create nested spans
        tracer.in_span("child-operation", |_cx| {
            println!("Doing more work...");
        });
    });

    if let Err(err) = provider.force_flush() {
        println!("Error flushing provider: {:?}", err);
    }
}
