use crate::events::{ParsedPlatformEvent, PlatformEventData, TelemetrySpan};
use chrono::{DateTime, Utc};
use lambda_extension::Status as LambdaStatus;
use opentelemetry::{
    trace::{SpanContext, SpanId, SpanKind, Status as OtelStatus, TraceFlags, TraceId, TraceState},
    InstrumentationScope, KeyValue,
};
use opentelemetry_sdk::trace::{SpanData, SpanEvents, SpanLinks};
use rand::Rng;
use std::borrow::Cow;
use std::time::{Duration as StdDuration, SystemTime};

// Define constants for synthesized span names
const INIT_PHASE_NAME: &str = "Lambda/Init";
const LAMBDA_INVOKE_NAME: &str = "Lambda/Invoke";
// Define constants for platform span names we want to map
const RESPONSE_LATENCY_NAME: &str = "Response/Latency";
const RESPONSE_DURATION_NAME: &str = "Response/Duration";
const EXTENSION_OVERHEAD_NAME: &str = "Overhead/Extension";
const RUNTIME_OVERHEAD_NAME: &str = "Overhead/Runtime";

#[derive(Debug)]
pub struct SpanAggregator {
    pub request_id: String,

    pub trace_id: Option<TraceId>,
    pub span_id: Option<SpanId>,
    pub function_root_span_id: Option<SpanId>,
    pub trace_flags: TraceFlags,

    pub start_time: Option<SystemTime>,
    pub end_time: Option<SystemTime>,
    pub status: OtelStatus,

    pub name: String,
    pub kind: SpanKind,
    pub attributes: Vec<KeyValue>,
    pub child_spans_data: Vec<SpanData>,

    pub received_event_types: Vec<String>,
    pub first_seen_timestamp: DateTime<Utc>,
    pub last_updated_timestamp: DateTime<Utc>,
}

impl SpanAggregator {
    pub fn new(request_id: String, timestamp: DateTime<Utc>) -> Self {
        Self {
            request_id,
            trace_id: None,
            span_id: None,
            function_root_span_id: None,
            trace_flags: TraceFlags::NOT_SAMPLED,
            start_time: None,
            end_time: None,
            status: OtelStatus::Unset,
            name: LAMBDA_INVOKE_NAME.to_string(),
            kind: SpanKind::Server,
            attributes: Vec::new(),
            child_spans_data: Vec::new(),
            received_event_types: Vec::new(),
            first_seen_timestamp: timestamp,
            last_updated_timestamp: timestamp,
        }
    }

    /// Sets the trace context information for this aggregator from the execution_trace_map.
    /// This method will only set the fields if they have not been set yet (are None).
    pub fn set_trace_context(&mut self, trace_id: TraceId, root_span_id: SpanId) {
        // Only set trace_id if it's not already set
        if self.trace_id.is_none() {
            tracing::debug!(%trace_id, %root_span_id, "Setting trace context for request_id: {}", self.request_id);

            self.trace_id = Some(trace_id);
            self.function_root_span_id = Some(root_span_id);

            // Generate and store the span_id for *this* aggregator's span ("Lambda Invoke")
            if self.span_id.is_none() {
                let mut rng = rand::rng();
                self.span_id = Some(SpanId::from_bytes(rng.random::<[u8; 8]>()));
                tracing::debug!(generated_span_id = ?self.span_id, "Generated span_id for Lambda Invoke span");
            }

            // Mark as sampled since we have trace information
            self.trace_flags = TraceFlags::SAMPLED;
        }
    }

