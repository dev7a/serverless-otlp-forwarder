//! Core functionality for OpenTelemetry initialization and configuration in Lambda functions.
//!
//! This module provides the initialization and configuration components for OpenTelemetry in Lambda:
//! - `init_telemetry`: Main entry point for telemetry setup
//! - `TelemetryConfig`: Configuration builder with environment-based defaults
//! - `TelemetryCompletionHandler`: Controls span export timing based on processing mode
//!
//! # Architecture
//!
//! The initialization flow:
//! 1. Configuration is built from environment and/or builder options
//! 2. Span processor is created based on processing mode
//! 3. Resource attributes are detected from Lambda environment
//! 4. Tracer provider is initialized with the configuration
//! 5. Completion handler is returned for managing span export
//!
//! # Environment Configuration
//!
//! Core environment variables:
//! - `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: "sync" (default), "async", or "finalize"
//! - `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Maximum spans in buffer (default: 2048)
//! - `OTEL_SERVICE_NAME`: Override auto-detected service name
//!
//! # Basic Usage
//!
//! ```no_run
//! use lambda_otel_lite::{init_telemetry, TelemetryConfig};
//! use lambda_runtime::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // Initialize with default configuration
//!     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
//!     Ok(())
//! }
//! ```
//!
//! Custom configuration with custom resource attributes:
//! ```no_run
//! use lambda_otel_lite::{init_telemetry, TelemetryConfig};
//! use opentelemetry::KeyValue;
//! use opentelemetry_sdk::Resource;
//! use lambda_runtime::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let resource = Resource::new(vec![
//!         KeyValue::new("service.version", "1.0.0"),
//!         KeyValue::new("deployment.environment", "production"),
//!     ]);
//!
//!     let config = TelemetryConfig::builder()
//!         .resource(resource)
//!         .build();
//!
//!     let completion_handler = init_telemetry(config).await?;
//!     Ok(())
//! }
//! ```
//!
//! Custom configuration with custom span processor:
//! ```no_run
//! use lambda_otel_lite::{init_telemetry, TelemetryConfig};
//! use opentelemetry_sdk::trace::SimpleSpanProcessor;
//! use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
//! use lambda_runtime::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let config = TelemetryConfig::builder()
//!         .with_span_processor(SimpleSpanProcessor::new(
//!             Box::new(OtlpStdoutSpanExporter::default())
//!         ))
//!         .library_name("instrumented-service".to_string())
//!         .enable_fmt_layer(true)
//!         .build();
//!
//!     let completion_handler = init_telemetry(config).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Environment Variables
//!
//! The following environment variables affect the configuration:
//! - `OTEL_SERVICE_NAME`: Service name for spans
//! - `OTEL_RESOURCE_ATTRIBUTES`: Additional resource attributes
//! - `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Span buffer size (default: 2048)
//! - `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: Export compression (default: 6)
//! - `LAMBDA_TRACING_ENABLE_FMT_LAYER`: Enable formatting layer (default: false)
//! - `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Processing mode (sync/async/finalize)
//! - `RUST_LOG` or `AWS_LAMBDA_LOG_LEVEL`: Log level configuration

use crate::{
    extension::register_extension,
    processor::{LambdaSpanProcessor, ProcessorConfig, ProcessorMode},
    resource::get_lambda_resource,
};
use bon::Builder;
use lambda_runtime::Error;
use opentelemetry::propagation::{TextMapCompositePropagator, TextMapPropagator};
use opentelemetry::{global, global::set_tracer_provider, trace::TracerProvider as _};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{Builder as TracerProviderBuilder, SpanProcessor, TracerProvider as SdkTracerProvider},
    Resource,
};
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::{borrow::Cow, env, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;
use tracing_subscriber::layer::SubscriberExt;

static LEAKED_NAMES: LazyLock<Mutex<HashMap<String, &'static str>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get or create a static string reference, leaking memory only once per unique string.
///
/// # Note on Memory Management
///
/// This function uses `Box::leak` to convert strings into static references, which is required
/// by the OpenTelemetry API. To minimize memory leaks, it maintains a cache of previously
/// leaked strings, ensuring each unique string is only leaked once.
///
/// While this still technically leaks memory, it's bounded by the number of unique library
/// names used in the application. In a Lambda context, this is typically just one name
/// per function, making the leak negligible.
fn get_static_str(s: String) -> &'static str {
    let mut cache = LEAKED_NAMES.lock().unwrap();
    if let Some(&static_str) = cache.get(&s) {
        static_str
    } else {
        let leaked = Box::leak(s.clone().into_boxed_str());
        cache.insert(s, leaked);
        leaked
    }
}

