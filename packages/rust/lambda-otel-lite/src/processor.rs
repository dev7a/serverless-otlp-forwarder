//! Span processor implementation optimized for AWS Lambda functions.
//!
//! This module provides a Lambda-optimized span processor that efficiently manages OpenTelemetry spans
//! in a serverless environment. It uses a ring buffer to store spans in memory and provides efficient
//! batch processing capabilities.
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
//! - `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Controls buffer size
//!   - Defaults to 2048 spans
//!   - Should be tuned based on span volume
//!
//! - `LAMBDA_SPAN_PROCESSOR_BATCH_SIZE`: Controls batch size
//!   - Defaults to 512 spans
//!   - Should be tuned based on span volume
//!
//! # Usage Examples
//!
//! Basic setup with default configuration:
//!
//! ```no_run
//! use lambda_otel_lite::LambdaSpanProcessor;
//! use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
//!
//! let processor = LambdaSpanProcessor::builder()
//!     .exporter(OtlpStdoutSpanExporter::default())
//!     .build();
//! ```
//!
//! Using with an OTLP HTTP exporter:
//!
//! ```no_run
//! use lambda_otel_lite::LambdaSpanProcessor;
//! use opentelemetry_otlp::{SpanExporter, Protocol};
//! use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
//!
//! // Important: When using HTTP exporters, always use reqwest::blocking::Client
//! // Using async clients will cause deadlocks
//! let exporter = SpanExporter::builder()
//!     .with_http()
//!     .with_http_client(reqwest::blocking::Client::new())
//!     .with_protocol(Protocol::HttpBinary)
//!     .build()
//!     .expect("Failed to create exporter");
//!
//! let processor = LambdaSpanProcessor::builder()
//!     .exporter(exporter)
//!     .max_queue_size(4096)
//!     .max_batch_size(1024)
//!     .build();
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
//!    - Batch processing reduces network overhead
//!    - Configurable batch size allows tuning for your use case
//!    - Force flush available for immediate export when needed
//!
//! 3. **Reliability**:
//!    - Spans may be dropped if buffer fills
//!    - Warning logs indicate dropped spans
//!    - Consider increasing buffer size if spans are dropped
//!
//! # Best Practices
//!
//! 1. **Buffer Sizing**:
//!    - Monitor dropped_spans metric
//!    - Size based on max spans per invocation
//!    - Consider function memory when sizing
//!
//! 2. **Batch Configuration**:
//!    - Larger batches improve throughput but increase memory usage
//!    - Smaller batches reduce memory but increase network overhead
//!    - Default values work well for most use cases
//!
//! 3. **Error Handling**:
//!    - Export errors are logged but don't fail function
//!    - Monitor for export failures in logs
//!    - Consider retry strategies in custom exporters

use crate::logger::Logger;
use bon::bon;

/// Module-specific logger
static LOGGER: Logger = Logger::const_new("processor");

use opentelemetry::Context;
use opentelemetry_sdk::{
    error::{OTelSdkError, OTelSdkResult},
    trace::{Span, SpanProcessor},
    trace::{SpanData, SpanExporter},
    Resource,
};
use std::env;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

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

impl Default for SpanRingBuffer {
    fn default() -> Self {
        Self::new(2048) // Default capacity
    }
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

    fn take_batch(&mut self, max_batch_size: usize) -> Vec<SpanData> {
        let batch_size = self.size.min(max_batch_size);
        let mut result = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            if let Some(span) = self.buffer[self.tail].take() {
                result.push(span);
            }
            self.tail = (self.tail + 1) % self.capacity;
            self.size -= 1;
        }

        if self.size == 0 {
            self.head = 0;
            self.tail = 0;
        }

        result
    }

    fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// A span processor optimized for AWS Lambda functions.
///
/// This processor efficiently manages spans in a Lambda environment:
/// - Uses a fixed-size ring buffer to prevent memory growth
/// - Supports synchronous and asynchronous export modes
/// - Handles graceful shutdown for Lambda termination
///
/// # Examples
///
/// ```
/// use lambda_otel_lite::LambdaSpanProcessor;
/// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
///
/// let processor = LambdaSpanProcessor::builder()
///     .exporter(OtlpStdoutSpanExporter::default())
///     .build();
/// ```
///
/// With custom configuration:
///
/// ```
/// use lambda_otel_lite::LambdaSpanProcessor;
/// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
///
/// let processor = LambdaSpanProcessor::builder()
///     .exporter(OtlpStdoutSpanExporter::default())
///     .max_queue_size(1000)
///     .max_batch_size(100)
///     .build();
/// ```
#[derive(Debug)]
pub struct LambdaSpanProcessor<E>
where
    E: SpanExporter + std::fmt::Debug,
{
    /// The exporter used to export spans
    exporter: Mutex<E>,

    /// Internal buffer for storing spans
    spans: Mutex<SpanRingBuffer>,

    /// Flag indicating whether the processor is shut down
    is_shutdown: Arc<AtomicBool>,

    /// Counter for dropped spans
    dropped_count: AtomicUsize,

    /// Maximum number of spans to export in a single batch
    max_batch_size: usize,
}

