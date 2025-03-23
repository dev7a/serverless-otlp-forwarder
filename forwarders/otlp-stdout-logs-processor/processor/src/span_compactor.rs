//! Module for compacting multiple OTLP span payloads into a single request

use lambda_runtime::Error as LambdaError;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use prost::Message;
use tracing;

use crate::telemetry::TelemetryData;

/// Decodes a protobuf-serialized OTLP payload
///
/// This function assumes the payload is in binary protobuf format and not compressed.
fn decode_otlp_payload(payload: &[u8]) -> Result<ExportTraceServiceRequest, LambdaError> {
    // Decode protobuf directly
    ExportTraceServiceRequest::decode(payload)
        .map_err(|e| LambdaError::from(format!("Failed to decode protobuf: {}", e)))
}

/// Encodes an OTLP request to binary protobuf format (uncompressed)
fn encode_otlp_payload(request: &ExportTraceServiceRequest) -> Vec<u8> {
    // Serialize to protobuf
    request.encode_to_vec()
}

/// Configuration for span compaction
#[derive(Debug, Clone)]
pub struct SpanCompactionConfig {
    /// Whether to enable span compaction
    pub enabled: bool,
    /// Maximum size of a compacted payload in bytes
    pub max_payload_size: usize,
    /// GZIP compression level (0-9)
    pub compression_level: u32,
}

impl Default for SpanCompactionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_payload_size: 5_000_000, // 5MB
            compression_level: 6,
        }
    }
}

