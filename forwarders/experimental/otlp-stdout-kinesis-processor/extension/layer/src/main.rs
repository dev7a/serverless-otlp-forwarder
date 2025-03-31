use aws_sdk_kinesis::primitives::Blob;
use aws_sdk_kinesis::types::PutRecordsRequestEntry;
use aws_sdk_kinesis::Client as KinesisClient;
use lambda_extension::{
    service_fn, tracing, Error, Extension, LambdaEvent, LambdaTelemetry, LambdaTelemetryRecord,
    LogBuffering, NextEvent, SharedService,
};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

// Kinesis limit for a single record
const MAX_RECORD_SIZE_BYTES: usize = 1_048_576; // 1MB per record
const ENV_VAR_STREAM_NAME: &str = "OTLP_STDOUT_KINESIS_STREAM_NAME";
const RECORD_PREFIX: &str = r#"{"__otel_otlp_stdout":"#;

// Default buffering values
const DEFAULT_BUFFER_TIMEOUT_MS: u32 = 100;
const DEFAULT_BUFFER_MAX_BYTES: usize = 256 * 1024; // 256KB
const DEFAULT_BUFFER_MAX_ITEMS: usize = 1000;

// Environment variable names for buffering config
const ENV_VAR_BUFFER_TIMEOUT_MS: &str = "OTLP_STDOUT_KINESIS_BUFFER_TIMEOUT_MS";
const ENV_VAR_BUFFER_MAX_BYTES: &str = "OTLP_STDOUT_KINESIS_BUFFER_MAX_BYTES";
const ENV_VAR_BUFFER_MAX_ITEMS: &str = "OTLP_STDOUT_KINESIS_BUFFER_MAX_ITEMS";

#[derive(Debug)]
struct Config {
    kinesis_stream_name: String,
    buffer_timeout_ms: u32,
    buffer_max_bytes: usize,
    buffer_max_items: usize,
}

impl Config {
    fn from_env() -> Result<Self, Error> {
        // Parse required stream name
        let kinesis_stream_name = env::var(ENV_VAR_STREAM_NAME).map_err(|e| {
            Error::from(format!(
                "Failed to get stream name: {}. Make sure to set the {} environment variable.",
                e, ENV_VAR_STREAM_NAME
            ))
        })?;

        // Parse optional buffering config with defaults
        let buffer_timeout_ms = env::var(ENV_VAR_BUFFER_TIMEOUT_MS)
            .map(|v| v.parse::<u32>().unwrap_or(DEFAULT_BUFFER_TIMEOUT_MS))
            .unwrap_or(DEFAULT_BUFFER_TIMEOUT_MS);

        let buffer_max_bytes = env::var(ENV_VAR_BUFFER_MAX_BYTES)
            .map(|v| v.parse::<usize>().unwrap_or(DEFAULT_BUFFER_MAX_BYTES))
            .unwrap_or(DEFAULT_BUFFER_MAX_BYTES);

        let buffer_max_items = env::var(ENV_VAR_BUFFER_MAX_ITEMS)
            .map(|v| v.parse::<usize>().unwrap_or(DEFAULT_BUFFER_MAX_ITEMS))
            .unwrap_or(DEFAULT_BUFFER_MAX_ITEMS);

        tracing::debug!(
            "Configuration: buffer_timeout_ms={}, buffer_max_bytes={}, buffer_max_items={}",
            buffer_timeout_ms,
            buffer_max_bytes,
            buffer_max_items
        );

        Ok(Self {
            kinesis_stream_name,
            buffer_timeout_ms,
            buffer_max_bytes,
            buffer_max_items,
        })
    }
}

#[derive(Default)]
struct KinesisBatch {
    records: Vec<PutRecordsRequestEntry>,
}

impl KinesisBatch {
    fn add_record(&mut self, record: String) -> Result<(), Error> {
        if record.len() > MAX_RECORD_SIZE_BYTES {
            tracing::warn!(
                "Record size {} bytes exceeds maximum size of {} bytes, skipping",
                record.len(),
                MAX_RECORD_SIZE_BYTES
            );
            return Ok(());
        }

        match PutRecordsRequestEntry::builder()
            .data(Blob::new(record))
            .partition_key(Uuid::new_v4().to_string())
            .build()
        {
            Ok(entry) => {
                self.records.push(entry);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to build Kinesis record entry: {}", e);
                Err(Error::from(e))
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    fn clear(&mut self) {
        self.records.clear();
    }
}

// Application state
struct AppState {
    kinesis_client: KinesisClient,
    stream_name: String,
    batch: Mutex<KinesisBatch>,
}

impl AppState {
    async fn flush_batch(&self) -> Result<(), Error> {
        let mut batch = self.batch.lock().await;
        if batch.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            "Sending batch of {} records to Kinesis",
            batch.records.len()
        );

        let result = self
            .kinesis_client
            .put_records()
            .stream_name(&self.stream_name)
            .set_records(Some(batch.records.clone()))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Kinesis batch error: {}", e);
                Error::from(format!("Failed to send records to Kinesis: {}", e))
            })?;

        // Check if any records failed
        let failed_count = result.failed_record_count.unwrap_or(0);
        if failed_count > 0 {
            tracing::warn!("Failed to put {} records", failed_count);

            // Log details of failed records
            let records = result.records();
            for (i, record) in records.iter().enumerate() {
                if let Some(error_code) = &record.error_code {
                    tracing::warn!(
                        "Record {} failed with error: {} - {}",
                        i,
                        error_code,
                        record
                            .error_message
                            .as_deref()
                            .unwrap_or("No error message")
                    );
                }
            }
        } else {
            tracing::debug!("Successfully sent all records to Kinesis");
        }

        batch.clear();
        Ok(())
    }
}

