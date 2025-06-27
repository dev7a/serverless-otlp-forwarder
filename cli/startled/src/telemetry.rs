use anyhow::Result;
use aws_credential_types::provider::ProvideCredentials;
use opentelemetry::global;
use opentelemetry::propagation::TextMapCompositePropagator;
use opentelemetry::trace::TracerProvider;
use opentelemetry_aws::trace::{XrayIdGenerator, XrayPropagator};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use otlp_sigv4_client::SigV4ClientBuilder;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize basic tracing for commands that don't need OpenTelemetry
///
/// This is useful for the report command to enable debug logging without AWS dependencies
pub fn init_tracing() {
    let show_console = std::env::var("TRACING_STDOUT")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if show_console {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::Layer::default())
            .init();
    } else {
        tracing_subscriber::registry().with(env_filter).init();
    }
}

/// Initialize OpenTelemetry with configuration from environment variables
///
/// Environment variables:
/// - OTEL_EXPORTER_OTLP_ENDPOINT: The OTLP endpoint URL (default: AWS App Signals endpoint)
/// - OTEL_SERVICE_NAME: Service name for telemetry (default: "lambda-benchmark")
/// - OTEL_EXPORTER_OTLP_PROTOCOL: Protocol to use (http/protobuf or http/json)
/// - AWS_REGION: AWS region for signing requests
/// - RUST_LOG: Log level (e.g. "info" to see telemetry data)
pub async fn init_telemetry() -> Result<SdkTracerProvider> {
    let config = aws_config::load_from_env().await;
    let region = config.region().expect("AWS region is required").to_string();

    let credentials = config
        .credentials_provider()
        .expect("AWS credentials provider is required")
        .provide_credentials()
        .await?;

    // Build HTTP client with AWS SigV4 signing
    let http_client = SigV4ClientBuilder::new()
        .with_client(
            // This is a blocking call, so we need to spawn a thread to run it, and is required since otel 0.28.0
            std::thread::spawn(move || {
                reqwest::blocking::Client::builder()
                    .build()
                    .expect("Failed to build HTTP client")
            })
            .join()
            .expect("Failed to join HTTP client thread"),
        )
        .with_credentials(credentials)
        .with_region(region)
        .with_service("xray") // For AWS App Signals
        .with_signing_predicate(Box::new(|request| {
            // Only sign requests to AWS endpoints
            request
                .uri()
                .host()
                .is_some_and(|host| host.ends_with(".amazonaws.com"))
        }))
        .build()?;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_http_client(http_client)
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(std::time::Duration::from_secs(3))
        .build()?;

    let tracer_provider = SdkTracerProvider::builder()
        .with_id_generator(XrayIdGenerator::default())
        .with_batch_exporter(exporter)
        .build();

    let tracer = tracer_provider.tracer(env!("CARGO_PKG_NAME"));

    let show_console = std::env::var("TRACING_STDOUT")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false); // default: OFF

    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(OpenTelemetryLayer::new(tracer));

    // Conditionally add console layer
    if show_console {
        subscriber
            .with(tracing_subscriber::fmt::Layer::default())
            .init();
    } else {
        subscriber.init();
    }

    // Create a composite propagator with both W3C and X-Ray propagators
    let composite_propagator = TextMapCompositePropagator::new(vec![
        Box::new(TraceContextPropagator::new())
            as Box<dyn opentelemetry::propagation::TextMapPropagator + Send + Sync>,
        Box::new(XrayPropagator::default())
            as Box<dyn opentelemetry::propagation::TextMapPropagator + Send + Sync>,
    ]);

    // Set the composite propagator as the global propagator
    global::set_text_map_propagator(composite_propagator);

    // Initialize the OpenTelemetry subscriber
    Ok(tracer_provider)
}
