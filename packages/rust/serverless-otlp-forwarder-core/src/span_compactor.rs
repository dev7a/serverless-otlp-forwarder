//! Module for compacting multiple OTLP span payloads into a single request

use anyhow::Result; // Changed from LambdaError
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use prost::Message;
use std::env;
use tracing::{self, instrument}; // For reading environment variables

use crate::telemetry::TelemetryData; // This should be correct once telemetry.rs is in the same crate

/// Decodes a protobuf-serialized OTLP payload
///
/// This function assumes the payload is in binary protobuf format and not compressed.
fn decode_otlp_payload(payload: &[u8]) -> Result<ExportTraceServiceRequest> {
    // Changed from LambdaError
    // Decode protobuf directly
    ExportTraceServiceRequest::decode(payload)
        .map_err(|e| anyhow::anyhow!("Failed to decode protobuf: {}", e)) // Changed from LambdaError
}

/// Encodes an OTLP request to binary protobuf format (uncompressed)
fn encode_otlp_payload(request: &ExportTraceServiceRequest) -> Vec<u8> {
    // Serialize to protobuf
    request.encode_to_vec()
}

/// Enum to represent OTLP compression preference
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressionPreference {
    Gzip,
    None,
}

/// Configuration for span compaction
#[derive(Debug, Clone)]
pub struct SpanCompactionConfig {
    /// Maximum size of a structurally compacted payload in bytes (before final compression)
    pub max_payload_size: usize, // This check is not yet implemented, placeholder
    /// Compression preference for the final payload
    pub compression: CompressionPreference,
    /// GZIP compression level (0-9) if Gzip compression is used
    pub gzip_compression_level: u32,
}

impl Default for SpanCompactionConfig {
    fn default() -> Self {
        let compression_preference = env::var("OTEL_EXPORTER_OTLP_TRACES_COMPRESSION")
            .or_else(|_| env::var("OTEL_EXPORTER_OTLP_COMPRESSION"))
            .ok()
            .map_or(CompressionPreference::None, |val| { // Default to None if var is not set
                match val.to_lowercase().as_str() {
                    "gzip" => CompressionPreference::Gzip,
                    "none" => CompressionPreference::None, // Explicitly none
                    _ => { // Any other value, including empty string if var is set but empty, or invalid
                        tracing::warn!(
                            "Invalid or unrecognized value for OTEL_EXPORTER_OTLP_COMPRESSION: '{}'. Defaulting to no compression.", 
                            val
                        );
                        CompressionPreference::None
                    }
                }
            });

        let default_compression_level = 9;
        let gzip_compression_level = env::var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL")
            .ok()
            .and_then(|val_str| {
                match val_str.parse::<u32>() {
                    Ok(level) if (0..=9).contains(&level) => Some(level),
                    Ok(_) => {
                        tracing::warn!(
                            "Invalid value for OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL: '{}'. Must be between 0 and 9. Defaulting to {}.",
                            val_str, default_compression_level
                        );
                        None // Will cause fallback to default_compression_level
                    }
                    Err(_) => {
                        tracing::warn!(
                            "Failed to parse OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL: '{}'. Defaulting to {}.",
                            val_str, default_compression_level
                        );
                        None // Will cause fallback to default_compression_level
                    }
                }
            })
            .unwrap_or(default_compression_level);

        Self {
            max_payload_size: 5_000_000, // 5MB
            compression: compression_preference,
            gzip_compression_level, // Use the determined level
        }
    }
}

