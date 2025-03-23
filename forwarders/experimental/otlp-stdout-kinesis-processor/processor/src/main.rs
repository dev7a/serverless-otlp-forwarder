//! AWS Lambda function that forwards Kinesis wrapped OTLP records to OpenTelemetry collectors.
//!
//! This Lambda function:
//! 1. Receives Kinesis events containing otlp-stdout format records
//! 2. Decodes and decompresses the data
//! 3. Converts records to TelemetryData with binary protobuf format
//! 4. Compacts multiple records into a single payload
//! 5. Forwards the data to collectors
//!
//! The function supports:
//! - Multiple collectors with different endpoints
//! - Custom headers and authentication
//! - Base64 encoded payloads
//! - Gzip compressed data
//! - OpenTelemetry instrumentation

use anyhow::{Context, Result};
use lambda_runtime::{tower::ServiceBuilder, Error as LambdaError, LambdaEvent, Runtime};
use std::sync::Arc;

use aws_credential_types::provider::ProvideCredentials;
use lambda_otlp_forwarder::{
    collectors::Collectors,
    processing::process_telemetry_batch,
    span_compactor::{compact_telemetry_payloads, SpanCompactionConfig},
    telemetry::TelemetryData,
    AppState, KinesisEventWrapper,
};
use otlp_sigv4_client::SigV4ClientBuilder;

use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};

use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::trace::BatchSpanProcessor;

/// Convert a Kinesis record into TelemetryData
fn convert_kinesis_record(
    record: &aws_lambda_events::event::kinesis::KinesisEventRecord,
) -> Result<TelemetryData> {
    let data = String::from_utf8(record.kinesis.data.0.clone())
        .context("Failed to decode Kinesis record data as UTF-8")?;

    // Parse as a standard LogRecord
    let log_record = serde_json::from_str(&data)
        .with_context(|| format!("Failed to parse Kinesis record: {}", data))?;

    // Convert to TelemetryData (will be in uncompressed protobuf format)
    TelemetryData::from_log_record(log_record)
}

async fn function_handler(
    event: LambdaEvent<KinesisEventWrapper>,
    state: Arc<AppState>,
) -> Result<(), LambdaError> {
    tracing::debug!("Function handler started");

    // Check and refresh collectors cache if stale
    Collectors::init(&state.secrets_client).await?;

    let records = &event.payload.0.records;

    // Convert all records to TelemetryData (sequentially)
    let telemetry_batch: Vec<TelemetryData> = records
        .iter()
        .filter_map(|record| match convert_kinesis_record(record) {
            Ok(telemetry) => Some(telemetry),
            Err(e) => {
                tracing::warn!("Failed to convert Kinesis record: {}", e);
                None
            }
        })
        .collect();

    // If we have telemetry data, process it
    if !telemetry_batch.is_empty() {
        tracing::info!("Processing {} telemetry records", telemetry_batch.len());

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
    } else {
        tracing::info!("No valid telemetry records to process");
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
            request.uri().host().is_some_and(|host| host.ends_with(".amazonaws.com"))
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
    use aws_lambda_events::event::kinesis::{KinesisEventRecord, KinesisRecord};
    use aws_lambda_events::kinesis::KinesisEncryptionType;

    #[test]
    fn test_convert_kinesis_record() {
        use aws_lambda_events::encodings::SecondTimestamp;
        use chrono::{TimeZone, Utc};
        // This is the raw string that the extension sends to Kinesis
        let record_str = r#"{
            "__otel_otlp_stdout": "0.2.2",
            "source": "test-service",
            "endpoint": "http://example.com",
            "method": "POST",
            "payload": {"resourceSpans": []},
            "headers": {
                "content-type": "application/json"
            },
            "content-type": "application/json",
            "content-encoding": null,
            "base64": false
        }"#;

        let record = KinesisEventRecord {
            kinesis: KinesisRecord {
                data: aws_lambda_events::encodings::Base64Data(record_str.as_bytes().to_vec()),
                partition_key: "test-key".to_string(),
                sequence_number: "test-sequence".to_string(),
                kinesis_schema_version: Some("1.0".to_string()),
                encryption_type: KinesisEncryptionType::None,
                approximate_arrival_timestamp: SecondTimestamp(
                    Utc.timestamp_opt(1234567890, 0).unwrap(),
                ),
            },
            aws_region: Some("us-east-1".to_string()),
            event_id: Some("test-event-id".to_string()),
            event_name: Some("aws:kinesis:record".to_string()),
            event_source: Some("aws:kinesis".to_string()),
            event_source_arn: Some(
                "arn:aws:kinesis:us-east-1:123456789012:stream/test-stream".to_string(),
            ),
            event_version: Some("1.0".to_string()),
            invoke_identity_arn: Some("arn:aws:iam::123456789012:role/test-role".to_string()),
        };

        let result = convert_kinesis_record(&record);
        if let Err(e) = &result {
            println!("Error converting Kinesis record: {}", e);
        }
        assert!(result.is_ok());
        let telemetry = result.unwrap();
        assert_eq!(telemetry.source, "test-service");
        assert_eq!(telemetry.content_type, "application/x-protobuf");
        assert_eq!(telemetry.content_encoding, None); // No compression at this stage
    }

    #[test]
    fn test_convert_kinesis_record_invalid_json() {
        use aws_lambda_events::encodings::SecondTimestamp;
        use chrono::{TimeZone, Utc};

        let invalid_record_str = "invalid json";

        let record = KinesisEventRecord {
            kinesis: KinesisRecord {
                data: aws_lambda_events::encodings::Base64Data(
                    invalid_record_str.as_bytes().to_vec(),
                ),
                partition_key: "test-key".to_string(),
                sequence_number: "test-sequence".to_string(),
                kinesis_schema_version: Some("1.0".to_string()),
                encryption_type: KinesisEncryptionType::None,
                approximate_arrival_timestamp: SecondTimestamp(
                    Utc.timestamp_opt(1234567890, 0).unwrap(),
                ),
            },
            aws_region: Some("us-east-1".to_string()),
            event_id: Some("test-event-id".to_string()),
            event_name: Some("aws:kinesis:record".to_string()),
            event_source: Some("aws:kinesis".to_string()),
            event_source_arn: Some(
                "arn:aws:kinesis:us-east-1:123456789012:stream/test-stream".to_string(),
            ),
            event_version: Some("1.0".to_string()),
            invoke_identity_arn: Some("arn:aws:iam::123456789012:role/test-role".to_string()),
        };

        let result = convert_kinesis_record(&record);
        assert!(result.is_err());
    }
}
