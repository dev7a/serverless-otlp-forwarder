use anyhow::Result;
use aws_lambda_events::event::kinesis::KinesisEvent;
use lambda_otel_lite::{
    init_telemetry, LambdaSpanProcessor, OtelTracingLayer, SpanAttributes, SpanAttributesExtractor,
    TelemetryConfig,
};
use lambda_runtime::{tower::ServiceBuilder, Error as LambdaError, LambdaEvent, Runtime};
use opentelemetry::Value as OtelValue;
use opentelemetry_otlp::{Protocol, WithExportConfig};
use reqwest::Client as ReqwestClient;
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use serde::{Deserialize, Serialize};
use serverless_otlp_forwarder_core::{
    processor::process_event_batch, span_compactor::SpanCompactionConfig, InstrumentedHttpClient,
};
use std::collections::HashMap;
use std::sync::Arc;

// The specific parser for this Lambda
mod parser;
use parser::KinesisOtlpStdoutParser;

// Wrapper for KinesisEvent to implement SpanAttributesExtractor
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KinesisEventProcessorWrapper(KinesisEvent);

impl SpanAttributesExtractor for KinesisEventProcessorWrapper {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let mut attributes: HashMap<String, OtelValue> = HashMap::new();
        let records = &self.0.records;

        attributes.insert(
            "faas.trigger.type".to_string(),
            OtelValue::String("aws_kinesis".into()),
        );
        attributes.insert(
            "aws.kinesis.records.count".to_string(),
            OtelValue::I64(records.len() as i64),
        );

        if let Some(first_record) = records.first() {
            if let Some(event_source_arn) = &first_record.event_source_arn {
                // Attempt to extract stream name from ARN: arn:aws:kinesis:region:account-id:stream/stream-name
                if let Some(stream_name) = event_source_arn
                    .split(':')
                    .nth(5)
                    .and_then(|s| s.split('/').nth(1))
                {
                    attributes.insert(
                        "aws.kinesis.stream_name".to_string(),
                        OtelValue::String(stream_name.to_string().into()),
                    );
                }
                attributes.insert(
                    "aws.kinesis.event_source_arn".to_string(),
                    OtelValue::String(event_source_arn.clone().into()),
                );
            }
            if let Some(event_id) = &first_record.event_id {
                attributes.insert(
                    "aws.kinesis.event_id_prefix".to_string(),
                    OtelValue::String(event_id.split(':').next().unwrap_or("").to_string().into()),
                );
            }
        }

        SpanAttributes::builder()
            .span_name("kinesis_event_processor".to_string())
            .kind("consumer".to_string())
            .attributes(attributes)
            .build()
    }
}

async fn function_handler(
    event: LambdaEvent<KinesisEventProcessorWrapper>,
    http_client: Arc<InstrumentedHttpClient>,
) -> Result<(), LambdaError> {
    tracing::info!("otlp-stdout-kinesis-processor: function_handler started.");

    let source_identifier = event
        .payload
        .0
        .records
        .first()
        .and_then(|r| r.event_source_arn.as_ref())
        .map_or_else(|| "kinesis_stream_unknown".to_string(), |arn| arn.clone());

    let parser = KinesisOtlpStdoutParser;
    let compaction_config = SpanCompactionConfig::default();

    match process_event_batch(
        event.payload.0,
        &parser,
        &source_identifier,
        http_client.as_ref(), // Pass &InstrumentedOtlpClient
        &compaction_config,
    )
    .await
    {
        Ok(_) => {
            tracing::info!("otlp-stdout-kinesis-processor: Batch processed successfully.");
            Ok(())
        }
        Err(e) => {
            tracing::error!(error = %e, "otlp-stdout-kinesis-processor: Error processing event batch.");
            Err(LambdaError::from(e.to_string()))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    let otlp_http_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()?;

    let (_, completion_handler) = init_telemetry(
        TelemetryConfig::builder()
            .with_span_processor(
                LambdaSpanProcessor::builder()
                    .exporter(otlp_http_exporter)
                    .build(),
            )
            .build(),
    )
    .await?;
    tracing::info!(
        "lambda-otel-lite initialized with OTLP HTTP exporter for otlp-stdout-kinesis-processor."
    );

    let base_reqwest_client = ReqwestClient::new();
    // Wrap it with tracing middleware
    let client_with_middleware = ClientBuilder::new(base_reqwest_client)
        .with(TracingMiddleware::default())
        .build();
    // Wrap the ClientWithMiddleware in our newtype
    let instrumented_client = InstrumentedHttpClient::new(client_with_middleware);
    let http_client_for_forwarding = Arc::new(instrumented_client); // Now Arc<InstrumentedHttpClient>

    tracing::info!("Instrumented HTTP client for data forwarding initialized.");

    let service = ServiceBuilder::new()
        .layer(OtelTracingLayer::new(completion_handler))
        .service_fn(move |event: LambdaEvent<KinesisEventProcessorWrapper>| {
            let client_for_handler = Arc::clone(&http_client_for_forwarding);
            async move { function_handler(event, client_for_handler).await }
        });

    tracing::info!("otlp-stdout-kinesis-processor starting Lambda runtime.");
    Runtime::new(service).run().await
}

#[cfg(test)]
mod tests {
    // Kinesis processor specific tests (if any) would go here.
    // For now, main logic is tested in core and parser.rs has its own tests.
}
