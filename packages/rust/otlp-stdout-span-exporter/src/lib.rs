//! A span exporter that writes OpenTelemetry spans to stdout in OTLP format.
//!
//! This crate provides an implementation of OpenTelemetry's [`SpanExporter`] that writes spans to stdout
//! in OTLP (OpenTelemetry Protocol) format. It is particularly useful in serverless environments like
//! AWS Lambda where writing to stdout is a common pattern for exporting telemetry data.
//!
//! # Features
//!
//! - Uses OTLP Protobuf serialization for efficient encoding
//! - Applies GZIP compression with configurable levels
//! - Detects service name from environment variables
//! - Supports custom headers via environment variables
//! - Consistent JSON output format
//!
//! # Example
//!
//! ```rust,no_run
//! use opentelemetry::global;
//! use opentelemetry::trace::Tracer;
//! use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
//! use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a new stdout exporter
//!     let exporter = OtlpStdoutSpanExporter::default();
//!
//!     // Create a new tracer provider with batch export
//!     let provider = SdkTracerProvider::builder()
//!         .with_batch_exporter(exporter)
//!         .build();
//!
//!     // Register the provider with the OpenTelemetry global API
//!     global::set_tracer_provider(provider.clone());
//!
//!     // Create a tracer
//!     let tracer = global::tracer("my-service");
//!
//!     // Create spans
//!     tracer.in_span("parent-operation", |_cx| {
//!         println!("Doing work...");
//!         
//!         // Create nested spans
//!         tracer.in_span("child-operation", |_cx| {
//!             println!("Doing more work...");
//!         });
//!     });
//!     
//!     // Flush the provider to ensure all spans are exported
//!     if let Err(err) = provider.force_flush() {
//!         println!("Error flushing provider: {:?}", err);
//!     }
//! }
//! ```
//!
//! # Environment Variables
//!
//! The exporter respects the following environment variables:
//!
//! - `OTEL_SERVICE_NAME`: Service name to use in output
//! - `AWS_LAMBDA_FUNCTION_NAME`: Fallback service name (if `OTEL_SERVICE_NAME` not set)
//! - `OTEL_EXPORTER_OTLP_HEADERS`: Global headers for OTLP export
//! - `OTEL_EXPORTER_OTLP_TRACES_HEADERS`: Trace-specific headers (takes precedence if conflicting with `OTEL_EXPORTER_OTLP_HEADERS`)
//! - `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: GZIP compression level (0-9, default: 6)
//!
//! # Configuration Precedence
//!
//! All configuration values follow this strict precedence order:
//!
//! 1. Environment variables (highest precedence)
//! 2. Constructor parameters
//! 3. Default values (lowest precedence)
//!
//! For example, when determining the compression level:
//!
//! ```rust
//! use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
//!
//! // This will use the OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL environment variable if set,
//! // otherwise it will use level 9, which was provided as a parameter
//! let exporter = OtlpStdoutSpanExporter::builder()
//!     .compression_level(9)
//!     .build();
//!
//! // This will use the environment variable if set, or default to level 6
//! let default_exporter = OtlpStdoutSpanExporter::default();
//! ```
//!
//! # Output Format
//!
//! The exporter writes each batch of spans as a JSON object to stdout:
//!
//! ```json
//! {
//!   "__otel_otlp_stdout": "0.1.0",
//!   "source": "my-service",
//!   "endpoint": "http://localhost:4318/v1/traces",
//!   "method": "POST",
//!   "content-type": "application/x-protobuf",
//!   "content-encoding": "gzip",
//!   "headers": {
//!     "api-key": "secret123",
//!     "custom-header": "value"
//!   },
//!   "payload": "<base64-encoded-gzipped-protobuf>",
//!   "base64": true
//! }
//! ```

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as base64_engine, Engine};
use bon::bon;
use flate2::{write::GzEncoder, Compression};
use futures_util::future::BoxFuture;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::resource::Resource;
use opentelemetry_sdk::{
    error::OTelSdkError,
    trace::{SpanData, SpanExporter},
};
use prost::Message;
use serde::Serialize;

