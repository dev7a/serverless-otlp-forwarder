mod otlp;
mod parser;

use anyhow::Result;
use aws_lambda_events::event::cloudwatch_logs::LogsEvent;
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

use parser::AwsAppSignalSpanParser;

// Wrapper for LogsEvent for this specific processor to implement SpanAttributesExtractor
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AwsSpanProcessorEventWrapper(LogsEvent);

impl SpanAttributesExtractor for AwsSpanProcessorEventWrapper {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let mut attributes: HashMap<String, OtelValue> = HashMap::new();
        let log_data = &self.0.aws_logs.data;

        attributes.insert(
            "faas.trigger.type".to_string(),
            OtelValue::String("cloudwatch_logs".into()),
        );
        attributes.insert(
            "aws.cloudwatch.log_group".to_string(),
            OtelValue::String(log_data.log_group.clone().into()),
        );
        attributes.insert(
            "aws.cloudwatch.log_stream".to_string(),
            OtelValue::String(log_data.log_stream.clone().into()),
        );
        attributes.insert(
            "aws.cloudwatch.owner".to_string(),
            OtelValue::String(log_data.owner.clone().into()),
        );
        attributes.insert(
            "aws.cloudwatch.events.count".to_string(),
            OtelValue::I64(log_data.log_events.len() as i64),
        );
        attributes.insert(
            "processor.type".to_string(),
            OtelValue::String("aws_appsignal_span_processor".into()),
        );

        SpanAttributes::builder()
            .span_name(format!("aws_span_processor_{}", log_data.log_group.clone()))
            .kind("consumer".to_string())
            .attributes(attributes)
            .build()
    }
}

async fn function_handler(
    event: LambdaEvent<AwsSpanProcessorEventWrapper>,
    http_client: Arc<InstrumentedHttpClient>,
) -> Result<(), LambdaError> {
    tracing::info!("aws-span-processor: function_handler started.");

    let log_group = event.payload.0.aws_logs.data.log_group.clone();
    let parser = AwsAppSignalSpanParser;
    let compaction_config = SpanCompactionConfig::default();

    match process_event_batch(
        event.payload.0,
        &parser,
        &log_group,
        http_client.as_ref(),
        &compaction_config,
    )
    .await
    {
        Ok(_) => {
            tracing::info!("aws-span-processor: Batch processed successfully.");
            Ok(())
        }
        Err(e) => {
            tracing::error!(error = %e, "aws-span-processor: Error processing event batch.");
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
    tracing::info!("lambda-otel-lite initialized with OTLP HTTP exporter for aws-span-processor.");

    let base_reqwest_client = ReqwestClient::new();
    let client_with_middleware = ClientBuilder::new(base_reqwest_client)
        .with(TracingMiddleware::default())
        .build();
    let instrumented_client = InstrumentedHttpClient::new(client_with_middleware);
    let http_client_for_forwarding = Arc::new(instrumented_client);

    tracing::info!("Instrumented HTTP client for data forwarding initialized.");

    let service = ServiceBuilder::new()
        .layer(OtelTracingLayer::new(completion_handler))
        .service_fn(move |event: LambdaEvent<AwsSpanProcessorEventWrapper>| {
            let client_for_handler = Arc::clone(&http_client_for_forwarding);
            async move { function_handler(event, client_for_handler).await }
        });

    tracing::info!("aws-span-processor starting Lambda runtime.");
    Runtime::new(service).run().await
}

#[cfg(test)]
mod tests {
    // Main logic is tested in the core library and the local parser.rs.
    // Tests for otlp.rs (the JSON to Protobuf conversion) should be within otlp.rs itself.
    // This main.rs primarily orchestrates, so unit tests here would be minimal,
    // focusing on integration if any (e.g., ensuring correct components are wired).
}
