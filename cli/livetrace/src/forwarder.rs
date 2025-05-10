//! Manages the forwarding of OTLP (OpenTelemetry Protocol) trace data to a configured endpoint.
//!
//! This module includes functionality for:
//! - Parsing OTLP headers from string representations.
//! - Compacting a batch of telemetry data (multiple `TelemetryData` items) into a single
//!   `ExportTraceServiceRequest` by merging resource spans. This is done before compression
//!   and sending.
//! - Sending the (potentially compacted and then gzipped) OTLP payload via HTTP POST
//!   to the specified OTLP receiver.

use anyhow::{Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client as ReqwestClient,
};
use std::str::FromStr;

// Need CliArgs for headers
use crate::processing::{
    // Need processing functions/structs
    compact_telemetry_payloads,
    send_telemetry_payload,
    SpanCompactionConfig,
    TelemetryData,
};

/// Parses OTLP headers from a vector of header strings.
pub fn parse_otlp_headers_from_vec(headers_vec: &[String]) -> Result<HeaderMap> {
    let mut otlp_header_map = HeaderMap::new();
    for header_str in headers_vec {
        // Iterate over the provided vector
        let parts: Vec<&str> = header_str.splitn(2, '=').collect();
        if parts.len() == 2 {
            let header_name = HeaderName::from_str(parts[0])
                .with_context(|| format!("Invalid OTLP header name: {}", parts[0]))?;
            let header_value = HeaderValue::from_str(parts[1]).with_context(|| {
                format!("Invalid OTLP header value for {}: {}", parts[0], parts[1])
            })?;
            otlp_header_map.insert(header_name, header_value);
        } else {
            tracing::warn!(
                "Ignoring malformed OTLP header (expected Key=Value): {}",
                header_str
            );
        }
    }
    Ok(otlp_header_map)
}

/// Sends a batch of telemetry data to the OTLP endpoint, handling compaction.
pub async fn send_batch(
    http_client: &ReqwestClient,
    endpoint: &str,
    batch: Vec<TelemetryData>,
    compaction_config: &SpanCompactionConfig,
    headers: HeaderMap,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    // Always use compact_telemetry_payloads
    tracing::debug!("Compacting batch of {} item(s)...", batch.len());
    match compact_telemetry_payloads(batch, compaction_config) {
        Ok(compacted_data) => {
            tracing::debug!(
                "Sending compacted batch ({} bytes) to {}",
                compacted_data.payload.len(),
                endpoint
            );
            if let Err(e) =
                send_telemetry_payload(http_client, endpoint, compacted_data.payload, headers).await
            {
                tracing::error!("Failed to send compacted batch: {}", e);
                // Log and continue
            }
        }
        Err(e) => {
            tracing::error!("Failed to compact telemetry batch: {}", e);
            // Don't send if compaction failed
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn test_parse_otlp_headers_valid() {
        let headers_vec = vec![
            "Authorization=Bearer 123".to_string(),
            "X-Custom-Header=Value456".to_string(),
        ];
        let result = parse_otlp_headers_from_vec(&headers_vec).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result.get("Authorization").unwrap(),
            &HeaderValue::from_str("Bearer 123").unwrap()
        );
        assert_eq!(
            result.get("x-custom-header").unwrap(),
            &HeaderValue::from_str("Value456").unwrap()
        );
    }

    #[test]
    fn test_parse_otlp_headers_malformed() {
        let headers_vec = vec![
            "Authorization=Bearer 123".to_string(),
            "MalformedHeader".to_string(),   // No equals sign
            "EmptyValue=".to_string(),       // Empty value
            "=NoKey".to_string(),            // Technically invalid HeaderName
            "Key=Val1,Key=Val2".to_string(), // Should be separate strings
        ];
        // We expect Ok, but only valid headers should be parsed. Invalid HeaderName causes an Err.
        let result = parse_otlp_headers_from_vec(&headers_vec);

        // The "=NoKey" case should cause an error due to invalid HeaderName
        assert!(result.is_err());
        // If we wanted to ignore the error and just check parsed headers:
        // assert!(result.is_ok());
        // let header_map = result.unwrap();
        // assert_eq!(header_map.len(), 2); // Only Auth and EmptyValue=
        // assert_eq!(header_map.get("Authorization").unwrap(), &HeaderValue::from_str("Bearer 123").unwrap());
        // assert_eq!(header_map.get("EmptyValue").unwrap(), &HeaderValue::from_str("").unwrap());
    }

    #[test]
    fn test_parse_otlp_headers_empty_input() {
        let headers_vec: Vec<String> = vec![];
        let result = parse_otlp_headers_from_vec(&headers_vec).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_otlp_headers_invalid_name() {
        // Test case specifically for an invalid header name that should fail
        let headers_vec = vec!["=NoKey".to_string()];
        let result = parse_otlp_headers_from_vec(&headers_vec);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid OTLP header name"));
    }
}
