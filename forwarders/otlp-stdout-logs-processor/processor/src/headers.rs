//! Header management for the Serverless OTLP forwarder.
//!
//! This module handles two types of headers:
//! - Log record specific headers for forwarding requests
//! - Authentication headers for collectors
//!
//! The headers are used when forwarding log records to their respective collectors.

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::str::FromStr;

use crate::collectors::Collector;
use crate::telemetry::TelemetryData;
use aws_credential_types::Credentials;
use otlp_sigv4_client::signing::sign_request;
use otlp_stdout_span_exporter::ExporterOutput;
/// Headers builder for outgoing log record requests.
/// Uses the builder pattern to construct the final set of headers
/// from various sources (log record, collector auth).
#[derive(Debug)]
pub struct LogRecordHeaders(HeaderMap);

pub const CONTENT_TYPE_HEADER: &str = "content-type";
pub const CONTENT_ENCODING_HEADER: &str = "content-encoding";

impl LogRecordHeaders {
    /// Creates a new empty set of headers.
    pub fn new() -> Self {
        LogRecordHeaders(HeaderMap::new())
    }

    /// Adds headers from the log record itself.
    /// This includes both custom headers and content-type/encoding headers.
    pub fn with_log_record(mut self, log_record: &ExporterOutput) -> Result<Self> {
        if let Some(headers) = &log_record.headers {
            self.extract_headers(headers)?;
        }
        self.add_content_headers(log_record)?;
        Ok(self)
    }

    /// Adds authentication headers from the collector configuration.
    pub fn with_collector_auth(
        mut self,
        collector: &Collector,
        payload: &[u8],
        credentials: &Credentials,
        region: &str,
    ) -> Result<Self> {
        if let Some(auth) = &collector.auth {
            match auth.to_lowercase().as_str() {
                "sigv4" | "iam" => {
                    // Create a new HeaderMap with headers required for SigV4
                    let mut headers_to_sign = HeaderMap::new();
                    for (key, value) in self.0.iter() {
                        let header_name = key.as_str().to_lowercase();
                        if matches!(
                            header_name.as_str(),
                            "content-type" | "content-encoding" | "content-length" | "user-agent"
                        ) {
                            headers_to_sign.insert(key.clone(), value.clone());
                        }
                    }
                    let signed_headers = sign_request(
                        credentials,
                        &collector.endpoint,
                        "POST",
                        &headers_to_sign,
                        payload,
                        region,
                        "xray",
                    )
                    .map_err(|e| anyhow::anyhow!("Failed to sign request: {}", e))?;
                    self.0.extend(signed_headers);
                }
                _ if auth.contains('=') => {
                    let (name, value) = auth
                        .split_once('=')
                        .context("Invalid auth format in collector config")?;
                    self.0
                        .insert(HeaderName::from_str(name)?, HeaderValue::from_str(value)?);
                }
                _ => {
                    tracing::warn!("Unknown auth type: {}", auth);
                }
            }
        }
        Ok(self)
    }

    /// Adds headers from telemetry data.
    /// This includes content-type and content-encoding headers.
    pub fn with_telemetry(mut self, telemetry: &TelemetryData) -> Result<Self> {
        if !telemetry.content_type.is_empty() {
            self.0.insert(
                HeaderName::from_static(CONTENT_TYPE_HEADER),
                HeaderValue::from_str(&telemetry.content_type)?,
            );
            tracing::debug!(
                "Added/Updated content-type header: {}",
                telemetry.content_type
            );
        }
        if let Some(content_encoding) = &telemetry.content_encoding {
            self.0.insert(
                HeaderName::from_static(CONTENT_ENCODING_HEADER),
                HeaderValue::from_str(content_encoding)?,
            );
        }
        Ok(self)
    }

    /// Finalizes the headers and returns the underlying HeaderMap.
    pub fn build(self) -> HeaderMap {
        self.0
    }

