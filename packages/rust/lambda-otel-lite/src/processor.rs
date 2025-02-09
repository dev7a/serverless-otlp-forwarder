//! Span processor implementation optimized for AWS Lambda functions.
//!
//! This module provides a Lambda-optimized span processor that efficiently manages OpenTelemetry spans
//! in a serverless environment. It uses a ring buffer to store spans in memory and supports different
//! processing modes to balance latency and reliability.
//!
//! # Processing Modes
//!
//! The processor supports three modes for span export:
//!
//! 1. **Sync Mode** (default):
//!    - Direct, synchronous export in handler thread
//!    - Recommended for low-volume telemetry or when latency is not critical
//!    - Best for development and debugging
//!    - Set via `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE=sync`
//!
//! 2. **Async Mode**:
//!    - Export via Lambda extension using AWS Lambda Extensions API
//!    - Spans are queued and exported after handler completion
//!    - Uses channel-based communication between handler and extension
//!    - Best for production use with high telemetry volume
//!    - Set via `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE=async`
//!
//! 3. **Finalize Mode**:
//!    - Only registers extension with no events
//!    - Ensures SIGTERM handler for graceful shutdown
//!    - Compatible with BatchSpanProcessor for custom export strategies
//!    - Best for specialized export requirements
//!    - Set via `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE=finalize`
//!
//! # Architecture
//!
//! The processor is designed specifically for the Lambda execution environment:
//!
//! 1. **Ring Buffer Storage**:
//!    - Fixed-size circular buffer prevents memory growth
//!    - O(1) push operations with no memory reallocation
//!    - FIFO ordering ensures spans are processed in order
//!    - Efficient batch removal for export
//!    - When full, new spans are dropped (with warning logs)
//!
//! 2. **Thread Safety**:
//!    - All operations are thread-safe
//!    - Uses Mutex for span buffer access
//!    - Atomic operations for state management
//!    - Safe for concurrent span submission
//!
//! # Configuration
//!
//! The processor can be configured through environment variables:
//!
//! - `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Controls processing mode
//!   - "sync" for Sync mode (default)
//!   - "async" for Async mode
//!   - "finalize" for Finalize mode
//!
//! - `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Controls buffer size
//!   - Defaults to 2048 spans
//!   - Should be tuned based on span volume
//!
//! # Usage Examples
//!
//! Basic setup with default configuration:
//!
//! ```no_run
//! use lambda_otel_lite::{ProcessorConfig, LambdaSpanProcessor};
//! use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
//!
//! let processor = LambdaSpanProcessor::new(
//!     Box::new(OtlpStdoutSpanExporter::default()),
//!     ProcessorConfig::default()
//! );
//! ```
//!
//! Custom configuration for high-volume scenarios:
//!
//! ```no_run
//! use lambda_otel_lite::{ProcessorConfig, LambdaSpanProcessor};
//! use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
//!
//! let processor = LambdaSpanProcessor::new(
//!     Box::new(OtlpStdoutSpanExporter::default()),
//!     ProcessorConfig {
//!         max_queue_size: 4096, // Larger buffer for high volume
//!     }
//! );
//! ```
//!
//! # Performance Considerations
//!
//! 1. **Memory Usage**:
//!    - Fixed memory footprint based on queue size
//!    - Each span typically uses 100-500 bytes
//!    - Default 2048 spans ≈ 0.5-1MB memory
//!
//! 2. **Latency Impact**:
//!    - Sync mode: Adds export time to handler latency
//!    - Async mode: Minimal impact (just span queueing)
//!    - Finalize mode: Depends on processor implementation
//!
//! 3. **Reliability**:
//!    - Spans may be dropped if buffer fills
//!    - Warning logs indicate dropped spans
//!    - Consider increasing buffer size if spans are dropped
//!
//! # Best Practices
//!
//! 1. **Mode Selection**:
//!    - Consider payload size and memory/CPU configuration
//!    - Use Sync mode for simple exports or low resource environments
//!    - Use Async mode the telemetrry payload is expected to be large, and the extension overhead is an acceptable trade-off with the handler latency
//!    - Use Finalize mode for custom export strategies
//!
//! 2. **Buffer Sizing**:
//!    - Monitor dropped_spans metric
//!    - Size based on max spans per invocation
//!    - Consider function memory when sizing
//!
//! 3. **Error Handling**:
//!    - Export errors are logged but don't fail function
//!    - Monitor for export failures in logs
//!    - Consider retry strategies in custom exporters

