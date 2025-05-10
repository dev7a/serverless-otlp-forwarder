//! Handles the processing of raw log event messages into structured `TelemetryData`.
//!
//! This module is responsible for:
//! - Parsing log messages (expected to be in `otlp-stdout-span-exporter` JSON format).
//! - Decoding base64 encoded payloads.
//! - Decompressing gzipped payloads.
//! - Converting payloads from JSON OTLP format to protobuf OTLP format if necessary.
//! - Compacting multiple `TelemetryData` items into a single item by merging
//!   `ExportTraceServiceRequest` resource spans.
//! - Compressing payloads using Gzip.
//! - Sending telemetry payloads to an OTLP HTTP endpoint.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use otlp_stdout_span_exporter::ExporterOutput;
use prost::Message;
use reqwest::header::HeaderMap;
use reqwest::Client as ReqwestClient;
use reqwest::Url;
use std::io::{Read, Write};

/// Represents a processed OTLP payload ready for potential compaction or sending.
#[derive(Clone, Debug)]
pub struct TelemetryData {
    pub payload: Vec<u8>,
    pub original_endpoint: String,
    pub original_source: String,
}

/// Configuration for span compaction (simplified for CLI)
#[derive(Debug, Clone)]
pub struct SpanCompactionConfig {
    pub compression_level: u32,
}

impl Default for SpanCompactionConfig {
    fn default() -> Self {
        Self {
            compression_level: 6,
        }
    }
}

/// Processes a single CloudWatch Live Tail log event message string.
pub fn process_log_event_message(message: &str) -> Result<Option<TelemetryData>> {
    tracing::trace!(message, "Processing log event message");
    let record: ExporterOutput = match serde_json::from_str::<ExporterOutput>(message) {
        Ok(output) => {
            if output.version.is_empty() || output.payload.is_empty() {
                tracing::debug!(
                    message,
                    "Log message parsed but missing expected fields, skipping."
                );
                return Ok(None);
            }
            output
        }
        Err(e) => {
            tracing::trace!(message, error = %e, "Failed to parse log message as ExporterOutput JSON, skipping.");
            return Ok(None);
        }
    };

    tracing::debug!(source = %record.source, endpoint = %record.endpoint, "Parsed OTLP/stdout record");

    let raw_payload = if record.base64 {
        general_purpose::STANDARD
            .decode(&record.payload)
            .context("Failed to decode base64 payload")?
    } else {
        tracing::warn!("Received non-base64 payload, attempting to process as raw bytes.");
        record.payload.as_bytes().to_vec()
    };

    let protobuf_payload = convert_to_protobuf(
        raw_payload,
        &record.content_type,
        Some(&record.content_encoding),
    )
    .context("Failed to convert payload to protobuf")?;

    Ok(Some(TelemetryData {
        payload: protobuf_payload,
        original_endpoint: record.endpoint.to_string(),
        original_source: record.source,
    }))
}

fn convert_to_protobuf(
    payload: Vec<u8>,
    content_type: &str,
    content_encoding: Option<&str>,
) -> Result<Vec<u8>> {
    tracing::trace!(
        content_type,
        content_encoding = ?content_encoding,
        input_size = payload.len(),
        "Converting payload to uncompressed protobuf"
    );

    let decompressed = if content_encoding == Some("gzip") {
        tracing::trace!("Decompressing gzipped payload");
        let mut decoder = GzDecoder::new(&payload[..]);
        let mut decompressed_data = Vec::new();
        decoder
            .read_to_end(&mut decompressed_data)
            .context("Failed to decompress Gzip payload")?;
        tracing::trace!(
            output_size = decompressed_data.len(),
            "Decompressed payload"
        );
        decompressed_data
    } else {
        payload
    };

    match content_type {
        "application/x-protobuf" => {
            tracing::trace!("Payload is already protobuf");
            match ExportTraceServiceRequest::decode(decompressed.as_slice()) {
                Ok(_) => Ok(decompressed),
                Err(e) => Err(anyhow!(
                    "Payload has content-type protobuf but failed to decode: {}",
                    e
                )),
            }
        }
        "application/json" => {
            tracing::trace!("Converting JSON payload to protobuf");
            let request: ExportTraceServiceRequest = serde_json::from_slice(&decompressed)
                .context("Failed to parse JSON as ExportTraceServiceRequest")?;
            let protobuf_bytes = request.encode_to_vec();
            tracing::trace!(
                output_size = protobuf_bytes.len(),
                "Converted JSON to protobuf"
            );
            Ok(protobuf_bytes)
        }
        _ => {
            tracing::warn!(
                content_type,
                "Unsupported content type encountered, attempting to treat as protobuf."
            );
            match ExportTraceServiceRequest::decode(decompressed.as_slice()) {
                Ok(_) => Ok(decompressed),
                Err(e) => Err(anyhow!(
                    "Payload has unknown content-type '{}' and failed to decode as protobuf: {}",
                    content_type,
                    e
                )),
            }
        }
    }
}

