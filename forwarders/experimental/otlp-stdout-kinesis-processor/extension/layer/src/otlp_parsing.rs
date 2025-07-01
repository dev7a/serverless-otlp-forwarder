use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine};
use flate2::read::GzDecoder;
use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use prost::Message;
use serde::Deserialize;
use std::io::Read;

// OTLP Span Flags constants for remote parent check
const SPAN_FLAGS_CONTEXT_HAS_IS_REMOTE_MASK: u32 = 0x100;
const SPAN_FLAGS_CONTEXT_IS_REMOTE_MASK: u32 = 0x200;

// A simplified struct matching the relevant fields of otlp_stdout_span_exporter::ExporterOutput
#[derive(Deserialize, Debug)]
struct OtlpStdoutJsonLine {
    payload: String,
    #[serde(default)]
    base64: bool,
    #[serde(rename = "content-encoding", default)]
    content_encoding: String,
    #[serde(rename = "content-type", default)]
    content_type: String,
    // Other fields like source, endpoint, version are ignored for now
}

/// Parses an OTLP/stdout JSON line, decodes/decompresses the payload,
/// and extracts the TraceId and SpanId of the function's entry span.
/// This is either the span with no parent_span_id or the first span indicating a remote parent.
///
/// Returns `Ok(None)` if the line isn't valid JSON, the payload is empty,
/// or no suitable entry span is found. Returns `Err` for decoding/decompression issues.
pub fn extract_trace_info_from_json_line(line: &str) -> Result<Option<(TraceId, SpanId)>> {
    let parsed_line: OtlpStdoutJsonLine = match serde_json::from_str(line) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    if parsed_line.payload.is_empty() {
        return Ok(None);
    }
    let raw_payload = if parsed_line.base64 {
        general_purpose::STANDARD
            .decode(&parsed_line.payload)
            .context("Failed to decode base64 payload")?
    } else {
        parsed_line.payload.into_bytes()
    };
    let decompressed_payload = if parsed_line.content_encoding == "gzip" {
        let mut decoder = GzDecoder::new(&raw_payload[..]);
        let mut decompressed_data = Vec::new();
        decoder
            .read_to_end(&mut decompressed_data)
            .context("Failed to decompress Gzip payload")?;
        decompressed_data
    } else {
        raw_payload
    };
    let trace_request = if parsed_line.content_type == "application/x-protobuf" {
        ExportTraceServiceRequest::decode(decompressed_payload.as_slice())
            .context("Failed to decode OTLP protobuf payload")?
    } else {
        return Ok(None);
    };

    for resource_span in trace_request.resource_spans {
        for scope_span in resource_span.scope_spans {
            for span in scope_span.spans {
                let mut is_entry_span = false;
                let mut reason = "";

                // Check 1: Is it a root span (no parent)?
                if span.parent_span_id.is_empty() {
                    is_entry_span = true;
                    reason = "no parent_id";
                }
                // Check 2: Does it have a remote parent?
                else if (span.flags & SPAN_FLAGS_CONTEXT_HAS_IS_REMOTE_MASK) != 0
                    && (span.flags & SPAN_FLAGS_CONTEXT_IS_REMOTE_MASK) != 0
                {
                    is_entry_span = true;
                    reason = "remote parent flag set";
                }

                if is_entry_span {
                    // Attempt to extract IDs
                    let trace_id_bytes_res = span.trace_id.as_slice().try_into();
                    let span_id_bytes_res = span.span_id.as_slice().try_into();

                    match (trace_id_bytes_res, span_id_bytes_res) {
                        (Ok(trace_id_bytes), Ok(span_id_bytes)) => {
                            let trace_id = TraceId::from_bytes(trace_id_bytes);
                            let span_id = SpanId::from_bytes(span_id_bytes);

                            // Check for invalid IDs
                            if trace_id != TraceId::INVALID && span_id != SpanId::INVALID {
                                tracing::debug!(%trace_id, %span_id, %reason, "Extracted trace info from function entry span");
                                return Ok(Some((trace_id, span_id))); // Return the first qualifying span
                            } else {
                                tracing::warn!(%reason, "Found potential entry span with invalid trace_id or span_id, continuing search.");
                                // Continue searching in case of invalid IDs
                            }
                        }
                        _ => {
                            tracing::warn!(%reason, "Found potential entry span with invalid trace_id or span_id length, continuing search.");
                            // Continue searching if ID conversion fails
                        }
                    }
                }
            }
        }
    }

    tracing::debug!("No suitable entry span found in the OTLP payload.");
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*; // Import function to test
    use base64::engine::general_purpose::STANDARD as base64_engine;
    use flate2::{write::GzEncoder, Compression};
    use opentelemetry_proto::tonic::{
        // Import OTLP types for creating test data
        resource::v1::Resource,
        trace::v1::{span::SpanKind, status::StatusCode, ResourceSpans, ScopeSpans, Span, Status},
    };
    use prost::Message;
    use std::io::Write;

    // Helper function to create a basic Span proto message
    fn create_proto_span(
        trace_id: &[u8],
        span_id: &[u8],
        parent_span_id: Option<&[u8]>,
        name: &str,
        flags: Option<u32>,
    ) -> Span {
        Span {
            trace_id: trace_id.to_vec(),
            span_id: span_id.to_vec(),
            parent_span_id: parent_span_id.map_or(vec![], |id| id.to_vec()),
            name: name.to_string(),
            kind: SpanKind::Server as i32,
            start_time_unix_nano: 1704067200000000000, // 2024-01-01T00:00:00Z
            end_time_unix_nano: 1704067201000000000,   // +1 second
            status: Some(Status {
                code: StatusCode::Ok as i32,
                ..Default::default()
            }),
            flags: flags.unwrap_or(0), // Default to 0 if not specified
            ..Default::default()
        }
    }

    // Helper function to create a test ExportTraceServiceRequest
    fn create_test_request(spans: Vec<Span>) -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource::default()),
                scope_spans: vec![ScopeSpans {
                    scope: None,
                    spans,
                    schema_url: String::new(),
                }],
                schema_url: String::new(),
            }],
        }
    }

    // Helper function to create the final JSON line
    fn create_test_json_line(request: ExportTraceServiceRequest) -> String {
        let proto_bytes = request.encode_to_vec();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&proto_bytes).unwrap();
        let compressed_bytes = encoder.finish().unwrap();
        let payload_base64 = base64_engine.encode(compressed_bytes);

        let json_data = serde_json::json!({
            "payload": payload_base64,
            "base64": true,
            "content-encoding": "gzip",
            "content-type": "application/x-protobuf"
        });
        serde_json::to_string(&json_data).unwrap()
    }

    // --- Test Cases ---

    #[test]
    fn test_valid_root_span() {
        let trace_id_hex = "0102030405060708090a0b0c0d0e0f10";
        let span_id_hex = "1112131415161718";
        let trace_id_bytes = TraceId::from_hex(trace_id_hex).unwrap().to_bytes();
        let span_id_bytes = SpanId::from_hex(span_id_hex).unwrap().to_bytes();
        let span = create_proto_span(&trace_id_bytes, &span_id_bytes, None, "root_span", None);
        let request = create_test_request(vec![span]);
        let json_line = create_test_json_line(request);

        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_ok());
        let trace_info = result.unwrap();
        assert!(trace_info.is_some());
        let (tid, sid) = trace_info.unwrap();
        assert_eq!(format!("{:032x}", tid), trace_id_hex);
        assert_eq!(format!("{:016x}", sid), span_id_hex);
    }

    #[test]
    fn test_valid_remote_parent_span() {
        let trace_id_hex = "aabbccddeeff00112233445566778899";
        let span_id_hex = "aabbccddeeff0011";
        let parent_id_hex = "1122334455667788"; // Parent exists
        let trace_id_bytes = TraceId::from_hex(trace_id_hex).unwrap().to_bytes();
        let span_id_bytes = SpanId::from_hex(span_id_hex).unwrap().to_bytes();
        let parent_id_bytes = SpanId::from_hex(parent_id_hex).unwrap().to_bytes();

        // Set the remote parent flags
        let flags = SPAN_FLAGS_CONTEXT_HAS_IS_REMOTE_MASK | SPAN_FLAGS_CONTEXT_IS_REMOTE_MASK;

        let span = create_proto_span(
            &trace_id_bytes,
            &span_id_bytes,
            Some(&parent_id_bytes),
            "remote_parent_entry",
            Some(flags),
        );
        let request = create_test_request(vec![span]);
        let json_line = create_test_json_line(request);

        let result = extract_trace_info_from_json_line(&json_line);
        assert!(
            result.is_ok(),
            "Expected Ok result, got Err: {:?}",
            result.err()
        );
        let trace_info = result.unwrap();
        assert!(trace_info.is_some(), "Expected Some trace info, got None");
        let (tid, sid) = trace_info.unwrap();
        assert_eq!(format!("{:032x}", tid), trace_id_hex);
        assert_eq!(format!("{:016x}", sid), span_id_hex);
    }

    #[test]
    fn test_no_entry_span_found() {
        let trace_id_bytes = TraceId::from_hex("cccccccccccccccccccccccccccccccc")
            .unwrap()
            .to_bytes();
        let span1_id_bytes = SpanId::from_hex("aaaaaaaaaaaaaaaa").unwrap().to_bytes();
        let span2_id_bytes = SpanId::from_hex("bbbbbbbbbbbbbbbb").unwrap().to_bytes();
        let parent_id_bytes = SpanId::from_hex("1111111111111111").unwrap().to_bytes();

        // Span 1: Has parent, no remote flags
        let span1 = create_proto_span(
            &trace_id_bytes,
            &span1_id_bytes,
            Some(&parent_id_bytes),
            "internal_span_1",
            None, // No flags
        );
        // Span 2: Has parent, no remote flags
        let span2 = create_proto_span(
            &trace_id_bytes,
            &span2_id_bytes,
            Some(&span1_id_bytes), // Parent is span1
            "internal_span_2",
            None, // No flags
        );

        let request = create_test_request(vec![span1, span2]);
        let json_line = create_test_json_line(request);

        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_ok());
        let trace_info = result.unwrap();
        assert!(
            trace_info.is_none(),
            "Expected None when no root or remote parent span exists"
        );
    }

    #[test]
    fn test_payload_with_no_spans() {
        let request = create_test_request(vec![]); // Empty spans vector
        let json_line = create_test_json_line(request);

        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_ok());
        let trace_info = result.unwrap();
        assert!(
            trace_info.is_none(),
            "Expected None when payload has no spans"
        );
    }

    #[test]
    fn test_empty_payload_string() {
        let json_data = serde_json::json!({
            "payload": "", // Empty string payload
            "base64": true,
            "content-encoding": "gzip",
            "content-type": "application/x-protobuf"
        });
        let json_line = serde_json::to_string(&json_data).unwrap();
        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "Expected None for empty payload string"
        );
    }

    #[test]
    fn test_invalid_json() {
        let json_line = "{ not valid json";
        let result = extract_trace_info_from_json_line(json_line);
        assert!(result.is_ok()); // Function handles parse error gracefully
        assert!(result.unwrap().is_none(), "Expected None for invalid JSON");
    }

    #[test]
    fn test_invalid_base64() {
        let json_data = serde_json::json!({
            "payload": "!!not base64!!",
            "base64": true,
            "content-encoding": "gzip",
            "content-type": "application/x-protobuf"
        });
        let json_line = serde_json::to_string(&json_data).unwrap();
        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_err(), "Expected Err for invalid base64");
    }

    #[test]
    fn test_invalid_gzip() {
        let payload_bytes = b"this is not gzipped data";
        let payload_base64 = base64_engine.encode(payload_bytes);
        let json_data = serde_json::json!({
            "payload": payload_base64,
            "base64": true,
            "content-encoding": "gzip", // Claim it's gzip
            "content-type": "application/x-protobuf"
        });
        let json_line = serde_json::to_string(&json_data).unwrap();
        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_err(), "Expected Err for invalid gzip");
        // Optionally check error message content
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to decompress Gzip payload"));
    }

    #[test]
    fn test_invalid_protobuf() {
        let payload_bytes = b"this is not protobuf data";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload_bytes).unwrap();
        let compressed_bytes = encoder.finish().unwrap();
        let payload_base64 = base64_engine.encode(compressed_bytes);

        let json_data = serde_json::json!({
            "payload": payload_base64,
            "base64": true,
            "content-encoding": "gzip",
            "content-type": "application/x-protobuf" // Claim it's protobuf
        });
        let json_line = serde_json::to_string(&json_data).unwrap();
        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_err(), "Expected Err for invalid protobuf");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to decode OTLP protobuf payload"));
    }

    #[test]
    fn test_invalid_ids_in_entry_span() {
        let trace_id_bytes = TraceId::INVALID.to_bytes(); // All zeros
        let span_id_bytes = SpanId::from_hex("1111111111111111").unwrap().to_bytes();

        let span = create_proto_span(
            &trace_id_bytes,
            &span_id_bytes,
            None,
            "invalid_trace_id_span",
            None,
        );
        let request = create_test_request(vec![span]);
        let json_line = create_test_json_line(request);

        let result = extract_trace_info_from_json_line(&json_line);
        assert!(result.is_ok());
        let trace_info = result.unwrap();
        // Current logic continues search and returns None if only invalid IDs found
        assert!(
            trace_info.is_none(),
            "Expected None when entry span has invalid IDs"
        );
    }
}