    /// Updates the aggregator state based on a received platform event.
    pub fn update_from_event(&mut self, event: &ParsedPlatformEvent) {
        self.last_updated_timestamp = event.timestamp;
        let event_type_str = match &event.data {
            PlatformEventData::InitStart { .. } => "platform.initStart",
            PlatformEventData::Start { .. } => "platform.start",
            PlatformEventData::RuntimeDone { .. } => "platform.runtimeDone",
            PlatformEventData::Report { .. } => "platform.report",
        };
        self.received_event_types.push(event_type_str.to_string());

        match &event.data {
            PlatformEventData::InitStart { .. } => {
                tracing::trace!(request_id=%self.request_id, "Processing InitStart event in aggregator (no-op for now).");
            }
            PlatformEventData::Start {
                version,
                ..  // Remove trace_context from pattern
            } => {
                if self.start_time.is_none() {
                    self.start_time = Some(event.timestamp.into());
                }
                if let Some(v) = version {
                    self.attributes
                        .push(KeyValue::new("faas.instance", v.clone()));
                }
            }
            PlatformEventData::RuntimeDone {
                status,
                error_type,
                metrics,
                spans,
                ..  // Remove trace_context from pattern
            } => {
                if self.status == OtelStatus::Unset {
                    self.set_otel_status(status.clone(), error_type.as_deref());
                }
                for (k, v) in metrics {
                    self.attributes
                        .push(KeyValue::new(format!("lambda.runtime.{}", k), v.clone()));
                }
                self.add_child_spans(spans);
            }
            PlatformEventData::Report {
                status,
                error_type,
                metrics,
                spans,
                ..  // Remove trace_context from pattern
            } => {
                self.end_time = Some(event.timestamp.into());
                self.set_otel_status(status.clone(), error_type.as_deref());
                for (k, v) in metrics {
                    self.attributes
                        .push(KeyValue::new(format!("lambda.report.{}", k), v.clone()));
                }
                self.attributes
                    .push(KeyValue::new("faas.execution", self.request_id.clone()));
                self.add_child_spans(spans);
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        self.received_event_types
            .contains(&"platform.runtimeDone".to_string())
            && self
                .received_event_types
                .contains(&"platform.report".to_string())
    }

    pub fn to_otel_span_data(&self) -> Option<SpanData> {
        let trace_id = self.trace_id?;
        let span_id = self.span_id?;
        let start_time = self.start_time?;
        let end_time = self
            .end_time
            .unwrap_or_else(|| self.last_updated_timestamp.into());

        let span_context = SpanContext::new(
            trace_id,
            span_id,
            self.trace_flags,
            false,
            TraceState::default(),
        );

        Some(SpanData {
            span_context,
            parent_span_id: self
                .function_root_span_id
                .unwrap_or_else(|| SpanId::from_bytes([0; 8])),
            span_kind: self.kind.clone(),
            name: self.name.clone().into(),
            start_time,
            end_time,
            attributes: self.attributes.clone(),
            events: SpanEvents::default(),
            links: SpanLinks::default(),
            status: self.status.clone(),
            dropped_attributes_count: 0,
            instrumentation_scope: InstrumentationScope::default(),
        })
    }

    fn set_otel_status(&mut self, lambda_status: LambdaStatus, error_type: Option<&str>) {
        match lambda_status {
            LambdaStatus::Success => {
                if matches!(self.status, OtelStatus::Unset) {
                    self.status = OtelStatus::Ok;
                }
            }
            LambdaStatus::Failure | LambdaStatus::Error | LambdaStatus::Timeout => {
                self.status = OtelStatus::Error {
                    description: error_type
                        .unwrap_or("Lambda platform error")
                        .to_string()
                        .into(),
                };
            }
        }
    }

    fn add_child_spans(&mut self, spans: &[TelemetrySpan]) {
        // Check for trace_id first
        let trace_id = match self.trace_id {
            Some(id) => id,
            None => {
                tracing::warn!(
                    "Cannot create child spans without a trace ID for aggregate invoke span."
                );
                return; // Cannot proceed without trace_id
            }
        };

        // Check for parent_span_id (this aggregate span's ID, i.e., "Lambda Invoke")
        let parent_span_id = match self.span_id {
            Some(id) => id,
            None => {
                // This case should be less likely as set_trace_context generates span_id
                tracing::error!(
                    request_id=%self.request_id,
                    "Cannot create platform child spans: parent span_id (Lambda Invoke ID) is missing."
                );
                return;
            }
        };

        let mut rng = rand::rng();

        for span in spans {
            let child_start_time: SystemTime = span.start.into();
            let child_duration = StdDuration::from_secs_f64(span.duration_ms / 1000.0);
            let child_end_time = child_start_time + child_duration;
            let child_span_id = SpanId::from_bytes(rng.random::<[u8; 8]>());

            let child_span_context = SpanContext::new(
                trace_id,
                child_span_id,
                self.trace_flags,
                false,
                TraceState::default(),
            );

            let child_span_data = SpanData {
                span_context: child_span_context,
                parent_span_id,
                span_kind: SpanKind::Internal,
                name: Self::map_platform_span_name(span.name.as_str()),
                start_time: child_start_time,
                end_time: child_end_time,
                attributes: vec![],
                events: SpanEvents::default(),
                links: SpanLinks::default(),
                status: OtelStatus::Unset,
                dropped_attributes_count: 0,
                instrumentation_scope: InstrumentationScope::default(),
            };
            self.child_spans_data.push(child_span_data);
        }
    }

    /// Adds a synthesized span representing the Lambda Init phase.
    /// This is called when InitDataAvailable is processed, using the stored start time
    /// and the duration from the first invoke's report.
    pub fn add_init_phase_span(&mut self, start_time: SystemTime, duration_ms: f64) {
        // Ensure we have the necessary context from the main function trace
        let trace_id = if let Some(id) = self.trace_id {
            id
        } else {
            tracing::warn!(request_id=%self.request_id, "Cannot add init phase span: trace_id is missing.");
            return;
        };
        let parent_span_id = if let Some(id) = self.function_root_span_id {
            id
        } else {
            // Should only happen if OTLP data wasn't parsed before report w/ initDuration
            tracing::warn!(request_id=%self.request_id, "Cannot add init phase span: function root span_id is missing.");
            return;
        };

        tracing::debug!(request_id=%self.request_id, %trace_id, %parent_span_id, start=?start_time, duration_ms, "Adding Init Phase span.");

        // Calculate end time
        let duration = StdDuration::from_secs_f64(duration_ms / 1000.0);
        let end_time = start_time + duration;

        // Generate a unique span ID for the init phase span
        let mut rng = rand::rng();
        let init_span_id = SpanId::from_bytes(rng.random::<[u8; 8]>());

        let init_span_context = SpanContext::new(
            trace_id,
            init_span_id,
            self.trace_flags, // Inherit flags from parent
            false,
            TraceState::default(),
        );

        let init_span_data = SpanData {
            span_context: init_span_context,
            parent_span_id, // Parent is the actual function's root span
            span_kind: SpanKind::Internal,
            name: INIT_PHASE_NAME.into(),
            start_time,
            end_time,
            attributes: vec![], // TODO: Could add lambda.init_type attribute later
            events: SpanEvents::default(),
            links: SpanLinks::default(),
            status: OtelStatus::Ok, // Assume OK if we got this far
            dropped_attributes_count: 0,
            instrumentation_scope: InstrumentationScope::default(),
        };

        self.child_spans_data.push(init_span_data);
    }

    /// Maps known platform span names to standardized constant names.
    /// Returns the original name if no mapping exists.
    fn map_platform_span_name(original_name: &str) -> Cow<'static, str> {
        match original_name {
            "responseLatency" => Cow::Borrowed(RESPONSE_LATENCY_NAME),
            "responseDuration" => Cow::Borrowed(RESPONSE_DURATION_NAME),
            "extensionOverhead" => Cow::Borrowed(EXTENSION_OVERHEAD_NAME),
            "runtimeOverhead" => Cow::Borrowed(RUNTIME_OVERHEAD_NAME),
            // Add other mappings here if needed
            _ => Cow::Owned(original_name.to_string()), // Fallback to original name (owned copy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Import items from outer module
    use chrono::TimeZone;
    use opentelemetry::trace::TraceFlags;
    use opentelemetry::Value as OtelValue;
    use std::borrow::Cow;
    use std::collections::HashMap;

    // Helper to create a default timestamp
    fn default_ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()
    }

    #[test]
    fn test_aggregator_new() {
        let request_id = "test-req-id".to_string();
        let timestamp = default_ts();
        let agg = SpanAggregator::new(request_id.clone(), timestamp);

        assert_eq!(agg.request_id, request_id);
        assert!(agg.trace_id.is_none());
        assert!(agg.span_id.is_none());
        assert!(agg.function_root_span_id.is_none());
        assert_eq!(agg.trace_flags, TraceFlags::NOT_SAMPLED);
        assert!(agg.start_time.is_none());
        assert!(agg.end_time.is_none());
        assert_eq!(agg.status, OtelStatus::Unset);
        assert_eq!(agg.name, "Lambda Invoke");
        assert!(matches!(agg.kind, SpanKind::Server));
        assert!(agg.attributes.is_empty());
        assert!(agg.child_spans_data.is_empty());
        assert!(agg.received_event_types.is_empty());
        assert_eq!(agg.first_seen_timestamp, timestamp);
        assert_eq!(agg.last_updated_timestamp, timestamp);
    }

    #[test]
    fn test_update_from_start_event() {
        let request_id = "req-start".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        let trace_id = TraceId::from_hex("0102030405060708090a0b0c0d0e0f10").unwrap();
        let root_span_id = SpanId::from_hex("0102030405060708").unwrap();

        // First, set the trace context directly
        agg.set_trace_context(trace_id, root_span_id);

        // Then create and apply the start event (without trace context)
        let start_event = ParsedPlatformEvent {
            timestamp,
            request_id,
            data: PlatformEventData::Start {
                version: Some("1.0".to_string()),
            },
        };

        agg.update_from_event(&start_event);

        assert_eq!(agg.start_time, Some(timestamp.into()));
        assert_eq!(agg.trace_id, Some(trace_id));
        assert_eq!(agg.function_root_span_id, Some(root_span_id));
        assert_eq!(agg.trace_flags, TraceFlags::SAMPLED);
        assert!(agg.span_id.is_some()); // A span_id should be generated
        assert_eq!(agg.received_event_types, vec!["platform.start"]);
        assert_eq!(agg.last_updated_timestamp, timestamp);
        // Check attribute was added
        assert!(agg.attributes.iter().any(
            |kv| kv.key == KeyValue::new("faas.instance", "").key && kv.value.as_str() == "1.0"
        ));
    }

    #[test]
    fn test_update_from_runtime_done_event_success() {
        let request_id = "req-rtd-success".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        let mut metrics = HashMap::new();
        metrics.insert("runtime.durationMs".to_string(), OtelValue::F64(123.45));
        metrics.insert("runtime.producedBytes".to_string(), OtelValue::I64(1024));

        let runtime_done_event = ParsedPlatformEvent {
            timestamp: timestamp + chrono::Duration::milliseconds(200), // Slightly later ts
            request_id,
            data: PlatformEventData::RuntimeDone {
                status: LambdaStatus::Success,
                error_type: None,
                metrics,       // Pass the created metrics map
                spans: vec![], // No child spans for this test
            },
        };

        agg.update_from_event(&runtime_done_event);

        assert_eq!(agg.status, OtelStatus::Ok); // Status should be Ok
        assert_eq!(agg.received_event_types, vec!["platform.runtimeDone"]);
        assert_eq!(agg.last_updated_timestamp, runtime_done_event.timestamp);
        // Check metric attributes were added
        assert!(agg.attributes.iter().any(|kv| kv.key
            == KeyValue::new("lambda.runtime.runtime.durationMs", "").key
            && kv.value == OtelValue::F64(123.45)));
        assert!(agg.attributes.iter().any(|kv| kv.key
            == KeyValue::new("lambda.runtime.runtime.producedBytes", "").key
            && kv.value == OtelValue::I64(1024)));
    }

    #[test]
    fn test_update_from_runtime_done_event_failure() {
        let request_id = "req-rtd-fail".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        let runtime_done_event = ParsedPlatformEvent {
            timestamp: timestamp + chrono::Duration::milliseconds(200),
            request_id,
            data: PlatformEventData::RuntimeDone {
                status: LambdaStatus::Failure,
                error_type: Some("Error".to_string()),
                metrics: HashMap::new(),
                spans: vec![],
            },
        };

        agg.update_from_event(&runtime_done_event);

        assert!(matches!(agg.status, OtelStatus::Error { .. }));
        if let OtelStatus::Error { description } = agg.status {
            assert_eq!(description, Cow::Borrowed("Error"));
        }
        assert_eq!(agg.received_event_types, vec!["platform.runtimeDone"]);
        assert_eq!(agg.last_updated_timestamp, runtime_done_event.timestamp);
    }

    #[test]
    fn test_update_from_report_event() {
        let request_id = "req-report".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        // Simulate receiving RuntimeDone first (optional, but good test)
        let runtime_done_event = ParsedPlatformEvent {
            timestamp: timestamp + chrono::Duration::milliseconds(200),
            request_id: request_id.clone(),
            data: PlatformEventData::RuntimeDone {
                status: LambdaStatus::Success, // Initially success
                error_type: None,
                metrics: HashMap::new(),
                spans: vec![],
            },
        };
        agg.update_from_event(&runtime_done_event);
        assert_eq!(agg.status, OtelStatus::Ok);

        // Now receive Report event with failure
        let report_timestamp = timestamp + chrono::Duration::milliseconds(300);
        let mut report_metrics = HashMap::new();
        report_metrics.insert("report.durationMs".to_string(), OtelValue::F64(250.0));
        report_metrics.insert("report.billedDurationMs".to_string(), OtelValue::I64(300));

        let report_event = ParsedPlatformEvent {
            timestamp: report_timestamp,
            request_id,
            data: PlatformEventData::Report {
                status: LambdaStatus::Failure, // Failure in report
                error_type: Some("ReportError".to_string()),
                metrics: report_metrics,
                spans: vec![],
            },
        };

        agg.update_from_event(&report_event);

        // Check end_time is set
        assert_eq!(agg.end_time, Some(report_timestamp.into()));
        // Check status is updated to Error by Report
        assert!(matches!(agg.status, OtelStatus::Error { .. }));
        if let OtelStatus::Error { description } = agg.status {
            assert_eq!(description, Cow::Borrowed("ReportError"));
        }
        assert_eq!(
            agg.received_event_types,
            vec!["platform.runtimeDone", "platform.report"]
        );
        assert_eq!(agg.last_updated_timestamp, report_timestamp);
        // Check report metrics are added
        assert!(agg.attributes.iter().any(|kv| kv.key
            == KeyValue::new("lambda.report.report.durationMs", "").key
            && kv.value == OtelValue::F64(250.0)));
        // Check faas.execution attribute added
        assert!(agg
            .attributes
            .iter()
            .any(|kv| kv.key == KeyValue::new("faas.execution", "").key
                && kv.value == OtelValue::String("req-report".into())));
    }

    #[test]
    fn test_is_complete() {
        let request_id = "req-complete".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        assert!(!agg.is_complete());

        // Add Start
        let start_event = ParsedPlatformEvent {
            timestamp,
            request_id: request_id.clone(),
            data: PlatformEventData::Start { version: None },
        };
        agg.update_from_event(&start_event);
        assert!(!agg.is_complete());

        // Add RuntimeDone
        let runtime_done_event = ParsedPlatformEvent {
            timestamp: timestamp + chrono::Duration::milliseconds(100),
            request_id: request_id.clone(),
            data: PlatformEventData::RuntimeDone {
                status: LambdaStatus::Success,
                error_type: None,
                metrics: HashMap::new(),
                spans: vec![],
            },
        };
        agg.update_from_event(&runtime_done_event);
        assert!(!agg.is_complete()); // Still needs Report

        // Add Report
        let report_event = ParsedPlatformEvent {
            timestamp: timestamp + chrono::Duration::milliseconds(200),
            request_id: request_id.clone(),
            data: PlatformEventData::Report {
                status: LambdaStatus::Success,
                error_type: None,
                metrics: HashMap::new(),
                spans: vec![],
            },
        };
        agg.update_from_event(&report_event);
        assert!(agg.is_complete()); // Now complete
    }

    #[test]
    fn test_to_otel_span_data_missing_fields() {
        let request_id = "req-otel-missing".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        // Initially, all fields are missing
        assert!(
            agg.to_otel_span_data().is_none(),
            "Should be None initially"
        );

        // Set start_time only
        agg.start_time = Some(timestamp.into());
        assert!(
            agg.to_otel_span_data().is_none(),
            "Should be None with only start_time"
        );

        // Set trace_id only (start_time is still set)
        agg.trace_id = Some(TraceId::from_hex("01000000000000000000000000000001").unwrap());
        assert!(
            agg.to_otel_span_data().is_none(),
            "Should be None with start_time and trace_id"
        );

        // Set span_id (now all required fields are present)
        agg.span_id = Some(SpanId::from_hex("0100000000000002").unwrap());
        assert!(
            agg.to_otel_span_data().is_some(),
            "Should be Some when trace_id, span_id, and start_time are set"
        );
    }

    #[test]
    fn test_to_otel_span_data_success() {
        let request_id = "req-otel-success".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        let trace_id = TraceId::from_hex("0102030405060708090a0b0c0d0e0f11").unwrap();
        let span_id = SpanId::from_hex("1112131415161718").unwrap();
        let root_span_id = SpanId::from_hex("2122232425262728").unwrap();
        let start_system_time: SystemTime = timestamp.into();
        let end_system_time: SystemTime = (timestamp + chrono::Duration::milliseconds(100)).into();

        // Set required fields
        agg.trace_id = Some(trace_id);
        agg.span_id = Some(span_id);
        agg.function_root_span_id = Some(root_span_id);
        agg.start_time = Some(start_system_time);
        agg.end_time = Some(end_system_time);
        agg.trace_flags = TraceFlags::SAMPLED;
        agg.status = OtelStatus::Ok;
        // Use ::from for OtelValue::String
        agg.attributes
            .push(KeyValue::new("test.key", OtelValue::from("test.value")));

        let span_data_opt = agg.to_otel_span_data();
        assert!(span_data_opt.is_some());
        let span_data = span_data_opt.unwrap();

        assert_eq!(span_data.span_context.trace_id(), trace_id);
        assert_eq!(span_data.span_context.span_id(), span_id);
        assert_eq!(span_data.span_context.trace_flags(), TraceFlags::SAMPLED);
        assert_eq!(span_data.parent_span_id, root_span_id);
        assert_eq!(span_data.name.as_ref(), "Lambda Invoke"); // Compare Cow as &str
        assert_eq!(span_data.start_time, start_system_time);
        assert_eq!(span_data.end_time, end_system_time);
        assert_eq!(span_data.status, OtelStatus::Ok);
        assert_eq!(span_data.attributes.len(), 1);
        assert_eq!(
            span_data.attributes[0].key,
            KeyValue::new("test.key", "").key
        );
        // Use ::from for comparison value as well
        assert_eq!(span_data.attributes[0].value, OtelValue::from("test.value"));
    }

    #[test]
    fn test_to_otel_span_data_fallback_end_time() {
        let request_id = "req-otel-fallback".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        let start_system_time: SystemTime = timestamp.into();
        let last_update_system_time: SystemTime =
            (timestamp + chrono::Duration::milliseconds(500)).into();

        agg.trace_id = Some(TraceId::from_hex("01000000000000000000000000000001").unwrap());
        agg.span_id = Some(SpanId::from_hex("0100000000000002").unwrap());
        agg.start_time = Some(start_system_time);
        // No end_time set
        agg.last_updated_timestamp = timestamp + chrono::Duration::milliseconds(500);

        let span_data = agg.to_otel_span_data().unwrap();
        // End time should be the last_updated_timestamp converted to SystemTime
        assert_eq!(span_data.end_time, last_update_system_time);
    }

    #[test]
    fn test_child_span_creation() {
        let request_id = "req-child-spans".to_string();
        let timestamp = default_ts();
        let mut agg = SpanAggregator::new(request_id.clone(), timestamp);

        // Set trace_id and span_id for the parent (aggregator)
        let trace_id = TraceId::from_hex("0102030405060708090a0b0c0d0e0f12").unwrap();
        let root_span_id = SpanId::from_hex("1112131415161719").unwrap();
        agg.trace_id = Some(trace_id);
        agg.span_id = Some(root_span_id);

        // Create TelemetrySpans to add
        let child_start1_dt = timestamp + chrono::Duration::milliseconds(50);
        let child_start2_dt = timestamp + chrono::Duration::milliseconds(150);
        let child_start1_st: SystemTime = child_start1_dt.into();
        let child_start2_st: SystemTime = child_start2_dt.into();

        let telemetry_spans = vec![
            TelemetrySpan {
                duration_ms: 50.0,
                name: "child1".to_string(),
                start: child_start1_dt,
            },
            TelemetrySpan {
                duration_ms: 75.5,
                name: "child2".to_string(),
                start: child_start2_dt,
            },
        ];

        // Use a RuntimeDone event to add the spans
        let runtime_done_event = ParsedPlatformEvent {
            timestamp: timestamp + chrono::Duration::milliseconds(300),
            request_id,
            data: PlatformEventData::RuntimeDone {
                status: LambdaStatus::Success,
                error_type: None,
                metrics: HashMap::new(),
                spans: telemetry_spans, // Pass the child spans
            },
        };

        agg.update_from_event(&runtime_done_event);

        // Verify child_spans_data
        assert_eq!(agg.child_spans_data.len(), 2);

        // Check first child span
        let child1 = &agg.child_spans_data[0];
        assert_eq!(child1.span_context.trace_id(), trace_id);
        assert_ne!(child1.span_context.span_id(), root_span_id);
        assert_ne!(child1.span_context.span_id(), SpanId::INVALID);
        assert_eq!(child1.parent_span_id, root_span_id);
        assert_eq!(child1.name.as_ref(), "child1");
        // Compare SystemTime directly
        assert_eq!(child1.start_time, child_start1_st);
        let expected_end1: SystemTime =
            (child_start1_dt + chrono::Duration::microseconds(50000)).into();
        assert_eq!(child1.end_time, expected_end1);
        assert!(matches!(child1.span_kind, SpanKind::Internal));

        // Check second child span
        let child2 = &agg.child_spans_data[1];
        assert_eq!(child2.span_context.trace_id(), trace_id);
        assert_ne!(child2.span_context.span_id(), root_span_id);
        assert_ne!(child2.span_context.span_id(), child1.span_context.span_id());
        assert_eq!(child2.parent_span_id, root_span_id);
        assert_eq!(child2.name.as_ref(), "child2");
        // Compare SystemTime directly
        assert_eq!(child2.start_time, child_start2_st);
        let expected_end2: SystemTime =
            (child_start2_dt + chrono::Duration::microseconds(75500)).into();
        assert_eq!(child2.end_time, expected_end2);
        assert!(matches!(child2.span_kind, SpanKind::Internal));
    }
}
