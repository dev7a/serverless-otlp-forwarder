//! AWS Lambda function that forwards CloudWatch log wrapped OTLP records to OpenTelemetry collectors.
//!
//! This Lambda function:
//! 1. Receives CloudWatch log events as otlp-stout format
//! 2. Decodes and decompresses the log data
//! 3. Converts logs to TelemetryData
//! 4. Forwards the data to collectors in parallel
//!
//! The function supports:
//! - Multiple collectors with different endpoints
//! - Custom headers and authentication
//! - Base64 encoded payloads
//! - Gzip compressed data
//! - OpenTelemetry instrumentation

use anyhow::{Context, Result};
use aws_lambda_events::event::cloudwatch_logs::{LogEntry, LogsEvent};
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use lambda_otel_utils::{HttpTracerProviderBuilder, OpenTelemetrySubscriberBuilder};
use lambda_runtime::{
    layers::{OpenTelemetryFaasTrigger, OpenTelemetryLayer as OtelLayer},
    Error as LambdaError, LambdaEvent, Runtime,
};
use reqwest::Client as ReqwestClient;
use std::sync::Arc;
use tracing::instrument;

use aws_credential_types::{provider::ProvideCredentials, Credentials};
use otlp_sigv4_client::SigV4ClientBuilder;
use lambda_otlp_forwarder::{
    collectors::Collectors, processing::process_telemetry_batch, telemetry::TelemetryData,
};

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

/// Convert a CloudWatch log event into TelemetryData
fn convert_log_event(event: &LogEntry) -> Result<TelemetryData> {
    let record = &event.message;

    // Parse as a standard LogRecord
    let log_record = serde_json::from_str(record)
        .with_context(|| format!("Failed to parse log record: {}", record))?;

    // Convert to TelemetryData
    TelemetryData::from_log_record(log_record)
}

#[instrument(skip_all, fields(otel.kind="consumer", forwarder.log.group, forwarder.events.count))]
async fn function_handler(
    event: LambdaEvent<LogsEvent>,
    state: Arc<AppState>,
) -> Result<(), LambdaError> {
    tracing::debug!("Function handler started");

    // Check and refresh collectors cache if stale
    Collectors::init(&state.secrets_client).await?;

    let log_group = event.payload.aws_logs.data.log_group;
    let log_events = event.payload.aws_logs.data.log_events;
    let current_span = tracing::Span::current();
    current_span.record("forwarder.events.count", log_events.len());
    current_span.record("forwarder.log_group", &log_group);
    // Convert all events to TelemetryData (sequentially)
    let telemetry_records = log_events
        .iter()
        .filter_map(|event| match convert_log_event(event) {
            Ok(telemetry) => Some(telemetry),
            Err(e) => {
                tracing::warn!("Failed to convert span event: {}", e);
                None
            }
        })
        .collect();

    // Process all records in parallel
    process_telemetry_batch(
        telemetry_records,
        &state.http_client,
        &state.credentials,
        &state.region,
    )
    .await?;

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

    // Initialize OpenTelemetry
    let tracer_provider = HttpTracerProviderBuilder::default()
        .with_http_client(
            SigV4ClientBuilder::new()
                .with_client(ReqwestClient::new())
                .with_credentials(credentials)
                .with_region(region.to_string())
                .with_service("xray")
                .with_signing_predicate(Box::new(|request| {
                    // Only sign requests to AWS endpoints
                    request
                        .uri()
                        .host()
                        .map_or(false, |host| host.ends_with(".amazonaws.com"))
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
        .with_service_name("lambda-otlp-forwarder-logs")
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_convert_log_event() {
        // Test standard LogRecord
        let log_record = json!({
            "__otel_otlp_stdout": "otlp-stdout-client@0.2.2",
            "source": "test-service",
            "endpoint": "http://example.com",
            "method": "POST",
            "payload": {"test": "data"},
            "headers": {
                "content-type": "application/json"
            },
            "content-type": "application/json",
            "content-encoding": "gzip",
            "base64": false
        });

        let event = LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message: serde_json::to_string(&log_record).unwrap(),
        };

        let result = convert_log_event(&event);
        if let Err(e) = &result {
            println!("Error converting log event: {}", e);
        }
        assert!(result.is_ok());
        let telemetry = result.unwrap();
        assert_eq!(telemetry.source, "test-service");
        assert_eq!(telemetry.content_type, "application/json");
        assert_eq!(telemetry.content_encoding, Some("gzip".to_string()));
    }
}