fn json_log(event_type: &str, record: &serde_json::Value, timestamp: &str) {
    // Add type and timestamp to the JSON
    let log_data = json!({
        "timestamp": timestamp,
        "level": "INFO",
        "type": event_type,
        "record": record
    });
    // Print the formatted JSON
    println!(
        "{}",
        serde_json::to_string(&log_data).unwrap_or_else(|_| String::from("{}"))
    );
}

async fn telemetry_handler(
    events: Vec<LambdaTelemetry>,
    state: Arc<AppState>,
) -> Result<(), Error> {
    for event in events {
        // Use the event's timestamp for all logging
        let timestamp = event.time.to_rfc3339();

        match event.record {
            LambdaTelemetryRecord::Function(record) => {
                // Process OTLP records for batching
                if record.starts_with(RECORD_PREFIX) {
                    let mut batch = state.batch.lock().await;
                    batch.add_record(record)?;
                }
                // Skip other function logs
            }
            LambdaTelemetryRecord::PlatformInitStart { .. } => {
                let record =
                    serde_json::to_value(&event.record).unwrap_or_else(|_| serde_json::json!({}));
                json_log("platform.initStart", &record, &timestamp);
            }
            LambdaTelemetryRecord::PlatformRuntimeDone { .. } => {
                let record =
                    serde_json::to_value(&event.record).unwrap_or_else(|_| serde_json::json!({}));
                json_log("platform.runtimeDone", &record, &timestamp);
            }
            LambdaTelemetryRecord::PlatformReport { .. } => {
                let record =
                    serde_json::to_value(&event.record).unwrap_or_else(|_| serde_json::json!({}));
                json_log("platform.report", &record, &timestamp);
            }
            LambdaTelemetryRecord::PlatformInitReport { .. } => {
                let record =
                    serde_json::to_value(&event.record).unwrap_or_else(|_| serde_json::json!({}));
                json_log("platform.initReport", &record, &timestamp);
            }
            LambdaTelemetryRecord::PlatformLogsDropped { .. } => {
                let record =
                    serde_json::to_value(&event.record).unwrap_or_else(|_| serde_json::json!({}));
                // Use a different level for warnings
                let log_data = json!({
                    "timestamp": timestamp,
                    "level": "WARN",
                    "type": "platform.logsDropped",
                    "record": record
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&log_data).unwrap_or_else(|_| String::from("{}"))
                );
            }
            // Skip all other record types
            _ => {}
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    tracing::debug!("Starting OTLP Stdout Kinesis Extension");

    // Load configuration
    let config = Config::from_env()?;

    // Initialize AWS services
    let aws_config = aws_config::from_env().load().await;
    let kinesis_client = KinesisClient::new(&aws_config);

    // Create app state with batch buffer
    let app_state = Arc::new(AppState {
        kinesis_client,
        stream_name: config.kinesis_stream_name,
        batch: Mutex::new(KinesisBatch::default()),
    });

    let telemetry_state = app_state.clone();
    let events_state = app_state.clone();

    // Create telemetry handler function
    let handler_fn = move |events: Vec<LambdaTelemetry>| {
        let state = telemetry_state.clone();
        async move { telemetry_handler(events, state).await }
    };

    // Create events processor to handle INVOKE/SHUTDOWN
    let events_processor = service_fn(|event: LambdaEvent| {
        let state = events_state.clone();
        async move {
            match event.next {
                NextEvent::Invoke(_) => {
                    tracing::debug!("Received INVOKE event, flushing batched records");
                    if let Err(e) = state.flush_batch().await {
                        tracing::error!("Error flushing batch on INVOKE: {}", e);
                    }
                }
                NextEvent::Shutdown(_) => {
                    tracing::info!("Received SHUTDOWN event, ensuring all records are flushed");
                    if let Err(e) = state.flush_batch().await {
                        tracing::error!("Error flushing batch on SHUTDOWN: {}", e);
                    }
                }
            }
            Ok::<(), Error>(())
        }
    });

    // Create and run extension with both processors
    Extension::new()
        .with_events(&["INVOKE", "SHUTDOWN"])
        .with_events_processor(events_processor)
        .with_telemetry_processor(SharedService::new(service_fn(handler_fn)))
        .with_telemetry_types(&["function", "platform"])
        .with_telemetry_buffering(LogBuffering {
            timeout_ms: config.buffer_timeout_ms as usize,
            max_bytes: config.buffer_max_bytes,
            max_items: config.buffer_max_items,
        })
        .run()
        .await?;

    Ok(())
}
