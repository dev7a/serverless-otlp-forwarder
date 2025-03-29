use crate::constants;
use crate::logger::Logger;

/// Module-specific logger
static LOGGER: Logger = Logger::const_new("mode");

use std::{env, fmt};

/// Controls how spans are processed and exported.
///
/// This enum determines when and how OpenTelemetry spans are flushed from the buffer
/// to the configured exporter. Each mode offers different tradeoffs between latency,
/// reliability, and flexibility.
///
/// # Modes
///
/// - `Sync`: Immediate flush in handler thread
///   - Spans are flushed before handler returns
///   - Direct export without extension coordination
///   - May be more efficient for small payloads and low memory configurations
///   - Guarantees span delivery before response
///
/// - `Async`: Flush via Lambda extension
///   - Spans are flushed after handler returns
///   - Requires coordination with extension process
///   - Additional overhead from IPC with extension
///   - Provides retry capabilities through extension
///
/// - `Finalize`: Delegated to processor
///   - Spans handled by configured processor
///   - Compatible with BatchSpanProcessor
///   - Best for custom export strategies
///   - Full control over export timing
///
/// # Configuration
///
/// The mode can be configured in two ways:
///
/// 1. Using the `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE` environment variable:
///    - "sync" for Sync mode (default)
///    - "async" for Async mode
///    - "finalize" for Finalize mode
///
/// 2. Programmatically through `TelemetryConfig`:
///    ```no_run
///    use lambda_otel_lite::{ProcessorMode, TelemetryConfig};
///    
///    let config = TelemetryConfig::builder()
///        .processor_mode(ProcessorMode::Async)
///        .build();
///    ```
///
/// The environment variable takes precedence over programmatic configuration.
///
/// # Example
///
/// ```no_run
/// use lambda_otel_lite::ProcessorMode;
/// use std::env;
///
/// // Set mode via environment variable
/// env::set_var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", "async");
///
/// // Get mode from environment
/// let mode = ProcessorMode::resolve(None);
/// assert!(matches!(mode, ProcessorMode::Async));
///
/// // Programmatically provide a default but let environment override it
/// let mode = ProcessorMode::resolve(Some(ProcessorMode::Sync));
/// assert!(matches!(mode, ProcessorMode::Async)); // Environment still takes precedence
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessorMode {
    /// Synchronous flush in handler thread. Best for development and debugging.
    Sync,
    /// Asynchronous flush via extension. Best for production use to minimize latency.
    Async,
    /// Let processor handle flushing. Best with BatchSpanProcessor for custom export strategies.
    Finalize,
}

impl fmt::Display for ProcessorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessorMode::Sync => write!(f, "sync"),
            ProcessorMode::Async => write!(f, "async"),
            ProcessorMode::Finalize => write!(f, "finalize"),
        }
    }
}

impl ProcessorMode {
    /// Resolve processor mode from environment variable or provided configuration.
    ///
    /// If LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE environment variable is set, it takes precedence.
    /// Otherwise, uses the provided mode or defaults to Sync mode if neither is set.
    pub fn resolve(config_mode: Option<ProcessorMode>) -> Self {
        // Environment variable takes precedence if set
        let result = match env::var(constants::env_vars::PROCESSOR_MODE)
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Ok("sync") => ProcessorMode::Sync,
            Ok("async") => ProcessorMode::Async,
            Ok("finalize") => ProcessorMode::Finalize,
            Ok(value) => {
                LOGGER.warn(format!(
                    "ProcessorMode.resolve: invalid processor mode in env: {}, using config or default",
                    value
                ));
                config_mode.unwrap_or(ProcessorMode::Sync)
            }
            Err(_) => {
                // No environment variable set, use config mode or default
                config_mode.unwrap_or(ProcessorMode::Sync)
            }
        };

        // Log the resolved mode
        LOGGER.debug(format!(
            "ProcessorMode.resolve: using {} processor mode",
            result
        ));

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    // Helper function to set processor mode environment variable
    fn set_processor_mode(value: Option<&str>) {
        match value {
            Some(v) => env::set_var(constants::env_vars::PROCESSOR_MODE, v),
            None => env::remove_var(constants::env_vars::PROCESSOR_MODE),
        }
    }

    #[test]
    #[serial]
    fn test_processor_mode_env_only() {
        // Default to sync mode (env var not set)
        set_processor_mode(None);
        assert!(matches!(ProcessorMode::resolve(None), ProcessorMode::Sync));

        // Explicit mode tests
        let test_cases = [
            ("sync", ProcessorMode::Sync),
            ("async", ProcessorMode::Async),
            ("finalize", ProcessorMode::Finalize),
            ("invalid", ProcessorMode::Sync), // Invalid mode defaults to sync
        ];

        for (env_value, expected_mode) in test_cases {
            set_processor_mode(Some(env_value));
            let result = ProcessorMode::resolve(None);
            assert_eq!(result, expected_mode, "Failed for env value: {}", env_value);
        }
    }

    #[test]
    #[serial]
    fn test_processor_mode_resolve() {
        // Test environment variable precedence over config
        let precedence_tests = [
            // (env_value, config_value, expected)
            (
                Some("sync"),
                Some(ProcessorMode::Async),
                ProcessorMode::Sync,
            ),
            (
                Some("async"),
                Some(ProcessorMode::Sync),
                ProcessorMode::Async,
            ),
            (
                Some("invalid"),
                Some(ProcessorMode::Finalize),
                ProcessorMode::Finalize,
            ),
            (None, Some(ProcessorMode::Async), ProcessorMode::Async),
            (None, None, ProcessorMode::Sync),
        ];

        for (env_value, config_mode, expected) in precedence_tests {
            set_processor_mode(env_value);
            let result = ProcessorMode::resolve(config_mode.clone());
            assert_eq!(
                result, expected,
                "Failed for env: {:?}, config: {:?}",
                env_value, config_mode
            );
        }
    }
}