use opentelemetry::{otel_debug, otel_warn};
use opentelemetry::{
    trace::{TraceError, TraceResult},
    Context,
};
use opentelemetry_sdk::{
    export::trace::{SpanData, SpanExporter},
    trace::{Span, SpanProcessor},
    Resource,
};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

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
        match std::env::var("LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE")
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

/// Configuration for the Lambda span processor.
///
/// This struct allows customizing the behavior of the span processor to match your
/// workload's requirements. The configuration affects memory usage, span handling
/// capacity, and potential span loss under high load.
///
/// # Configuration Options
///
/// - `max_queue_size`: Maximum number of spans that can be stored in memory
///   - Determines memory usage (each span ≈ 100-500 bytes)
///   - When full, new spans are dropped with warning logs
///   - Should be sized based on expected span volume
///
/// # Environment Variables
///
/// The queue size can be configured using the `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`
/// environment variable. If not set, defaults to 2048 spans.
///
/// # Example
///
/// ```no_run
/// use lambda_otel_lite::ProcessorConfig;
///
/// // Default configuration (2048 spans)
/// let config = ProcessorConfig::default();
///
/// // Custom configuration for high volume
/// let config = ProcessorConfig {
///     max_queue_size: 4096,
/// };
/// ```
#[derive(Debug)]
pub struct ProcessorConfig {
    /// Maximum number of spans that can be stored in memory
    pub max_queue_size: usize,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 2048,
        }
    }
}

/// A fixed-size ring buffer for storing spans efficiently.
///
/// This implementation provides a memory-efficient way to store spans with
/// predictable performance characteristics:
///
/// # Performance Characteristics
///
/// - Push Operation: O(1)
/// - Memory Usage: Fixed based on capacity
/// - Order: FIFO (First In, First Out)
/// - Batch Operations: Efficient removal of all spans
///
/// # Implementation Details
///
/// The buffer uses a circular array with head and tail pointers:
/// - `head`: Points to next write position
/// - `tail`: Points to next read position
/// - `size`: Current number of elements
/// - `capacity`: Maximum number of elements
///
/// When the buffer is full, new spans are rejected rather than overwriting old ones.
/// This ensures no data loss occurs silently.
#[derive(Debug)]
struct SpanRingBuffer {
    buffer: Vec<Option<SpanData>>,
    head: usize, // Where to write next
    tail: usize, // Where to read next
    size: usize, // Current number of elements
    capacity: usize,
}

impl SpanRingBuffer {
    fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.extend((0..capacity).map(|_| None));
        Self {
            buffer,
            head: 0,
            tail: 0,
            size: 0,
            capacity,
        }
    }

    fn push(&mut self, span: SpanData) -> bool {
        if self.size == self.capacity {
            return false;
        }

        self.buffer[self.head] = Some(span);
        self.head = (self.head + 1) % self.capacity;
        self.size += 1;
        true
    }

    fn take_all(&mut self) -> Vec<SpanData> {
        let mut result = Vec::with_capacity(self.size);
        while self.size > 0 {
            if let Some(span) = self.buffer[self.tail].take() {
                result.push(span);
            }
            self.tail = (self.tail + 1) % self.capacity;
            self.size -= 1;
        }
        self.head = 0;
        self.tail = 0;
        result
    }

    fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// Lambda-optimized span processor implementation.
///
/// This processor is designed specifically for AWS Lambda functions, providing:
/// - Efficient span storage through a ring buffer
/// - Configurable processing modes for different use cases
/// - Thread-safe operations for concurrent span submission
/// - Automatic span sampling and filtering
/// - Batch export capabilities
///
/// # Memory Usage
///
/// The processor uses a fixed amount of memory based on the configured queue size:
/// - Each span typically uses 100-500 bytes
/// - Default configuration (2048 spans) uses 0.5-1MB
/// - When buffer is full, new spans are dropped with warnings
///
/// # Thread Safety
///
/// All operations are thread-safe through:
/// - Mutex protection for span buffer access
/// - Atomic operations for state management
/// - Safe sharing between threads with Arc
///
/// # Example
///
/// ```no_run
/// use lambda_otel_lite::{LambdaSpanProcessor, ProcessorConfig};
/// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
///
/// let processor = LambdaSpanProcessor::new(
///     Box::new(OtlpStdoutSpanExporter::default()),
///     ProcessorConfig::default(),
/// );
///
/// // Processor can be safely shared between threads
/// let processor = std::sync::Arc::new(processor);
/// ```
///
/// # Error Handling
///
/// The processor handles errors gracefully:
/// - Export failures are logged but don't fail the function
/// - Dropped spans are counted and logged with warnings
/// - Buffer overflow warnings help with capacity planning
#[derive(Debug)]
pub struct LambdaSpanProcessor {
    exporter: Mutex<Box<dyn SpanExporter>>,
    spans: Mutex<SpanRingBuffer>,
    is_shutdown: Arc<AtomicBool>,
    dropped_count: AtomicUsize,
}