/// Compacts multiple telemetry payloads into a single payload
/// Since all log events in a single Lambda invocation come from the same log group,
/// we can assume they all have the same metadata (source, endpoint, headers)
pub fn compact_telemetry_payloads(
    batch: Vec<TelemetryData>,
    config: &SpanCompactionConfig,
) -> Result<TelemetryData, LambdaError> {
    // Early return if compaction is disabled or there's only one item
    if !config.enabled || batch.len() <= 1 {
        let mut result = batch.into_iter().next().unwrap_or_default();

        // Apply compression to the result
        result
            .compress(config.compression_level)
            .map_err(|e| LambdaError::from(format!("Failed to compress payload: {}", e)))?;

        return Ok(result);
    }

    let original_count = batch.len();

    // Decode all payloads to ExportTraceServiceRequest
    let mut decoded_requests = Vec::new();
    for telemetry in &batch {
        match decode_otlp_payload(&telemetry.payload) {
            Ok(request) => decoded_requests.push(request),
            Err(e) => {
                // Log the error but continue with other payloads
                tracing::warn!("Failed to decode payload: {}", e);
            }
        }
    }

    // If all payloads failed to decode, return an error
    if decoded_requests.is_empty() {
        return Err(LambdaError::from("All payloads failed to decode"));
    }

    // Merge all resource spans
    let mut merged_resource_spans = Vec::new();
    for request in decoded_requests {
        merged_resource_spans.extend(request.resource_spans);
    }

    // Create merged request
    let merged_request = ExportTraceServiceRequest {
        resource_spans: merged_resource_spans,
    };

    // Encode the merged request to protobuf (uncompressed)
    let payload = encode_otlp_payload(&merged_request);

    // Use the metadata from the first telemetry item but with the new payload
    let first_telemetry = &batch[0];

    tracing::info!(
        "Compacted {} payloads into a single request",
        original_count
    );

    // Create the result telemetry with uncompressed payload
    let mut result = TelemetryData {
        source: first_telemetry.source.clone(),
        endpoint: first_telemetry.endpoint.clone(),
        payload,
        content_type: "application/x-protobuf".to_string(),
        content_encoding: None, // No compression yet
    };

    // Apply compression to the result
    result
        .compress(config.compression_level)
        .map_err(|e| LambdaError::from(format!("Failed to compress payload: {}", e)))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
    use std::io::Read;

    // Helper function to create a test ExportTraceServiceRequest with a specified number of spans
    fn create_test_request(span_count: usize) -> ExportTraceServiceRequest {
        let mut spans = Vec::new();
        for i in 0..span_count {
            spans.push(Span {
                name: format!("test-span-{}", i),
                ..Default::default()
            });
        }

        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    spans,
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
    }

    // Helper function to create a test TelemetryData with a specified number of spans
    fn create_test_telemetry(span_count: usize) -> TelemetryData {
        let request = create_test_request(span_count);
        let payload = encode_otlp_payload(&request);

        TelemetryData {
            source: "test-service".to_string(),
            endpoint: "http://example.com/v1/traces".to_string(),
            payload,
            content_type: "application/x-protobuf".to_string(),
            content_encoding: None, // Uncompressed for testing
        }
    }

    #[test]
    fn test_decode_encode_roundtrip() {
        // Create a simple request
        let request = create_test_request(1);

        // Encode it
        let encoded = encode_otlp_payload(&request);

        // Decode it back
        let decoded = decode_otlp_payload(&encoded).unwrap();

        // Verify resource_spans count is the same
        assert_eq!(request.resource_spans.len(), decoded.resource_spans.len());
    }

    #[test]
    fn test_compact_single_payload() {
        // Test that a single payload is returned as-is but compressed
        let telemetry = create_test_telemetry(1);
        let result =
            compact_telemetry_payloads(vec![telemetry.clone()], &SpanCompactionConfig::default())
                .unwrap();

        assert_eq!(result.source, telemetry.source);
        assert_eq!(result.content_type, "application/x-protobuf");
        assert_eq!(result.content_encoding, Some("gzip".to_string())); // Should be compressed
    }

    #[test]
    fn test_compact_multiple_payloads() {
        // Test that multiple payloads are compacted into one
        let telemetry1 = create_test_telemetry(2);
        let telemetry2 = create_test_telemetry(3);

        let result = compact_telemetry_payloads(
            vec![telemetry1.clone(), telemetry2.clone()],
            &SpanCompactionConfig::default(),
        )
        .unwrap();

        // Result should be compressed
        assert_eq!(result.content_encoding, Some("gzip".to_string()));

        // Decompress to verify contents
        let mut decoder = GzDecoder::new(&result.payload[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();

        // Decode the decompressed payload
        let decoded = ExportTraceServiceRequest::decode(decompressed.as_slice()).unwrap();

        // Count total spans
        let mut span_count = 0;
        for resource_span in &decoded.resource_spans {
            for scope_span in &resource_span.scope_spans {
                span_count += scope_span.spans.len();
            }
        }

        // Should have 5 spans total (2 + 3)
        assert_eq!(span_count, 5);
    }

    #[test]
    fn test_compact_mixed_format_payloads() {
        // Create a test config
        let config = SpanCompactionConfig::default();

        // Create two telemetry objects with different payloads
        let telemetry1 = create_test_telemetry(2);
        let telemetry2 = create_test_telemetry(3);

        // Test compaction
        let batch = vec![telemetry1, telemetry2];

        // This should succeed because both payloads are in protobuf format
        let result = compact_telemetry_payloads(batch, &config);

        // Verify the result
        assert!(
            result.is_ok(),
            "Compaction should succeed with protobuf payloads"
        );

        let compacted = result.unwrap();
        assert_eq!(compacted.content_type, "application/x-protobuf");
        assert_eq!(compacted.content_encoding, Some("gzip".to_string()));

        // Decompress to verify contents
        let mut decoder = GzDecoder::new(&compacted.payload[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();

        // Decode the decompressed payload
        let decoded = ExportTraceServiceRequest::decode(decompressed.as_slice()).unwrap();

        // Count total spans
        let mut span_count = 0;
        for resource_span in &decoded.resource_spans {
            for scope_span in &resource_span.scope_spans {
                span_count += scope_span.spans.len();
            }
        }

        // Should have 5 spans total (2 + 3)
        assert_eq!(span_count, 5);
    }
}