/// Compacts multiple telemetry payloads into a single payload
/// Since all log events in a single Lambda invocation come from the same log group,
/// we can assume they all have the same metadata (source, endpoint, headers)
#[instrument(name="span_compactor/compact_telemetry_payloads", skip_all, fields(compact_telemetry_payloads.records.count = batch.len() as i64, compression = ?config.compression))]
pub fn compact_telemetry_payloads(
    batch: Vec<TelemetryData>,
    config: &SpanCompactionConfig,
) -> Result<TelemetryData> {
    // Changed from LambdaError
    if batch.is_empty() {
        return Err(anyhow::anyhow!(
            "Cannot compact an empty batch of telemetry data."
        ));
    }

    // If only one item, just apply compression preference based on config and return
    if batch.len() == 1 {
        let mut telemetry_to_return = batch.into_iter().next().unwrap();
        // Input TelemetryData.payload is expected to be uncompressed protobuf.
        // TelemetryData.content_encoding is expected to be None.
        match config.compression {
            CompressionPreference::Gzip => {
                // compress() method itself should be idempotent or handle already compressed state if necessary,
                // but our contract is that input TelemetryData here is uncompressed.
                telemetry_to_return
                    .compress(config.gzip_compression_level)
                    .map_err(|e| anyhow::anyhow!("Failed to compress single payload: {}", e))?;
            }
            CompressionPreference::None => {
                // Ensure content_encoding is None. Payload is already uncompressed per contract.
                telemetry_to_return.content_encoding = None;
            }
        }
        return Ok(telemetry_to_return);
    }

    // Proceed with structural compaction for batch.len() > 1
    let original_count = batch.len();
    let mut decoded_requests = Vec::new();

    // Get metadata from the first element before consuming the batch by value.
    let first_item_source = batch[0].source.clone();
    let first_item_endpoint = batch[0].endpoint.clone();

    for telemetry_item in batch {
        // Consume batch
        match decode_otlp_payload(&telemetry_item.payload) {
            Ok(request) => decoded_requests.push(request),
            Err(e) => {
                tracing::warn!(
                    "Failed to decode TelemetryData payload for compaction: {}. Skipping item.",
                    e
                );
            }
        }
    }

    if decoded_requests.is_empty() {
        return Err(anyhow::anyhow!(
            "All payloads in batch failed to decode for compaction"
        ));
    }

    let mut merged_resource_spans = Vec::new();
    for request in decoded_requests {
        merged_resource_spans.extend(request.resource_spans);
    }

    let merged_request = ExportTraceServiceRequest {
        resource_spans: merged_resource_spans,
    };

    let merged_payload = encode_otlp_payload(&merged_request);

    let mut result_telemetry_data = TelemetryData {
        source: first_item_source,     // Use cloned metadata
        endpoint: first_item_endpoint, // Use cloned metadata
        payload: merged_payload,
        content_type: "application/x-protobuf".to_string(),
        content_encoding: None, // Start as uncompressed before final compression decision
    };

    match config.compression {
        CompressionPreference::Gzip => {
            result_telemetry_data
                .compress(config.gzip_compression_level)
                .map_err(|e| anyhow::anyhow!("Failed to compress merged payload: {}", e))?;
        }
        CompressionPreference::None => {
            // Ensure content_encoding is None (already set as default if not compressed)
            result_telemetry_data.content_encoding = None;
        }
    }

    tracing::info!(
        "Compacted {} telemetry items into a single request. Final compression: {:?}.",
        original_count,
        result_telemetry_data.content_encoding
    );
    Ok(result_telemetry_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::TelemetryData; // Ensure TelemetryData is in scope for tests
    use flate2::read::GzDecoder;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
    use serial_test::serial;
    use std::io::Read; // For tests that modify environment variables

    // Helper function to create a test ExportTraceServiceRequest with a specified number of spans
    fn create_test_request(span_count: usize) -> ExportTraceServiceRequest {
        let mut spans = Vec::new();
        for i in 0..span_count {
            spans.push(Span {
                name: format!("test-span-{i}"),
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
    fn create_test_telemetry_uncompressed(span_count: usize, source: &str) -> TelemetryData {
        let request = create_test_request(span_count);
        let payload = encode_otlp_payload(&request);

        TelemetryData {
            source: source.to_string(),
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
    #[serial] // Modifies env vars
    fn test_span_compaction_config_default_is_none_if_no_env_var() {
        std::env::remove_var("OTEL_EXPORTER_OTLP_TRACES_COMPRESSION");
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION");
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.compression, CompressionPreference::None);
        assert_eq!(config.gzip_compression_level, 9);
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_span_compaction_config_env_none_explicitly() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION", "none");
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.compression, CompressionPreference::None);
        assert_eq!(config.gzip_compression_level, 9); // Default level
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_span_compaction_config_env_gzip_explicitly() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION", "gzip");
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.compression, CompressionPreference::Gzip);
        assert_eq!(config.gzip_compression_level, 9); // Default level
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_span_compaction_config_env_traces_precedence_gzip() {
        std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_COMPRESSION", "gzip");
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION", "none"); // Should be ignored
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.compression, CompressionPreference::Gzip);
        assert_eq!(config.gzip_compression_level, 9);
        std::env::remove_var("OTEL_EXPORTER_OTLP_TRACES_COMPRESSION");
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_span_compaction_config_env_traces_precedence_none() {
        std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_COMPRESSION", "none");
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION", "gzip"); // Should be ignored
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.compression, CompressionPreference::None);
        assert_eq!(config.gzip_compression_level, 9);
        std::env::remove_var("OTEL_EXPORTER_OTLP_TRACES_COMPRESSION");
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_span_compaction_config_invalid_env_defaults_to_none() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION", "invalid_value");
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.compression, CompressionPreference::None);
        assert_eq!(config.gzip_compression_level, 9);
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_compression_level_env_var_valid() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL", "5");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.gzip_compression_level, 5);
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_compression_level_env_var_invalid_string() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL", "not_a_number");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.gzip_compression_level, 9); // Should default
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_compression_level_env_var_invalid_too_high() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL", "10");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.gzip_compression_level, 9); // Should default
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_compression_level_env_var_invalid_negative() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL", "-1");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.gzip_compression_level, 9); // Should default
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_compression_level_env_var_empty_string() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL", "");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.gzip_compression_level, 9); // Should default
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
    }

    #[test]
    #[serial] // Modifies env vars
    fn test_compression_level_zero_is_valid() {
        std::env::set_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL", "0");
        let config = SpanCompactionConfig::default();
        assert_eq!(config.gzip_compression_level, 0);
        std::env::remove_var("OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL");
    }

    #[test]
    fn test_compact_single_payload_with_gzip_preference() {
        let telemetry = create_test_telemetry_uncompressed(1, "s1");
        let config = SpanCompactionConfig {
            compression: CompressionPreference::Gzip,
            max_payload_size: 5_000_000,
            gzip_compression_level: 9,
        };
        let result = compact_telemetry_payloads(vec![telemetry.clone()], &config).unwrap();
        assert_eq!(result.content_encoding, Some("gzip".to_string()));
    }

    #[test]
    fn test_compact_single_payload_with_none_preference() {
        let telemetry = create_test_telemetry_uncompressed(1, "s1");
        let config = SpanCompactionConfig {
            compression: CompressionPreference::None,
            max_payload_size: 5_000_000,
            gzip_compression_level: 9,
        };
        let result = compact_telemetry_payloads(vec![telemetry.clone()], &config).unwrap();
        assert_eq!(result.content_encoding, None);
        assert_eq!(result.payload, telemetry.payload); // Payload should be unchanged if already uncompressed
    }

    #[test]
    fn test_compact_multiple_payloads_with_gzip_preference() {
        let telemetry1 = create_test_telemetry_uncompressed(2, "s1");
        let telemetry2 = create_test_telemetry_uncompressed(3, "s2");
        let config = SpanCompactionConfig {
            compression: CompressionPreference::Gzip,
            max_payload_size: 5_000_000,
            gzip_compression_level: 9,
        };
        let result = compact_telemetry_payloads(vec![telemetry1, telemetry2], &config).unwrap();
        assert_eq!(result.content_encoding, Some("gzip".to_string()));
        let mut decoder = GzDecoder::new(&result.payload[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        let decoded_request = ExportTraceServiceRequest::decode(decompressed.as_slice()).unwrap();
        assert_eq!(
            decoded_request.resource_spans[0].scope_spans[0].spans.len()
                + decoded_request.resource_spans[1].scope_spans[0].spans.len(),
            5
        );
    }

    #[test]
    fn test_compact_multiple_payloads_with_none_preference() {
        let telemetry1 = create_test_telemetry_uncompressed(2, "s1");
        let telemetry2 = create_test_telemetry_uncompressed(3, "s2");
        let config = SpanCompactionConfig {
            compression: CompressionPreference::None,
            max_payload_size: 5_000_000,
            gzip_compression_level: 9,
        };
        let result = compact_telemetry_payloads(vec![telemetry1, telemetry2], &config).unwrap();
        assert_eq!(result.content_encoding, None);
        let decoded_request = ExportTraceServiceRequest::decode(result.payload.as_slice()).unwrap();
        assert_eq!(
            decoded_request.resource_spans[0].scope_spans[0].spans.len()
                + decoded_request.resource_spans[1].scope_spans[0].spans.len(),
            5
        );
    }

    #[test]
    fn test_compact_empty_batch_returns_error() {
        let config = SpanCompactionConfig::default();
        let result = compact_telemetry_payloads(Vec::new(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_compact_with_one_decode_failure() {
        let telemetry_good = create_test_telemetry_uncompressed(1, "s1");
        let telemetry_bad_payload = TelemetryData {
            payload: vec![0, 1, 2], // Invalid protobuf
            ..create_test_telemetry_uncompressed(0, "s2")
        };
        let config = SpanCompactionConfig {
            compression: CompressionPreference::None,
            max_payload_size: 5_000_000,
            gzip_compression_level: 9,
        };
        let result =
            compact_telemetry_payloads(vec![telemetry_good, telemetry_bad_payload], &config)
                .unwrap();
        // Should compact the good one, skipping the bad one
        let decoded_request = ExportTraceServiceRequest::decode(result.payload.as_slice()).unwrap();
        assert_eq!(
            decoded_request.resource_spans[0].scope_spans[0].spans.len(),
            1
        );
    }

    #[test]
    fn test_compact_all_decode_failures() {
        let telemetry_bad1 = TelemetryData {
            payload: vec![1],
            ..create_test_telemetry_uncompressed(0, "s1")
        };
        let telemetry_bad2 = TelemetryData {
            payload: vec![2],
            ..create_test_telemetry_uncompressed(0, "s2")
        };
        let config = SpanCompactionConfig::default();
        let result = compact_telemetry_payloads(vec![telemetry_bad1, telemetry_bad2], &config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("All payloads in batch failed to decode"));
    }
}