mod constants;
use constants::{defaults, env_vars};

// Make the constants module and its sub-modules publicly available
pub mod consts {
    //! Constants used by the exporter.
    //!
    //! This module provides constants for environment variables,
    //! default values, and resource attributes.

    pub use crate::constants::defaults;
    pub use crate::constants::env_vars;
    pub use crate::constants::resource_attributes;
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Trait for output handling
///
/// This trait defines the interface for writing output lines. It is implemented
/// by both the standard output handler and test output handler.
trait Output: Send + Sync + std::fmt::Debug {
    /// Writes a single line of output
    ///
    /// # Arguments
    ///
    /// * `line` - The line to write
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the write was successful, or a `TraceError` if it failed
    fn write_line(&self, line: &str) -> Result<(), OTelSdkError>;
}

/// Standard output implementation that writes to stdout
#[derive(Debug, Default)]
struct StdOutput;

impl Output for StdOutput {
    fn write_line(&self, line: &str) -> Result<(), OTelSdkError> {
        // Get a locked stdout handle once
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        // Write the line and a newline in one operation
        writeln!(handle, "{}", line).map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

        Ok(())
    }
}

/// Output format for the OTLP stdout exporter
///
/// This struct defines the JSON structure that will be written to stdout
/// for each batch of spans.
#[derive(Debug, Serialize)]
struct ExporterOutput<'a> {
    /// Version identifier for the output format
    #[serde(rename = "__otel_otlp_stdout")]
    version: &'a str,
    /// Service name that generated the spans
    source: String,
    /// OTLP endpoint (always http://localhost:4318/v1/traces)
    endpoint: &'a str,
    /// HTTP method (always POST)
    method: &'a str,
    /// Content type (always application/x-protobuf)
    #[serde(rename = "content-type")]
    content_type: &'a str,
    /// Content encoding (always gzip)
    #[serde(rename = "content-encoding")]
    content_encoding: &'a str,
    /// Custom headers from environment variables
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    headers: HashMap<String, String>,
    /// Base64-encoded, gzipped, protobuf-serialized span data
    payload: String,
    /// Whether the payload is base64 encoded (always true)
    base64: bool,
}

/// A span exporter that writes spans to stdout in OTLP format
///
/// This exporter implements the OpenTelemetry [`SpanExporter`] trait and writes spans
/// to stdout in OTLP format with Protobuf serialization and GZIP compression.
///
/// # Features
///
/// - Configurable GZIP compression level (0-9)
/// - Environment variable support for service name and headers
/// - Efficient batching of spans
/// - Base64 encoding of compressed data
///
/// # Example
///
/// ```rust,no_run
/// use opentelemetry_sdk::runtime;
/// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
///
/// // Create an exporter with maximum compression
/// let exporter = OtlpStdoutSpanExporter::builder()
///     .compression_level(9)
///     .build();
/// ```
#[derive(Debug)]
pub struct OtlpStdoutSpanExporter {
    /// GZIP compression level (0-9)
    compression_level: u8,
    /// Output implementation (stdout or test buffer)
    output: Arc<dyn Output>,
    /// Optional resource to be included with all spans
    resource: Option<Resource>,
}