    /// Helper method to extract and normalize custom headers from a HashMap
    fn extract_headers(&mut self, headers: &HashMap<String, String>) -> Result<()> {
        for (key, value) in headers {
            let normalized_key = key.to_lowercase();
            let header_name = HeaderName::from_str(&normalized_key)
                .with_context(|| format!("Invalid header name: {}", normalized_key))?;
            let header_value = HeaderValue::from_str(value).with_context(|| {
                format!("Invalid header value for {}: {}", normalized_key, value)
            })?;

            self.0.insert(header_name, header_value);
        }
        Ok(())
    }

    /// Helper method to add content-type and content-encoding headers
    fn add_content_headers(&mut self, log_record: &ExporterOutput) -> Result<()> {
        self.0.insert(
            HeaderName::from_static(CONTENT_TYPE_HEADER),
            HeaderValue::from_str(&log_record.content_type)?,
        );
        self.0.insert(
            HeaderName::from_static(CONTENT_ENCODING_HEADER),
            HeaderValue::from_str(&log_record.content_encoding)?,
        );
        Ok(())
    }
}

impl Default for LogRecordHeaders {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_credential_types::Credentials;
    use std::collections::HashMap;

    fn create_test_log_record() -> ExporterOutput {
        let mut headers = HashMap::new();
        headers.insert("x-custom-header".to_string(), "custom-value".to_string());
        headers.insert("x-another-header".to_string(), "another-value".to_string());

        ExporterOutput {
            version: "test".to_string(),
            source: "test-source".to_string(),
            endpoint: "http://example.com".to_string(),
            method: "POST".to_string(),
            content_type: "application/json".to_string(),
            content_encoding: "gzip".to_string(),
            headers: Some(headers),
            payload: "test-payload".to_string(),
            base64: true,
            level: Some("info".to_string()),
        }
    }

    fn create_test_telemetry() -> TelemetryData {
        TelemetryData {
            source: "test-source".to_string(),
            endpoint: "http://example.com".to_string(),
            payload: b"test-payload".to_vec(),
            content_type: "application/json".to_string(),
            content_encoding: Some("gzip".to_string()),
        }
    }

    fn create_test_credentials() -> Credentials {
        Credentials::new("test-key", "test-secret", None, None, "test-provider")
    }

    #[test]
    fn test_default_headers() {
        let headers = LogRecordHeaders::default().build();
        assert!(headers.is_empty());
    }

    #[test]
    fn test_with_log_record() {
        let log_record = create_test_log_record();
        let headers = LogRecordHeaders::default()
            .with_log_record(&log_record)
            .unwrap()
            .build();

        // Check custom headers
        assert_eq!(headers.get("x-custom-header").unwrap(), "custom-value");
        assert_eq!(headers.get("x-another-header").unwrap(), "another-value");

        // Check content headers
        assert_eq!(
            headers.get(CONTENT_TYPE_HEADER).unwrap(),
            "application/json"
        );
        assert_eq!(headers.get(CONTENT_ENCODING_HEADER).unwrap(), "gzip");
    }

    #[test]
    fn test_with_telemetry() {
        let telemetry = create_test_telemetry();
        let headers = LogRecordHeaders::default()
            .with_telemetry(&telemetry)
            .unwrap()
            .build();

        assert_eq!(
            headers.get(CONTENT_TYPE_HEADER).unwrap(),
            "application/json"
        );
        assert_eq!(headers.get(CONTENT_ENCODING_HEADER).unwrap(), "gzip");
    }

    #[test]
    fn test_with_custom_auth() {
        let collector = Collector {
            name: "test-collector".to_string(),
            endpoint: "http://example.com".to_string(),
            auth: Some("x-api-key=test-key".to_string()),
            exclude: None,
            disabled: false,
        };

        let headers = LogRecordHeaders::default()
            .with_collector_auth(
                &collector,
                b"test-payload",
                &create_test_credentials(),
                "us-west-2",
            )
            .unwrap()
            .build();

        assert_eq!(headers.get("x-api-key").unwrap(), "test-key");
    }