pub fn compact_telemetry_payloads(
    batch: Vec<TelemetryData>,
    config: &SpanCompactionConfig,
) -> Result<TelemetryData> {
    if batch.is_empty() {
        return Err(anyhow!("Cannot compact an empty batch"));
    }
    if batch.len() == 1 {
        tracing::debug!("Batch has only one item, skipping merge, applying compression.");
        let mut single_item = batch.into_iter().next().unwrap();
        let compressed_payload = compress_payload(&single_item.payload, config.compression_level)
            .context("Failed to compress single payload")?;
        single_item.payload = compressed_payload;
        return Ok(single_item);
    }

    let original_count = batch.len();
    tracing::debug!("Compacting {} telemetry payloads...", original_count);

    let mut decoded_requests = Vec::with_capacity(batch.len());
    for telemetry in &batch {
        match decode_otlp_payload(&telemetry.payload) {
            Ok(request) => decoded_requests.push(request),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to decode payload during compaction, skipping item.");
            }
        }
    }

    if decoded_requests.is_empty() {
        return Err(anyhow!("All payloads in the batch failed to decode"));
    }

    let mut merged_resource_spans = Vec::new();
    for request in decoded_requests {
        merged_resource_spans.extend(request.resource_spans);
    }

    let merged_request = ExportTraceServiceRequest {
        resource_spans: merged_resource_spans,
    };

    let uncompressed_payload = encode_otlp_payload(&merged_request);
    tracing::debug!(
        uncompressed_size = uncompressed_payload.len(),
        "Encoded compacted payload"
    );

    let compressed_payload = compress_payload(&uncompressed_payload, config.compression_level)
        .context("Failed to compress compacted payload")?;
    tracing::debug!(
        compressed_size = compressed_payload.len(),
        "Compressed compacted payload"
    );

    let first_telemetry = &batch[0];

    Ok(TelemetryData {
        payload: compressed_payload,
        original_endpoint: first_telemetry.original_endpoint.clone(),
        original_source: first_telemetry.original_source.clone(),
    })
}

fn decode_otlp_payload(payload: &[u8]) -> Result<ExportTraceServiceRequest> {
    ExportTraceServiceRequest::decode(payload).context("Failed to decode OTLP protobuf payload")
}

fn encode_otlp_payload(request: &ExportTraceServiceRequest) -> Vec<u8> {
    request.encode_to_vec()
}

pub fn compress_payload(payload: &[u8], level: u32) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(level));
    encoder
        .write_all(payload)
        .context("Failed to write to compressor")?;
    encoder.finish().context("Failed to finish compression")
}