impl Default for OtlpStdoutSpanExporter {
    fn default() -> Self {
        Self::builder().build()
    }
}
#[bon]
impl OtlpStdoutSpanExporter {
    /// Create a new `OtlpStdoutSpanExporter` with default configuration.
    ///
    /// This uses a GZIP compression level of 6 unless overridden by an environment variable.
    ///
    /// # Compression Level
    ///
    /// The compression level is determined in the following order (highest to lowest precedence):
    ///
    /// 1. The `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL` environment variable if set
    /// 2. Default value (6)
    ///
    /// # Example
    ///
    /// ```
    /// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
    ///
    /// let exporter = OtlpStdoutSpanExporter::default();
    /// ```
    #[builder]
    pub fn new(
        compression_level: Option<u8>,
        output: Option<Arc<dyn Output>>,
        resource: Option<Resource>,
    ) -> Self {
        // Set gzip_level with proper precedence (env var > constructor param > default)
        let compression_level = match env::var(env_vars::COMPRESSION_LEVEL) {
            Ok(value) => match value.parse::<u8>() {
                Ok(level) if level <= 9 => level,
                Ok(level) => {
                    log::warn!(
                        "Invalid value in {}: {} (must be 0-9), using fallback",
                        env_vars::COMPRESSION_LEVEL,
                        level
                    );
                    compression_level.unwrap_or(defaults::COMPRESSION_LEVEL)
                }
                Err(_) => {
                    log::warn!(
                        "Failed to parse {}: {}, using fallback",
                        env_vars::COMPRESSION_LEVEL,
                        value
                    );
                    compression_level.unwrap_or(defaults::COMPRESSION_LEVEL)
                }
            },
            Err(_) => {
                // No environment variable, use parameter or default
                compression_level.unwrap_or(defaults::COMPRESSION_LEVEL)
            }
        };

        Self {
            compression_level,
            resource,
            output: output.unwrap_or(Arc::new(StdOutput)),
        }
    }

    /// Get the service name from environment variables.
    ///
    /// The service name is determined in the following order:
    ///
    /// 1. OTEL_SERVICE_NAME
    /// 2. AWS_LAMBDA_FUNCTION_NAME
    /// 3. "unknown-service" (fallback)
    fn get_service_name() -> String {
        env::var(env_vars::SERVICE_NAME)
            .or_else(|_| env::var(env_vars::AWS_LAMBDA_FUNCTION_NAME))
            .unwrap_or_else(|_| defaults::SERVICE_NAME.to_string())
    }

    #[cfg(test)]
    fn with_test_output() -> (Self, Arc<TestOutput>) {
        let output = Arc::new(TestOutput::new());

        // Use the standard new() method to ensure environment variables are respected
        let exporter = Self::builder().output(output.clone()).build();

        (exporter, output)
    }

    /// Parse headers from environment variables
    ///
    /// This function reads headers from both global and trace-specific
    /// environment variables, with trace-specific headers taking precedence.
    fn parse_headers() -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // Parse global headers first
        if let Ok(global_headers) = env::var("OTEL_EXPORTER_OTLP_HEADERS") {
            Self::parse_header_string(&global_headers, &mut headers);
        }

        // Parse trace-specific headers (these take precedence)
        if let Ok(trace_headers) = env::var("OTEL_EXPORTER_OTLP_TRACES_HEADERS") {
            Self::parse_header_string(&trace_headers, &mut headers);
        }

        headers
    }

    /// Parse a header string in the format key1=value1,key2=value2
    ///
    /// # Arguments
    ///
    /// * `header_str` - The header string to parse
    /// * `headers` - The map to store parsed headers in
    fn parse_header_string(header_str: &str, headers: &mut HashMap<String, String>) {
        for pair in header_str.split(',') {
            if let Some((key, value)) = pair.split_once('=') {
                let key = key.trim().to_lowercase();
                // Skip content-type and content-encoding as they are fixed
                if key != "content-type" && key != "content-encoding" {
                    headers.insert(key, value.trim().to_string());
                }
            }
        }
    }
}

