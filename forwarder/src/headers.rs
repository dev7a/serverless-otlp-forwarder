//! Header management for the Lambda OTLP forwarder.
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
use aws_credential_types::Credentials;
use otlp_sigv4_client::signing::sign_request;
use otlp_stdout_client::{LogRecord, CONTENT_ENCODING_HEADER, CONTENT_TYPE_HEADER};

/// Headers builder for outgoing log record requests.
/// Uses the builder pattern to construct the final set of headers
/// from various sources (log record, collector auth).
pub(crate) struct LogRecordHeaders(HeaderMap);

impl LogRecordHeaders {
    /// Creates a new empty set of headers.
    pub(crate) fn new() -> Self {
        LogRecordHeaders(HeaderMap::new())
    }

    /// Adds headers from the log record itself.
    /// This includes both custom headers and content-type/encoding headers.
    pub(crate) fn with_log_record(mut self, log_record: &LogRecord) -> Result<Self> {
        self.extract_headers(&log_record.headers)?;
        self.add_content_headers(log_record)?;
        Ok(self)
    }

    /// Adds authentication headers from the collector configuration.
    pub(crate) fn with_collector_auth(
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

    /// Finalizes the headers and returns the underlying HeaderMap.
    pub(crate) fn build(self) -> HeaderMap {
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
    fn add_content_headers(&mut self, log_record: &LogRecord) -> Result<()> {
        if !log_record.content_type.is_empty() {
            self.0.insert(
                HeaderName::from_static(CONTENT_TYPE_HEADER),
                HeaderValue::from_str(&log_record.content_type)?,
            );
            tracing::debug!(
                "Added/Updated content-type header: {}",
                log_record.content_type
            );
        }
        if let Some(content_encoding) = &log_record.content_encoding {
            self.0.insert(
                HeaderName::from_static(CONTENT_ENCODING_HEADER),
                HeaderValue::from_str(content_encoding)?,
            );
        }
        Ok(())
    }
}
