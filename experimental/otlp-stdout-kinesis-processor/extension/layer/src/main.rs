use aws_sdk_kinesis::primitives::Blob;
use aws_sdk_kinesis::Client as KinesisClient;
use lambda_extension::{
    service_fn, tracing, Error, Extension, LambdaTelemetry, LambdaTelemetryRecord, SharedService,
};
use std::env;
use std::sync::Arc;
use uuid::Uuid;

// Kinesis limit for a single record
const MAX_RECORD_SIZE_BYTES: usize = 1_048_576; // 1MB per record

#[derive(Debug)]
struct ExtensionConfig {
    kinesis_stream_name: String,
}

impl ExtensionConfig {
    fn from_env() -> Result<Self, Error> {
        Ok(Self {
            kinesis_stream_name: env::var("OTLP_STDOUT_KINESIS_STREAM_NAME").map_err(|e| {
                Error::from(format!(
                    "Failed to get stream name: {}. Ensure that the OTLP_STDOUT_KINESIS_STREAM_NAME environment variable is set.",
                    e
                ))
            })?,
        })
    }
}

struct TelemetryHandler {
    kinesis_client: Arc<KinesisClient>,
    stream_name: String,
}

impl TelemetryHandler {
    async fn new() -> Result<Self, Error> {
        let config = ExtensionConfig::from_env()?;

        let aws_config = aws_config::from_env().load().await;

        let kinesis_client = Arc::new(KinesisClient::new(&aws_config));

        Ok(Self {
            kinesis_client,
            stream_name: config.kinesis_stream_name,
        })
    }

    async fn send_record(&self, record: String) -> Result<(), Error> {
        if record.len() > MAX_RECORD_SIZE_BYTES {
            tracing::warn!(
                "Record size {} bytes exceeds maximum size of {} bytes, skipping",
                record.len(),
                MAX_RECORD_SIZE_BYTES
            );
            return Ok(());
        }

        self.kinesis_client
            .put_record()
            .stream_name(&self.stream_name)
            .data(Blob::new(record))
            .partition_key(Uuid::new_v4().to_string())
            .send()
            .await
            .map_err(|e| Error::from(format!("Failed to send record to Kinesis: {}", e)))?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    tracing::info!("Starting Rust extension");

    let handler = Arc::new(TelemetryHandler::new().await?);
    let handler_clone = handler.clone();

    let telemetry_processor =
        SharedService::new(service_fn(move |events: Vec<LambdaTelemetry>| {
            let handler = handler_clone.clone();
            async move {
                for event in events {
                    if let LambdaTelemetryRecord::Function(record) = event.record {
                        if record.starts_with(r#"{"__otel_otlp_stdout":"#) {
                            handler.send_record(record).await?;
                        }
                    }
                }
                Ok::<(), Error>(())
            }
        }));

    Extension::new()
        .with_telemetry_processor(telemetry_processor)
        .with_telemetry_types(&["function"])
        .run()
        .await?;

    Ok(())
}
