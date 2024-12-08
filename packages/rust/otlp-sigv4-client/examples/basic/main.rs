use aws_credential_types::provider::ProvideCredentials;
use opentelemetry::{
    global,
    trace::{TraceContextExt, Tracer},
    KeyValue,
};
use opentelemetry_otlp::{HttpExporterBuilder, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    runtime::Tokio,
    trace::{RandomIdGenerator, Sampler},
    Resource,
};
use otlp_sigv4_client::SigV4ClientBuilder;
use reqwest::Client as ReqwestClient;
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

    // Create the SigV4 client
    let sigv4_client = SigV4ClientBuilder::new()
        .with_client(ReqwestClient::new())
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
            request.uri().host().map_or(false, |host| {
                // Sign requests to AWS endpoints (*.amazonaws.com)
                // You might want to be more specific based on your needs
                host.ends_with(".amazonaws.com")
            })
        }))
        .build()?;

    // Configure and build the OTLP exporter
    let exporter = HttpExporterBuilder::default()
        .with_http_client(sigv4_client)
        .with_endpoint("https://xray.us-east-1.amazonaws.com")
        .build_span_exporter()?;

    // Initialize the tracer
    let tracer = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(Resource::new(vec![
            KeyValue::new("service.name", "example-service"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ]))
        .with_batch_exporter(exporter, Tokio)
        .build();

    // Set the tracer as the global tracer
    global::set_tracer_provider(tracer);

    // Get a tracer from the provider
    let tracer = global::tracer("example");

    // Create a span using in_span
    tracer.in_span("main", |cx| {
        // Add some attributes to the span
        cx.span()
            .set_attribute(KeyValue::new("example.key", "example.value"));

        // Do some work...
        std::thread::sleep(std::time::Duration::from_secs(1));
    });

    // Shut down the tracer provider
    global::shutdown_tracer_provider();
    println!("Traces sent to AWS X-Ray successfully");

    Ok(())
}
