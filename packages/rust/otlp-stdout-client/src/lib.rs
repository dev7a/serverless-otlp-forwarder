//! # otlp-stdout-client
//!
//! `otlp-stdout-client` is a Rust library that provides an OpenTelemetry exporter
//! designed for serverless environments, particularly AWS Lambda functions. This crate
//! is part of the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder/)
//! project, which provides a comprehensive solution for OpenTelemetry telemetry collection
//! in AWS Lambda environments.
//!
//! This exporter implements the `opentelemetry_http::HttpClient` interface and can be used
//! in an OpenTelemetry OTLP pipeline to send OTLP data (both JSON and Protobuf formats)
//! to stdout. This allows the data to be easily ingested and forwarded to an OTLP collector.
//!
//! ## Key Features
//!
//! - Implements `opentelemetry_http::HttpClient` for use in OTLP pipelines
//! - Exports OpenTelemetry data to stdout in a structured format
//! - Designed for serverless environments, especially AWS Lambda
//! - Configurable through environment variables
//! - Optional GZIP compression of payloads
//! - Supports both JSON and Protobuf payloads
//! ## Usage
//!
//! This library can be integrated into OpenTelemetry OTLP pipelines to redirect
//! telemetry data to stdout. It's particularly useful in serverless environments
//! where direct network access to a collector might not be available or desired.
//!
//! ```rust
//! use otlp_stdout_client::StdoutClient;
//! use opentelemetry_sdk::trace::TracerProvider;
//! use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
//! use opentelemetry::trace::Tracer;
//! use opentelemetry::global;
//!
//! fn init_tracer_provider() -> Result<TracerProvider, Box<dyn std::error::Error>> {
//!     let exporter = opentelemetry_otlp::SpanExporter::builder()
//!         .with_http()
//!         .with_http_client(StdoutClient::default())
//!         .build()?;
//!     
//!     let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
//!         .with_simple_exporter(exporter)
//!         .build();
//!
//!     Ok(tracer_provider)
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let tracer_provider = init_tracer_provider()?;
//!     global::set_tracer_provider(tracer_provider);
//!     
//!     let tracer = global::tracer("my_tracer");
//!     
//!     // Use the tracer for instrumenting your code
//!     // For example:
//!     tracer.in_span("example_span", |_cx| {
//!         // Your code here
//!     });
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Configuration
//!
//! The exporter can be configured using the following standard OTEL environment variables:
//!
//! - `OTEL_EXPORTER_OTLP_PROTOCOL`: Specifies the protocol to use for the OTLP exporter.
//!   Valid values are:
//!   - "http/json" (default): Uses HTTP with JSON payload
//!   - "http/protobuf": Uses HTTP with Protobuf payload
//!
//! - `OTEL_EXPORTER_OTLP_ENDPOINT`: Sets the endpoint for the OTLP exporter.
//!   Specify the endpoint of your OTLP collector.
//!
//! - `OTEL_EXPORTER_OTLP_HEADERS`: Sets additional headers for the OTLP exporter.
//!   Format: "key1=value1,key2=value2"
//!
//! - `OTEL_EXPORTER_OTLP_COMPRESSION`: Specifies the compression algorithm to use.
//!   Valid values are:
//!   - "gzip": Compresses the payload using GZIP
//!   - If not set or any other value, no compression is applied
//!   
//!
//! For more detailed information on usage and configuration, please refer to the README.md file.

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as Base64Engine};
use bytes::Bytes;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use http::{Request, Response};

use opentelemetry_http::HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::{env, error::Error as StdError, fmt::Debug, io::Read};
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

// Constants for content types
pub const CONTENT_TYPE_JSON: &str = "application/json";
pub const CONTENT_TYPE_PROTOBUF: &str = "application/x-protobuf";

// Constants for headers
pub const CONTENT_TYPE_HEADER: &str = "content-type";
pub const CONTENT_ENCODING_HEADER: &str = "content-encoding";

// Constants for JSON keys
pub const KEY_SOURCE: &str = "source";
pub const KEY_ENDPOINT: &str = "endpoint";
pub const KEY_METHOD: &str = "method";
pub const KEY_PAYLOAD: &str = "payload";
pub const KEY_BASE64: &str = "base64";
pub const KEY_HEADERS: &str = "headers";

// Constant for OTEL version prefix
pub const OTEL_VERSION_PREFIX: &str = "oltp-stdout-";

// Constant for GZIP encoding
pub const ENCODING_GZIP: &str = "gzip";

#[derive(Debug, Serialize, Deserialize)]
pub struct LogRecord {
    #[serde(rename = "__otel_otlp_stdout")]
    pub _otel: String,
    pub source: String,
    pub endpoint: String,
    pub method: String,
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(rename = "content-type")]
    pub content_type: String,
    #[serde(rename = "content-encoding", skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<bool>,
}
pub struct StdoutClient {
    service_name: String,
    content_encoding_gzip: Option<String>,
    writer: Arc<Mutex<Box<dyn AsyncWrite + Send + Sync + Unpin>>>,
    version_identifier: String,
}