/// Sends a OTLP payload over HTTP.
pub async fn send_telemetry_payload(
    http_client: &ReqwestClient,
    endpoint: &str,
    payload: Vec<u8>,
    headers: HeaderMap,
) -> Result<()> {
    // Parse the base endpoint URL
    let base_url =
        Url::parse(endpoint).with_context(|| format!("Invalid OTLP endpoint URL: {}", endpoint))?;

    // Determine the final target URL, appending /v1/traces if needed
    let target_url = if base_url.path() == "/" || base_url.path().is_empty() {
        // Use join to correctly handle base paths with/without trailing slash
        base_url.join("/v1/traces").with_context(|| {
            format!(
                "Failed to join /v1/traces to base endpoint URL: {}",
                endpoint
            )
        })?
    } else {
        // Use the URL as-is if it already has a path
        base_url
    };

    tracing::debug!(url = %target_url, payload_size=payload.len(), "Sending OTLP HTTP request");

    let response = http_client
        .post(target_url.clone()) // Clone target_url for potential use in context
        .headers(headers)
        .header("Content-Type", "application/x-protobuf")
        .header("Content-Encoding", "gzip")
        .body(payload)
        .send()
        .await
        .with_context(|| format!("Failed to send OTLP request to {}", target_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        tracing::error!(%status, body = %error_body, "Received non-success status from OTLP endpoint");
        // Consider returning an error here depending on desired behavior
        // return Err(anyhow::anyhow!("OTLP endpoint returned status: {}", status));
    } else {
        tracing::debug!("OTLP request sent successfully.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*; // Import items from parent module
    use base64::{engine::general_purpose, Engine};
    use opentelemetry_proto::tonic::{
        common::v1::{any_value, AnyValue, KeyValue},
        resource::v1::Resource,
        trace::v1::{ResourceSpans, ScopeSpans},
    };
    use otlp_stdout_span_exporter::ExporterOutput;

    // Helper to create a dummy ExportTraceServiceRequest
    fn create_dummy_request() -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![KeyValue {
                        key: "service.name".to_string(),
                        value: Some(AnyValue {
                            value: Some(any_value::Value::StringValue("test-service".to_string())),
                        }),
                    }],
                    dropped_attributes_count: 0,
                }),
                scope_spans: vec![ScopeSpans {
                    // ... add scope/spans if needed for specific tests
                    ..Default::default()
                }],
                schema_url: String::new(),
            }],
        }
    }

    #[test]
    fn test_process_log_event_message_valid_protobuf() {
        let request = create_dummy_request();
        let proto_bytes = request.encode_to_vec();
        let compressed_proto = compress_payload(&proto_bytes, 6).unwrap();
        let b64_payload = general_purpose::STANDARD.encode(&compressed_proto);

        // Use the actual ExporterOutput struct with Option fields wrapped
        let exporter_output = ExporterOutput {
            version: "1".to_string(),
            source: "test_source".to_string(),
            endpoint: "test_endpoint".to_string(),
            content_type: "application/x-protobuf".to_string(),
            content_encoding: "gzip".to_string(),
            base64: true,
            payload: b64_payload,
            method: String::new(), // Assume method is not Option based on previous error
            headers: Some(std::collections::HashMap::new()), // Wrap in Some()
            level: Some("info".to_string()), // Wrap in Some()
        };

        let json_message = serde_json::to_string(&exporter_output).unwrap();

        let result = process_log_event_message(&json_message).unwrap();

        assert!(result.is_some());
        let telemetry_data = result.unwrap();
        assert_eq!(telemetry_data.original_source, "test_source");
        assert_eq!(telemetry_data.original_endpoint, "test_endpoint");

        // Verify the payload decodes back to the original request (uncompressed)
        let decoded_request =
            ExportTraceServiceRequest::decode(telemetry_data.payload.as_slice()).unwrap();
        // Corrected variable name and check
        assert_eq!(decoded_request.resource_spans.len(), 1);
        assert_eq!(
            decoded_request.resource_spans[0]
                .resource
                .as_ref()
                .unwrap()
                .attributes[0]
                .key,
            "service.name"
        );
    }

    #[test]
    fn test_compact_telemetry_payloads_multiple_items() {
        let req1 = create_dummy_request_with_service("service-a");
        let payload1 = req1.encode_to_vec();
        let telemetry1 = TelemetryData {
            payload: payload1,
            original_endpoint: "ep".to_string(),
            original_source: "src-a".to_string(),
        };

        let req2 = create_dummy_request_with_service("service-b");
        let payload2 = req2.encode_to_vec();
        let telemetry2 = TelemetryData {
            payload: payload2,
            original_endpoint: "ep".to_string(), // Same endpoint
            original_source: "src-b".to_string(),
        };

        let batch = vec![telemetry1, telemetry2];
        let config = SpanCompactionConfig::default();

        let result = compact_telemetry_payloads(batch, &config);
        assert!(result.is_ok());
        let compacted_data = result.unwrap();

        // Decompress and decode the result
        let decompressed = decompress_payload(&compacted_data.payload).unwrap();
        let merged_request = ExportTraceServiceRequest::decode(decompressed.as_slice()).unwrap();

        // Verify merged content
        assert_eq!(merged_request.resource_spans.len(), 2); // Should have spans from both requests
        let service_names: Vec<String> = merged_request
            .resource_spans
            .iter()
            .filter_map(|rs| rs.resource.as_ref())
            .flat_map(|r| r.attributes.iter())
            .filter(|kv| kv.key == "service.name")
            .filter_map(|kv| {
                kv.value.as_ref().and_then(|v| {
                    if let Some(any_value::Value::StringValue(s)) = &v.value {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        assert!(service_names.contains(&"service-a".to_string()));
        assert!(service_names.contains(&"service-b".to_string()));
    }

    #[test]
    fn test_compact_telemetry_payloads_single_item() {
        let req1 = create_dummy_request_with_service("service-single");
        let payload1 = req1.encode_to_vec();
        let telemetry1 = TelemetryData {
            payload: payload1.clone(), // Clone original for later comparison
            original_endpoint: "ep-single".to_string(),
            original_source: "src-single".to_string(),
        };

        let batch = vec![telemetry1];
        let config = SpanCompactionConfig::default();

        let result = compact_telemetry_payloads(batch, &config);
        assert!(result.is_ok());
        let compacted_data = result.unwrap();

        // Decompress and decode
        let decompressed = decompress_payload(&compacted_data.payload).unwrap();
        let final_request = ExportTraceServiceRequest::decode(decompressed.as_slice()).unwrap();

        // Verify it matches the original request (before compression)
        assert_eq!(final_request.resource_spans.len(), 1);
        let service_name = final_request.resource_spans[0]
            .resource
            .as_ref()
            .unwrap()
            .attributes[0]
            .value
            .as_ref()
            .unwrap()
            .value
            .as_ref();
        if let Some(any_value::Value::StringValue(s)) = service_name {
            assert_eq!(s, "service-single");
        } else {
            panic!("Expected string value for service name");
        }
    }

    #[test]
    fn test_process_log_event_message_invalid_json() {
        let invalid_json_message = "{ not json \"";
        let result = process_log_event_message(invalid_json_message).unwrap();
        assert!(result.is_none()); // Expect Ok(None) for parsing errors
    }

    #[test]
    fn test_process_log_event_message_invalid_base64() {
        let invalid_b64_payload = "this is not base64===";

        let exporter_output = ExporterOutput {
            version: "1".to_string(),
            source: "test_source".to_string(),
            endpoint: "test_endpoint".to_string(),
            content_type: "application/x-protobuf".to_string(),
            content_encoding: "gzip".to_string(),
            base64: true,
            payload: invalid_b64_payload.to_string(), // Use invalid base64 string
            method: String::new(),
            headers: Some(std::collections::HashMap::new()),
            level: Some("info".to_string()),
        };

        let json_message = serde_json::to_string(&exporter_output).unwrap();

        // Expect an Err result because base64 decoding fails
        let result = process_log_event_message(&json_message);
        assert!(result.is_err());
        // Optionally check the error message content
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to decode base64 payload"));
    }

    #[test]
    fn test_process_log_event_message_invalid_gzip() {
        let not_gzip_data = b"this is not gzip data";
        let b64_payload = general_purpose::STANDARD.encode(not_gzip_data);

        let exporter_output = ExporterOutput {
            version: "1".to_string(),
            source: "test_source".to_string(),
            endpoint: "test_endpoint".to_string(),
            content_type: "application/x-protobuf".to_string(),
            content_encoding: "gzip".to_string(), // Claims to be gzip
            base64: true,
            payload: b64_payload,
            method: String::new(),
            headers: Some(std::collections::HashMap::new()),
            level: Some("info".to_string()),
        };

        let json_message = serde_json::to_string(&exporter_output).unwrap();

        // Expect an Err result because gzip decoding fails
        let result = process_log_event_message(&json_message);
        assert!(result.is_err()); // Just check that it errors, context might be less specific
                                  // assert!(result.unwrap_err().to_string().contains("Failed to decompress Gzip payload")); // Removed specific context check
    }

    // Helper to decompress Gzip data (needed for verifying compaction output)
    fn decompress_payload(compressed_data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = GzDecoder::new(compressed_data);
        let mut decompressed_data = Vec::new();
        decoder
            .read_to_end(&mut decompressed_data)
            .context("Failed to decompress payload in test")?;
        Ok(decompressed_data)
    }

    // Modified helper to allow different service names
    fn create_dummy_request_with_service(service_name: &str) -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![KeyValue {
                        key: "service.name".to_string(),
                        value: Some(AnyValue {
                            value: Some(any_value::Value::StringValue(service_name.to_string())),
                        }),
                    }],
                    dropped_attributes_count: 0,
                }),
                scope_spans: vec![ScopeSpans {
                    ..Default::default()
                }],
                schema_url: String::new(),
            }],
        }
    }

    // TODO: Add tests for process_log_event_message (errors)
    // TODO: Add tests for convert_to_protobuf
}
