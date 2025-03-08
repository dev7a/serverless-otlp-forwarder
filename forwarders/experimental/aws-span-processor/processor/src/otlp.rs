use opentelemetry_proto::tonic::{
    common::v1::{any_value, AnyValue, ArrayValue, KeyValue},
    resource::v1::Resource,
    trace::v1::{
        span::{Event, Link, SpanKind},
        ResourceSpans, ScopeSpans, Span, Status,
        status::StatusCode,
    },
    collector::trace::v1::ExportTraceServiceRequest,
};
use prost::Message;
use serde_json::{Map, Value};
use anyhow::{Context, Result};

/// Maps a string status code to the OTLP StatusCode enum
fn map_status_code(code: &str) -> StatusCode {
    match code.to_uppercase().as_str() {
        "OK" => StatusCode::Ok,
        "ERROR" => StatusCode::Error,
        _ => StatusCode::Unset,
    }
}

/// Maps a string span kind to the OTLP SpanKind enum
fn map_span_kind(kind: &str) -> SpanKind {
    match kind.to_uppercase().as_str() {
        "INTERNAL" => SpanKind::Internal,
        "SERVER" => SpanKind::Server,
        "CLIENT" => SpanKind::Client,
        "PRODUCER" => SpanKind::Producer,
        "CONSUMER" => SpanKind::Consumer,
        _ => SpanKind::Unspecified,
    }
}

/// Converts a JSON value to an OTLP AnyValue
fn convert_value(value: &Value) -> AnyValue {
    match value {
        Value::Bool(b) => AnyValue {
            value: Some(any_value::Value::BoolValue(*b)),
        },
        Value::Number(n) => {
            if n.is_i64() {
                AnyValue {
                    value: Some(any_value::Value::IntValue(n.as_i64().unwrap())),
                }
            } else {
                AnyValue {
                    value: Some(any_value::Value::DoubleValue(n.as_f64().unwrap())),
                }
            }
        }
        Value::Array(arr) => {
            let values = arr.iter().map(convert_value).collect();
            AnyValue {
                value: Some(any_value::Value::ArrayValue(ArrayValue { values })),
            }
        },
        Value::String(s) => AnyValue {
            value: Some(any_value::Value::StringValue(s.to_string())),
        },
        _ => AnyValue {
            value: Some(any_value::Value::StringValue(value.to_string())),
        },
    }
}

/// Converts a JSON map to a vector of OTLP KeyValue pairs
fn convert_attributes(attrs: &Map<String, Value>) -> Vec<KeyValue> {
    attrs
        .iter()
        .map(|(k, v)| KeyValue {
            key: k.clone(),
            value: Some(convert_value(v)),
        })
        .collect()
}