impl Debug for StdoutClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdoutClient")
            .field("content_encoding_gzip", &self.content_encoding_gzip)
            .field("writer", &"Box<dyn Write + Send + Sync>")
            .field("service_name", &self.service_name)
            .finish()
    }
}

impl StdoutClient {
    /// Creates a new `StdoutClient` with the default writer (`stdout`).
    ///
    /// # Example
    ///
    /// ```rust
    /// use otlp_stdout_client::StdoutClient;
    ///
    /// let client = StdoutClient::new();
    /// ```
    pub fn new() -> Self {
        StdoutClient {
            content_encoding_gzip: Self::parse_compression(),
            writer: Arc::new(Mutex::new(Box::new(tokio::io::stdout()))),
            service_name: Self::get_service_name(),
            version_identifier: Self::get_version_identifier(),
        }
    }

    /// Creates a new `StdoutClient` with a custom writer.
    ///
    /// This method allows you to specify an alternative writer, such as a file or an in-memory buffer.
    ///
    /// # Arguments
    ///
    /// * `writer` - Any writer implementing `AsyncWrite + Send + Sync + Unpin + 'static`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use otlp_stdout_client::StdoutClient;
    /// use tokio::fs::File;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// // Using a file as the writer
    /// let file = File::create("output.log").await?;
    /// let client = StdoutClient::new_with_writer(file);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_with_writer<W>(writer: W) -> Self
    where
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        StdoutClient {
            content_encoding_gzip: Self::parse_compression(),
            writer: Arc::new(Mutex::new(Box::new(writer))),
            service_name: Self::get_service_name(),
            version_identifier: Self::get_version_identifier(),
        }
    }

    /// Parses the compression setting from the environment variable.
    ///
    /// This function reads the `OTEL_EXPORTER_OTLP_COMPRESSION` environment variable
    /// and returns `Some("gzip")` if the value is "gzip", otherwise it returns `None`.
    ///
    /// # Returns
    ///
    /// An `Option<String>` containing "gzip" if compression is enabled, or `None` otherwise.
    fn parse_compression() -> Option<String> {
        env::var("OTEL_EXPORTER_OTLP_COMPRESSION")
            .ok()
            .filter(|v| v.eq_ignore_ascii_case(ENCODING_GZIP))
            .map(|_| ENCODING_GZIP.to_string())
    }

    /// Gets the service name from environment variables.
    ///
    /// This function reads the `OTEL_SERVICE_NAME` or `AWS_LAMBDA_FUNCTION_NAME` environment variable
    /// and returns its value. If neither is set, it returns "unknown-service".
    ///
    /// # Returns
    ///
    /// A String containing the service name.
    fn get_service_name() -> String {
        env::var("OTEL_SERVICE_NAME")
            .or_else(|_| env::var("AWS_LAMBDA_FUNCTION_NAME"))
            .unwrap_or_else(|_| "unknown-service".to_string())
    }

    /// Processes the HTTP request payload, handling compression, decompression,
    /// and JSON optimization based on content type and encoding.
    ///
    /// Logic flow:
    /// 1. For JSON payloads:
    ///    - If input is gzipped, decompress it
    ///    - Always optimize the JSON
    ///    - Only base64 encode if we're going to compress it for output
    ///
    /// 2. For non-JSON payloads (protobuf):
    ///    - Always base64 encode (since it's binary)
    ///    - Keep the original payload as-is
    ///    - If output compression is enabled, compress it
    ///
    /// 3. For all payloads:
    ///    - If output compression is enabled, compress and mark for base64 encoding
    ///    - Base64 encode if either:
    ///      - It's a binary payload (protobuf)
    ///      - We compressed it for output
    ///
    /// # Arguments
    ///
    /// * `request` - The incoming HTTP request containing the payload
    /// * `content_type` - The MIME content type of the payload
    /// * `content_encoding` - The content encoding of the payload, if any
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure
    async fn process_payload(
        &self,
        request: &Request<bytes::Bytes>,
        content_type: &str,
        content_encoding: Option<&str>,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let is_input_gzipped = content_encoding == Some(ENCODING_GZIP);
        let is_json = content_type == CONTENT_TYPE_JSON;
        let mut should_encode_base64 = false;

        let mut payload = request.body().clone();
        // if input is json, optionally decompress it and optimize it
        if is_json {
            let decompressed = if is_input_gzipped {
                Self::decompress_payload(request.body())?.into()
            } else {
                request.body().clone()
            };
            payload = Self::optimize_json(&decompressed)?.into();
        } else {
            // if input is not json, we need to encode it as base64 in any case
            should_encode_base64 = true;
        }

        // Compress payload if output compression is enabled
        if self.content_encoding_gzip.is_some() {
            payload = Self::compress_payload(&payload)?.into();
            // if we compressed, we need to encode as base64
            should_encode_base64 = true;
        }

        // Prepare final payload so it can be serialized to json
        let final_payload = if should_encode_base64 {
            Value::String(Self::encode_base64(&payload))
        } else {
            serde_json::from_slice(&payload)?
        };

        // Create the log record
        let log_record = LogRecord {
            _otel: self.version_identifier.clone(),
            source: self.service_name.clone(),
            endpoint: request.uri().to_string(),
            method: request.method().to_string(),
            payload: final_payload,
            headers: Some(Self::headers_to_hashmap(request.headers())),
            content_type: content_type.to_string(),
            content_encoding: self.content_encoding_gzip.clone(),
            base64: Some(should_encode_base64),
        };

        // Write the log record
        let mut writer = self.writer.lock().await;
        let json = format!("{}\n", serde_json::to_string(&log_record)?);
        writer.write_all(json.as_bytes()).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Decompresses a GZIP-compressed payload.
    ///
    /// # Arguments
    ///
    /// * `payload` - The compressed payload as a byte slice.
    ///
    /// # Returns
    ///
    /// A `Result` containing the decompressed payload as a `Vec<u8>`.
    fn decompress_payload(payload: &[u8]) -> Result<Vec<u8>, Box<dyn StdError + Send + Sync>> {
        let mut decoder = GzDecoder::new(payload);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }

    /// Optimizes a JSON payload by parsing and re-serializing it.
    ///
    /// This removes unnecessary whitespace and ensures a consistent JSON format.
    ///
    /// # Arguments
    ///
    /// * `payload` - The JSON payload as a byte slice.
    ///
    /// # Returns
    ///
    /// A `Result` containing the optimized JSON payload as a `Vec<u8>`.
    fn optimize_json(payload: &[u8]) -> Result<Vec<u8>, Box<dyn StdError + Send + Sync>> {
        let json_value: Value = serde_json::from_slice(payload)?;
        let optimized = serde_json::to_vec(&json_value)?;
        Ok(optimized)
    }

    /// Compresses the payload using GZIP compression.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload as a byte slice.
    ///
    /// # Returns
    ///
    /// A `Result` containing the compressed payload as a `Vec<u8>`.
    fn compress_payload(payload: &[u8]) -> Result<Vec<u8>, Box<dyn StdError + Send + Sync>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, payload)?;
        Ok(encoder.finish()?)
    }

    /// Encodes a byte slice to a base64 string.
    ///
    /// This function takes a byte slice and encodes it to a base64 string
    /// using the standard base64 alphabet.
    ///
    /// # Arguments
    ///
    /// * `payload` - A byte slice containing the data to be encoded.
    ///
    /// # Returns
    ///
    /// A `String` containing the base64 encoded representation of the input payload.
    fn encode_base64(payload: &[u8]) -> String {
        general_purpose::STANDARD.encode(payload)
    }

    /// Converts HTTP headers to a `HashMap` with lowercase header names.
    ///
    /// # Arguments
    ///
    /// * `headers` - The HTTP headers from the request.
    ///
    /// # Returns
    ///
    /// A `HashMap` mapping header names to their values.
    fn headers_to_hashmap(headers: &http::HeaderMap) -> HashMap<String, String> {
        headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.as_str().to_lowercase(), v.to_string()))
            })
            .collect()
    }

    /// Gets the version identifier string for the client.
    ///
    /// This function returns a string containing the package name and version,
    /// formatted as "{package_name}@{version}". The values are obtained from
    /// cargo environment variables at compile time.
    ///
    /// # Returns
    ///
    /// A String containing the version identifier in the format "package@version"
    fn get_version_identifier() -> String {
        format!("{}@{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    }
}

impl Default for StdoutClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Implements the `HttpClient` trait for `StdoutClient`.
///
/// This implementation allows the `StdoutClient` to be used as an HTTP client
/// for sending OTLP (OpenTelemetry Protocol) data. It processes the request body
/// and writes it to stdout in a JSON format suitable for log parsing.
///
/// The `send` method handles both JSON and non-JSON payloads, formatting them
/// appropriately for stdout output.
#[async_trait]
impl HttpClient for StdoutClient {
    async fn send_bytes(
        &self,
        request: Request<bytes::Bytes>,
    ) -> Result<Response<Bytes>, Box<dyn StdError + Send + Sync>> {
        let headers = request.headers();
        let content_type = headers
            .get(CONTENT_TYPE_HEADER)
            .and_then(|ct| ct.to_str().ok());
        let content_encoding = headers
            .get(CONTENT_ENCODING_HEADER)
            .and_then(|ct| ct.to_str().ok());

        match content_type {
            Some(content_type) => {
                self.process_payload(&request, content_type, content_encoding)
                    .await?;
            }
            _ => {
                let message = match content_type {
                    Some(ct) => format!("Content type '{}' is not supported", ct),
                    None => "Content type not specified".to_string(),
                };
                tracing::warn!("{message}. Skipping processing.");
                return Ok(Response::builder().status(200).body(Bytes::new()).unwrap());
            }
        }

        Ok(Response::builder().status(200).body(Bytes::new()).unwrap())
    }
}

#[cfg(test)]
mod tests;

#[cfg(doctest)]
extern crate doc_comment;

#[cfg(doctest)]
use doc_comment::doctest;

#[cfg(doctest)]
doctest!("../README.md", readme);