    #[test]
    fn test_with_sigv4_auth() {
        let collector = Collector {
            name: "test-collector".to_string(),
            endpoint: "http://example.com".to_string(),
            auth: Some("sigv4".to_string()),
            exclude: None,
            disabled: false,
        };

        let telemetry = create_test_telemetry();
        let headers = LogRecordHeaders::default()
            .with_telemetry(&telemetry)
            .unwrap()
            .with_collector_auth(
                &collector,
                &telemetry.payload,
                &create_test_credentials(),
                "us-west-2",
            )
            .unwrap()
            .build();

        // Check for AWS SigV4 headers - at least one of these should be present
        let has_sigv4_headers = headers.contains_key("authorization")
            || headers.contains_key("x-amz-date")
            || headers.contains_key("x-amz-content-sha256");

        assert!(has_sigv4_headers, "No SigV4 headers found in the request");

        // Print headers for debugging
        for (key, value) in headers.iter() {
            println!(
                "Header: {} = {}",
                key,
                value.to_str().unwrap_or("invalid utf-8")
            );
        }
    }

    #[test]
    fn test_invalid_header_name() {
        let mut headers = HashMap::new();
        headers.insert("invalid header name".to_string(), "value".to_string());

        let log_record = ExporterOutput {
            version: "test".to_string(),
            source: "test-source".to_string(),
            endpoint: "http://example.com".to_string(),
            method: "POST".to_string(),
            content_type: "application/json".to_string(),
            content_encoding: "gzip".to_string(),
            headers: Some(headers),
            payload: "test-payload".to_string(),
            base64: true,
            level: Some("info".to_string()),
        };

        let result = LogRecordHeaders::default().with_log_record(&log_record);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid header name"));
    }

    #[test]
    fn test_invalid_header_value() {
        let mut headers = HashMap::new();
        headers.insert("x-test".to_string(), "invalid\u{0000}value".to_string());

        let log_record = ExporterOutput {
            version: "test".to_string(),
            source: "test-source".to_string(),
            endpoint: "http://example.com".to_string(),
            method: "POST".to_string(),
            content_type: "application/json".to_string(),
            content_encoding: "gzip".to_string(),
            headers: Some(headers),
            payload: "test-payload".to_string(),
            base64: true,
            level: Some("info".to_string()),
        };

        let result = LogRecordHeaders::default().with_log_record(&log_record);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid header value"));
    }

    #[test]
    fn test_unknown_auth_type() {
        let collector = Collector {
            name: "test-collector".to_string(),
            endpoint: "http://example.com".to_string(),
            auth: Some("unknown".to_string()),
            exclude: None,
            disabled: false,
        };

        let result = LogRecordHeaders::default().with_collector_auth(
            &collector,
            b"test-payload",
            &create_test_credentials(),
            "us-west-2",
        );

        // Unknown auth type should not cause an error, just a warning
        assert!(result.is_ok());
        assert!(result.unwrap().build().is_empty());
    }

    #[test]
    fn test_invalid_auth_format() {
        let collector = Collector {
            name: "test-collector".to_string(),
            endpoint: "http://example.com".to_string(),
            auth: Some("invalid-format".to_string()),
            exclude: None,
            disabled: false,
        };

        let result = LogRecordHeaders::default().with_collector_auth(
            &collector,
            b"test-payload",
            &create_test_credentials(),
            "us-west-2",
        );

        // Invalid auth format should not cause an error, just a warning
        assert!(result.is_ok());
        assert!(result.unwrap().build().is_empty());
    }

    #[test]
    fn test_header_overrides() {
        let log_record = create_test_log_record();
        let telemetry = create_test_telemetry();

        // Create headers with both log record and telemetry
        let headers = LogRecordHeaders::default()
            .with_log_record(&log_record)
            .unwrap()
            .with_telemetry(&telemetry)
            .unwrap()
            .build();

        // Content headers should be present and match the last set values
        assert_eq!(
            headers.get(CONTENT_TYPE_HEADER).unwrap(),
            "application/json"
        );
        assert_eq!(headers.get(CONTENT_ENCODING_HEADER).unwrap(), "gzip");

        // Custom headers should still be present
        assert_eq!(headers.get("x-custom-header").unwrap(), "custom-value");
    }
}
