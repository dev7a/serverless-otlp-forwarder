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

use anyhow::Result;
use aws_credential_types::provider::ProvideCredentials;
use aws_lambda_events::event::cloudwatch_logs::LogEntry;
use otlp_sigv4_client::SigV4ClientBuilder;
use otlp_stdout_span_exporter::ExporterOutput;
use serverless_otlp_forwarder::{
    collectors::Collectors,
    processing::process_telemetry_batch,
    span_compactor::{compact_telemetry_payloads, SpanCompactionConfig},
    telemetry::TelemetryData,
    AppState, LogsEventWrapper,
};

use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};

use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::trace::BatchSpanProcessor;

use lambda_runtime::{tower::ServiceBuilder, Error as LambdaError, LambdaEvent, Runtime};
use std::sync::Arc;
/// Convert a CloudWatch log event into TelemetryData
fn convert_log_event(event: &LogEntry) -> Result<TelemetryData> {
    let log_record = &event.message;

    tracing::debug!("Received log record: {}", log_record);

    // Parse the JSON into a serde_json::Value first
    let record: ExporterOutput = match serde_json::from_str(log_record) {
        Ok(output) => output,
        Err(err) => {
            return Err(anyhow::anyhow!(
                "Failed to parse log record as JSON: {} - Error details: {}",
                log_record,
                err
            ));
        }
    };

    tracing::debug!(
        "Successfully parsed log record with version: {}",
        record.version
    );

    // Convert to TelemetryData (will be in uncompressed protobuf format)
    TelemetryData::from_log_record(record)
}

