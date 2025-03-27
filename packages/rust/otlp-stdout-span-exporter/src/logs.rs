use std::{env, io::Write, sync::Arc};

use bon::bon;
use flate2::{write::GzEncoder, Compression};
use opentelemetry_proto::{
    tonic::collector::logs::v1::ExportLogsServiceRequest,
    transform::{
        common::tonic::ResourceAttributesWithSchema, logs::tonic::group_logs_by_resource_and_scope,
    },
};
use opentelemetry_sdk::{
    error::OTelSdkError,
    logs::{LogBatch, LogExporter},
    Resource,
};

use base64::{engine::general_purpose::STANDARD as base64_engine, Engine};
use prost::Message;

use crate::{
    consts::{defaults, env_vars},
    parse_headers,
    utils::get_service_name,
    ExporterOutput, Output, StdOutput, VERSION,
};

/// A log exporter that writes logs to stdout in OTLP format
///
/// This exporter implements the OpenTelemetry [`LogExporter`] trait and writes logs
/// to stdout in OTLP format with Protobuf serialization and GZIP compression.
///
/// # Features
///
/// - Configurable GZIP compression level (0-9)
/// - Environment variable support for service name and headers
/// - Efficient batching of logs
/// - Base64 encoding of compressed data
///
/// # Example
///
/// ```rust,no_run
/// use opentelemetry_sdk::runtime;
/// use otlp_stdout_span_exporter::OtlpStdoutLogExporter;
///
/// // Create an exporter with maximum compression
/// let exporter = OtlpStdoutLogExporter::builder()
///     .compression_level(9)
///     .build();
/// ```
#[derive(Debug)]
pub struct OtlpStdoutLogExporter {
    /// GZIP compression level (0-9)
    compression_level: u8,
    /// Output implementation (stdout or test buffer)
    output: Arc<dyn Output>,
    /// Optional resource to be included with all logs
    resource: Option<Resource>,
}

impl Default for OtlpStdoutLogExporter {
    fn default() -> Self {
        Self::builder().build()
    }
}
#[bon]
impl OtlpStdoutLogExporter {
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
}

impl LogExporter for OtlpStdoutLogExporter {
    /// Export logs to stdout in OTLP format
    ///
    /// This function:
    /// 1. Converts logs to OTLP format
    /// 2. Serializes them to protobuf
    /// 3. Compresses the data with GZIP
    /// 4. Base64 encodes the result
    /// 5. Writes a JSON object to stdout
    ///
    /// # Arguments
    ///
    /// * `batch` - A vector of logs to export
    ///
    /// # Returns
    ///
    /// Returns a resolved future with `Ok(())` if the export was successful, or a `TraceError` if it failed
    fn export(
        &self,
        batch: LogBatch<'_>,
    ) -> impl std::future::Future<Output = Result<(), OTelSdkError>> + Send {
        // Do all work synchronously
        let result = (|| {
            // Convert spans to OTLP format
            let resource = self
                .resource
                .clone()
                .unwrap_or_else(|| opentelemetry_sdk::Resource::builder_empty().build());
            let resource_attrs = ResourceAttributesWithSchema::from(&resource);
            let resource_logs = group_logs_by_resource_and_scope(batch, &resource_attrs);
            let request = ExportLogsServiceRequest { resource_logs };

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
                source: get_service_name(),
                endpoint: defaults::LOGS_ENDPOINT,
                method: "POST",
                content_type: "application/x-protobuf",
                content_encoding: "gzip",
                headers: parse_headers(),
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
}
