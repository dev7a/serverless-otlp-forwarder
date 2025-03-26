use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use otlp_stdout_span_exporter::ExporterOutput;
use prost::Message;
use serde_json::Value;
use std::io::{Read, Write};
use tracing;
/// Core structure representing telemetry data to be forwarded
#[derive(Clone)]
pub struct TelemetryData {
    /// Source of the telemetry data (e.g., service name or log group)
    pub source: String,
    /// Target endpoint for the telemetry data
    pub endpoint: String,
    /// The actual payload bytes
    pub payload: Vec<u8>,
    /// Content type of the payload
    pub content_type: String,
    /// Optional content encoding (e.g., gzip)
    pub content_encoding: Option<String>,
}

impl Default for TelemetryData {
    fn default() -> Self {
        Self {
            source: "unknown".to_string(),
            endpoint: "http://localhost:4318/v1/traces".to_string(),
            payload: Vec::new(),
            content_type: "application/x-protobuf".to_string(),
            content_encoding: None, // No compression by default
        }
    }
}

impl TelemetryData {
    /// Converts payload data to binary protobuf format (uncompressed)
    ///
    /// This method ensures that all telemetry data is in a consistent format
    /// before it reaches the span compactor, which simplifies compaction logic.
    ///
    /// # Arguments
    ///
    /// * `payload` - The raw payload bytes
    /// * `content_type` - The content type of the payload
    /// * `content_encoding` - The optional content encoding of the payload
    ///
    /// # Returns
    ///
    /// The binary protobuf payload
    fn convert_to_protobuf(
        payload: Vec<u8>,
        content_type: &str,
        content_encoding: Option<&str>,
    ) -> Result<Vec<u8>> {
        tracing::debug!(
            "Converting payload from {}/{:?} to protobuf",
            content_type,
            content_encoding
        );

        // First, decompress if needed
        let decompressed = if content_encoding == Some("gzip") {
            tracing::debug!("Decompressing gzipped payload");
            let mut decoder = GzDecoder::new(&payload[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .context("Failed to decompress payload")?;
            decompressed
        } else {
            payload
        };

        // Then convert to protobuf based on content type
        match content_type {
            "application/x-protobuf" => {
                // Already protobuf, no conversion needed
                tracing::debug!("Payload already in protobuf format");
                Ok(decompressed)
            }
            "application/json" => {
                // Convert JSON to protobuf
                tracing::debug!("Converting JSON to protobuf");
                Self::convert_json_to_protobuf(&decompressed)
            }
            _ => {
                // Unknown format, log warning and return as-is
                tracing::warn!("Unknown content type: {}, keeping as is", content_type);
                Ok(decompressed)
            }
        }
    }

    /// Converts JSON to protobuf using the OTLP schema
    ///
    /// Since the JSON schema matches the OTLP protobuf schema, we can directly
    /// deserialize the JSON into the protobuf structure and then serialize it back
    /// to binary protobuf format.
    fn convert_json_to_protobuf(json_bytes: &[u8]) -> Result<Vec<u8>> {
        // Parse the JSON into an ExportTraceServiceRequest
        let request: ExportTraceServiceRequest = serde_json::from_slice(json_bytes)
            .context("Failed to parse JSON as ExportTraceServiceRequest")?;

        // Serialize to protobuf binary format
        let protobuf_bytes = request.encode_to_vec();

        tracing::debug!(
            "Successfully converted JSON to protobuf (size: {} bytes)",
            protobuf_bytes.len()
        );

        Ok(protobuf_bytes)
    }

    /// Applies gzip compression to the payload
    ///
    /// This should only be called on the final compacted payload
    /// to avoid unnecessary compression/decompression cycles.
    pub fn compress(&mut self, compression_level: u32) -> Result<()> {
        // Only compress if not already compressed
        if self.content_encoding != Some("gzip".to_string()) {
            tracing::debug!("Compressing payload with level {}", compression_level);

            let mut encoder = GzEncoder::new(Vec::new(), Compression::new(compression_level));
            encoder
                .write_all(&self.payload)
                .context("Failed to compress payload")?;

            self.payload = encoder.finish().context("Failed to finish compression")?;

            self.content_encoding = Some("gzip".to_string());

            tracing::debug!(
                "Compressed payload from {} to {} bytes",
                self.payload.len(),
                self.payload.len()
            );
        }

        Ok(())
    }

    /// Creates a TelemetryData instance from a LogRecord
    pub fn from_log_record(record: ExporterOutput<'_>) -> Result<Self> {
        // Decode base64 payload
        let raw_payload = if record.base64 {
            general_purpose::STANDARD
                .decode(&record.payload)
                .context("Failed to decode base64 payload")?
        } else {
            record.payload.as_bytes().to_vec()
        };

        // Convert to uncompressed protobuf format
        let protobuf_payload = Self::convert_to_protobuf(
            raw_payload,
            record.content_type,
            Some(record.content_encoding),
        )?;

        Ok(Self {
            source: record.source.clone(),
            endpoint: record.endpoint.to_string(),
            payload: protobuf_payload,
            content_type: "application/x-protobuf".to_string(),
            content_encoding: None, // Decompressed at this stage
        })
    }

    /// Creates a TelemetryData instance from a raw span (as serialized JSON)
    pub fn from_raw_span(span: Value, log_group: &str) -> Result<Self> {
        // Serialize the span data
        let json_string =
            serde_json::to_string(&span).context("Failed to serialize span data to JSON string")?;

        let raw_payload = json_string.as_bytes().to_vec();

        // Convert to protobuf format (uncompressed)
        let protobuf_payload = Self::convert_to_protobuf(raw_payload, "application/json", None)?;

        Ok(Self {
            source: log_group.to_string(),
            endpoint: "https://localhost:4318/v1/traces".to_string(),
            payload: protobuf_payload,
            content_type: "application/x-protobuf".to_string(),
            content_encoding: None, // No compression at this stage
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose, Engine};
    use flate2::{write::GzEncoder, Compression};
    use serde_json::json;
    use std::collections::HashMap;
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

    #[test]
    fn test_from_log_record() {
        let record = ExporterOutput {
            version: "test",
            source: "test-service".to_string(),
            endpoint: "http://example.com",
            method: "POST",
            payload: create_test_payload(),
            headers: HashMap::new(),
            content_type: "application/x-protobuf",
            content_encoding: "gzip",
            base64: true,
        };

        let telemetry = TelemetryData::from_log_record(record).unwrap();
        assert_eq!(telemetry.source, "test-service");
        assert_eq!(telemetry.endpoint, "http://example.com");
        assert_eq!(telemetry.content_type, "application/x-protobuf");
        // Since we're decompressing at from_log_record level, it should be None
        assert_eq!(telemetry.content_encoding, None);
    }

    #[test]
    fn test_from_raw_span() {
        // Create a valid OTLP JSON structure
        let span = json!({
            "resourceSpans": []
        });

        let telemetry = TelemetryData::from_raw_span(span, "aws/spans").unwrap();
        assert_eq!(telemetry.source, "aws/spans");
        assert_eq!(telemetry.content_type, "application/x-protobuf");
        assert_eq!(telemetry.content_encoding, None); // No compression at this stage
    }

    #[test]
    fn test_compress() {
        // Create a telemetry object with uncompressed data
        let mut telemetry = TelemetryData {
            source: "test".to_string(),
            endpoint: "http://example.com".to_string(),
            payload: vec![1, 2, 3, 4, 5],
            content_type: "application/x-protobuf".to_string(),
            content_encoding: None,
        };

        // Compress it
        telemetry.compress(6).unwrap();

        // Verify it's now compressed
        assert_eq!(telemetry.content_encoding, Some("gzip".to_string()));

        // Decompress to verify the data is intact
        let mut decoder = GzDecoder::new(&telemetry.payload[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();

        assert_eq!(decompressed, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_convert_to_protobuf_already_protobuf() {
        // Test that protobuf data is not modified
        let original_payload = vec![1, 2, 3, 4];
        let converted = TelemetryData::convert_to_protobuf(
            original_payload.clone(),
            "application/x-protobuf",
            None,
        )
        .unwrap();

        assert_eq!(converted, original_payload);
    }

    #[test]
    fn test_convert_to_protobuf_from_json() {
        // Create a minimal valid OTLP JSON payload
        let json_data = json!({
            "resourceSpans": []
        });
        let json_bytes = serde_json::to_vec(&json_data).unwrap();

        let converted =
            TelemetryData::convert_to_protobuf(json_bytes, "application/json", None).unwrap();

        // Verify we can decode it as an ExportTraceServiceRequest
        let request = ExportTraceServiceRequest::decode(converted.as_slice()).unwrap();
        assert_eq!(request.resource_spans.len(), 0);
    }

    #[test]
    fn test_convert_to_protobuf_from_gzipped_json() {
        // Create a minimal valid OTLP JSON payload
        let json_data = json!({
            "resourceSpans": []
        });
        let json_bytes = serde_json::to_vec(&json_data).unwrap();

        // Compress it
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&json_bytes).unwrap();
        let compressed = encoder.finish().unwrap();

        let converted =
            TelemetryData::convert_to_protobuf(compressed, "application/json", Some("gzip"))
                .unwrap();

        // Verify we can decode it as an ExportTraceServiceRequest
        let request = ExportTraceServiceRequest::decode(converted.as_slice()).unwrap();
        assert_eq!(request.resource_spans.len(), 0);
    }
}