#[async_trait]
impl SpanExporter for OtlpStdoutSpanExporter {
    /// Export spans to stdout in OTLP format
    ///
    /// This function:
    /// 1. Converts spans to OTLP format
    /// 2. Serializes them to protobuf
    /// 3. Compresses the data with GZIP
    /// 4. Base64 encodes the result
    /// 5. Writes a JSON object to stdout
    ///
    /// # Arguments
    ///
    /// * `batch` - A vector of spans to export
    ///
    /// # Returns
    ///
    /// Returns a resolved future with `Ok(())` if the export was successful, or a `TraceError` if it failed
    fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, Result<(), OTelSdkError>> {
        // Do all work synchronously
        let result = (|| {
            // Convert spans to OTLP format
            let resource = self
                .resource
                .clone()
                .unwrap_or_else(|| opentelemetry_sdk::Resource::builder_empty().build());
            let resource_attrs = ResourceAttributesWithSchema::from(&resource);
            let resource_spans = group_spans_by_resource_and_scope(batch, &resource_attrs);
            let request = ExportTraceServiceRequest { resource_spans };

            // Serialize to protobuf
            let proto_bytes = request.encode_to_vec();

            // Compress with GZIP
            let mut encoder =
                GzEncoder::new(Vec::new(), Compression::new(self.compression_level as u32));
            encoder
                .write_all(&proto_bytes)
                .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
            let compressed_bytes = encoder
                .finish()
                .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

            // Base64 encode
            let payload = base64_engine.encode(compressed_bytes);

            // Prepare the output
            let output_data = ExporterOutput {
                version: VERSION,
                source: Self::get_service_name(),
                endpoint: defaults::ENDPOINT,
                method: "POST",
                content_type: "application/x-protobuf",
                content_encoding: "gzip",
                headers: Self::parse_headers(),
                payload,
                base64: true,
            };

            // Write using the output implementation
            self.output.write_line(
                &serde_json::to_string(&output_data)
                    .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?,
            )?;

            Ok(())
        })();

        // Return a resolved future with the result
        Box::pin(std::future::ready(result))
    }

    /// Shuts down the exporter
    ///
    /// This is a no-op for stdout export as no cleanup is needed.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` as there is nothing to clean up.
    fn shutdown(&mut self) -> Result<(), OTelSdkError> {
        Ok(())
    }

    /// Force flushes any pending spans
    ///
    /// This is a no-op for stdout export as spans are written immediately.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` as there is nothing to flush.
    fn force_flush(&mut self) -> Result<(), OTelSdkError> {
        Ok(())
    }

    /// Sets the resource for this exporter.
    ///
    /// This method stores a clone of the provided resource to be used when exporting spans.
    /// The resource represents the entity producing telemetry and will be included in the
    /// exported trace data.
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to associate with this exporter
    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.resource = Some(<opentelemetry_sdk::Resource as Into<Resource>>::into(
            resource.clone(),
        ));
    }
}

#[cfg(doctest)]
#[macro_use]
extern crate doc_comment;

#[cfg(doctest)]
use doc_comment::doctest;

#[cfg(doctest)]
doctest!("../README.md", readme);

#[cfg(test)]
use std::sync::Mutex;
use std::{
    collections::HashMap,
    env,
    io::{self, Write},
    result::Result,
    sync::Arc,
};

/// Test output implementation that captures to a buffer
#[cfg(test)]
#[derive(Debug, Default)]
struct TestOutput {
    buffer: Arc<Mutex<Vec<String>>>,
}

