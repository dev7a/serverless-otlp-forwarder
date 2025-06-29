use crate::core_parser::EventParser;
use crate::http_sender::{send_telemetry_batch, HttpOtlpForwarderClient};
use crate::span_compactor::{compact_telemetry_payloads, SpanCompactionConfig};
use anyhow::Result;
use tracing::{debug, error, info, instrument};

/// Processes a batch of events from a specific AWS Lambda event source.
///
/// This function orchestrates the parsing, compaction, and sending of telemetry data.
///
/// # Type Parameters
///
/// * `E`: The type of the raw event input (e.g., `aws_lambda_events::event::cloudwatch_logs::LogsEvent`).
/// * `P`: The type of the parser that implements `EventParser<EventInput = E>`.
/// * `C`: The type of the HTTP client that implements `HttpOtlpForwarderClient`.
///
/// # Arguments
///
/// * `event_payload`: The raw event payload from AWS Lambda.
/// * `parser`: An instance of the event parser for the specific event type.
/// * `source_identifier`: A string identifying the source of the event (e.g., log group name, Kinesis stream name).
/// * `http_client`: A reference to the HTTP client for making HTTP requests.
/// * `compaction_config`: Configuration for span compaction.
///
#[instrument(name="processor/process_event_batch", skip_all, fields(source = %source_identifier))]
pub async fn process_event_batch<
    E,
    P: EventParser<EventInput = E> + Sync + Send,
    C: HttpOtlpForwarderClient,
