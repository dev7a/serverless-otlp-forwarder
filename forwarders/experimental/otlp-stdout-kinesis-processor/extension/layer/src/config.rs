use lambda_extension::{tracing, Error};
use std::env;

// Environment variable name for Kinesis stream
pub const ENV_VAR_STREAM_NAME: &str = "OTEL_LITE_EXTENSION_STREAM_NAME";

// Default buffering values
pub const DEFAULT_BUFFER_TIMEOUT_MS: u32 = 100;
pub const DEFAULT_BUFFER_MAX_BYTES: usize = 256 * 1024; // 256KB
pub const DEFAULT_BUFFER_MAX_ITEMS: usize = 1000;

// Environment variable names for buffering config
pub const ENV_VAR_BUFFER_TIMEOUT_MS: &str = "OTEL_LITE_EXTENSION_BUFFER_TIMEOUT_MS";
pub const ENV_VAR_BUFFER_MAX_BYTES: &str = "OTEL_LITE_EXTENSION_BUFFER_MAX_BYTES";
pub const ENV_VAR_BUFFER_MAX_ITEMS: &str = "OTEL_LITE_EXTENSION_BUFFER_MAX_ITEMS";

// Environment variable name for enabling platform telemetry
pub const ENV_VAR_ENABLE_PLATFORM_TELEMETRY: &str = "OTEL_LITE_EXTENSION_ENABLE_PLATFORM_TELEMETRY";

#[derive(Debug, Clone)]
pub struct Config {
    pub kinesis_stream_name: Option<String>,
    pub buffer_timeout_ms: u32,
    pub buffer_max_bytes: usize,
    pub buffer_max_items: usize,
    pub enable_platform_telemetry: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let kinesis_stream_name = env::var(ENV_VAR_STREAM_NAME).ok();

        if kinesis_stream_name.is_none() {
            tracing::info!(
                "extension: {} not set, disabling Kinesis output. Will write recordsto stdout.",
                ENV_VAR_STREAM_NAME
            );
        } else {
            tracing::info!(
                "extension: Kinesis stream name set: {}",
                kinesis_stream_name.as_ref().unwrap()
            );
        }

        let buffer_timeout_ms = env::var(ENV_VAR_BUFFER_TIMEOUT_MS)
            .map(|v| v.parse::<u32>().unwrap_or(DEFAULT_BUFFER_TIMEOUT_MS))
            .unwrap_or(DEFAULT_BUFFER_TIMEOUT_MS);

        let buffer_max_bytes = env::var(ENV_VAR_BUFFER_MAX_BYTES)
            .map(|v| v.parse::<usize>().unwrap_or(DEFAULT_BUFFER_MAX_BYTES))
            .unwrap_or(DEFAULT_BUFFER_MAX_BYTES);

        let buffer_max_items = env::var(ENV_VAR_BUFFER_MAX_ITEMS)
            .map(|v| v.parse::<usize>().unwrap_or(DEFAULT_BUFFER_MAX_ITEMS))
            .unwrap_or(DEFAULT_BUFFER_MAX_ITEMS);

        let enable_platform_telemetry = env::var(ENV_VAR_ENABLE_PLATFORM_TELEMETRY)
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        tracing::debug!(
            "Configuration: buffer_timeout_ms={}, buffer_max_bytes={}, buffer_max_items={}, enable_platform_telemetry={}",
            buffer_timeout_ms,
            buffer_max_bytes,
            buffer_max_items,
            enable_platform_telemetry
        );

        Ok(Self {
            kinesis_stream_name,
            buffer_timeout_ms,
            buffer_max_bytes,
            buffer_max_items,
            enable_platform_telemetry,
        })
    }
}
