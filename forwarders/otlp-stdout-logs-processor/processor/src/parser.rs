use anyhow::Result;
use aws_lambda_events::event::cloudwatch_logs::LogsEvent;
use otlp_stdout_span_exporter::ExporterOutput;
use serverless_otlp_forwarder_core::core_parser::EventParser;
use serverless_otlp_forwarder_core::telemetry::TelemetryData;
use tracing;

// Define a local struct for parsing CloudWatch Logs events containing OTLP stdout format.
pub struct CloudWatchLogsOtlpStdoutParser;

impl EventParser for CloudWatchLogsOtlpStdoutParser {
    type EventInput = LogsEvent;

    fn parse(
        &self,
        event_payload: Self::EventInput,
        _log_group: &str,
    ) -> Result<Vec<TelemetryData>> {
        let log_events = event_payload.aws_logs.data.log_events;
        let mut telemetry_items = Vec::with_capacity(log_events.len());

        for log_event in log_events {
            let log_record_str = &log_event.message;
            tracing::debug!(
                "Received log record string for OTLP stdout processing: {}",
                log_record_str
            );

            let record: ExporterOutput = match serde_json::from_str(log_record_str) {
                Ok(output) => output,
                Err(err) => {
                    tracing::warn!(
                        "Failed to parse log record as ExporterOutput JSON: {}. Error details: {}. Skipping record.",
                        log_record_str,
                        err
                    );
                    continue; // Skip this log event if it doesn't parse to ExporterOutput
                }
            };

            tracing::debug!(
                "Successfully parsed log record as ExporterOutput with version: {}",
                record.version
            );

            // TelemetryData::from_log_record is now part of the core library
            // and handles the conversion from ExporterOutput to the core TelemetryData format.
            match TelemetryData::from_log_record(record) {
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
    use aws_lambda_events::event::cloudwatch_logs::{AwsLogs, LogData, LogEntry};
    use serde_json::json;

    // A valid base64-encoded, gzipped, minimal OTLP protobuf payload string.
    // Represents an empty ExportTraceServiceRequest.
    const VALID_TEST_PAYLOAD_STRING: &str = "H4sIAAAAAAAAAAMAAAAAAAAAAAA="; // gzip(protobuf([]))

    // Helper to create a JSON string that mimics otlp_stdout_span_exporter::ExporterOutput format
    fn create_test_exporter_output_log_message(source: &str) -> String {
        let output = json!({
            "__otel_otlp_stdout": "otlp-stdout-span-exporter@0.2.2",
            "source": source,
            "endpoint": "http://original.collector/v1/traces",
            "method": "POST",
            "payload": VALID_TEST_PAYLOAD_STRING,
            "headers": {
                "content-type": "application/x-protobuf"
            },
            "content-type": "application/x-protobuf", // This is the type *before* base64/gzip in ExporterOutput model
            "content-encoding": "gzip", // This is the encoding *before* base64 in ExporterOutput model
            "base64": true
        });
        serde_json::to_string(&output).unwrap()
    }

    #[test]
    fn test_cloudwatch_logs_parser_success() {
        let parser = CloudWatchLogsOtlpStdoutParser;
        let log_message1 = create_test_exporter_output_log_message("service-a");
        let log_message2 = create_test_exporter_output_log_message("service-b");

        let event = LogsEvent {
            aws_logs: AwsLogs {
                data: LogData {
                    owner: "123456789012".to_string(),
                    log_group: "/aws/lambda/test-func".to_string(),
                    log_stream: "stream1".to_string(),
                    message_type: "DATA_MESSAGE".to_string(),
                    subscription_filters: vec!["filter1".to_string()],
                    log_events: vec![
                        LogEntry {
                            id: "e1".to_string(),
                            timestamp: 1000,
                            message: log_message1,
                        },
                        LogEntry {
                            id: "e2".to_string(),
                            timestamp: 2000,
                            message: log_message2,
                        },
                    ],
                },
            },
        };

        let result = parser.parse(event, "/aws/lambda/test-func").unwrap();
        assert_eq!(result.len(), 2);
        // The `source` in TelemetryData comes from ExporterOutput.source
        assert_eq!(result[0].source, "service-a");
        assert_eq!(result[1].source, "service-b");
        // TelemetryData::from_log_record ensures payload is decoded & content_type is protobuf
        assert_eq!(result[0].content_type, "application/x-protobuf");
        assert_eq!(result[0].content_encoding, None); // Should be decompressed by from_log_record
    }

    #[test]
    fn test_cloudwatch_logs_parser_malformed_json_in_log_entry() {
        let parser = CloudWatchLogsOtlpStdoutParser;
        let malformed_json_message = "{\"key\": \"value\" but not closed";
        let valid_json_message = create_test_exporter_output_log_message("service-ok");

        let event = LogsEvent {
            aws_logs: AwsLogs {
                data: LogData {
                    owner: "123456789012".to_string(),
                    log_group: "/aws/lambda/test-func".to_string(),
                    log_stream: "stream1".to_string(),
                    message_type: "DATA_MESSAGE".to_string(),
                    subscription_filters: vec!["filter1".to_string()],
                    log_events: vec![
                        LogEntry {
                            id: "e1".to_string(),
                            timestamp: 1000,
                            message: malformed_json_message.to_string(),
                        },
                        LogEntry {
                            id: "e2".to_string(),
                            timestamp: 2000,
                            message: valid_json_message,
                        },
                    ],
                },
            },
        };

        let result = parser.parse(event, "/aws/lambda/test-func").unwrap();
        assert_eq!(result.len(), 1); // Skips the malformed one, processes the valid one
        assert_eq!(result[0].source, "service-ok");
    }

    #[test]
    fn test_cloudwatch_logs_parser_empty_log_events_list() {
        let parser = CloudWatchLogsOtlpStdoutParser;
        let event = LogsEvent {
            aws_logs: AwsLogs {
                data: LogData {
                    owner: "123456789012".to_string(),
                    log_group: "/aws/lambda/test-func".to_string(),
                    log_stream: "stream1".to_string(),
                    message_type: "DATA_MESSAGE".to_string(),
                    subscription_filters: vec!["filter1".to_string()],
                    log_events: vec![], // Empty log events list
                },
            },
        };

        let result = parser.parse(event, "/aws/lambda/test-func").unwrap();
        assert!(result.is_empty());
    }
}
