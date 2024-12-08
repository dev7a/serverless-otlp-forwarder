use opentelemetry::global;
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry::KeyValue;
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{runtime::Tokio, trace::TracerProvider};
use otlp_stdout_client::StdoutClient;

/// Initialize the tracer provider with the StdoutClient
fn init_tracer_provider() -> Result<TracerProvider, Box<dyn std::error::Error>> {
    // Read protocol from env var
    let protocol = match std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "http/protobuf" => Protocol::HttpBinary,
        "http/json" | "" => Protocol::HttpJson,
        unsupported => {
            eprintln!(
                "Warning: OTEL_EXPORTER_OTLP_PROTOCOL value '{}' is not supported. Defaulting to HTTP JSON.",
                unsupported
            );
            Protocol::HttpJson
        }
    };

    let exporter = SpanExporter::builder()
        .with_http()
        .with_protocol(protocol)
        .with_http_client(StdoutClient::default())
        .build()?;

    let tracer_provider = TracerProvider::builder()
        .with_batch_exporter(exporter, Tokio)
        .with_resource(opentelemetry_sdk::Resource::default())
        .build();

    Ok(tracer_provider)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize and set the global tracer provider
    let tracer_provider = init_tracer_provider()?;
    global::set_tracer_provider(tracer_provider);

    // Get a tracer from the provider
    let tracer = global::tracer("basic-example");

    // Create a sample span
    tracer.in_span("hello_world", |cx| {
        // Emit an event with attributes
        cx.span().add_event(
            "span_started",
            vec![KeyValue::new("message", "Hello from inside the span!")],
        );

        // Create a nested span
        tracer.in_span("nested_operation", |cx| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            // Emit an event with attributes in the nested span
            cx.span().add_event(
                "operation_progress",
                vec![
                    KeyValue::new("message", "Doing some work in a nested span..."),
                    KeyValue::new("duration_ms", 100),
                ],
            );
        });
    });

    // Ensure all spans are exported before exit
    global::shutdown_tracer_provider();

    Ok(())
}
