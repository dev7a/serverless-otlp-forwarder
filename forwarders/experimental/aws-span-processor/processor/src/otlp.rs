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

/// Decodes a hex string to bytes
fn decode_hex(s: &str) -> Result<Vec<u8>> {
    // Handle both with and without 0x prefix
    let s = s.trim_start_matches("0x");
    
    // Remove any non-hex characters (like hyphens)
    let s: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    
    // Decode hex string to bytes
    let mut result = Vec::new();
    for i in (0..s.len()).step_by(2) {
        if i + 2 <= s.len() {
            let byte = u8::from_str_radix(&s[i..i+2], 16)
                .map_err(|e| anyhow::anyhow!("Invalid hex string: {}", e))?;
            result.push(byte);
        } else if i + 1 == s.len() {
            // Handle odd number of digits by padding with 0
            let byte = u8::from_str_radix(&format!("{}0", &s[i..i+1]), 16)
                .map_err(|e| anyhow::anyhow!("Invalid hex string: {}", e))?;
            result.push(byte);
        }
    }
    
    Ok(result)
}

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
        .map(decode_hex)
        .transpose()
        .context("Invalid traceId format")?
        .unwrap_or_default();
    
    let span_id = record
        .get("spanId")
        .and_then(Value::as_str)
        .map(decode_hex)
        .transpose()
        .context("Invalid spanId format")?
        .unwrap_or_default();
    
    let parent_span_id = record
        .get("parentSpanId")
        .and_then(Value::as_str)
        .map(decode_hex)
        .transpose()
        .context("Invalid parentSpanId format")?
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

    // Convert events
    let events = record
        .get("events")
        .and_then(Value::as_array)
        .map(|events_array| {
            events_array
                .iter()
                .filter_map(|event| {
                    if let Some(event_obj) = event.as_object() {
                        let time = event_obj
                            .get("timeUnixNano")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        
                        let name = event_obj
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        
                        let attrs = event_obj
                            .get("attributes")
                            .and_then(Value::as_object)
                            .map(convert_attributes)
                            .unwrap_or_default();
                        
                        Some(Event {
                            time_unix_nano: time,
                            name,
                            attributes: attrs,
                            dropped_attributes_count: 0,
                        })
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

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
        events,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_decode_hex() {
        // Test valid hex string
        let result = decode_hex("0123456789abcdef");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);

        // Test with 0x prefix
        let result = decode_hex("0x0123456789abcdef");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);

        // Test with hyphens
        let result = decode_hex("01-23-45-67-89-ab-cd-ef");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);

        // Test invalid hex string
        let result = decode_hex("0123456789abcdefg");
        assert!(result.is_ok()); // Now it should be ok because we filter out non-hex chars
        assert_eq!(result.unwrap(), vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    }

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
        
        // Verify IDs were properly decoded
        assert_eq!(span.trace_id, vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
        assert_eq!(span.span_id, vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
        assert_eq!(span.parent_span_id, vec![0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10]);
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

    #[test]
    fn test_convert_span_to_otlp_protobuf_complete() {
        // Create a complete test span with all fields
        let span_record = json!({
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

        let result = convert_span_to_otlp_protobuf(span_record);
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
    fn test_convert_span_to_otlp_protobuf_with_events() {
        // Create a test span with events
        let span_record = json!({
            "name": "test-span",
            "traceId": "0123456789abcdef0123456789abcdef",
            "spanId": "0123456789abcdef",
            "parentSpanId": "fedcba9876543210",
            "kind": "SERVER",
            "startTimeUnixNano": 1619712000000000000_u64,
            "endTimeUnixNano": 1619712001000000000_u64,
            "attributes": {
                "service.name": "test-service"
            },
            "status": {
                "code": "OK"
            },
            "resource": {
                "attributes": {
                    "service.name": "test-service"
                }
            },
            "scope": {
                "name": "test-scope",
                "version": "1.0.0"
            },
            "events": [
                {
                    "timeUnixNano": 1619712000500000000_u64,
                    "name": "Event 1",
                    "attributes": {
                        "event.key1": "value1",
                        "event.key2": 123
                    }
                },
                {
                    "timeUnixNano": 1619712000800000000_u64,
                    "name": "Event 2",
                    "attributes": {
                        "event.key3": "value3"
                    }
                }
            ]
        });

        let result = convert_span_to_otlp_protobuf(span_record);
        assert!(result.is_ok());
        
        // Verify we can decode it back
        let bytes = result.unwrap();
        let decoded = ExportTraceServiceRequest::decode(bytes.as_slice());
        assert!(decoded.is_ok());
        
        let request = decoded.unwrap();
        let span = &request.resource_spans[0].scope_spans[0].spans[0];
        
        // Verify events were properly converted
        assert_eq!(span.events.len(), 2);
        assert_eq!(span.events[0].name, "Event 1");
        assert_eq!(span.events[0].time_unix_nano, 1619712000500000000_u64);
        assert_eq!(span.events[1].name, "Event 2");
        assert_eq!(span.events[1].time_unix_nano, 1619712000800000000_u64);
        
        // Verify event attributes
        let event1_attrs = &span.events[0].attributes;
        assert!(!event1_attrs.is_empty());
        
        // Find the event.key1 attribute
        let key1_attr = event1_attrs.iter().find(|attr| attr.key == "event.key1");
        assert!(key1_attr.is_some());
        if let Some(attr) = key1_attr {
            if let Some(any_value::Value::StringValue(value)) = &attr.value.as_ref().unwrap().value {
                assert_eq!(value, "value1");
            } else {
                panic!("event.key1 is not a string value");
            }
        }
    }
}