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
use aws_credential_types::provider::ProvideCredentials;
use aws_lambda_events::event::cloudwatch_logs::LogEntry;
use lambda_runtime::{tower::ServiceBuilder, Error as LambdaError, LambdaEvent, Runtime};
use otlp_sigv4_client::SigV4ClientBuilder;
use serde_json::Value as JsonValue;
use serverless_otlp_forwarder::{
    collectors::Collectors, processing::process_telemetry_batch, telemetry::TelemetryData,
    AppState, LogsEventWrapper,
};
use std::sync::Arc;

use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};

use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::trace::BatchSpanProcessor;

/// Convert a CloudWatch log event containing a raw span into TelemetryData
fn convert_span_event(event: &LogEntry, log_group: &str) -> Option<TelemetryData> {
    // Parse the raw span
    let span: JsonValue = match serde_json::from_str(&event.message) {
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
            tracing::debug!("Failed to convert span to OTLP protobuf: {}", e);
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

async fn function_handler(
    event: LambdaEvent<LogsEventWrapper>,
    state: Arc<AppState>,
) -> Result<(), LambdaError> {
    tracing::debug!("Function handler started");

    // Check and refresh collectors cache if stale
    Collectors::init(&state.secrets_client).await?;

    let log_group = &event.payload.0.aws_logs.data.log_group;
    let log_events = &event.payload.0.aws_logs.data.log_events;

    // Convert all events to TelemetryData
    let telemetry_records = log_events
        .iter()
        .filter_map(|event| convert_span_event(event, log_group))
        .collect::<Vec<_>>();

    // Only process if we have records
    if !telemetry_records.is_empty() {
        process_telemetry_batch(
            telemetry_records,
            &state.http_client,
            &state.credentials,
            &state.region,
        )
        .await?;
    } else {
        tracing::debug!("No valid telemetry records to process");
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

    let sigv4_client = SigV4ClientBuilder::new()
        .with_client(
            reqwest::blocking::Client::builder()
                .build()
                .map_err(|e| LambdaError::from(format!("Failed to build HTTP client: {}", e)))?,
        )
        .with_credentials(credentials)
        .with_region(region.to_string())
        .with_service("xray")
        .with_signing_predicate(Box::new(|request| {
            // Only sign requests to AWS endpoints
            request
                .uri()
                .host()
                .is_some_and(|host| host.ends_with(".amazonaws.com"))
        }))
        .build()?;

    // Create a new exporter for BatchSpanProcessor
    let batch_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_http_client(sigv4_client)
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(std::time::Duration::from_secs(3))
        .build()?;

    let (_, completion_handler) = init_telemetry(
        TelemetryConfig::builder()
            .with_span_processor(BatchSpanProcessor::builder(batch_exporter).build())
            .build(),
    )
    .await?;

    // Initialize shared application state
    let state = Arc::new(AppState::new().await?);

    // Initialize collectors using state's secrets client
    Collectors::init(&state.secrets_client).await?;

    let service = ServiceBuilder::new()
        .layer(OtelTracingLayer::new(completion_handler))
        .service_fn(|event| {
            let state = Arc::clone(&state);
            async move { function_handler(event, state).await }
        });

    // Create and run the Lambda runtime
    let runtime = Runtime::new(service);
    runtime.run().await
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
            },
            "events": [
                {
                    "timeUnixNano": 1619712000500000000_u64,
                    "name": "Event 1",
                    "attributes": {
                        "event.key1": "value1",
                        "event.key2": 123
                    }
                },
                {
                    "timeUnixNano": 1619712000800000000_u64,
                    "name": "Event 2",
                    "attributes": {
                        "event.key3": "value3"
                    }
                }
            ]
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
