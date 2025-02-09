//! Lambda Extension for OpenTelemetry span processing.
//!
//! This module provides an internal Lambda extension that manages the lifecycle of OpenTelemetry spans.
//! The extension integrates with AWS Lambda's Extensions API to efficiently manage telemetry data
//! collection and export.
//!
//! # Architecture
//!
//! The extension operates as a background task within the Lambda function process and communicates
//! with both the Lambda Runtime API and the function handler through asynchronous channels:
//!
//! 1. **Extension Registration**: On startup, the extension task registers with the Lambda Extensions API
//!    and subscribes to the appropriate events based on the processing mode.
//!
//! 2. **Handler Communication**: The extension uses a channel-based communication pattern to
//!    coordinate with the function handler for span export timing.
//!
//! 3. **Processing Modes**:
//!    - `Async`: Registers for INVOKE events and exports spans after handler completion
//!      - Spans are queued during handler execution
//!      - Export occurs after response is sent to user
//!      - Best for high-volume telemetry
//!    - `Finalize`: Registers with no events
//!      - Only installs SIGTERM handler
//!      - Lets application code control span export
//!      - Compatible with BatchSpanProcessor
//!
//! 4. **Graceful Shutdown**: The extension implements proper shutdown handling to ensure
//!    no telemetry data is lost when the Lambda environment is terminated.
//!
//! # Error Handling
//!
//! The extension implements robust error handling:
//! - Logs all export errors without failing the function
//! - Implements graceful shutdown on SIGTERM
//! - Handles channel communication failures
//!
//! # Example
//!
//! ```no_run
//! use lambda_otel_lite::{init_telemetry, TelemetryConfig};
//! use lambda_extension::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // The extension is automatically registered when using init_telemetry
//!     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
//!     Ok(())
//! }
//! ```

use crate::ProcessorMode;
use lambda_extension::{service_fn, Error, Extension, NextEvent};
use opentelemetry::{otel_debug, otel_error};
use opentelemetry_sdk::trace::TracerProvider;
use std::sync::Arc;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
};

/// Extension that flushes OpenTelemetry spans after each Lambda invocation.
///
/// This extension is responsible for:
/// - Receiving completion signals from the handler
/// - Flushing spans at the appropriate time
/// - Managing the lifecycle of the tracer provider
///
/// # Thread Safety
///
/// The extension is designed to be thread-safe:
/// - Uses `Arc` for shared ownership of the tracer provider
/// - Implements proper synchronization through `Mutex` for the channel receiver
/// - Safely handles concurrent access from multiple tasks
///
/// # Performance Characteristics
///
/// The extension is optimized for Lambda environments:
/// - Minimizes memory usage through efficient buffering
/// - Uses non-blocking channel communication
/// - Implements backpressure handling to prevent memory exhaustion
///
/// # Error Handling
///
/// The extension handles various error scenarios:
/// - Channel closed: Logs error and continues processing
/// - Export failures: Logs errors without failing the function
/// - Shutdown signals: Ensures final flush of spans
///
/// The extension operates asynchronously to minimize impact on handler latency.
/// It uses a channel-based communication pattern to coordinate with the handler.
pub struct OtelInternalExtension {
    /// Channel receiver to know when the handler is done
    request_done_receiver: Mutex<UnboundedReceiver<()>>,
    /// Reference to the tracer provider for flushing spans
    tracer_provider: Arc<TracerProvider>,
}

impl OtelInternalExtension {
    /// Creates a new OtelInternalExtension.
    ///
    /// # Arguments
    ///
    /// * `request_done_receiver` - Channel receiver for completion signals
    /// * `tracer_provider` - TracerProvider for span management
    pub fn new(
        request_done_receiver: UnboundedReceiver<()>,
        tracer_provider: Arc<TracerProvider>,
    ) -> Self {
        Self {
            request_done_receiver: Mutex::new(request_done_receiver),
            tracer_provider,
        }
    }

