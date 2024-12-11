use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Serialize)]
pub struct OtlpSpan {
    #[serde(rename = "traceId")]
    trace_id: String,
    #[serde(rename = "spanId")]
    span_id: String,
    #[serde(rename = "parentSpanId", skip_serializing_if = "Option::is_none")]
    parent_span_id: Option<String>,
    name: String,
    kind: i32,
    #[serde(rename = "startTimeUnixNano")]
    start_time_unix_nano: u64,
    #[serde(rename = "endTimeUnixNano")]
    end_time_unix_nano: u64,
    attributes: Vec<KeyValue>,
    status: Status,
    events: Vec<Value>,
    links: Vec<Value>,
    #[serde(rename = "droppedAttributesCount")]
    dropped_attributes_count: u32,
    #[serde(rename = "droppedEventsCount")]
    dropped_events_count: u32,
    #[serde(rename = "droppedLinksCount")]
    dropped_links_count: u32,
}

#[derive(Debug, Serialize)]
pub struct KeyValue {
    key: String,
    value: AnyValue,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AnyValue {
    String { string_value: String },
    Bool { bool_value: bool },
    Int { int_value: i64 },
    Double { double_value: f64 },
    Array { array_value: ArrayValue },
}

#[derive(Debug, Serialize)]
pub struct ArrayValue {
    values: Vec<AnyValue>,
}

#[derive(Debug, Serialize)]
pub struct Status {
    code: i32,
}

#[derive(Debug, Serialize)]
pub struct OtlpTrace {
    resource_spans: Vec<ResourceSpans>,
}

#[derive(Debug, Serialize)]
pub struct ResourceSpans {
    resource: Resource,
    scope_spans: Vec<ScopeSpans>,
}

#[derive(Debug, Serialize)]
pub struct Resource {
    attributes: Vec<KeyValue>,
}

#[derive(Debug, Serialize)]
pub struct ScopeSpans {
    scope: InstrumentationScope,
    spans: Vec<OtlpSpan>,
}

#[derive(Debug, Serialize)]
pub struct InstrumentationScope {
    name: String,
    version: String,
}

pub fn map_status_code(code: &str) -> i32 {
    match code.to_uppercase().as_str() {
        "OK" => 1,
        "ERROR" => 2,
        _ => 0, // UNSET
    }
}

pub fn map_span_kind(kind: &str) -> i32 {
    match kind.to_uppercase().as_str() {
        "INTERNAL" => 1,
        "SERVER" => 2,
        "CLIENT" => 3,
        "PRODUCER" => 4,
        "CONSUMER" => 5,
        _ => 0, // UNSPECIFIED
    }
}

fn convert_value(value: &Value) -> AnyValue {
    match value {
        Value::Bool(b) => AnyValue::Bool { bool_value: *b },
        Value::Number(n) => {
            if n.is_i64() {
                AnyValue::Int {
                    int_value: n.as_i64().unwrap(),
                }
            } else {
                AnyValue::Double {
                    double_value: n.as_f64().unwrap(),
                }
            }
        }
        Value::Array(arr) => AnyValue::Array {
            array_value: ArrayValue {
                values: arr.iter().map(convert_value).collect(),
            },
        },
        Value::String(s) => AnyValue::String {
            string_value: s.to_string(),
        },
        _ => AnyValue::String {
            string_value: value.to_string(),
        },
    }
}

fn convert_attributes(attrs: &Map<String, Value>) -> Vec<KeyValue> {
    attrs
        .iter()
        .map(|(k, v)| KeyValue {
            key: k.clone(),
            value: convert_value(v),
        })
        .collect()
}

pub fn convert_span_to_otlp(record: Value) -> Value {
    let record = record.as_object().unwrap();
    let empty_map = Map::new();
    
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
        .map(map_span_kind)
        .unwrap_or(0);

    let start_time = record
        .get("startTimeUnixNano")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let end_time = record
        .get("endTimeUnixNano")
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
        .unwrap_or(0);

    let span = OtlpSpan {
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
        status: Status { code: status_code },
        events: Vec::new(),
        links: Vec::new(),
        dropped_attributes_count: 0,
        dropped_events_count: 0,
        dropped_links_count: 0,
    };

    let otlp = OtlpTrace {
        resource_spans: vec![ResourceSpans {
            resource: Resource {
                attributes: resource_attributes,
            },
            scope_spans: vec![ScopeSpans {
                scope: InstrumentationScope {
                    name: scope_name,
                    version: scope_version,
                },
                spans: vec![span],
            }],
        }],
    };

    serde_json::to_value(otlp).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_convert_span_to_otlp() {
        let input = json!({
            "resource": {
                "attributes": {
                    "aws.local.service": "stack-replay-recorder",
                    "cloud.region": "us-east-1",
                    "service.name": "stack-replay-recorder",
                    "faas.name": "stack-replay-recorder",
                    "telemetry.sdk.name": "opentelemetry",
                    "telemetry.sdk.language": "python"
                }
            },
            "scope": {
                "name": "opentelemetry.instrumentation.aws_lambda",
                "version": "0.48b0"
            },
            "traceId": "6759b1c50e59b643038a6a070e115043",
            "spanId": "2a22ad6d7aeb6ef2",
            "parentSpanId": "48fec93dfe40959d",
            "name": "app.lambda_handler",
            "kind": "SERVER",
            "startTimeUnixNano": 1733931461640310404_u64,
            "endTimeUnixNano": 1733931462360000208_u64,
            "attributes": {
                "aws.local.service": "stack-replay-recorder",
                "aws.local.operation": "stack-replay-recorder/FunctionHandler",
                "aws.span.kind": "LOCAL_ROOT",
                "PlatformType": "AWS::Lambda"
            },
            "status": {
                "code": "UNSET"
            }
        });

        let result = convert_span_to_otlp(input);
        
        // Verify the structure matches OTLP format
        assert!(result.get("resource_spans").is_some());
        let resource_spans = result.get("resource_spans").unwrap().as_array().unwrap();
        assert_eq!(resource_spans.len(), 1);
        
        let first_resource_span = &resource_spans[0];
        
        // Verify resource attributes
        let resource = first_resource_span.get("resource").unwrap();
        let resource_attrs = resource.get("attributes").unwrap().as_array().unwrap();
        let mut found_service_name = false;
        for attr in resource_attrs {
            if attr.get("key").unwrap() == "service.name" {
                let value = attr.get("value").unwrap().get("string_value").unwrap();
                assert_eq!(value, "stack-replay-recorder");
                found_service_name = true;
            }
        }
        assert!(found_service_name, "service.name attribute not found in resource attributes");
        
        // Verify scope
        let scope_spans = first_resource_span.get("scope_spans").unwrap().as_array().unwrap();
        let scope = scope_spans[0].get("scope").unwrap();
        assert_eq!(scope.get("name").unwrap(), "opentelemetry.instrumentation.aws_lambda");
        assert_eq!(scope.get("version").unwrap(), "0.48b0");
        
        // Verify span
        let spans = scope_spans[0].get("spans").unwrap().as_array().unwrap();
        let span = &spans[0];
        assert_eq!(span.get("name").unwrap(), "app.lambda_handler");
        assert_eq!(span.get("traceId").unwrap(), "6759b1c50e59b643038a6a070e115043");
        assert_eq!(span.get("spanId").unwrap(), "2a22ad6d7aeb6ef2");
        assert_eq!(span.get("parentSpanId").unwrap(), "48fec93dfe40959d");
        assert_eq!(span.get("kind").unwrap(), 2); // SERVER = 2
        assert_eq!(span.get("startTimeUnixNano").unwrap().as_u64().unwrap(), 1733931461640310404_u64);
        
        // Verify span attributes
        let span_attrs = span.get("attributes").unwrap().as_array().unwrap();
        let mut found_platform_type = false;
        for attr in span_attrs {
            if attr.get("key").unwrap() == "PlatformType" {
                let value = attr.get("value").unwrap().get("string_value").unwrap();
                assert_eq!(value, "AWS::Lambda");
                found_platform_type = true;
            }
        }
        assert!(found_platform_type, "PlatformType attribute not found in span attributes");
    }
} 