/// Manages the lifecycle of span export based on the processing mode.
///
/// This handler must be used to signal when spans should be exported. Its behavior
/// varies by processing mode:
/// - Sync: Forces immediate export
/// - Async: Signals the extension to export
/// - Finalize: Defers to span processor
///
/// # Thread Safety
///
/// This type is `Clone` and can be safely shared between threads.
#[derive(Clone)]
pub struct TelemetryCompletionHandler {
    provider: Arc<SdkTracerProvider>,
    sender: Option<UnboundedSender<()>>,
    mode: ProcessorMode,
}

impl TelemetryCompletionHandler {
    pub fn new(
        provider: Arc<SdkTracerProvider>,
        sender: Option<UnboundedSender<()>>,
        mode: ProcessorMode,
    ) -> Self {
        Self {
            provider,
            sender,
            mode,
        }
    }

    /// Complete telemetry processing for the current invocation
    ///
    /// In Sync mode, this will force flush the provider and log any errors that occur.
    /// In Async mode, this will send a completion signal to the extension.
    /// In Finalize mode, this will do nothing (handled by drop).
    pub fn complete(&self) {
        match self.mode {
            ProcessorMode::Sync => {
                if let Some(Err(e)) = self.provider.force_flush().into_iter().find(|r| r.is_err()) {
                    tracing::warn!(error = ?e, "Error flushing telemetry");
                }
            }
            ProcessorMode::Async => {
                if let Some(sender) = &self.sender {
                    if let Err(e) = sender.send(()) {
                        tracing::warn!(error = ?e, "Failed to send completion signal to extension");
                    }
                }
            }
            ProcessorMode::Finalize => {
                // Do nothing, handled by drop
            }
        }
    }
}

/// Configuration for OpenTelemetry initialization.
///
/// Provides configuration options for telemetry setup. Use `TelemetryConfig::default()`
/// for standard Lambda configuration, or the builder pattern for customization.
///
/// # Fields
///
/// * `enable_fmt_layer` - Enable console output (default: false)
/// * `set_global_provider` - Set as global tracer provider (default: true)
/// * `resource` - Custom resource attributes (default: auto-detected from Lambda)
/// * `library_name` - Name for the tracer (default: crate name)
/// * `propagators` - List of propagators for trace context propagation
///
/// # Examples
///
/// Default configuration:
/// ```no_run
/// use lambda_otel_lite::TelemetryConfig;
///
/// let config = TelemetryConfig::default();
/// ```
///
/// Custom configuration:
/// ```no_run
/// use lambda_otel_lite::TelemetryConfig;
/// use opentelemetry_sdk::Resource;
/// use opentelemetry::KeyValue;
///
/// let config = TelemetryConfig::builder()
///     .resource(Resource::new(vec![KeyValue::new("version", "1.0.0")]))
///     .enable_fmt_layer(true)
///     .build();
/// ```
#[derive(Builder, Debug)]
pub struct TelemetryConfig {
    // Custom fields for internal state
    #[builder(field)]
    provider_builder: TracerProviderBuilder,

    #[builder(field)]
    has_processor: bool,

    #[builder(field)]
    propagators: Vec<Box<dyn TextMapPropagator + Send + Sync>>,

    #[builder(default = false)]
    pub enable_fmt_layer: bool,

    #[builder(default = true)]
    pub set_global_provider: bool,

    pub resource: Option<Resource>,

    pub library_name: Option<String>,

    pub env_var_name: Option<String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Builder methods for adding span processors and other configuration
impl<S: telemetry_config_builder::State> TelemetryConfigBuilder<S> {
    /// Add a span processor to the tracer provider.
    ///
    /// This method allows adding custom span processors for trace data processing.
    /// Multiple processors can be added by calling this method multiple times.
    ///
    /// # Arguments
    ///
    /// * `processor` - A span processor implementing the [`SpanProcessor`] trait
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lambda_otel_lite::TelemetryConfig;
    /// use opentelemetry_sdk::trace::SimpleSpanProcessor;
    /// use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
    ///
    /// // Only use builder when adding custom processors
    /// let config = TelemetryConfig::builder()
    ///     .with_span_processor(SimpleSpanProcessor::new(
    ///         Box::new(OtlpStdoutSpanExporter::default())
    ///     ))
    ///     .build();
    /// ```
    pub fn with_span_processor<T>(mut self, processor: T) -> Self
    where
        T: SpanProcessor + 'static,
    {
        self.provider_builder = self.provider_builder.with_span_processor(processor);
        self.has_processor = true;
        self
    }

