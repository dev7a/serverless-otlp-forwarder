use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use otlp_stdout_span_exporter::ExporterOutput;
use prost::Message;
use reqwest::header::{HeaderMap, CONTENT_ENCODING, CONTENT_TYPE};
use reqwest::Client as ReqwestClient;
use std::io::{Read, Write};

/// Represents a processed OTLP payload ready for potential compaction or sending.
#[derive(Clone, Debug)]
pub struct TelemetryData {
    pub payload: Vec<u8>,
    pub original_endpoint: String,
    pub original_source: String,
}

/// Configuration for span compaction (simplified for CLI)
#[derive(Debug, Clone)]
pub struct SpanCompactionConfig {
    pub compression_level: u32,
}

impl Default for SpanCompactionConfig {
    fn default() -> Self {
        Self {
            compression_level: 6,
        }
    }
}

/// Processes a single CloudWatch Live Tail log event message string.
pub fn process_log_event_message(message: &str) -> Result<Option<TelemetryData>> {
    tracing::trace!(message, "Processing log event message");
    let record: ExporterOutput = match serde_json::from_str::<ExporterOutput>(message) {
        Ok(output) => {
            if output.version.is_empty() || output.payload.is_empty() {
                tracing::debug!(message, "Log message parsed but missing expected fields, skipping.");
                return Ok(None);
            }
            output
        }
        Err(e) => {
            tracing::trace!(message, error = %e, "Failed to parse log message as ExporterOutput JSON, skipping.");
            return Ok(None);
        }
    };

    tracing::debug!(source = %record.source, endpoint = %record.endpoint, "Parsed OTLP/stdout record");

    let raw_payload = if record.base64 {
        general_purpose::STANDARD
            .decode(&record.payload)
            .context("Failed to decode base64 payload")?
    } else {
        tracing::warn!("Received non-base64 payload, attempting to process as raw bytes.");
        record.payload.as_bytes().to_vec()
    };

    let protobuf_payload = convert_to_protobuf(
        raw_payload,
        &record.content_type,
        Some(&record.content_encoding),
    )
    .context("Failed to convert payload to protobuf")?;

    Ok(Some(TelemetryData {
        payload: protobuf_payload,
        original_endpoint: record.endpoint.to_string(),
        original_source: record.source,
    }))
}

fn convert_to_protobuf(
    payload: Vec<u8>,
    content_type: &str,
    content_encoding: Option<&str>,
) -> Result<Vec<u8>> {
    tracing::trace!(
        content_type,
        content_encoding = ?content_encoding,
        input_size = payload.len(),
        "Converting payload to uncompressed protobuf"
    );

    let decompressed = if content_encoding == Some("gzip") {
        tracing::trace!("Decompressing gzipped payload");
        let mut decoder = GzDecoder::new(&payload[..]);
        let mut decompressed_data = Vec::new();
        decoder
            .read_to_end(&mut decompressed_data)
            .context("Failed to decompress Gzip payload")?;
        tracing::trace!(output_size = decompressed_data.len(), "Decompressed payload");
        decompressed_data
    } else {
        payload
    };

    match content_type {
        "application/x-protobuf" => {
            tracing::trace!("Payload is already protobuf");
            match ExportTraceServiceRequest::decode(decompressed.as_slice()) {
                Ok(_) => Ok(decompressed),
                Err(e) => Err(anyhow!("Payload has content-type protobuf but failed to decode: {}", e)),
            }
        }
        "application/json" => {
            tracing::trace!("Converting JSON payload to protobuf");
            let request: ExportTraceServiceRequest = serde_json::from_slice(&decompressed)
                .context("Failed to parse JSON as ExportTraceServiceRequest")?;
            let protobuf_bytes = request.encode_to_vec();
            tracing::trace!(output_size = protobuf_bytes.len(), "Converted JSON to protobuf");
            Ok(protobuf_bytes)
        }
        _ => {
            tracing::warn!(content_type, "Unsupported content type encountered, attempting to treat as protobuf.");
            match ExportTraceServiceRequest::decode(decompressed.as_slice()) {
                Ok(_) => Ok(decompressed),
                Err(e) => Err(anyhow!("Payload has unknown content-type '{}' and failed to decode as protobuf: {}", content_type, e)),
            }
        }
    }
}