#[bon]
impl<E> LambdaSpanProcessor<E>
where
    E: SpanExporter + std::fmt::Debug,
{
    /// Returns the default max batch size from environment or fallback value
    fn default_max_batch_size() -> usize {
        env::var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(512)
    }

    /// Returns the default max queue size from environment or fallback value
    fn default_max_queue_size() -> usize {
        env::var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048)
    }

    /// Creates a new LambdaSpanProcessor with the given exporter and configuration
    #[builder]
    pub fn new(exporter: E, max_batch_size: Option<usize>, max_queue_size: Option<usize>) -> Self {
        let max_batch_size = max_batch_size.unwrap_or_else(Self::default_max_batch_size);
        let max_queue_size = max_queue_size.unwrap_or_else(Self::default_max_queue_size);

        Self {
            exporter: Mutex::new(exporter),
            spans: Mutex::new(SpanRingBuffer::new(max_queue_size)),
            is_shutdown: Arc::new(AtomicBool::new(false)),
            dropped_count: AtomicUsize::new(0),
            max_batch_size,
        }
    }
}

impl<E> SpanProcessor for LambdaSpanProcessor<E>
where
    E: SpanExporter + std::fmt::Debug,
{
    fn on_start(&self, _span: &mut Span, _cx: &Context) {
        // No-op, as we only process spans on end
    }

    fn on_end(&self, span: SpanData) {
        if self.is_shutdown.load(Ordering::Relaxed) {
            LOGGER.warn("LambdaSpanProcessor.on_end: processor is shut down, dropping span");
            self.dropped_count.fetch_add(1, Ordering::Relaxed);
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
                    LOGGER.warn(format!(
                        "LambdaSpanProcessor.on_end: Dropping span because buffer is full (dropped_spans={})",
                        prev + 1
                    ));
                }
            }
        } else {
            LOGGER.warn("LambdaSpanProcessor.on_end: Failed to acquire spans lock in on_end");
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        LOGGER.debug("LambdaSpanProcessor.force_flush: flushing spans");
        if let Ok(mut spans) = self.spans.lock() {
            if spans.is_empty() {
                return Ok(());
            }

            let mut exporter = self.exporter.lock().map_err(|_| {
                OTelSdkError::InternalFailure(
                    "Failed to acquire exporter lock in force_flush".to_string(),
                )
            })?;

            // Process spans in batches
            while !spans.is_empty() {
                let batch = spans.take_batch(self.max_batch_size);
                if !batch.is_empty() {
                    let result = futures_executor::block_on(exporter.export(batch));
                    if let Err(err) = &result {
                        LOGGER.debug(format!("LambdaSpanProcessor.force_flush.Error: {:?}", err));
                        return result;
                    }
                }
            }
            Ok(())
        } else {
            Err(OTelSdkError::InternalFailure(
                "Failed to acquire spans lock in force_flush".to_string(),
            ))
        }
    }

    fn shutdown(&self) -> OTelSdkResult {
        self.is_shutdown.store(true, Ordering::Relaxed);
        // Flush any remaining spans
        self.force_flush()?;
        if let Ok(mut exporter) = self.exporter.lock() {
            exporter.shutdown()
        } else {
            Err(OTelSdkError::InternalFailure(
                "Failed to acquire exporter lock in shutdown".to_string(),
            ))
        }
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
    use crate::logger::Logger;
    use opentelemetry::{
        trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState},
        InstrumentationScope,
    };
    use opentelemetry_sdk::{
        trace::SpanExporter,
        trace::{SpanEvents, SpanLinks},
    };
    use serial_test::serial;
    use std::{borrow::Cow, future::Future, pin::Pin, sync::Arc};
    use tokio::sync::Mutex;

    fn setup_test_logger() -> Logger {
        Logger::new("test")
    }

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
        ) -> Pin<Box<dyn Future<Output = OTelSdkResult> + Send>> {
            let spans = self.spans.clone();
            Box::pin(async move {
                let mut spans = spans.lock().await;
                spans.extend(batch);
                Ok(())
            })
        }

        fn shutdown(&mut self) -> OTelSdkResult {
            Ok(())
        }
    }

    // Helper function to create a test span
    fn create_test_span(name: &str) -> SpanData {
        let flags = TraceFlags::default().with_sampled(true);

        SpanData {
            span_context: SpanContext::new(
                TraceId::from_hex("01000000000000000000000000000000").unwrap(),
                SpanId::from_hex("0100000000000001").unwrap(),
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
    #[serial]
    fn test_ring_buffer_basic_operations() {
        let mut buffer = SpanRingBuffer::new(2);

        // Test empty buffer
        assert!(buffer.is_empty());
        assert_eq!(buffer.take_batch(2), vec![]);

        // Test adding spans
        buffer.push(create_test_span("span1"));
        buffer.push(create_test_span("span2"));

        assert!(!buffer.is_empty());

        // Test taking spans
        let spans = buffer.take_batch(2);
        assert_eq!(spans.len(), 2);
        assert!(buffer.is_empty());
    }

    #[test]
    #[serial]
    fn test_ring_buffer_overflow() {
        let mut buffer = SpanRingBuffer::new(2);

        // Fill buffer
        buffer.push(create_test_span("span1"));
        buffer.push(create_test_span("span2"));

        // Add one more span, should overwrite the oldest
        let success = buffer.push(create_test_span("span3"));
        assert!(!success); // Should fail since buffer is full

        let spans = buffer.take_batch(2);
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().any(|s| s.name == "span1"));
        assert!(spans.iter().any(|s| s.name == "span2"));
    }

    #[test]
    #[serial]
    fn test_ring_buffer_batch_operations() {
        let mut buffer = SpanRingBuffer::new(5);

        // Add 5 spans
        for i in 0..5 {
            buffer.push(create_test_span(&format!("span{}", i)));
        }

        assert_eq!(buffer.take_batch(2).len(), 2);
        assert_eq!(buffer.take_batch(2).len(), 2);
        assert_eq!(buffer.take_batch(2).len(), 1);
        assert!(buffer.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn test_processor_sync_mode() {
        let _logger = setup_test_logger();
        let mock_exporter = MockExporter::new();
        let spans_exported = mock_exporter.spans.clone();

        let processor = LambdaSpanProcessor::builder()
            .exporter(mock_exporter)
            .max_queue_size(10)
            .max_batch_size(5)
            .build();

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
    #[serial]
    async fn test_shutdown_exports_remaining_spans() {
        let _logger = setup_test_logger();
        let mock_exporter = MockExporter::new();
        let spans_exported = mock_exporter.spans.clone();

        let processor = LambdaSpanProcessor::builder()
            .exporter(mock_exporter)
            .max_queue_size(10)
            .max_batch_size(5)
            .build();

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
    #[serial]
    async fn test_concurrent_span_processing() {
        let _logger = setup_test_logger();
        let mock_exporter = MockExporter::new();
        let spans_exported = mock_exporter.spans.clone();

        let processor = Arc::new(
            LambdaSpanProcessor::builder()
                .exporter(mock_exporter)
                .max_queue_size(100)
                .max_batch_size(25)
                .build(),
        );

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

    #[test]
    #[serial]
    fn test_batch_processing() {
        let _logger = setup_test_logger();
        let mock_exporter = MockExporter::new();
        let processor = LambdaSpanProcessor::builder()
            .exporter(mock_exporter)
            .max_queue_size(10)
            .max_batch_size(3)
            .build();

        // Add 5 spans
        for i in 0..5 {
            processor.on_end(create_test_span(&format!("span{}", i)));
        }

        // Force flush should process in batches of 3
        processor.force_flush().unwrap();

        // Add 2 more spans
        processor.on_end(create_test_span("span5"));
        processor.on_end(create_test_span("span6"));

        // Final flush
        processor.force_flush().unwrap();
    }

    #[test]
    #[serial]
    fn test_builder_default_values() {
        let mock_exporter = MockExporter::new();

        // Remove any existing env vars to test true defaults
        env::remove_var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE");
        env::remove_var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE");

        let processor = LambdaSpanProcessor::builder()
            .exporter(mock_exporter)
            .build();

        // Check default values
        assert_eq!(processor.max_batch_size, 512); // Default batch size
        assert_eq!(processor.spans.lock().unwrap().capacity, 2048); // Default queue size
    }

    #[test]
    #[serial]
    fn test_builder_env_var_values() {
        let mock_exporter = MockExporter::new();

        // Set custom values via env vars
        env::set_var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE", "100");
        env::set_var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE", "1000");

        let processor = LambdaSpanProcessor::builder()
            .exporter(mock_exporter)
            .build();

        // Check that env var values were used
        assert_eq!(processor.max_batch_size, 100);
        assert_eq!(processor.spans.lock().unwrap().capacity, 1000);

        // Clean up
        env::remove_var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE");
        env::remove_var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE");
    }

    #[test]
    #[serial]
    fn test_builder_explicit_values_override_env() {
        let mock_exporter = MockExporter::new();

        // Set env vars that should be overridden
        env::set_var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE", "100");
        env::set_var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE", "1000");

        let processor = LambdaSpanProcessor::builder()
            .exporter(mock_exporter)
            .max_batch_size(200)
            .max_queue_size(2000)
            .build();

        // Check that explicit values were used instead of env vars
        assert_eq!(processor.max_batch_size, 200);
        assert_eq!(processor.spans.lock().unwrap().capacity, 2000);

        // Clean up
        env::remove_var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE");
        env::remove_var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE");
    }

    #[test]
    #[serial]
    fn test_builder_invalid_env_vars() {
        let mock_exporter = MockExporter::new();

        // Set invalid values in env vars
        env::set_var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE", "not_a_number");
        env::set_var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE", "also_not_a_number");

        let processor = LambdaSpanProcessor::builder()
            .exporter(mock_exporter)
            .build();

        // Check that defaults were used
        assert_eq!(processor.max_batch_size, 512);
        assert_eq!(processor.spans.lock().unwrap().capacity, 2048);

        // Clean up
        env::remove_var("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE");
        env::remove_var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE");
    }
}
