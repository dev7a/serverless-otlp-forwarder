use aws_credential_types::provider::ProvideCredentials;
use opentelemetry::{
    global,
    trace::{TraceContextExt, Tracer},
    KeyValue,
};
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler},
    Resource,
};
use otlp_sigv4_client::SigV4ClientBuilder;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Load AWS configuration from the environment and credentials provider chain
    let config = aws_config::load_from_env().await;
    let credentials = config
        .credentials_provider()
        .expect("No credentials provider found")
        .provide_credentials()
        .await?;

    // this is required since the breaking changes in otel 0.28.0
    // https://github.com/open-telemetry/opentelemetry-rust/blob/opentelemetry_sdk-0.28.0/opentelemetry-sdk/CHANGELOG.md
    // the http client needs to be a blocking client and to be created in a thread outside of the tokio runtime
    let http_client = std::thread::spawn(move || {
        reqwest::blocking::Client::builder()
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
    .join()
    .unwrap();

    // Create the SigV4 client wrapping the blocking http client
    let sigv4_client = SigV4ClientBuilder::new()
        .with_client(http_client)
        .with_credentials(credentials)
        .with_region(
            config
                .region()
                .map(|r| r.to_string())
                .unwrap_or_else(|| "us-east-1".to_string()),
        )
        .with_service("xray") // AWS X-Ray service name
        .with_signing_predicate(Box::new(|request| {
            // Only sign requests to AWS endpoints
            request.uri().host().is_some_and(|host| {
                // Sign requests to AWS endpoints (*.amazonaws.com)
                // You might want to be more specific based on your needs
                host.ends_with(".amazonaws.com")
            })
        }))
        .build()?;

    // Configure and build the OTLP exporter
    let exporter = SpanExporter::builder()
        .with_http()
        .with_http_client(sigv4_client)
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint("https://xray.us-east-1.amazonaws.com/v1/traces")
        .build()
        .expect("Failed to create trace exporter");

    // Initialize the tracer
    let trace_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(
            Resource::builder_empty()
                .with_attributes(vec![
                    KeyValue::new("service.name", "example-service"),
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ])
                .build(),
        )
        .with_batch_exporter(exporter)
        .build();

    // Set the tracer as the global tracer
    global::set_tracer_provider(trace_provider.clone());

    // Get a tracer from the provider
    let tracer = global::tracer("example");

    // Create a span using in_span
    tracer.in_span("main", |cx| {
        // Add some attributes to the span
        cx.span()
            .set_attribute(KeyValue::new("example.key", "example.value"));

        for i in 0..10 {
            tracer.in_span(format!("child-{}", i), |cx| {
                cx.span().add_event(
                    "child event",
                    vec![
                        KeyValue::new("child.key", "child.value"),
                        KeyValue::new("child.index", i),
                    ],
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            });
        }
    });

    // Flush the traces
    if let Err(e) = trace_provider.force_flush() {
        println!("Error flushing traces: {}", e);
    }

    // Shut down the tracer provider
    let result = trace_provider.shutdown();
    if let Err(e) = result {
        println!("Error shutting down tracer provider: {}", e);
    } else {
        println!("Traces sent to AWS X-Ray successfully");
    }

    Ok(())
}