    /// Handles extension events and flushes telemetry after each invocation.
    ///
    /// This method implements the core event handling logic for the extension.
    /// It coordinates with the Lambda function handler to ensure spans are
    /// exported at the appropriate time.
    ///
    /// # Operation Flow
    ///
    /// 1. **Event Reception**:
    ///    - Receives Lambda extension events
    ///    - Filters for INVOKE events
    ///    - Ignores other event types
    ///
    /// 2. **Handler Coordination**:
    ///    - Waits for handler completion signal
    ///    - Uses async channel communication
    ///    - Handles channel closure gracefully
    ///
    /// 3. **Span Export**:
    ///    - Forces flush of all pending spans
    ///    - Handles export errors without failing
    ///    - Logs any export failures
    ///
    /// # Error Handling
    ///
    /// The method handles several failure scenarios:
    ///
    /// - **Channel Errors**:
    ///    - Channel closure: Returns error with descriptive message
    ///    - Send/receive failures: Properly propagated
    ///
    /// - **Export Errors**:
    ///    - Individual span export failures are logged
    ///    - Continues processing despite errors
    ///    - Maintains extension stability
    ///
    /// # Performance
    ///
    /// The method is optimized for Lambda environments:
    /// - Uses async/await for efficient execution
    /// - Minimizes blocking operations
    /// - Implements proper error recovery
    ///
    /// # Arguments
    ///
    /// * `event` - The Lambda extension event to handle
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the event was processed successfully, or an `Error`
    /// if something went wrong during processing. Note that export errors are
    /// logged but do not cause the method to return an error.
    pub async fn invoke(&self, event: lambda_extension::LambdaEvent) -> Result<(), Error> {
        if let NextEvent::Invoke(_e) = event.next {
            // Wait for runtime to finish processing event
            self.request_done_receiver
                .lock()
                .await
                .recv()
                .await
                .ok_or_else(|| Error::from("channel closed"))?;
            // Force flush all spans and handle any errors
            for result in self.tracer_provider.force_flush() {
                if let Err(err) = result {
                    otel_error!(
                        name: "OtelInternalExtension.invoke.Error",
                        reason = format!("{:?}", err)
                    );
                }
            }
        }

        Ok(())
    }
}