impl LambdaSpanProcessor {
    /// Creates a new LambdaSpanProcessor with the given configuration
    pub fn new(exporter: Box<dyn SpanExporter>, config: ProcessorConfig) -> Self {
        Self {
            exporter: Mutex::new(exporter),
            spans: Mutex::new(SpanRingBuffer::new(config.max_queue_size)),
            is_shutdown: Arc::new(AtomicBool::new(false)),
            dropped_count: AtomicUsize::new(0),
        }
    }
}

impl SpanProcessor for LambdaSpanProcessor {
    fn on_start(&self, _span: &mut Span, _cx: &Context) {
        // No-op, as we only process spans on end
    }

    fn on_end(&self, span: SpanData) {
        if self.is_shutdown.load(Ordering::Relaxed) {
            return;
        }

        // Skip unsampled spans
        if !span.span_context.is_sampled() {
            return;
        }

        // Try to add span to the buffer
        if let Ok(mut spans) = self.spans.lock() {
            if !spans.push(span) {
                let prev = self.dropped_count.fetch_add(1, Ordering::Relaxed);
                if prev == 0 || prev % 100 == 0 {
                    otel_warn!(
                        name: "LambdaSpanProcessor.on_end",
                        message = "Dropping span because buffer is full",
                        dropped_spans = prev + 1
                    );
                }
            }
        } else {
            otel_warn!(
                name: "LambdaSpanProcessor.on_end",
                message = "Failed to acquire spans lock in on_end"
            );
        }
    }

    fn force_flush(&self) -> TraceResult<()> {
        // Take the spans while holding the lock
        let batch = {
            if let Ok(mut spans) = self.spans.lock() {
                if spans.is_empty() {
                    return Ok(());
                }
                spans.take_all()
            } else {
                return Err(TraceError::Other(
                    "Failed to acquire spans lock in force_flush".into(),
                ));
            }
        };

        let result = self
            .exporter
            .lock()
            .map_err(|_| TraceError::Other("LambdaSpanProcessor mutex poison".into()))
            .and_then(|mut exporter| futures_executor::block_on(exporter.export(batch)));
        if let Err(err) = result {
            otel_debug!(
                name: "LambdaSpanProcessor.force_flush.Error",
                reason = format!("{:?}", err)
            );
        }

        Ok(())
    }

    fn shutdown(&self) -> TraceResult<()> {
        self.is_shutdown.store(true, Ordering::Relaxed);
        // Flush any remaining spans
        self.force_flush()?;
        if let Ok(mut exporter) = self.exporter.lock() {
            exporter.shutdown();
        }
        Ok(())
    }