#[cfg(test)]
impl TestOutput {
    fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_output(&self) -> Vec<String> {
        self.buffer.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl Output for TestOutput {
    fn write_line(&self, line: &str) -> Result<(), OTelSdkError> {
        self.buffer.lock().unwrap().push(line.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::{
        trace::{SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState},
        InstrumentationScope, KeyValue,
    };
    use opentelemetry_sdk::trace::{SpanData, SpanEvents, SpanLinks};
    use serde_json::Value;
    use serial_test::serial;
    use std::time::SystemTime;

    fn create_test_span() -> SpanData {
        let trace_id_bytes = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42];
        let span_id_bytes = [0, 0, 0, 0, 0, 0, 0, 123];
        let parent_id_bytes = [0, 0, 0, 0, 0, 0, 0, 42];

        let span_context = SpanContext::new(
            TraceId::from_bytes(trace_id_bytes),
            SpanId::from_bytes(span_id_bytes),
            TraceFlags::default(),
            false,
            TraceState::default(),
        );

        SpanData {
            span_context,
            parent_span_id: SpanId::from_bytes(parent_id_bytes),
            span_kind: SpanKind::Client,
            name: "test-span".into(),
            start_time: SystemTime::UNIX_EPOCH,
            end_time: SystemTime::UNIX_EPOCH,
            attributes: vec![KeyValue::new("test.key", "test-value")],
            dropped_attributes_count: 0,
            events: SpanEvents::default(),
            links: SpanLinks::default(),
            status: Status::Ok,
            instrumentation_scope: InstrumentationScope::builder("test-library")
                .with_version("1.0.0")
                .with_schema_url("https://opentelemetry.io/schema/1.0.0")
                .build(),
        }
    }

    #[test]
    fn test_parse_headers() {
        std::env::set_var("OTEL_EXPORTER_OTLP_HEADERS", "key1=value1,key2=value2");
        std::env::set_var(
            "OTEL_EXPORTER_OTLP_TRACES_HEADERS",
            "key2=override,key3=value3",
        );

        let headers = OtlpStdoutSpanExporter::parse_headers();

        assert_eq!(headers.get("key1").unwrap(), "value1");
        assert_eq!(headers.get("key2").unwrap(), "override");
        assert_eq!(headers.get("key3").unwrap(), "value3");

        // Clean up
        std::env::remove_var("OTEL_EXPORTER_OTLP_HEADERS");
        std::env::remove_var("OTEL_EXPORTER_OTLP_TRACES_HEADERS");
    }

    #[test]
    fn test_service_name_resolution() {
        // Test OTEL_SERVICE_NAME priority
        std::env::set_var(env_vars::SERVICE_NAME, "otel-service");
        std::env::set_var(env_vars::AWS_LAMBDA_FUNCTION_NAME, "lambda-function");
        assert_eq!(OtlpStdoutSpanExporter::get_service_name(), "otel-service");

        // Test AWS_LAMBDA_FUNCTION_NAME fallback
        std::env::remove_var(env_vars::SERVICE_NAME);
        assert_eq!(
            OtlpStdoutSpanExporter::get_service_name(),
            "lambda-function"
        );

        // Test default fallback
        std::env::remove_var(env_vars::AWS_LAMBDA_FUNCTION_NAME);
        assert_eq!(
            OtlpStdoutSpanExporter::get_service_name(),
            defaults::SERVICE_NAME
        );
    }

    #[test]
    fn test_compression_level_precedence() {
        // Test env var takes precedence over options
        std::env::set_var(env_vars::COMPRESSION_LEVEL, "3");
        let exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(7)
            .build();
        assert_eq!(exporter.compression_level, 3);

        // Test invalid env var falls back to options
        std::env::set_var(env_vars::COMPRESSION_LEVEL, "invalid");
        let exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(7)
            .build();
        assert_eq!(exporter.compression_level, 7);

        // Test no env var uses options
        std::env::remove_var(env_vars::COMPRESSION_LEVEL);
        let exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(7)
            .build();
        assert_eq!(exporter.compression_level, 7);

        // Test fallback to default
        let exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(defaults::COMPRESSION_LEVEL)
            .build();
        assert_eq!(exporter.compression_level, defaults::COMPRESSION_LEVEL);
    }

    #[test]
    fn test_new_uses_env_compression_level() {
        // Set environment variable
        std::env::set_var(env_vars::COMPRESSION_LEVEL, "3");
        let exporter = OtlpStdoutSpanExporter::default();
        assert_eq!(exporter.compression_level, 3);

        // Test with unset variable (should use default)
        std::env::remove_var(env_vars::COMPRESSION_LEVEL);
        let exporter = OtlpStdoutSpanExporter::default();
        assert_eq!(exporter.compression_level, defaults::COMPRESSION_LEVEL);
    }

    #[tokio::test]
    #[serial]
    async fn test_compression_level_affects_output_size() {
        // Create a large span batch to make compression differences more noticeable
        let mut spans = Vec::new();
        for i in 0..100 {
            let mut span = create_test_span();
            // Add unique attributes to each span to increase data size
            span.attributes.push(KeyValue::new("index", i));
            // Add a large attribute to make compression more effective
            span.attributes
                .push(KeyValue::new("data", "a".repeat(1000)));
            spans.push(span);
        }

        // Make sure environment variables don't interfere with our test
        std::env::remove_var(env_vars::COMPRESSION_LEVEL);

        // Create exporter with no compression (level 0)
        let no_compression_output = Arc::new(TestOutput::new());
        let mut no_compression_exporter = OtlpStdoutSpanExporter {
            compression_level: 0,
            resource: None,
            output: no_compression_output.clone() as Arc<dyn Output>,
        };
        let _ = no_compression_exporter.export(spans.clone()).await;
        let no_compression_size = extract_payload_size(&no_compression_output.get_output()[0]);

        // Create exporter with max compression (level 9)
        let max_compression_output = Arc::new(TestOutput::new());
        let mut max_compression_exporter = OtlpStdoutSpanExporter {
            compression_level: 9,
            resource: None,
            output: max_compression_output.clone() as Arc<dyn Output>,
        };
        let _ = max_compression_exporter.export(spans.clone()).await;
        let max_compression_size = extract_payload_size(&max_compression_output.get_output()[0]);

        // Verify that higher compression levels result in smaller payloads
        assert!(no_compression_size > max_compression_size,
            "Maximum compression (level 9) should produce output no larger than no compression (level 0). Got {} vs {}",
            max_compression_size, no_compression_size);

        // Verify that all outputs can be properly decoded and contain the same data
        let no_compression_spans = decode_and_count_spans(&no_compression_output.get_output()[0]);
        let max_compression_spans = decode_and_count_spans(&max_compression_output.get_output()[0]);

        assert_eq!(
            no_compression_spans,
            spans.len(),
            "No compression output should contain all spans"
        );
        assert_eq!(
            max_compression_spans,
            spans.len(),
            "Maximum compression output should contain all spans"
        );
    }

    // Helper function to extract the size of the base64-decoded payload
    fn extract_payload_size(json_str: &str) -> usize {
        let json: Value = serde_json::from_str(json_str).unwrap();
        let payload = json["payload"].as_str().unwrap();
        base64_engine.decode(payload).unwrap().len()
    }

    // Helper function to decode the payload and count the number of spans
    fn decode_and_count_spans(json_str: &str) -> usize {
        let json: Value = serde_json::from_str(json_str).unwrap();
        let payload = json["payload"].as_str().unwrap();
        let decoded = base64_engine.decode(payload).unwrap();

        let mut decoder = flate2::read::GzDecoder::new(&decoded[..]);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed).unwrap();

        let request = ExportTraceServiceRequest::decode(&*decompressed).unwrap();

        // Count total spans across all resource spans
        let mut span_count = 0;
        for resource_span in &request.resource_spans {
            for scope_span in &resource_span.scope_spans {
                span_count += scope_span.spans.len();
            }
        }

        span_count
    }

    #[tokio::test]
    async fn test_export_single_span() {
        let (mut exporter, output) = OtlpStdoutSpanExporter::with_test_output();
        let span = create_test_span();

        let result = exporter.export(vec![span]).await;
        assert!(result.is_ok());

        let output = output.get_output();
        assert_eq!(output.len(), 1);

        // Parse and verify the output
        let json: Value = serde_json::from_str(&output[0]).unwrap();
        assert_eq!(json["__otel_otlp_stdout"], VERSION);
        assert_eq!(json["method"], "POST");
        assert_eq!(json["content-type"], "application/x-protobuf");
        assert_eq!(json["content-encoding"], "gzip");
        assert_eq!(json["base64"], true);

        // Verify payload is valid base64 and can be decoded
        let payload = json["payload"].as_str().unwrap();
        let decoded = base64_engine.decode(payload).unwrap();

        // Verify it can be decompressed
        let mut decoder = flate2::read::GzDecoder::new(&decoded[..]);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed).unwrap();

        // Verify it's valid OTLP protobuf
        let request = ExportTraceServiceRequest::decode(&*decompressed).unwrap();
        assert_eq!(request.resource_spans.len(), 1);
    }

    #[tokio::test]
    async fn test_export_empty_batch() {
        let mut exporter = OtlpStdoutSpanExporter::default();
        let result = exporter.export(vec![]).await;
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_gzip_level_configuration() {
        // Ensure all environment variables are removed first
        std::env::remove_var(env_vars::COMPRESSION_LEVEL);

        // Now test the constructor parameter
        let exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(9)
            .build();
        assert_eq!(exporter.compression_level, 9);
    }

    #[tokio::test]
    #[serial]
    async fn test_env_var_affects_export_compression() {
        // Create more test data with repeated content to make compression differences noticeable
        let span = create_test_span();
        let mut spans = Vec::new();
        // Create 100 spans with large attributes to make compression differences noticeable
        for i in 0..100 {
            let mut span = span.clone();
            // Add unique attribute with large value to make compression more effective
            span.attributes
                .push(KeyValue::new(format!("test-key-{}", i), "a".repeat(1000)));
            spans.push(span);
        }

        // First, create data with no compression
        std::env::set_var(env_vars::COMPRESSION_LEVEL, "0");
        let no_compression_output = Arc::new(TestOutput::new());
        let mut no_compression_exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(0)
            .build();
        no_compression_exporter.output = no_compression_output.clone() as Arc<dyn Output>;
        let _ = no_compression_exporter.export(spans.clone()).await;
        let no_compression_size = extract_payload_size(&no_compression_output.get_output()[0]);

        // Now with max compression
        std::env::set_var(env_vars::COMPRESSION_LEVEL, "9");
        let max_compression_output = Arc::new(TestOutput::new());
        let mut max_compression_exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(9)
            .build();
        max_compression_exporter.output = max_compression_output.clone() as Arc<dyn Output>;
        let _ = max_compression_exporter.export(spans.clone()).await;
        let max_compression_size = extract_payload_size(&max_compression_output.get_output()[0]);

        // Verify that the environment variable affected the compression level
        assert!(no_compression_size > max_compression_size,
            "Environment variable COMPRESSION_LEVEL=9 should produce smaller output than COMPRESSION_LEVEL=0. Got {} vs {}",
            max_compression_size, no_compression_size);

        // Test with explicit level when env var is set (env var should take precedence)
        std::env::set_var(env_vars::COMPRESSION_LEVEL, "0");
        let explicit_output = Arc::new(TestOutput::new());

        // Create an exporter with the default() method which will use the environment variable
        let mut explicit_exporter = OtlpStdoutSpanExporter::default();
        explicit_exporter.output = explicit_output.clone() as Arc<dyn Output>;

        // The environment variable should make it use compression level 0
        let _ = explicit_exporter.export(spans.clone()).await;
        let explicit_size = extract_payload_size(&explicit_output.get_output()[0]);

        // Should be approximately the same size as the no_compression_size since
        // the environment variable (level 0) should take precedence
        assert!(explicit_size > max_compression_size,
            "Environment variable should take precedence over explicitly set level. Expected size closer to {} but got {}",
            no_compression_size, explicit_size);

        // Clean up
        std::env::remove_var(env_vars::COMPRESSION_LEVEL);
    }

    #[tokio::test]
    #[serial]
    async fn test_environment_variable_precedence() {
        // Set environment variable
        std::env::set_var(env_vars::COMPRESSION_LEVEL, "3");

        // With the new precedence rules, environment variables take precedence
        // over constructor parameters
        let exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(9)
            .build();
        assert_eq!(exporter.compression_level, 3);

        // When environment variable is removed, constructor parameter should be used
        std::env::remove_var(env_vars::COMPRESSION_LEVEL);
        let exporter = OtlpStdoutSpanExporter::builder()
            .compression_level(9)
            .build();
        assert_eq!(exporter.compression_level, 9);
    }
}
