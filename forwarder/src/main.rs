//! AWS Lambda function that forwards CloudWatch logs to OpenTelemetry collectors.
//!
//! This Lambda function:
//! 1. Receives CloudWatch log events
//! 2. Decodes and decompresses the log data
//! 3. Matches each log record with an appropriate collector
//! 4. Forwards the logs to their respective collectors with proper headers
//!
//! The function supports:
//! - Multiple collectors with different endpoints
//! - Custom headers and authentication
//! - Base64 encoded payloads
//! - Gzip compressed data
//! - OpenTelemetry instrumentation

use anyhow::{Context, Result};
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use base64::{engine::general_purpose, Engine};
use flate2::read::GzDecoder;
use futures::future::join_all;
use lambda_otel_utils::{HttpTracerProviderBuilder, OpenTelemetrySubscriberBuilder};
use lambda_runtime::{
    layers::{OpenTelemetryFaasTrigger, OpenTelemetryLayer as OtelLayer},
    Error as LambdaError, LambdaEvent, Runtime,
};
use opentelemetry::trace::SpanKind;
use otlp_stdout_client::LogRecord;
use reqwest::{header::HeaderMap, Client as ReqwestClient};
use serde_json::Value;
use std::io::Read;
use std::sync::Arc;
use tracing::{instrument, Instrument};
mod collectors;
mod headers;
use aws_credential_types::{provider::ProvideCredentials, Credentials};
use collectors::Collectors;
use otlp_sigv4_client::SigV4ClientBuilder;

/// Shared application state across Lambda invocations
struct AppState {
    http_client: ReqwestClient,
    credentials: Credentials,
    secrets_client: SecretsManagerClient,
    region: String,
}

impl AppState {
    async fn new() -> Result<Self, LambdaError> {
        let config = aws_config::load_from_env().await;
        let credentials = config
            .credentials_provider()
            .expect("No credentials provider found")
            .provide_credentials()
            .await?;
        let region = config.region().expect("No region found").to_string();

        Ok(Self {
            http_client: ReqwestClient::new(),
            credentials,
            secrets_client: SecretsManagerClient::new(&config),
            region,
        })
    }
}

/// Decodes a payload from a log record.
///
/// # Arguments
/// * `log_record` - The log record containing the payload
///
/// # Returns
/// * `Result<Vec<u8>>` - The decoded payload as bytes
///
/// # Details
/// - If payload is a string: decodes from base64 if base64 flag is true
/// - If payload is a JSON value: serializes to string first
#[instrument(skip_all)]
fn decode_payload(log_record: &LogRecord) -> Result<Vec<u8>, anyhow::Error> {
    let payload = match &log_record.payload {
        Value::String(s) => s.to_string(),
        _ => serde_json::to_string(&log_record.payload)
            .context("Failed to serialize JSON payload")?,
    };

    match log_record.base64 {
        Some(true) => general_purpose::STANDARD
            .decode(&payload)
            .context("Failed to decode base64 payload"),
        _ => Ok(payload.as_bytes().to_vec()),
    }
}

/// Sends an HTTP POST request to the specified endpoint with the given headers and payload.
/// Includes OpenTelemetry instrumentation for request tracking.
#[instrument(skip_all, fields(
    otel.kind = ?SpanKind::Client,
    otel.status_code,
    http.method = "POST",
    http.url = endpoint,
    http.request.headers.content_type,
    http.request.headers.content_encoding,
    http.status_code,
))]
async fn send_event(
    client: &ReqwestClient,
    endpoint: &str,
    headers: HeaderMap,
    payload: Vec<u8>,
) -> Result<(), anyhow::Error> {
    let current_span = tracing::Span::current();
    headers
        .get("content-type")
        .map(|ct| current_span.record("http.request.headers.content_type", ct.as_bytes()));
    headers
        .get("content-encoding")
        .map(|ce| current_span.record("http.request.headers.content_encoding", ce.as_bytes()));
    let response = client
        .post(endpoint)
        .headers(headers.clone())
        .body(payload.clone())
        .send()
        .await
        .context("Failed to send POST request")?;

    let status = response.status();

    // Record the HTTP status code
    current_span.record("http.status_code", status.as_u16());

    if !status.is_success() {
        current_span.record("otel.status_code", "ERROR");
        let error_body = match response.text().await {
            Ok(text) => text,
            Err(_) => "Could not read response body".to_string(),
        };
        tracing::warn!(
            name = "error posting log record",
            endpoint = endpoint,
            status = status.as_u16(),
            status_text = %status.canonical_reason().unwrap_or("Unknown status"),
            error = %error_body,
        );
    }

    Ok(())
}

