//! AWS Lambda function that forwards CloudWatch logs to OpenTelemetry collectors.
//!
//! This Lambda function:
//! 1. Receives CloudWatch log events as raw JSON OTLP Span
//! 2. Converts logs to TelemetryData
//! 3. Forwards the data to collectors in parallel
//!
//! The function supports:
//! - Multiple collectors with different endpoints
//! - Custom headers and authentication
//! - Base64 encoded payloads
//! - Gzip compressed data
//! - OpenTelemetry instrumentation

mod otlp;

use anyhow::Result;
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
use lambda_otlp_forwarder::{
    collectors::Collectors, processing::process_telemetry_batch, telemetry::TelemetryData,
};
use otlp_sigv4_client::SigV4ClientBuilder;
use serde_json::Value;

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

/// Convert a CloudWatch log event containing a raw span into TelemetryData
fn convert_span_event(event: &LogEntry, log_group: &str) -> Option<TelemetryData> {
    // Parse the raw span
    let span: Value = match serde_json::from_str(&event.message) {
        Ok(span) => span,
        Err(e) => {
            tracing::warn!("Failed to parse span JSON: {}", e);
            return None;
        }
    };

    // Convert directly to OTLP protobuf
    let protobuf_bytes = match otlp::convert_span_to_otlp_protobuf(span) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("Failed to convert span to OTLP protobuf: {}", e);
            return None;
        }
    };

    // Create TelemetryData with the protobuf payload
    Some(TelemetryData {
        source: log_group.to_string(),
        endpoint: "https://localhost:4318/v1/traces".to_string(),
        payload: protobuf_bytes,
        content_type: "application/x-protobuf".to_string(),
        content_encoding: None, // No compression at this stage
    })
}

#[instrument(skip_all, fields(otel.kind="consumer", forwarder.log_group, forwarder.events.count))]
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
        .filter_map(|event| convert_span_event(event, &log_group))
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
        .with_service_name("serverless-otlp-forwarder-spans")
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
    fn test_convert_span_event() {
        // Create a test span with all required fields
        let span_record = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            "kind": "SERVER",
            "startTimeUnixNano": 1619712000000000000_u64,
            "endTimeUnixNano": 1619712001000000000_u64,
            "attributes": {
                "service.name": "test-service"
            },
            "status": {
                "code": "OK"
            },
            "resource": {
                "attributes": {
                    "service.name": "test-service"
                }
            },
            "scope": {
                "name": "test-scope",
                "version": "1.0.0"
            }
        });

        let event = LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message: serde_json::to_string(&span_record).unwrap(),
        };

        let result = convert_span_event(&event, "aws/spans");
        assert!(result.is_some());
        let telemetry = result.unwrap();
        assert_eq!(telemetry.source, "aws/spans");
        assert_eq!(telemetry.content_type, "application/x-protobuf");
        assert_eq!(telemetry.content_encoding, None);
    }

    #[test]
    fn test_convert_span_event_invalid_json() {
        let event = LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message: "invalid json".to_string(),
        };

        let result = convert_span_event(&event, "aws/spans");
        assert!(result.is_none());
    }

    #[test]
    fn test_convert_span_event_missing_endtime() {
        let span_record = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            // endTimeUnixNano is missing
        });

        let event = LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message: serde_json::to_string(&span_record).unwrap(),
        };

        let result = convert_span_event(&event, "aws/spans");
        assert!(result.is_none());
    }

    #[test]
    fn test_convert_span_event_null_endtime() {
        let span_record = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            "endTimeUnixNano": null
        });

        let event = LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message: serde_json::to_string(&span_record).unwrap(),
        };

        let result = convert_span_event(&event, "aws/spans");
        assert!(result.is_none());
    }

    #[test]
    fn test_convert_span_event_complete() {
        // Create a complete test span with all fields
        let span_record = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            "parentSpanId": "fedcba9876543210",
            "kind": "SERVER",
            "startTimeUnixNano": 1619712000000000000_u64,
            "endTimeUnixNano": 1619712001000000000_u64,
            "attributes": {
                "service.name": "test-service",
                "http.method": "GET",
                "http.url": "https://example.com",
                "http.status_code": 200
            },
            "status": {
                "code": "OK"
            },
            "resource": {
                "attributes": {
                    "service.name": "test-service",
                    "service.version": "1.0.0"
                }
            },
            "scope": {
                "name": "test-scope",
                "version": "1.0.0"
            }
        });

        let event = LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message: serde_json::to_string(&span_record).unwrap(),
        };

        let result = convert_span_event(&event, "aws/spans");
        assert!(result.is_some());
        let telemetry = result.unwrap();
        assert_eq!(telemetry.source, "aws/spans");
        assert_eq!(telemetry.content_type, "application/x-protobuf");
        assert_eq!(telemetry.content_encoding, None);
        
        // Verify the payload is not empty
        assert!(!telemetry.payload.is_empty());
    }
}
