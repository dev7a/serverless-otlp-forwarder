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
//!     let exporter = OtlpStdoutSpanExporter::new();
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
#[cfg(test)]
use std::sync::Mutex;
use std::{
    collections::HashMap,
    env,
    io::{self, Write},
    result::Result,
    sync::Arc,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ENDPOINT: &str = "http://localhost:4318/v1/traces";
const DEFAULT_COMPRESSION_LEVEL: u8 = 6;
const COMPRESSION_LEVEL_ENV_VAR: &str = "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL";

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
/// let exporter = OtlpStdoutSpanExporter::with_gzip_level(9);
/// ```
#[derive(Debug)]
pub struct OtlpStdoutSpanExporter {
    /// GZIP compression level (0-9)
    gzip_level: u8,
    /// Optional resource to be included with all spans
    resource: Option<Resource>,
    /// Output implementation (stdout or test buffer)
    output: Arc<dyn Output>,
}

impl Default for OtlpStdoutSpanExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl OtlpStdoutSpanExporter {
    /// Creates a new exporter with default configuration
    ///
    /// The default GZIP compression level is determined by:
    /// 1. The `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL` environment variable if set
    /// 2. Otherwise, defaults to level 6
    ///
    /// # Example
    ///
    /// ```rust
    /// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
    ///
    /// let exporter = OtlpStdoutSpanExporter::new();
    /// ```
    pub fn new() -> Self {
        let gzip_level =
            Self::get_compression_level_from_env().unwrap_or(DEFAULT_COMPRESSION_LEVEL);
        Self {
            gzip_level,
            resource: None,
            output: Arc::new(StdOutput),
        }
    }

    /// Creates a new exporter with custom GZIP compression level
    ///
    /// This method explicitly sets the compression level, overriding any value
    /// set in the `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL` environment variable.
    ///
    /// # Arguments
    ///
    /// * `gzip_level` - GZIP compression level (0-9, where 0 is no compression and 9 is maximum compression)
    ///
    /// # Example
    ///
    /// ```rust
    /// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
    ///
    /// // Create an exporter with maximum compression
    /// let exporter = OtlpStdoutSpanExporter::with_gzip_level(9);
    /// ```
    pub fn with_gzip_level(gzip_level: u8) -> Self {
        Self {
            gzip_level,
            resource: None,
            output: Arc::new(StdOutput),
        }
    }

    /// Get the compression level from environment variable
    ///
    /// This function tries to read the compression level from the
    /// `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL` environment variable.
    /// It returns None if the variable is not set or cannot be parsed as a u8.
    fn get_compression_level_from_env() -> Option<u8> {
        env::var(COMPRESSION_LEVEL_ENV_VAR)
            .ok()
            .and_then(|val| val.parse::<u8>().ok())
            .and_then(|level| if level <= 9 { Some(level) } else { None })
    }

    #[cfg(test)]
    fn with_test_output() -> (Self, Arc<TestOutput>) {
        let output = Arc::new(TestOutput::new());
        let gzip_level =
            Self::get_compression_level_from_env().unwrap_or(DEFAULT_COMPRESSION_LEVEL);
        let exporter = Self {
            gzip_level,
            resource: None,
            output: output.clone() as Arc<dyn Output>,
        };
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

    /// Get the service name from environment variables
    ///
    /// This function tries to get the service name from:
    /// 1. OTEL_SERVICE_NAME
    /// 2. AWS_LAMBDA_FUNCTION_NAME
    /// 3. Falls back to "unknown-service" if neither is set
    fn get_service_name() -> String {
        env::var("OTEL_SERVICE_NAME")
            .or_else(|_| env::var("AWS_LAMBDA_FUNCTION_NAME"))
            .unwrap_or_else(|_| "unknown-service".to_string())
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
            let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.gzip_level as u32));
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
                endpoint: DEFAULT_ENDPOINT,
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
mod tests {
    use super::*;
    use opentelemetry::{
        trace::{SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState},
        InstrumentationScope, KeyValue,
    };
    use opentelemetry_sdk::trace::{SpanData, SpanEvents, SpanLinks};
    use serde_json::Value;
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
        std::env::set_var("OTEL_SERVICE_NAME", "otel-service");
        std::env::set_var("AWS_LAMBDA_FUNCTION_NAME", "lambda-function");
        assert_eq!(OtlpStdoutSpanExporter::get_service_name(), "otel-service");

        // Test AWS_LAMBDA_FUNCTION_NAME fallback
        std::env::remove_var("OTEL_SERVICE_NAME");
        assert_eq!(
            OtlpStdoutSpanExporter::get_service_name(),
            "lambda-function"
        );

        // Test default value
        std::env::remove_var("AWS_LAMBDA_FUNCTION_NAME");
        assert_eq!(
            OtlpStdoutSpanExporter::get_service_name(),
            "unknown-service"
        );
    }

    #[test]
    fn test_compression_level_from_env() {
        // Test with valid compression level
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "3");
        assert_eq!(
            OtlpStdoutSpanExporter::get_compression_level_from_env(),
            Some(3)
        );

        // Test with invalid compression level (>9)
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "10");
        assert_eq!(
            OtlpStdoutSpanExporter::get_compression_level_from_env(),
            None
        );

        // Test with non-numeric value
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "invalid");
        assert_eq!(
            OtlpStdoutSpanExporter::get_compression_level_from_env(),
            None
        );

        // Test with unset variable
        std::env::remove_var(COMPRESSION_LEVEL_ENV_VAR);
        assert_eq!(
            OtlpStdoutSpanExporter::get_compression_level_from_env(),
            None
        );
    }

    #[test]
    fn test_new_uses_env_compression_level() {
        // Set environment variable
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "3");
        let exporter = OtlpStdoutSpanExporter::new();
        assert_eq!(exporter.gzip_level, 3);

        // Test with unset variable (should use default)
        std::env::remove_var(COMPRESSION_LEVEL_ENV_VAR);
        let exporter = OtlpStdoutSpanExporter::new();
        assert_eq!(exporter.gzip_level, DEFAULT_COMPRESSION_LEVEL);
    }

    #[test]
    fn test_with_gzip_level_overrides_env() {
        // Set environment variable
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "3");

        // Explicit level should override environment
        let exporter = OtlpStdoutSpanExporter::with_gzip_level(8);
        assert_eq!(exporter.gzip_level, 8);

        // Clean up
        std::env::remove_var(COMPRESSION_LEVEL_ENV_VAR);
    }

    #[tokio::test]
    async fn test_compression_level_affects_output_size() {
        // Create a large span batch to make compression differences more noticeable
        let mut spans = Vec::new();
        for i in 0..100 {
            let mut span = create_test_span();
            // Add unique attributes to each span to increase data size
            span.attributes.push(KeyValue::new("index", i));
            span.attributes.push(KeyValue::new("data", "a".repeat(100)));
            spans.push(span);
        }

        // Test with no compression (level 0)
        let (mut no_compression_exporter, no_compression_output) =
            OtlpStdoutSpanExporter::with_test_output();
        no_compression_exporter.gzip_level = 0;
        let _ = no_compression_exporter.export(spans.clone()).await;
        let no_compression_size = extract_payload_size(&no_compression_output.get_output()[0]);

        // Test with medium compression (level 5)
        let (mut medium_compression_exporter, medium_compression_output) =
            OtlpStdoutSpanExporter::with_test_output();
        medium_compression_exporter.gzip_level = 5;
        let _ = medium_compression_exporter.export(spans.clone()).await;
        let medium_compression_size =
            extract_payload_size(&medium_compression_output.get_output()[0]);

        // Test with maximum compression (level 9)
        let (mut max_compression_exporter, max_compression_output) =
            OtlpStdoutSpanExporter::with_test_output();
        max_compression_exporter.gzip_level = 9;
        let _ = max_compression_exporter.export(spans.clone()).await;
        let max_compression_size = extract_payload_size(&max_compression_output.get_output()[0]);

        // Verify that higher compression levels result in smaller payloads
        assert!(no_compression_size > medium_compression_size,
            "Medium compression (level 5) should produce smaller output than no compression (level 0). Got {} vs {}",
            medium_compression_size, no_compression_size);

        assert!(medium_compression_size >= max_compression_size,
            "Maximum compression (level 9) should produce output no larger than medium compression (level 5). Got {} vs {}",
            max_compression_size, medium_compression_size);

        // Verify that all outputs can be properly decoded and contain the same data
        let no_compression_spans = decode_and_count_spans(&no_compression_output.get_output()[0]);
        let medium_compression_spans =
            decode_and_count_spans(&medium_compression_output.get_output()[0]);
        let max_compression_spans = decode_and_count_spans(&max_compression_output.get_output()[0]);

        assert_eq!(
            no_compression_spans,
            spans.len(),
            "No compression output should contain all spans"
        );
        assert_eq!(
            medium_compression_spans,
            spans.len(),
            "Medium compression output should contain all spans"
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
        let mut exporter = OtlpStdoutSpanExporter::new();
        let result = exporter.export(vec![]).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_gzip_level_configuration() {
        let exporter = OtlpStdoutSpanExporter::with_gzip_level(9);
        assert_eq!(exporter.gzip_level, 9);
    }

    #[tokio::test]
    async fn test_env_var_affects_export_compression() {
        // Create test data
        let span = create_test_span();
        let spans = vec![span];

        // Test with environment variable set to level 0 (no compression)
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "0");
        let (mut env_exporter_0, env_output_0) = OtlpStdoutSpanExporter::with_test_output();
        let _ = env_exporter_0.export(spans.clone()).await;
        let env_size_0 = extract_payload_size(&env_output_0.get_output()[0]);

        // Test with environment variable set to level 9 (max compression)
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "9");
        let (mut env_exporter_9, env_output_9) = OtlpStdoutSpanExporter::with_test_output();
        let _ = env_exporter_9.export(spans.clone()).await;
        let env_size_9 = extract_payload_size(&env_output_9.get_output()[0]);

        // Verify that the environment variable affected the compression level
        assert!(env_size_0 > env_size_9,
            "Environment variable COMPRESSION_LEVEL=9 should produce smaller output than COMPRESSION_LEVEL=0. Got {} vs {}",
            env_size_9, env_size_0);

        // Test with invalid environment variable (should use default)
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "invalid");
        let (mut env_exporter_invalid, _env_output_invalid) =
            OtlpStdoutSpanExporter::with_test_output();
        let _ = env_exporter_invalid.export(spans.clone()).await;

        // Test with explicit level (should override environment variable)
        std::env::set_var(COMPRESSION_LEVEL_ENV_VAR, "0");
        let (mut explicit_exporter, explicit_output) = OtlpStdoutSpanExporter::with_test_output();
        explicit_exporter.gzip_level = 9;
        let _ = explicit_exporter.export(spans.clone()).await;
        let explicit_size = extract_payload_size(&explicit_output.get_output()[0]);

        // Verify that explicit level overrides environment variable
        assert!(env_size_0 > explicit_size,
            "Explicit level 9 should produce smaller output than environment variable level 0. Got {} vs {}",
            explicit_size, env_size_0);

        // Clean up
        std::env::remove_var(COMPRESSION_LEVEL_ENV_VAR);
    }
}