/// Processes a single CloudWatch log event by:
/// 1. Extracting and parsing the log record from the event message
/// 2. Decoding its payload
/// 3. Getting all configured collectors
/// 4. Building request headers for each collector
/// 5. Sending the request to all collectors in parallel
async fn process_log_event(
    event: &Value,
    client: &ReqwestClient,
    credentials: &Credentials,
    region: &str,
) -> Result<()> {
    let record = event["message"]
        .as_str()
        .context("Log event 'message' field is not a string")?;

    let log_record: LogRecord = serde_json::from_str(record)
        .with_context(|| format!("Failed to parse log record: {}", record))?;

    // Record the source field for tracing
    tracing::Span::current().record("event.source", &log_record.source);

    tracing::debug!("Processing log event");

    let decoded_payload = decode_payload(&log_record)?;

    // Get all collectors with proper signal paths
    let collectors = Collectors::get_signal_endpoints(&log_record.endpoint)?;

    // Create futures for sending to each collector
    let futures: Vec<_> = collectors
        .into_iter()
        .map(|collector| -> Result<_, anyhow::Error> {
            let client = client.clone();
            let decoded_payload = decoded_payload.clone();
            let headers = match headers::LogRecordHeaders::new()
                .with_log_record(&log_record)
                .and_then(|h| {
                    h.with_collector_auth(&collector, &decoded_payload, credentials, region)
                }) {
                Ok(h) => h.build(),
                Err(e) => {
                    tracing::error!("Failed to build headers: {}", e);
                    return Err(e);
                }
            };

            Ok(async move {
                if let Err(e) =
                    send_event(&client, &collector.endpoint, headers, decoded_payload).await
                {
                    tracing::warn!("Failed to send to collector {}: {}", collector.name, e);
                    return Err(e);
                }
                Ok(())
            })
        })
        .filter_map(Result::ok)
        .collect();

    let results = join_all(futures).await;

    let results: Vec<Result<(), _>> = results.into_iter().collect();

    match results.iter().find(|r| r.is_ok()) {
        Some(_) => Ok(()), // At least one success
        None => {
            // Get the last error, if any
            let last_error = results
                .into_iter()
                .filter_map(|r| r.err())
                .last()
                .map(|e| format!("Last error: {}", e))
                .unwrap_or_else(|| "No error details".to_string());

            Err(anyhow::anyhow!("All collectors failed. {}", last_error))
        }
    }
}

#[instrument(skip_all, name = "function_handler")]
async fn function_handler(
    event: LambdaEvent<Value>,
    state: Arc<AppState>,
) -> Result<(), LambdaError> {
    tracing::debug!("Function handler started");

    // Check and refresh collectors cache if stale
    Collectors::init(&state.secrets_client).await?;

    // Extract and process the CloudWatch Logs data
    let decoded = general_purpose::STANDARD
        .decode(
            event
                .payload
                .get("awslogs")
                .context("No 'awslogs' object found in the payload")?
                .get("data")
                .and_then(|d| d.as_str())
                .context("No 'data' field found in awslogs or it's not a string")?,
        )
        .context("Failed to decode base64")?;

    // Decompress the gzipped log data
    let mut decoder = GzDecoder::new(&decoded[..]);
    let mut decompressed = String::new();
    decoder
        .read_to_string(&mut decompressed)
        .context("Failed to decompress data")?;

    // Parse the JSON log data
    let log_data: Value =
        serde_json::from_str(&decompressed).context("Failed to parse decompressed data as JSON")?;

    // Process each log event
    let log_events = log_data["logEvents"]
        .as_array()
        .context("No logEvents array found in the data")?;
    tracing::debug!("Processing {} log events", log_events.len());

    // Create an Arc<Vec<Value>> to share ownership of log_events
    let log_events = Arc::new(log_events.clone());

    let client = state.http_client.clone();
    let credentials = state.credentials.clone();
    let region = state.region.clone();
    let tasks: Vec<_> = log_events
        .iter()
        .map(|event| {
            let client = client.clone();
            let event = event.clone();
            let credentials = credentials.clone();
            let region = region.clone();
            async move {
                let result = process_log_event(&event, &client, &credentials, &region).await;
                if let Err(e) = &result {
                    tracing::warn!("Error processing log event: {}", e);
                }
                result
            }
            .instrument(tracing::info_span!(
                "process_log_event",
                event.source = tracing::field::Empty
            ))
        })
        .map(|future| tokio::spawn(future.in_current_span()))
        .collect();

    let results = join_all(tasks).await;
    for result in results {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::error!("Task error: {:?}", e),
            Err(e) => tracing::error!("Task panicked: {:?}", e),
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    let config = aws_config::load_from_env().await;
    let region = config.region().expect("No region found");
    let credentials = config
        .credentials_provider()
        .expect("No credentials provider found")
        .provide_credentials()
        .await?;

    // Initialize tracer provider
    let tracer_provider = HttpTracerProviderBuilder::default()
        .with_http_client(
            SigV4ClientBuilder::new()
                .with_client(ReqwestClient::new())
                .with_credentials(credentials)
                .with_region(region.to_string())
                .with_service("xray")
                .with_signing_predicate(Box::new(|request| {
                    // Only sign requests to AWS endpoints
                    request.uri().host().map_or(false, |host| {
                        host.ends_with(".amazonaws.com")
                    })
                }))
                .build()?,
        )
        .with_batch_exporter()
        .enable_global(true)
        .build()?;

    // Initialize the OpenTelemetry subscriber
    OpenTelemetrySubscriberBuilder::new()
        .with_env_filter(true)
        .with_tracer_provider(tracer_provider.clone())
        .with_service_name("lambda-otlp-forwarder")
        .init()?;

    // Initialize shared application state
    let state = Arc::new(AppState::new().await?);

    // Initialize collectors using state's secrets client
    Collectors::init(&state.secrets_client).await?;

    Runtime::new(lambda_runtime::service_fn(|event| {
        let state = Arc::clone(&state);
        async move { function_handler(event, state).await }
    }))
    .layer(
        OtelLayer::new(|| {
            tracer_provider.force_flush();
        })
        .with_trigger(OpenTelemetryFaasTrigger::PubSub),
    )
    .run()
    .await
}
