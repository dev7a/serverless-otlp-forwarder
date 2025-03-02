use opentelemetry::{otel_debug, otel_warn};
use std::env;

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
/// The mode can be configured using the `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE` environment variable:
/// - "sync" for Sync mode (default)
/// - "async" for Async mode
/// - "finalize" for Finalize mode
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
/// let mode = ProcessorMode::from_env();
/// assert!(matches!(mode, ProcessorMode::Async));
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

impl ProcessorMode {
    /// Create ProcessorMode from environment variable.
    ///
    /// Uses LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE environment variable.
    /// Defaults to Sync mode if not set or invalid.
    pub fn from_env() -> Self {
        match env::var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE")
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Ok("sync") => {
                otel_debug!(
                    name: "ProcessorMode.from_env",
                    message = "using sync processor mode"
                );
                ProcessorMode::Sync
            }
            Ok("async") => {
                otel_debug!(
                    name: "ProcessorMode.from_env",
                    message = "using async processor mode"
                );
                ProcessorMode::Async
            }
            Ok("finalize") => {
                otel_debug!(
                    name: "ProcessorMode.from_env",
                    message = "using finalize processor mode"
                );
                ProcessorMode::Finalize
            }
            Ok(value) => {
                otel_warn!(
                    name: "ProcessorMode.from_env",
                    message = format!("invalid processor mode: {}, defaulting to sync", value)
                );
                ProcessorMode::Sync
            }
            Err(_) => {
                otel_debug!(
                    name: "ProcessorMode.from_env",
                    message = "no processor mode set, defaulting to sync"
                );
                ProcessorMode::Sync
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_mode_from_env() {
        // Test default when not set
        env::remove_var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE");
        assert!(matches!(ProcessorMode::from_env(), ProcessorMode::Sync));

        // Test sync mode
        env::set_var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", "sync");
        assert!(matches!(ProcessorMode::from_env(), ProcessorMode::Sync));

        // Test async mode
        env::set_var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", "async");
        assert!(matches!(ProcessorMode::from_env(), ProcessorMode::Async));

        // Test finalize mode
        env::set_var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", "finalize");
        assert!(matches!(ProcessorMode::from_env(), ProcessorMode::Finalize));

        // Test invalid value
        env::set_var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", "invalid");
        assert!(matches!(ProcessorMode::from_env(), ProcessorMode::Sync));
    }
}
