use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine};
use flate2::{write::GzEncoder, Compression};
use otlp_stdout_client::LogRecord;
use serde_json::Value;
use std::io::Write;

/// Core structure representing telemetry data to be forwarded
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

impl TelemetryData {
    /// Creates a TelemetryData instance from a LogRecord
    pub fn from_log_record(record: LogRecord) -> Result<Self> {
        let payload = match &record.payload {
            Value::String(s) => s.to_string(),
            _ => serde_json::to_string(&record.payload)
                .context("Failed to serialize JSON payload")?,
        };

        let payload = match record.base64 {
            Some(true) => general_purpose::STANDARD
                .decode(&payload)
                .context("Failed to decode base64 payload"),
            _ => Ok(payload.as_bytes().to_vec()),
        }
        .context("Failed to decode base64 payload")?;

        Ok(Self {
            source: record.source,
            endpoint: record.endpoint,
            payload,
            content_type: record.content_type,
            content_encoding: record.content_encoding,
        })
    }

    /// Creates a TelemetryData instance from a raw span
    pub fn from_raw_span(span: Value, log_group: &str) -> Result<Self> {
        // Serialize the span data
        let json_string =
            serde_json::to_string(&span).context("Failed to serialize span data to JSON string")?;

        // Compress using gzip
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(json_string.as_bytes())?;
        let payload = encoder.finish()?;

        Ok(Self {
            source: log_group.to_string(),
            endpoint: "https://localhost:4318/v1/traces".to_string(),
            payload,
            content_type: "application/json".to_string(),
            content_encoding: Some("gzip".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_from_log_record() {
        let record = LogRecord {
            _otel: "test".to_string(),
            source: "test-service".to_string(),
            endpoint: "http://example.com".to_string(),
            method: "POST".to_string(),
            payload: json!({"test": "data"}),
            headers: std::collections::HashMap::new(),
            content_type: "application/json".to_string(),
            content_encoding: Some("gzip".to_string()),
            base64: None,
        };

        let telemetry = TelemetryData::from_log_record(record).unwrap();
        assert_eq!(telemetry.source, "test-service");
        assert_eq!(telemetry.endpoint, "http://example.com");
        assert_eq!(telemetry.content_type, "application/json");
        assert_eq!(telemetry.content_encoding, Some("gzip".to_string()));
    }

    #[test]
    fn test_from_raw_span() {
        let span = json!({
            "name": "test-span",
            "traceId": "test-trace-id",
        });

        let telemetry = TelemetryData::from_raw_span(span, "aws/spans").unwrap();
        assert_eq!(telemetry.source, "aws/spans");
        assert_eq!(telemetry.content_type, "application/json");
        assert_eq!(telemetry.content_encoding, Some("gzip".to_string()));
    }
}
