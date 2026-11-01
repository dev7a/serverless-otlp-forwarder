//! AWS Lambda function that forwards CloudWatch log wrapped OTLP records to OpenTelemetry collectors.
//!
//! This Lambda function:
//! 1. Receives CloudWatch log events in otlp-stdout format
//! 2. Decodes and decompresses the log data
//! 3. Converts logs to TelemetryData
//! 4. Compacts multiple telemetry items into batches
//! 5. Forwards the batched data to an OTLP collector endpoint
//!
//! The function supports:
//! - Configurable OTLP collector endpoint via environment variables
//! - Custom headers and authentication via environment variables
//! - Base64 encoded payloads
//! - Gzip compressed data
//! - Span compaction and batching for efficiency
//! - Self-instrumentation with OpenTelemetry tracing

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

use std::{collections::HashMap, sync::Arc};

// The specific parser for this Lambda, defined in the local parser.rs
mod parser;
use parser::CloudWatchLogsOtlpStdoutParser;

// Define a wrapper for LogsEvent to implement SpanAttributesExtractor
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogsEventProcessorWrapper(LogsEvent);

impl SpanAttributesExtractor for LogsEventProcessorWrapper {
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

        SpanAttributes::builder()
            .span_name(format!("log {}", log_data.log_group.clone()))
            .kind("consumer".to_string()) // As per OpenTelemetry semantic conventions for messaging
            .attributes(attributes)
            .build()
    }
}

// Main Lambda function handler - simplified to use the core library
async fn function_handler(
    event: LambdaEvent<LogsEventProcessorWrapper>,
    http_client: Arc<InstrumentedHttpClient>,
) -> Result<(), LambdaError> {
    tracing::info!("otlp-stdout-logs-processor: function_handler started.");

    let log_group = event.payload.0.aws_logs.data.log_group.clone();

    let parser = CloudWatchLogsOtlpStdoutParser;
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
            tracing::info!("otlp-stdout-logs-processor: Batch processed successfully.");
            Ok(())
        }
        Err(e) => {
            tracing::error!(error = %e, "otlp-stdout-logs-processor: Error processing event batch.");
            Err(LambdaError::from(e.to_string()))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    // Configure an OTLP HTTP exporter for the Lambda's own telemetry
    let otlp_http_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http() // Enables HTTP transport, uses default reqwest client from features
        .with_protocol(Protocol::HttpBinary)
        // Endpoint and Headers will be picked up from OTEL_EXPORTER_OTLP_TRACES_ENDPOINT / _HEADERS env vars by the SDK
        .build()?;

    let (_, completion_handler) = init_telemetry(
        TelemetryConfig::builder()
            .with_span_processor(
                LambdaSpanProcessor::builder() // Use LambdaSpanProcessor
                    .exporter(otlp_http_exporter) // Configure with the HTTP exporter
                    .build(),
            )
            .build(),
    )
    .await?;
    tracing::info!(
        "lambda-otel-lite initialized with OTLP HTTP exporter for otlp-stdout-logs-processor."
    );

    // Create a base reqwest client
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
        .service_fn(move |event: LambdaEvent<LogsEventProcessorWrapper>| {
            let client_for_handler = Arc::clone(&http_client_for_forwarding);
            async move { function_handler(event, client_for_handler).await }
        });

    tracing::info!("otlp-stdout-logs-processor starting Lambda runtime.");
    Runtime::new(service).run().await
}

#[cfg(test)]
mod tests {
    // Intentionally empty for now.
}