>(
    event_payload: E,
    parser: &P,
    source_identifier: &str,
    http_client: &C,
    compaction_config: &SpanCompactionConfig,
) -> Result<()> {
    info!("Starting to process event batch.");

    // 1. Parse the event payload
    let telemetry_items = match parser.parse(event_payload, source_identifier) {
        Ok(items) => items,
        Err(e) => {
            error!(error = %e, "Failed to parse event payload.");
            return Err(e.context("Event parsing failed"));
        }
    };

    if telemetry_items.is_empty() {
        info!("No telemetry items to process after parsing.");
        return Ok(());
    }
    debug!(
        "Successfully parsed {} telemetry items.",
        telemetry_items.len()
    );

    // 2. Compact the telemetry items into a single TelemetryData object
    let compacted_telemetry = match compact_telemetry_payloads(telemetry_items, compaction_config) {
        Ok(compacted) => compacted,
        Err(e) => {
            error!(error = %e, "Failed to compact telemetry items.");
            return Err(e.context("Telemetry compaction failed"));
        }
    };
    debug!("Successfully compacted telemetry items.");

    // 3. Send the compacted telemetry batch
    match send_telemetry_batch(http_client, compacted_telemetry).await {
        Ok(_) => {
            info!("Successfully sent telemetry batch.");
            Ok(())
        }
        Err(e) => {
            error!(error = %e, "Failed to send telemetry batch.");
            Err(e.context("Sending telemetry batch failed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_parser::EventParser;
    use crate::telemetry::TelemetryData;
    use anyhow::anyhow;
    use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
    use prost::Message;
    use reqwest::Client as ReqwestClient;
    use sealed_test::prelude::*;
    use std::env;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct EnvVarGuard {
        name: String,
        original_value: Option<String>,
    }

    impl EnvVarGuard {
        #[allow(dead_code)]
        fn set(name: &str, value: &str) -> Self {
            let original_value = env::var(name).ok();
            env::set_var(name, value);
            Self {
                name: name.to_string(),
                original_value,
            }
        }

        #[allow(dead_code)]
        fn remove(name: &str) -> Self {
            let original_value = env::var(name).ok();
            env::remove_var(name);
            Self {
                name: name.to_string(),
                original_value,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(val) = &self.original_value {
                env::set_var(&self.name, val);
            } else {
                env::remove_var(&self.name);
            }
        }
    }

    #[derive(Clone, Debug)]
    struct MockEventInput {
        records: Vec<String>,
        produce_valid_otlp_for_compaction: bool,
    }

    struct MockSuccessfulParser;
    impl EventParser for MockSuccessfulParser {
        type EventInput = MockEventInput;
        fn parse(
            &self,
            event_payload: Self::EventInput,
            _source_identifier: &str,
        ) -> Result<Vec<TelemetryData>> {
            let items = event_payload
                .records
                .into_iter()
                .enumerate()
                .map(|(i, r)| {
                    let payload_bytes = if event_payload.produce_valid_otlp_for_compaction {
                        let request = ExportTraceServiceRequest {
                            resource_spans: vec![
                                opentelemetry_proto::tonic::trace::v1::ResourceSpans {
                                    scope_spans: vec![
                                        opentelemetry_proto::tonic::trace::v1::ScopeSpans {
                                            spans: vec![
                                                opentelemetry_proto::tonic::trace::v1::Span {
                                                    name: format!("test-span-{r}-{i}"),
                                                    ..Default::default()
                                                },
                                            ],
                                            ..Default::default()
                                        },
                                    ],
                                    ..Default::default()
                                },
                            ],
                        };
                        request.encode_to_vec()
                    } else {
                        r.into_bytes()
                    };

                    TelemetryData {
                        payload: payload_bytes,
                        source: "mock_source".to_string(),
                        endpoint: "mock_endpoint".to_string(),
                        content_type: "application/x-protobuf".to_string(),
                        content_encoding: None,
                    }
                })
                .collect();
            Ok(items)
        }
    }

    struct MockFailingParser;
    impl EventParser for MockFailingParser {
        type EventInput = MockEventInput;
        fn parse(
            &self,
            _event_payload: Self::EventInput,
            _source_identifier: &str,
        ) -> Result<Vec<TelemetryData>> {
            Err(anyhow!("Mock parser failed intentionally"))
        }
    }

    struct MockEmptyParser;
    impl EventParser for MockEmptyParser {
        type EventInput = MockEventInput;
        fn parse(
            &self,
            _event_payload: Self::EventInput,
            _source_identifier: &str,
        ) -> Result<Vec<TelemetryData>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_process_event_batch_success() {
        let server = MockServer::start().await;
        let http_client = ReqwestClient::new();
        let parser = MockSuccessfulParser;
        let event = MockEventInput {
            records: vec!["data1".to_string(), "data2".to_string()],
            produce_valid_otlp_for_compaction: true,
        };
        let compaction_config = SpanCompactionConfig::default();

        Mock::given(method("POST"))
            .and(path("/v1/traces"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let _g = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}/v1/traces", server.uri()),
        );

        let result = process_event_batch(
            event,
            &parser,
            "test_source",
            &http_client,
            &compaction_config,
        )
        .await;

        assert!(
            result.is_ok(),
            "process_event_batch failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_process_event_batch_parser_fails() {
        let http_client = ReqwestClient::new();
        let parser = MockFailingParser;
        let event = MockEventInput {
            records: vec!["data1".to_string()],
            produce_valid_otlp_for_compaction: false,
        };
        let compaction_config = SpanCompactionConfig::default();

        let _g = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT");
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_ENDPOINT");

        let result = process_event_batch(
            event,
            &parser,
            "test_source",
            &http_client,
            &compaction_config,
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Event parsing failed"));
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_process_event_batch_empty_after_parsing() {
        let http_client = ReqwestClient::new();
        let parser = MockEmptyParser;
        let event = MockEventInput {
            records: vec![],
            produce_valid_otlp_for_compaction: false,
        };
        let compaction_config = SpanCompactionConfig::default();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let _g = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}/v1/traces", server.uri()),
        );

        let result = process_event_batch(
            event,
            &parser,
            "test_source_empty_parse",
            &http_client,
            &compaction_config,
        )
        .await;

        assert!(
            result.is_ok(),
            "Expected Ok for empty telemetry items, got: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_process_event_batch_sender_fails() {
        let server = MockServer::start().await;
        let http_client = ReqwestClient::new();
        let parser = MockSuccessfulParser;
        let event = MockEventInput {
            records: vec!["data1".to_string()],
            produce_valid_otlp_for_compaction: true,
        };
        let compaction_config = SpanCompactionConfig::default();

        Mock::given(method("POST"))
            .and(path("/v1/traces"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;

        let _g = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}/v1/traces", server.uri()),
        );

        let result = process_event_batch(
            event,
            &parser,
            "test_source",
            &http_client,
            &compaction_config,
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Sending telemetry batch failed"));
    }
}