    /// Add a propagator to the list of propagators.
    ///
    /// Multiple propagators can be added and will be combined into a composite propagator.
    /// The default propagator is TraceContextPropagator.
    ///
    /// # Arguments
    ///
    /// * `propagator` - A propagator implementing the [`TextMapPropagator`] trait
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lambda_otel_lite::TelemetryConfig;
    /// use opentelemetry_sdk::propagation::BaggagePropagator;
    ///
    /// let config = TelemetryConfig::builder()
    ///     .with_propagator(BaggagePropagator::new())
    ///     .build();
    /// ```
    pub fn with_propagator<T>(mut self, propagator: T) -> Self
    where
        T: TextMapPropagator + Send + Sync + 'static,
    {
        self.propagators.push(Box::new(propagator));
        self
    }
}

/// Initialize OpenTelemetry for AWS Lambda with the provided configuration.
///
/// # Arguments
///
/// * `config` - Configuration for telemetry initialization
///
/// # Returns
///
/// Returns a completion handler for managing span export timing
///
/// # Errors
///
/// Returns error if:
/// - Extension registration fails (async/finalize modes)
/// - Tracer provider initialization fails
/// - Environment variable parsing fails
///
/// # Examples
///
/// ```no_run
/// use lambda_otel_lite::{init_telemetry, TelemetryConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), lambda_runtime::Error> {
///     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
///     Ok(())
/// }
/// ```
pub async fn init_telemetry(
    mut config: TelemetryConfig,
) -> Result<TelemetryCompletionHandler, Error> {
    let mode = ProcessorMode::from_env();

    // Set up the propagator(s)
    if config.propagators.is_empty() {
        config
            .propagators
            .push(Box::new(TraceContextPropagator::new()));
    }

    let composite_propagator = TextMapCompositePropagator::new(config.propagators);
    global::set_text_map_propagator(composite_propagator);

    // Add default span processor if none was added
    if !config.has_processor {
        let compression_level = env::var("OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(6);
        let exporter = Box::new(OtlpStdoutSpanExporter::with_gzip_level(compression_level));
        let max_queue_size = env::var("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);
        let processor = LambdaSpanProcessor::new(exporter, ProcessorConfig { max_queue_size });
        config.provider_builder = config.provider_builder.with_span_processor(processor);
    }

    // Apply defaults and build the provider
    let resource = config.resource.unwrap_or_else(get_lambda_resource);
    let provider = Arc::new(config.provider_builder.with_resource(resource).build());

    // Convert library_name to a static str, reusing if possible
    let library_name = get_static_str(
        config
            .library_name
            .unwrap_or_else(|| Cow::Borrowed(env!("CARGO_PKG_NAME")).into_owned()),
    );
    let tracer = provider.tracer(library_name);

    // Register the extension if in async or finalize mode
    let sender = match mode {
        ProcessorMode::Async | ProcessorMode::Finalize => {
            Some(register_extension(provider.clone(), mode.clone()).await?)
        }
        _ => None,
    };

    if config.set_global_provider {
        // Set the provider as global
        set_tracer_provider(provider.as_ref().clone());
    }

    // Initialize tracing subscriber with smart env var selection
    let env_var_name = config.env_var_name.as_deref().unwrap_or_else(|| {
        if env::var("RUST_LOG").is_ok() {
            "RUST_LOG"
        } else {
            "AWS_LAMBDA_LOG_LEVEL"
        }
    });

    let env_filter = tracing_subscriber::EnvFilter::builder()
        .with_env_var(env_var_name)
        .from_env_lossy();

    let subscriber = tracing_subscriber::registry::Registry::default()
        .with(tracing_opentelemetry::OpenTelemetryLayer::new(
            tracer.clone(),
        ))
        .with(env_filter);

    // Always initialize the subscriber, with or without fmt layer
    if config.enable_fmt_layer {
        let is_json = env::var("AWS_LAMBDA_LOG_FORMAT")
            .unwrap_or_default()
            .to_uppercase()
            == "JSON";

        if is_json {
            tracing::subscriber::set_global_default(
                subscriber.with(
                    tracing_subscriber::fmt::layer()
                        .with_target(false)
                        .without_time()
                        .json(),
                ),
            )?;
        } else {
            tracing::subscriber::set_global_default(
                subscriber.with(
                    tracing_subscriber::fmt::layer()
                        .with_target(false)
                        .without_time()
                        .with_ansi(false),
                ),
            )?;
        }
    } else {
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(TelemetryCompletionHandler::new(provider, sender, mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::trace::SimpleSpanProcessor;
    use sealed_test::prelude::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_static_str_caching() {
        // First call should leak
        let first = get_static_str("test-name".to_string());

        // Second call with same string should reuse
        let second = get_static_str("test-name".to_string());

        // Verify we got the same pointer
        assert!(std::ptr::eq(first, second));

        // Different string should get different pointer
        let third = get_static_str("other-name".to_string());
        assert!(!std::ptr::eq(first, third));
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_init_telemetry_defaults() {
        let completion_handler = init_telemetry(TelemetryConfig::default()).await.unwrap();
        assert!(completion_handler.sender.is_none()); // Default mode is Sync
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_init_telemetry_custom() {
        let resource = Resource::new(vec![KeyValue::new("test", "value")]);
        let config = TelemetryConfig::builder()
            .resource(resource)
            .library_name("test".into())
            .enable_fmt_layer(true)
            .set_global_provider(false)
            .build();

        let completion_handler = init_telemetry(config).await.unwrap();
        assert!(completion_handler.sender.is_none());
    }

    #[test]
    fn test_telemetry_config_defaults() {
        let config = TelemetryConfig::builder().build();
        assert!(config.set_global_provider); // Should be true by default
        assert!(!config.has_processor);
        assert!(!config.enable_fmt_layer);
    }

    #[test]
    fn test_completion_handler_sync_mode() {
        let provider = Arc::new(
            TracerProviderBuilder::default()
                .with_span_processor(SimpleSpanProcessor::new(Box::new(
                    OtlpStdoutSpanExporter::default(),
                )))
                .build(),
        );

        let handler = TelemetryCompletionHandler::new(provider, None, ProcessorMode::Sync);

        // In sync mode, complete() should call force_flush
        handler.complete();
        // Note: We can't easily verify the flush was called since TracerProvider
        // doesn't expose this information, but we can verify it doesn't panic
    }

    #[tokio::test]
    async fn test_completion_handler_async_mode() {
        let provider = Arc::new(
            TracerProviderBuilder::default()
                .with_span_processor(SimpleSpanProcessor::new(Box::new(
                    OtlpStdoutSpanExporter::default(),
                )))
                .build(),
        );

        let (tx, mut rx) = mpsc::unbounded_channel();

        let completion_handler =
            TelemetryCompletionHandler::new(provider, Some(tx), ProcessorMode::Async);

        // In async mode, complete() should send a message through the channel
        completion_handler.complete();

        // Verify that we received the completion signal
        assert!(rx.try_recv().is_ok());
        // Verify channel is now empty
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_completion_handler_finalize_mode() {
        let provider = Arc::new(
            TracerProviderBuilder::default()
                .with_span_processor(SimpleSpanProcessor::new(Box::new(
                    OtlpStdoutSpanExporter::default(),
                )))
                .build(),
        );

        let (tx, _rx) = mpsc::unbounded_channel();

        let completion_handler =
            TelemetryCompletionHandler::new(provider, Some(tx), ProcessorMode::Finalize);

        // In finalize mode, complete() should do nothing
        completion_handler.complete();
        // Verify it doesn't panic or cause issues
    }

    #[test]
    fn test_completion_handler_clone() {
        let provider = Arc::new(
            TracerProviderBuilder::default()
                .with_span_processor(SimpleSpanProcessor::new(Box::new(
                    OtlpStdoutSpanExporter::default(),
                )))
                .build(),
        );

        let (tx, _rx) = mpsc::unbounded_channel();

        let completion_handler =
            TelemetryCompletionHandler::new(provider, Some(tx), ProcessorMode::Async);

        // Test that Clone is implemented correctly
        let cloned = completion_handler.clone();

        // Verify both handlers have the same mode
        assert!(matches!(cloned.mode, ProcessorMode::Async));
        assert!(cloned.sender.is_some());
    }

    #[test]
    fn test_completion_handler_sync_mode_error_handling() {
        let provider = Arc::new(
            TracerProviderBuilder::default()
                .with_span_processor(SimpleSpanProcessor::new(Box::new(
                    OtlpStdoutSpanExporter::default(),
                )))
                .build(),
        );

        let completion_handler =
            TelemetryCompletionHandler::new(provider, None, ProcessorMode::Sync);

        // Test that complete() doesn't panic
        completion_handler.complete();
    }

    #[tokio::test]
    async fn test_completion_handler_async_mode_error_handling() {
        let provider = Arc::new(
            TracerProviderBuilder::default()
                .with_span_processor(SimpleSpanProcessor::new(Box::new(
                    OtlpStdoutSpanExporter::default(),
                )))
                .build(),
        );

        // Use UnboundedSender instead of Sender
        let (tx, _rx) = mpsc::unbounded_channel();
        // Fill the channel by dropping the receiver
        drop(_rx);

        let completion_handler =
            TelemetryCompletionHandler::new(provider, Some(tx), ProcessorMode::Async);

        // Test that complete() doesn't panic when receiver is dropped
        completion_handler.complete();
    }
}