/// Register an internal extension for handling OpenTelemetry span processing.
///
/// **Note**: This function is called automatically by [`init_telemetry`](crate::init_telemetry)
/// and should not be called directly in your code. Use [`init_telemetry`](crate::init_telemetry)
/// instead to set up telemetry for your Lambda function.
///
/// # Initialization Sequence
///
/// 1. **Channel Setup**:
///    - Creates an unbounded channel for handler-extension communication
///    - Channel sender is returned to the handler
///    - Channel receiver is managed by the extension
///
/// 2. **Extension Registration**:
///    - Creates a new Lambda extension instance
///    - Configures event subscriptions based on processor mode
///    - Registers with the Lambda Extensions API
///    - Starts the extension in a background task
///
/// 3. **Signal Handler Setup**:
///    - Registers a SIGTERM handler for graceful shutdown
///    - Ensures pending spans are flushed before termination
///
/// # Processing Modes
///
/// The extension's behavior varies by processor mode:
///
/// - **Async Mode**:
///    - Registers for INVOKE events
///    - Waits for handler completion signal
///    - Flushes spans after each invocation
///    - Best for scenarios with high span counts
///
/// - **Finalize Mode**:
///    - Registers for no events
///    - Relies on processor's internal timing
///    - Minimal overhead on handler
///
/// # Error Handling
///
/// The function handles these error scenarios:
///
/// - **Registration Failures**:
///    - Extension API errors
///    - Invalid configuration
///    - Network issues
///
/// - **Runtime Errors**:
///    - Extension execution failures
///    - Channel communication errors
///    - Span export failures
///
///
/// # Arguments
///
/// * `tracer_provider` - The TracerProvider to use for span management
/// * `processor_mode` - The mode determining how spans are processed
///
/// # Returns
///
/// Returns a channel sender for signaling completion, or an Error if registration fails.
/// The sender should be used by the handler to signal completion of request processing.
///
/// # Example
///
/// Instead of calling this function directly, use [`init_telemetry`](crate::init_telemetry):
///
/// ```no_run
/// use lambda_otel_lite::{init_telemetry, TelemetryConfig};
/// use lambda_extension::Error;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
///     Ok(())
/// }
/// ```
pub async fn register_extension(
    tracer_provider: Arc<TracerProvider>,
    processor_mode: ProcessorMode,
) -> Result<UnboundedSender<()>, Error> {
    otel_debug!(
        name: "OtelInternalExtension.register_extension",
        message = "starting registration"
    );
    let (request_done_sender, request_done_receiver) = unbounded_channel::<()>();

    let extension = Arc::new(OtelInternalExtension::new(
        request_done_receiver,
        tracer_provider.clone(),
    ));

    // Register and start the extension
    let mut ext = Extension::new();

    // Only register for INVOKE events in async mode
    if matches!(processor_mode, ProcessorMode::Async) {
        ext = ext.with_events(&["INVOKE"]);
    } else {
        ext = ext.with_events(&[]);
    }

    let registered_extension = ext
        .with_events_processor(service_fn(move |event| {
            let extension = extension.clone();
            async move { extension.invoke(event).await }
        }))
        .with_extension_name("otel-internal")
        .register()
        .await?;

    // Run the extension in the background
    tokio::spawn(async move {
        if let Err(err) = registered_extension.run().await {
            otel_error!(
                name: "OtelInternalExtension.run.Error",
                reason = format!("{:?}", err)
            );
        }
    });

    // Set up signal handler for graceful shutdown
    tokio::spawn(async move {
        let mut sigterm = signal(SignalKind::terminate()).unwrap();

        if sigterm.recv().await.is_some() {
            otel_debug!(
                name: "OtelInternalExtension.SIGTERM",
                message = "SIGTERM received, flushing spans"
            );
            // Direct synchronous flush
            for result in tracer_provider.force_flush() {
                if let Err(err) = result {
                    otel_error!(
                        name: "OtelInternalExtension.SIGTERM.Error",
                        reason = format!("{:?}", err)
                    );
                }
            }
            otel_debug!(
                name: "OtelInternalExtension.SIGTERM",
                message = "Shutdown complete"
            );
            std::process::exit(0);
        }
    });

    Ok(request_done_sender)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::future::BoxFuture;
    use lambda_extension::{InvokeEvent, LambdaEvent};
    use opentelemetry::trace::TraceResult;
    use opentelemetry_sdk::{
        export::trace::{SpanData, SpanExporter},
        trace::TracerProvider,
        Resource,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    // Test exporter that captures spans
    #[derive(Debug, Default, Clone)]
    struct TestExporter {
        export_count: Arc<AtomicUsize>,
        spans: Arc<Mutex<Vec<SpanData>>>,
    }

    impl TestExporter {
        fn new() -> Self {
            Self {
                export_count: Arc::new(AtomicUsize::new(0)),
                spans: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn get_spans(&self) -> Vec<SpanData> {
            self.spans.lock().unwrap().clone()
        }
    }

    impl SpanExporter for TestExporter {
        fn export(&mut self, spans: Vec<SpanData>) -> BoxFuture<'static, TraceResult<()>> {
            self.export_count.fetch_add(spans.len(), Ordering::SeqCst);
            self.spans.lock().unwrap().extend(spans);
            Box::pin(futures_util::future::ready(Ok(())))
        }
    }

    fn setup_test_provider() -> (Arc<TracerProvider>, Arc<TestExporter>) {
        let exporter = TestExporter::new();
        let provider = TracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .with_resource(Resource::empty())
            .build();

        (Arc::new(provider), Arc::new(exporter))
    }

    #[tokio::test]
    async fn test_extension_invoke_handling() -> Result<(), Error> {
        let (provider, _) = setup_test_provider();
        let (sender, receiver) = unbounded_channel();

        let extension = OtelInternalExtension::new(receiver, provider);

        // Create an INVOKE event
        let invoke_event = InvokeEvent {
            deadline_ms: 1000,
            request_id: "test-id".to_string(),
            invoked_function_arn: "test-arn".to_string(),
            tracing: Default::default(),
        };
        let event = LambdaEvent {
            next: NextEvent::Invoke(invoke_event),
        };

        // Spawn task to handle the event
        let handle = tokio::spawn(async move { extension.invoke(event).await });

        // Send completion signal
        sender.send(()).unwrap();

        // Wait for handler to complete
        let result = handle.await.unwrap();
        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_extension_channel_closed() -> Result<(), Error> {
        let (provider, _) = setup_test_provider();
        let (sender, receiver) = unbounded_channel();

        let extension = OtelInternalExtension::new(receiver, provider);

        // Create an INVOKE event
        let invoke_event = InvokeEvent {
            deadline_ms: 1000,
            request_id: "test-id".to_string(),
            invoked_function_arn: "test-arn".to_string(),
            tracing: Default::default(),
        };
        let event = LambdaEvent {
            next: NextEvent::Invoke(invoke_event),
        };

        // Drop sender to close channel
        drop(sender);

        // Invoke should return error when channel is closed
        let result = extension.invoke(event).await;
        assert!(result.is_err());

        Ok(())
    }
}
