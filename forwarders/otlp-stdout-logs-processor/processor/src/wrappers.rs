//! Event wrapper types for Lambda functions
//!
//! This module provides wrapper types for different AWS event sources
//! that implement the SpanAttributesExtractor trait for OpenTelemetry instrumentation.

use aws_lambda_events::event::cloudwatch_logs::LogsEvent;
use aws_lambda_events::event::kinesis::KinesisEvent;
use lambda_otel_lite::{SpanAttributes, SpanAttributesExtractor};
use opentelemetry::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Wrapper for KinesisEvent to implement SpanAttributesExtractor
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KinesisEventWrapper(pub KinesisEvent);

impl SpanAttributesExtractor for KinesisEventWrapper {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let mut attributes: HashMap<String, Value> = HashMap::new();
        let records = &self.0.records;

        // Add attributes from the Kinesis event
        attributes.insert(
            "forwarder.events.count".to_string(),
            Value::I64(records.len() as i64),
        );

        if let Some(first_record) = records.first() {
            if let Some(event_source) = &first_record.event_source {
                attributes.insert(
                    "forwarder.stream.name".to_string(),
                    Value::String(event_source.clone().into()),
                );
            }
        }

        SpanAttributes::builder()
            .span_name("kinesis-processor".to_string())
            .kind("consumer".to_string())
            .attributes(attributes)
            .build()
    }
}

/// Wrapper for LogsEvent to implement SpanAttributesExtractor
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogsEventWrapper(pub LogsEvent);

impl SpanAttributesExtractor for LogsEventWrapper {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let mut attributes: HashMap<String, Value> = HashMap::new();
        let log_data = self.0.aws_logs.data.clone();

        // Add attributes from the LogsEvent
        attributes.insert(
            "forwarder.log_group".to_string(),
            Value::String(log_data.log_group.clone().into()),
        );
        attributes.insert(
            "forwarder.events.count".to_string(),
            Value::I64(log_data.log_events.len() as i64),
        );

        SpanAttributes::builder()
            .span_name(log_data.log_group)
            .kind("consumer".to_string())
            .attributes(attributes)
            .build()
    }
}
