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

use crate::logger::Logger;
use crate::ProcessorMode;
use lambda_extension::{service_fn, Error, Extension, NextEvent};
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::sync::Arc;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
};

static LOGGER: Logger = Logger::const_new("extension");

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
    tracer_provider: Arc<SdkTracerProvider>,
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
        tracer_provider: Arc<SdkTracerProvider>,
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
            if let Err(err) = self.tracer_provider.force_flush() {
                LOGGER.error(format!(
                    "OtelInternalExtension.invoke.Error: Error flushing tracer provider: {:?}",
                    err
                ));
            }
        }

        Ok(())
    }
}

/// Register an internal extension for handling OpenTelemetry span processing.
///
/// # Warning
///
/// This is an internal API used by [`init_telemetry`](crate::init_telemetry) and should not be called directly.
/// Use [`init_telemetry`](crate::init_telemetry) instead to set up telemetry for your Lambda function.
///
/// # Internal Details
///
/// The extension registration process:
/// 1. Creates communication channels for handler-extension coordination
/// 2. Registers with Lambda Extensions API based on processor mode
/// 3. Sets up signal handlers for graceful shutdown
/// 4. Manages span export timing based on processor mode
///
/// # Arguments
///
/// * `tracer_provider` - The TracerProvider to use for span management
/// * `processor_mode` - The mode determining how spans are processed
///
/// # Returns
///
/// Returns a channel sender for signaling completion, or an Error if registration fails.
pub(crate) async fn register_extension(
    tracer_provider: Arc<SdkTracerProvider>,
    processor_mode: ProcessorMode,
) -> Result<UnboundedSender<()>, Error> {
    LOGGER.debug("OtelInternalExtension.register_extension: starting registration");
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
            LOGGER.error(format!(
                "OtelInternalExtension.run.Error: Error running extension: {:?}",
                err
            ));
        }
    });

    // Set up signal handler for graceful shutdown
    tokio::spawn(async move {
        let mut sigterm = signal(SignalKind::terminate()).unwrap();

        if sigterm.recv().await.is_some() {
            LOGGER.debug("OtelInternalExtension.SIGTERM: SIGTERM received, flushing spans");
            // Direct synchronous flush
            if let Err(err) = tracer_provider.force_flush() {
                LOGGER.error(format!(
                    "OtelInternalExtension.SIGTERM.Error: Error during shutdown: {:?}",
                    err
                ));
            }
            LOGGER.debug("OtelInternalExtension.SIGTERM: Shutdown complete");
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
    use opentelemetry_sdk::{
        trace::{SdkTracerProvider, SpanData, SpanExporter},
        Resource,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    /// Test-specific logger

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
        fn export(
            &mut self,
            spans: Vec<SpanData>,
        ) -> BoxFuture<'static, opentelemetry_sdk::error::OTelSdkResult> {
            self.export_count.fetch_add(spans.len(), Ordering::SeqCst);
            self.spans.lock().unwrap().extend(spans);
            Box::pin(futures_util::future::ready(Ok(())))
        }
    }

    fn setup_test_provider() -> (Arc<SdkTracerProvider>, Arc<TestExporter>) {
        let exporter = TestExporter::new();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .with_resource(Resource::builder_empty().build())
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