    fn set_resource(&mut self, resource: &Resource) {
        if let Ok(mut exporter) = self.exporter.lock() {
            exporter.set_resource(resource);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::{
        trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState},
        InstrumentationScope,
    };
    use opentelemetry_sdk::{
        export::trace::SpanExporter,
        trace::{SpanEvents, SpanLinks},
    };
    use std::{borrow::Cow, future::Future, pin::Pin, sync::Arc};
    use tokio::sync::Mutex;

    // Mock exporter that captures exported spans
    #[derive(Debug)]
    struct MockExporter {
        spans: Arc<Mutex<Vec<SpanData>>>,
    }

    impl MockExporter {
        fn new() -> Self {
            Self {
                spans: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl SpanExporter for MockExporter {
        fn export(
            &mut self,
            batch: Vec<SpanData>,
        ) -> Pin<Box<dyn Future<Output = TraceResult<()>> + Send>> {
            let spans = self.spans.clone();
            Box::pin(async move {
                let mut spans = spans.lock().await;
                spans.extend(batch);
                Ok(())
            })
        }

        fn shutdown(&mut self) {}
    }

    // Helper function to create a test span
    fn create_test_span(name: &str) -> SpanData {
        let flags = TraceFlags::default().with_sampled(true);

        SpanData {
            span_context: SpanContext::new(
                TraceId::from_u128(1),
                SpanId::from_u64(1),
                flags,
                false,
                TraceState::default(),
            ),
            parent_span_id: SpanId::INVALID,
            span_kind: opentelemetry::trace::SpanKind::Internal,
            name: Cow::Owned(name.to_string()),
            start_time: std::time::SystemTime::now(),
            end_time: std::time::SystemTime::now(),
            attributes: Vec::new(),
            dropped_attributes_count: 0,
            events: SpanEvents::default(),
            links: SpanLinks::default(),
            status: opentelemetry::trace::Status::default(),
            instrumentation_scope: InstrumentationScope::builder("test").build(),
        }
    }

    #[test]
    fn test_ring_buffer_basic_operations() {
        let mut buffer = SpanRingBuffer::new(2);

        // Test empty buffer
        assert!(buffer.is_empty());
        assert_eq!(buffer.take_all(), vec![]);

        // Test adding spans
        buffer.push(create_test_span("span1"));
        buffer.push(create_test_span("span2"));

        assert!(!buffer.is_empty());

        // Test taking spans
        let spans = buffer.take_all();
        assert_eq!(spans.len(), 2);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buffer = SpanRingBuffer::new(2);

        // Fill buffer
        buffer.push(create_test_span("span1"));
        buffer.push(create_test_span("span2"));

        // Add one more span, should overwrite the oldest
        let success = buffer.push(create_test_span("span3"));
        assert!(!success); // Should fail since buffer is full

        let spans = buffer.take_all();
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().any(|s| s.name == "span1"));
        assert!(spans.iter().any(|s| s.name == "span2"));
    }

    #[tokio::test]
    async fn test_processor_sync_mode() {
        let mock_exporter = MockExporter::new();
        let spans_exported = mock_exporter.spans.clone();

        let processor = LambdaSpanProcessor::new(
            Box::new(mock_exporter),
            ProcessorConfig { max_queue_size: 10 },
        );

        // Test span processing
        processor.on_end(create_test_span("test_span"));

        // Force flush to ensure export
        processor.force_flush().unwrap();

        // Verify span was exported
        let exported = spans_exported.lock().await;
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].name, "test_span");
    }

    #[tokio::test]
    async fn test_shutdown_exports_remaining_spans() {
        let mock_exporter = MockExporter::new();
        let spans_exported = mock_exporter.spans.clone();

        let processor =
            LambdaSpanProcessor::new(Box::new(mock_exporter), ProcessorConfig::default());

        // Add some spans
        processor.on_end(create_test_span("span1"));
        processor.on_end(create_test_span("span2"));

        // Shutdown should export all spans
        processor.shutdown().unwrap();

        // Verify all spans were exported
        let exported = spans_exported.lock().await;
        assert_eq!(exported.len(), 2);

        // Verify new spans are dropped after shutdown
        processor.on_end(create_test_span("span3"));
        assert_eq!(exported.len(), 2); // No new spans after shutdown
    }

    #[tokio::test]
    async fn test_concurrent_span_processing() {
        let mock_exporter = MockExporter::new();
        let spans_exported = mock_exporter.spans.clone();

        let processor = Arc::new(LambdaSpanProcessor::new(
            Box::new(mock_exporter),
            ProcessorConfig {
                max_queue_size: 100,
            },
        ));

        let mut handles = Vec::new();

        // Spawn 10 tasks, each adding 10 spans
        for i in 0..10 {
            let processor = processor.clone();
            handles.push(tokio::spawn(async move {
                for j in 0..10 {
                    processor.on_end(create_test_span(&format!("span_{}_{}", i, j)));
                }
            }));
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Force flush and verify all spans were processed
        processor.force_flush().unwrap();

        let exported = spans_exported.lock().await;
        assert_eq!(exported.len(), 100);
        assert_eq!(processor.dropped_count.load(Ordering::Relaxed), 0);
    }
}