pub fn compact_telemetry_payloads(
    batch: Vec<TelemetryData>,
    config: &SpanCompactionConfig,
) -> Result<TelemetryData> {
    if batch.is_empty() {
        return Err(anyhow!("Cannot compact an empty batch"));
    }
    if batch.len() == 1 {
        tracing::debug!("Batch has only one item, skipping merge, applying compression.");
        let mut single_item = batch.into_iter().next().unwrap();
        let compressed_payload = compress_payload(&single_item.payload, config.compression_level)
            .context("Failed to compress single payload")?;
        single_item.payload = compressed_payload;
        return Ok(single_item);
    }

    let original_count = batch.len();
    tracing::info!("Compacting {} telemetry payloads...", original_count);

    let mut decoded_requests = Vec::with_capacity(batch.len());
    for telemetry in &batch {
        match decode_otlp_payload(&telemetry.payload) {
            Ok(request) => decoded_requests.push(request),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to decode payload during compaction, skipping item.");
            }
        }
    }

    if decoded_requests.is_empty() {
        return Err(anyhow!("All payloads in the batch failed to decode"));
    }

    let mut merged_resource_spans = Vec::new();
    for request in decoded_requests {
        merged_resource_spans.extend(request.resource_spans);
    }

    let merged_request = ExportTraceServiceRequest {
        resource_spans: merged_resource_spans,
    };

    let uncompressed_payload = encode_otlp_payload(&merged_request);
    tracing::debug!(
        uncompressed_size = uncompressed_payload.len(),
        "Encoded compacted payload"
    );

    let compressed_payload = compress_payload(&uncompressed_payload, config.compression_level)
        .context("Failed to compress compacted payload")?;
    tracing::info!(
        compressed_size = compressed_payload.len(),
        "Compressed compacted payload"
    );

    let first_telemetry = &batch[0];

    Ok(TelemetryData {
        payload: compressed_payload,
        original_endpoint: first_telemetry.original_endpoint.clone(),
        original_source: first_telemetry.original_source.clone(),
    })
}

fn decode_otlp_payload(payload: &[u8]) -> Result<ExportTraceServiceRequest> {
    ExportTraceServiceRequest::decode(payload).context("Failed to decode OTLP protobuf payload")
}

fn encode_otlp_payload(request: &ExportTraceServiceRequest) -> Vec<u8> {
    request.encode_to_vec()
}

pub fn compress_payload(payload: &[u8], level: u32) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(level));
    encoder
        .write_all(payload)
        .context("Failed to write to compressor")?;
    encoder.finish().context("Failed to finish compression")
}

#[tracing::instrument(skip_all, fields(
    http.method = "POST",
    http.url = %endpoint,
    payload_size = payload.len(),
    otel.status_code,
    http.status_code,
    error,
))]
pub async fn send_telemetry_payload(
    client: &ReqwestClient,
    endpoint: &str,
    payload: Vec<u8>,
    mut headers: HeaderMap,
) -> Result<()> {
    let current_span = tracing::Span::current();

    headers.insert(CONTENT_TYPE, "application/x-protobuf".try_into()?);
    headers.insert(CONTENT_ENCODING, "gzip".try_into()?);

    tracing::debug!(?headers, "Sending OTLP request");

    let response = match client
        .post(endpoint)
        .headers(headers)
        .body(payload)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            current_span.record("otel.status_code", "ERROR");
            current_span.record("error", true);
            tracing::error!(error = %e, "Failed to send OTLP request");
            return Err(anyhow!("Failed to send OTLP request").context(e));
        }
    };

    let status = response.status();
    current_span.record("http.status_code", status.as_u16());

    if !status.is_success() {
        current_span.record("otel.status_code", "ERROR");
        let error_body = match response.text().await {
            Ok(text) if !text.is_empty() => text,
            _ => format!("Status: {}", status),
        };
        tracing::error!(status = %status, body = %error_body, "OTLP endpoint returned error");
        return Err(anyhow!(
            "OTLP endpoint returned error: {}",
            error_body
        ));
    }

    tracing::debug!(status = %status, "Successfully sent OTLP data");
    current_span.record("otel.status_code", "OK");
    Ok(())
}