async fn function_handler(
    event: LambdaEvent<LogsEventWrapper>,
    state: Arc<AppState>,
) -> Result<(), LambdaError> {
    tracing::debug!("Function handler started");

    // Check and refresh collectors cache if stale
    Collectors::init(&state.secrets_client).await?;

    let log_events = event.payload.0.aws_logs.data.log_events;

    // Convert all events to TelemetryData (sequentially)
    let telemetry_batch: Vec<TelemetryData> = log_events
        .iter()
        .filter_map(|event| match convert_log_event(event) {
            Ok(telemetry) => Some(telemetry),
            Err(e) => {
                tracing::warn!("Failed to convert span event: {}", e);
                None
            }
        })
        .collect();

    // If we have telemetry data, process it
    if !telemetry_batch.is_empty() {
        // Compact multiple payloads into a single one
        // This will also apply compression to the final result
        let compacted_telemetry =
            match compact_telemetry_payloads(telemetry_batch, &SpanCompactionConfig::default()) {
                Ok(telemetry) => vec![telemetry],
                Err(e) => {
                    tracing::error!("Failed to compact telemetry payloads: {}", e);
                    return Err(e);
                }
            };

        // Process the compacted telemetry (single POST request)
        process_telemetry_batch(
            compacted_telemetry,
            &state.http_client,
            &state.credentials,
            &state.region,
        )
        .await?;
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
            if let Some(host) = request.uri().host() {
                host.ends_with(".amazonaws.com")
            } else {
                false
            }
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
    use base64::{engine::general_purpose, Engine};
    use flate2::{write::GzEncoder, Compression};
    use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
    use prost::Message;
    use serde_json::json;
    use std::io::Write;

    // Helper function to create gzipped, base64-encoded protobuf data
    fn create_test_payload() -> String {
        // Create a minimal valid OTLP protobuf payload
        let request = ExportTraceServiceRequest {
            resource_spans: vec![],
        };

        // Convert to protobuf bytes
        let proto_bytes = request.encode_to_vec();

        // Compress with gzip
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&proto_bytes).unwrap();
        let compressed_bytes = encoder.finish().unwrap();

        // Base64 encode
        general_purpose::STANDARD.encode(compressed_bytes)
    }

    // Helper function to create a test log entry
    fn create_test_log_entry(message: String) -> LogEntry {
        LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message,
        }
    }

    #[test]
    fn test_convert_log_event() {
        // Test standard LogRecord with valid OTLP structure
        let log_record = json!({
            "__otel_otlp_stdout": "otlp-stdout-span-exporter@0.2.2",
            "source": "test-service",
            "endpoint": "http://example.com",
            "method": "POST",
            "payload": create_test_payload(),
            "headers": {
                "content-type": "application/x-protobuf"
            },
            "content-type": "application/x-protobuf",
            "content-encoding": "gzip",
            "base64": true
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
        assert_eq!(telemetry.content_type, "application/x-protobuf"); // Now converted to protobuf
        assert_eq!(telemetry.content_encoding, None); // No compression at this stage
    }

    #[test]
    fn test_convert_uncompressed_payload() {
        use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};

        // Create a simple uncompressed protobuf payload
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        name: "test-span".to_string(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        // Convert to protobuf bytes without compression
        let proto_bytes = request.encode_to_vec();

        // Base64 encode the uncompressed bytes
        let uncompressed_payload = general_purpose::STANDARD.encode(&proto_bytes);

        // Create the log record
        let log_record = json!({
            "__otel_otlp_stdout": "otlp-stdout-span-exporter@0.2.2",
            "source": "test-service",
            "endpoint": "http://example.com",
            "method": "POST",
            "payload": uncompressed_payload,
            "headers": {
                "content-type": "application/x-protobuf"
            },
            "content-type": "application/x-protobuf",
            "content-encoding": "identity", // indicates no compression
            "base64": true
        });

        let event = create_test_log_entry(serde_json::to_string(&log_record).unwrap());

        let result = convert_log_event(&event);
        assert!(result.is_ok());

        let telemetry = result.unwrap();
        assert_eq!(telemetry.source, "test-service");
        assert_eq!(telemetry.content_type, "application/x-protobuf");
        assert_eq!(telemetry.content_encoding, None); // No compression

        // Verify we can decode the payload
        let decoded = ExportTraceServiceRequest::decode(telemetry.payload.as_slice()).unwrap();
        assert_eq!(decoded.resource_spans.len(), 1);

        // Verify the span content is preserved
        let span = &decoded.resource_spans[0].scope_spans[0].spans[0];
        assert_eq!(span.name, "test-span");
    }

    #[test]
    fn test_convert_json_payload() {
        // Create a JSON payload (not protobuf)
        let json_payload = json!({
            "resourceSpans": [{
                "scopeSpans": [{
                    "spans": [{
                        "name": "json-test-span"
                    }]
                }]
            }]
        });

        let json_bytes = serde_json::to_vec(&json_payload).unwrap();
        let encoded_json = general_purpose::STANDARD.encode(&json_bytes);

        // Create the log record with JSON content type
        let log_record = json!({
            "__otel_otlp_stdout": "otlp-stdout-span-exporter@0.2.2",
            "source": "test-service",
            "endpoint": "http://example.com",
            "method": "POST",
            "payload": encoded_json,
            "headers": {
                "content-type": "application/json"
            },
            "content-type": "application/json",
            "content-encoding": "identity",
            "base64": true
        });

        let event = create_test_log_entry(serde_json::to_string(&log_record).unwrap());

        let result = convert_log_event(&event);
        assert!(result.is_ok());

        let telemetry = result.unwrap();
        assert_eq!(telemetry.content_type, "application/x-protobuf"); // Should be converted to protobuf

        // Verify we can decode the converted payload as protobuf
        let decoded = ExportTraceServiceRequest::decode(telemetry.payload.as_slice()).unwrap();
        assert_eq!(decoded.resource_spans.len(), 1);

        // Verify the span content is preserved after JSON->protobuf conversion
        let span = &decoded.resource_spans[0].scope_spans[0].spans[0];
        assert_eq!(span.name, "json-test-span");
    }

    #[test]
    fn test_end_to_end_data_integrity() {
        use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};

        // Create test data with specific identifiable content
        let test_span_name = "unique-identifier-span-name-123";
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        name: test_span_name.to_string(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        // Convert to protobuf bytes
        let proto_bytes = request.encode_to_vec();

        // Compress with gzip
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&proto_bytes).unwrap();
        let compressed_bytes = encoder.finish().unwrap();

        // Base64 encode
        let encoded_payload = general_purpose::STANDARD.encode(&compressed_bytes);

        // Create the log record
        let log_record = json!({
            "__otel_otlp_stdout": "otlp-stdout-span-exporter@0.2.2",
            "source": "test-service",
            "endpoint": "http://example.com",
            "method": "POST",
            "payload": encoded_payload,
            "headers": {
                "content-type": "application/x-protobuf"
            },
            "content-type": "application/x-protobuf",
            "content-encoding": "gzip",
            "base64": true
        });

        let event = create_test_log_entry(serde_json::to_string(&log_record).unwrap());

        // Process through our conversion function
        let result = convert_log_event(&event);
        assert!(result.is_ok());

        let telemetry = result.unwrap();

        // Decode the output payload
        let decoded = ExportTraceServiceRequest::decode(telemetry.payload.as_slice()).unwrap();

        // Verify the data integrity - the unique span name should be preserved
        assert_eq!(decoded.resource_spans.len(), 1);
        let output_span = &decoded.resource_spans[0].scope_spans[0].spans[0];
        assert_eq!(output_span.name, test_span_name);
    }

    #[test]
    fn test_malformed_json() {
        // Test with invalid JSON
        let event = LogEntry {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            message: "This is not valid JSON".to_string(),
        };

        let result = convert_log_event(&event);
        assert!(result.is_err());

        // The error should contain a helpful message
        let error_msg = result.err().unwrap().to_string();
        assert!(error_msg.contains("Failed to parse log record as JSON"));
    }

    #[test]
    fn test_non_base64_payload() {
        // Create a plain text payload that is not base64 encoded
        let plain_text = "This is a test payload that is not base64 encoded";

        // Create the log record with base64 flag set to false
        let log_record = json!({
            "__otel_otlp_stdout": "otlp-stdout-span-exporter@0.2.2",
            "source": "test-service",
            "endpoint": "http://example.com",
            "method": "POST",
            "payload": plain_text,
            "headers": {
                "content-type": "text/plain"
            },
            "content-type": "text/plain",
            "content-encoding": "identity",
            "base64": false
        });

        let event = create_test_log_entry(serde_json::to_string(&log_record).unwrap());

        let result = convert_log_event(&event);
        // This should process without error even though it's not a protobuf format
        assert!(result.is_ok());

        let telemetry = result.unwrap();
        // The content type would still be set to protobuf as that's our standard format
        assert_eq!(telemetry.content_type, "application/x-protobuf");
    }
}