/// Converts a raw JSON span to an OTLP ExportTraceServiceRequest
pub fn convert_span_to_otlp_protobuf(record: Value) -> Result<Vec<u8>> {
    let record = record.as_object().context("Record is not an object")?;
    let empty_map = Map::new();

    // Skip spans with unset endTimeUnixNano
    let end_time = record
        .get("endTimeUnixNano")
        .and_then(|v| if v.is_null() { None } else { v.as_u64() })
        .context("Missing or invalid endTimeUnixNano")?;

    // Convert resource attributes
    let resource_attrs = record
        .get("resource")
        .and_then(|r| r.get("attributes"))
        .and_then(Value::as_object)
        .unwrap_or(&empty_map)
        .clone();

    let resource_attributes = convert_attributes(&resource_attrs);

    // Get scope information
    let scope = record
        .get("scope")
        .and_then(Value::as_object)
        .unwrap_or(&empty_map)
        .clone();
    let scope_name = scope
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let scope_version = scope
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Basic span fields
    let trace_id = record
        .get("traceId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .as_bytes()
        .to_vec();
    let span_id = record
        .get("spanId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .as_bytes()
        .to_vec();
    let parent_span_id = record
        .get("parentSpanId")
        .and_then(Value::as_str)
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_default();

    let kind = record
        .get("kind")
        .and_then(Value::as_str)
        .map(map_span_kind)
        .unwrap_or(SpanKind::Unspecified)
        .into();

    let start_time = record
        .get("startTimeUnixNano")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    // Convert span attributes
    let span_attrs = record
        .get("attributes")
        .and_then(Value::as_object)
        .unwrap_or(&empty_map)
        .clone();
    let attributes = convert_attributes(&span_attrs);

    // Status
    let status_code = record
        .get("status")
        .and_then(|s| s.get("code"))
        .and_then(Value::as_str)
        .map(map_status_code)
        .unwrap_or(StatusCode::Unset)
        .into();

    // Create the OTLP span
    let span = Span {
        trace_id,
        span_id,
        parent_span_id,
        name: record
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("UnnamedSpan")
            .to_string(),
        kind,
        start_time_unix_nano: start_time,
        end_time_unix_nano: end_time,
        attributes,
        status: Some(Status {
            code: status_code,
            message: String::new(),
        }),
        events: Vec::<Event>::new(),
        links: Vec::<Link>::new(),
        dropped_attributes_count: 0,
        dropped_events_count: 0,
        dropped_links_count: 0,
        ..Default::default()
    };

    // Create the OTLP request structure
    let request = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(Resource {
                attributes: resource_attributes,
                dropped_attributes_count: 0,
            }),
            scope_spans: vec![ScopeSpans {
                scope: Some(opentelemetry_proto::tonic::common::v1::InstrumentationScope {
                    name: scope_name,
                    version: scope_version,
                    attributes: Vec::new(),
                    dropped_attributes_count: 0,
                }),
                spans: vec![span],
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    };

    // Serialize to protobuf binary format
    let bytes = request.encode_to_vec();
    Ok(bytes)
}

/// Legacy function for backward compatibility
/// Converts a raw JSON span to an OTLP JSON format
pub fn convert_span_to_otlp(record: Value) -> Option<Value> {
    match convert_span_to_otlp_protobuf(record.clone()) {
        Ok(_) => {
            // If protobuf conversion works, we still need to return a JSON value
            // for backward compatibility
            let record = record.as_object().unwrap();
            let empty_map = Map::new();

            // Skip spans with unset endTimeUnixNano
            let end_time =
                record
                    .get("endTimeUnixNano")
                    .and_then(|v| if v.is_null() { None } else { v.as_u64() })?;

            // The rest of the original function...
            // This is kept for backward compatibility
            
            // Convert resource attributes
            let resource_attrs = record
                .get("resource")
                .and_then(|r| r.get("attributes"))
                .and_then(Value::as_object)
                .unwrap_or(&empty_map)
                .clone();

            let resource_attributes = convert_attributes(&resource_attrs);

            // Get scope information
            let scope = record
                .get("scope")
                .and_then(Value::as_object)
                .unwrap_or(&empty_map)
                .clone();
            let scope_name = scope
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let scope_version = scope
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            // Basic span fields
            let trace_id = record
                .get("traceId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let span_id = record
                .get("spanId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let parent_span_id = record
                .get("parentSpanId")
                .and_then(Value::as_str)
                .map(String::from);

            let kind = record
                .get("kind")
                .and_then(Value::as_str)
                .map(|k| map_span_kind(k) as i32)
                .unwrap_or(0);

            let start_time = record
                .get("startTimeUnixNano")
                .and_then(Value::as_u64)
                .unwrap_or(0);

            // Convert span attributes
            let span_attrs = record
                .get("attributes")
                .and_then(Value::as_object)
                .unwrap_or(&empty_map)
                .clone();
            let attributes = convert_attributes(&span_attrs);

            // Status
            let status_code = record
                .get("status")
                .and_then(|s| s.get("code"))
                .and_then(Value::as_str)
                .map(|s| map_status_code(s) as i32)
                .unwrap_or(0);

            // Create JSON structure
            let otlp = serde_json::json!({
                "resourceSpans": [{
                    "resource": {
                        "attributes": resource_attributes
                    },
                    "scopeSpans": [{
                        "scope": {
                            "name": scope_name,
                            "version": scope_version
                        },
                        "spans": [{
                            "traceId": trace_id,
                            "spanId": span_id,
                            "parentSpanId": parent_span_id,
                            "name": record.get("name").and_then(Value::as_str).unwrap_or("UnnamedSpan"),
                            "kind": kind,
                            "startTimeUnixNano": start_time,
                            "endTimeUnixNano": end_time,
                            "attributes": attributes,
                            "status": {
                                "code": status_code
                            },
                            "events": [],
                            "links": [],
                            "droppedAttributesCount": 0,
                            "droppedEventsCount": 0,
                            "droppedLinksCount": 0
                        }]
                    }]
                }]
            });

            Some(otlp)
        },
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_convert_span_to_otlp_protobuf() {
        let span = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            "parentSpanId": "fedcba9876543210",
            "kind": "SERVER",
            "startTimeUnixNano": 1619712000000000000_u64,
            "endTimeUnixNano": 1619712001000000000_u64,
            "attributes": {
                "service.name": "test-service",
                "http.method": "GET",
                "http.url": "https://example.com",
                "http.status_code": 200
            },
            "status": {
                "code": "OK"
            },
            "resource": {
                "attributes": {
                    "service.name": "test-service",
                    "service.version": "1.0.0"
                }
            },
            "scope": {
                "name": "test-scope",
                "version": "1.0.0"
            }
        });

        let result = convert_span_to_otlp_protobuf(span);
        assert!(result.is_ok());
        
        // Verify we can decode it back
        let bytes = result.unwrap();
        let decoded = ExportTraceServiceRequest::decode(bytes.as_slice());
        assert!(decoded.is_ok());
        
        let request = decoded.unwrap();
        assert_eq!(request.resource_spans.len(), 1);
        assert_eq!(request.resource_spans[0].scope_spans.len(), 1);
        assert_eq!(request.resource_spans[0].scope_spans[0].spans.len(), 1);
        
        let span = &request.resource_spans[0].scope_spans[0].spans[0];
        assert_eq!(span.name, "test-span");
        assert_eq!(span.kind, SpanKind::Server as i32);
    }

    #[test]
    fn test_convert_span_to_otlp_protobuf_missing_endtime() {
        let span = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            // endTimeUnixNano is missing
        });

        let result = convert_span_to_otlp_protobuf(span);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_span_to_otlp_protobuf_null_endtime() {
        let span = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            "endTimeUnixNano": null
        });

        let result = convert_span_to_otlp_protobuf(span);
        assert!(result.is_err());
    }
}
