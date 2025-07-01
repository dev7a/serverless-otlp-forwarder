use anyhow::Result;
use aws_lambda_events::event::kinesis::KinesisEvent;
use otlp_stdout_span_exporter::ExporterOutput;
use serverless_otlp_forwarder_core::core_parser::EventParser;
use serverless_otlp_forwarder_core::telemetry::TelemetryData; // For parsing the JSON string within Kinesis data

pub struct KinesisOtlpStdoutParser;

impl EventParser for KinesisOtlpStdoutParser {
    type EventInput = KinesisEvent;

    fn parse(
        &self,
        event_payload: Self::EventInput,
        _stream_name: &str,
    ) -> Result<Vec<TelemetryData>> {
        let records = event_payload.records;
        let mut telemetry_items = Vec::with_capacity(records.len());

        for kinesis_event_record in records {
            let data_bytes = &kinesis_event_record.kinesis.data.0; // .0 accesses the Vec<u8> from Base64Data
            let json_string = match String::from_utf8(data_bytes.clone()) {
                // Clone Vec<u8> for String conversion
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "Failed to decode Kinesis record data as UTF-8: {}. Skipping record.",
                        e
                    );
                    continue;
                }
            };

            tracing::debug!("Received Kinesis record (JSON string): {}", json_string);

            let exporter_output_record: ExporterOutput = match serde_json::from_str(&json_string) {
                Ok(output) => output,
                Err(err) => {
                    tracing::warn!(
                        "Failed to parse Kinesis record JSON string as ExporterOutput: {}. Error details: {}. Skipping record.",
                        json_string,
                        err
                    );
                    continue;
                }
            };

            tracing::debug!(
                "Successfully parsed Kinesis record as ExporterOutput with version: {}",
                exporter_output_record.version
            );

            match TelemetryData::from_log_record(exporter_output_record) {
                Ok(telemetry_data) => telemetry_items.push(telemetry_data),
                Err(e) => {
                    tracing::warn!(
                        "Failed to convert ExporterOutput to TelemetryData: {}. Skipping record.",
                        e
                    );
                }
            }
        }
        Ok(telemetry_items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lambda_events::encodings::{Base64Data, SecondTimestamp};
    use aws_lambda_events::event::kinesis::{
        KinesisEncryptionType, KinesisEventRecord, KinesisRecord,
    };
    use chrono::Utc;
    use serde_json::json;

    const VALID_TEST_PAYLOAD_STRING: &str = "H4sIAAAAAAAAAAMAAAAAAAAAAAA=";

    fn create_test_exporter_output_json_string(source: &str) -> String {
        let output = json!({
            "__otel_otlp_stdout": "otlp-stdout-span-exporter@0.2.2",
            "source": source,
            "endpoint": "http://original.collector/v1/traces",
            "method": "POST",
            "payload": VALID_TEST_PAYLOAD_STRING,
            "headers": {
                "content-type": "application/x-protobuf"
            },
            "content-type": "application/x-protobuf",
            "content-encoding": "gzip",
            "base64": true
        });
        serde_json::to_string(&output).unwrap()
    }

    fn create_kinesis_event_record(data_string: String) -> KinesisEventRecord {
        KinesisEventRecord {
            event_id: Some("shardId-000000000000:12345".to_string()),
            event_version: Some("1.0".to_string()),
            kinesis: KinesisRecord {
                kinesis_schema_version: Some("1.0".to_string()),
                partition_key: "test_partition_key".to_string(),
                sequence_number: "1234567890".to_string(),
                data: Base64Data(data_string.into_bytes()),
                approximate_arrival_timestamp: SecondTimestamp(Utc::now()),
                encryption_type: KinesisEncryptionType::None,
            },
            invoke_identity_arn: Some("arn:aws:iam::123456789012:role/lambda-role".to_string()),
            event_name: Some("aws:kinesis:record".to_string()),
            event_source: Some("aws:kinesis".to_string()),
            event_source_arn: Some(
                "arn:aws:kinesis:us-east-1:123456789012:stream/test-stream".to_string(),
            ),
            aws_region: Some("us-east-1".to_string()),
        }
    }

    #[test]
    fn test_kinesis_otlp_stdout_parser_success() {
        let parser = KinesisOtlpStdoutParser;
        let record_string1 = create_test_exporter_output_json_string("service-c");
        let record_string2 = create_test_exporter_output_json_string("service-d");

        let event = KinesisEvent {
            records: vec![
                create_kinesis_event_record(record_string1),
                create_kinesis_event_record(record_string2),
            ],
        };

        let result = parser.parse(event, "test-stream").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].source, "service-c");
        assert_eq!(result[1].source, "service-d");
        assert_eq!(result[0].content_type, "application/x-protobuf");
        assert_eq!(result[0].content_encoding, None);
    }

    #[test]
    fn test_kinesis_otlp_stdout_parser_invalid_utf8_in_data() {
        let parser = KinesisOtlpStdoutParser;
        let invalid_utf8_data = vec![0x80];
        let valid_record_string = create_test_exporter_output_json_string("service-ok");

        let kinesis_record_invalid_utf8 = KinesisRecord {
            kinesis_schema_version: Some("1.0".to_string()),
            partition_key: "pk_invalid_utf8".to_string(),
            sequence_number: "seq_invalid_utf8".to_string(),
            data: Base64Data(invalid_utf8_data),
            approximate_arrival_timestamp: SecondTimestamp(Utc::now()),
            encryption_type: KinesisEncryptionType::None,
        };

        let event_record_invalid_utf8 = KinesisEventRecord {
            event_id: Some("event_invalid_utf8_id".to_string()),
            event_version: Some("1.0".to_string()),
            kinesis: kinesis_record_invalid_utf8,
            invoke_identity_arn: Some(
                "arn:aws:iam::000000000000:role/lambda-role-minimal".to_string(),
            ),
            event_name: Some("aws:kinesis:record".to_string()),
            event_source: Some("aws:kinesis".to_string()),
            event_source_arn: Some(
                "arn:aws:kinesis:us-east-1:000000000000:stream/minimal-stream-utf8".to_string(),
            ),
            aws_region: Some("us-east-1".to_string()),
        };

        let event = KinesisEvent {
            records: vec![
                event_record_invalid_utf8,
                create_kinesis_event_record(valid_record_string),
            ],
        };

        let result = parser.parse(event, "test-stream").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, "service-ok");
    }

    #[test]
    fn test_kinesis_otlp_stdout_parser_malformed_json_string() {
        let parser = KinesisOtlpStdoutParser;
        let malformed_json_string = "{\"invalid_json".to_string();
        let valid_record_string = create_test_exporter_output_json_string("service-fine");

        let event = KinesisEvent {
            records: vec![
                create_kinesis_event_record(malformed_json_string),
                create_kinesis_event_record(valid_record_string),
            ],
        };

        let result = parser.parse(event, "test-stream").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, "service-fine");
    }

    #[test]
    fn test_kinesis_otlp_stdout_parser_empty_records() {
        let parser = KinesisOtlpStdoutParser;
        let event = KinesisEvent { records: vec![] };
        let result = parser.parse(event, "test-stream").unwrap();
        assert!(result.is_empty());
    }
}
