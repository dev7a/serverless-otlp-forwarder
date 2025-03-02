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
//! use lambda_otel_lite::telemetry::{init_telemetry, TelemetryConfig};
//! use lambda_runtime::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;
//!     Ok(())
//! }
//! ```
//!
//! Custom configuration with custom resource attributes:
//! ```no_run
//! use lambda_otel_lite::telemetry::{init_telemetry, TelemetryConfig};
//! use opentelemetry::KeyValue;
//! use opentelemetry_sdk::Resource;
//! use lambda_runtime::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let resource = Resource::builder()
//!         .with_attributes(vec![
//!             KeyValue::new("service.version", "1.0.0"),
//!             KeyValue::new("deployment.environment", "production"),
//!         ])
//!         .build();
//!
//!     let config = TelemetryConfig::builder()
//!         .resource(resource)
//!         .build();
//!
//!     let (_, completion_handler) = init_telemetry(config).await?;
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
//!         .enable_fmt_layer(true)
//!         .build();
//!
//!     let (_, completion_handler) = init_telemetry(config).await?;
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
    extension::register_extension, mode::ProcessorMode, processor::LambdaSpanProcessor,
    resource::get_lambda_resource,
};
use bon::Builder;
use lambda_runtime::Error;
use opentelemetry::propagation::{TextMapCompositePropagator, TextMapPropagator};
use opentelemetry::{global, global::set_tracer_provider, trace::TracerProvider as _, KeyValue};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{SdkTracerProvider, SpanProcessor, TracerProviderBuilder},
    Resource,
};
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;
use std::{borrow::Cow, env, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;
use tracing_subscriber::layer::SubscriberExt;

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
    tracer: opentelemetry_sdk::trace::Tracer,
}

impl TelemetryCompletionHandler {
    pub fn new(
        provider: Arc<SdkTracerProvider>,
        sender: Option<UnboundedSender<()>>,
        mode: ProcessorMode,
    ) -> Self {
        // Create instrumentation scope with attributes
        let scope = opentelemetry::InstrumentationScope::builder(env!("CARGO_PKG_NAME"))
            .with_version(Cow::Borrowed(env!("CARGO_PKG_VERSION")))
            .with_schema_url(Cow::Borrowed("https://opentelemetry.io/schemas/1.30.0"))
            .with_attributes(vec![
                KeyValue::new("library.language", "rust"),
                KeyValue::new("library.type", "instrumentation"),
                KeyValue::new("library.runtime", "aws_lambda"),
            ])
            .build();

        // Create tracer with instrumentation scope
        let tracer = provider.tracer_with_scope(scope);

        Self {
            provider,
            sender,
            mode,
            tracer,
        }
    }

    /// Get the tracer instance for creating spans.
    ///
    /// Returns the cached tracer instance configured with this package's instrumentation scope.
    /// The tracer is configured with the provider's settings and will automatically use
    /// the correct span processor based on the processing mode.
    pub fn get_tracer(&self) -> &opentelemetry_sdk::trace::Tracer {
        &self.tracer
    }

    /// Complete telemetry processing for the current invocation
    ///
    /// In Sync mode, this will force flush the provider and log any errors that occur.
    /// In Async mode, this will send a completion signal to the extension.
    /// In Finalize mode, this will do nothing (handled by drop).
    pub fn complete(&self) {
        println!("Completing telemetry");
        match self.mode {
            ProcessorMode::Sync => {
                if let Err(e) = self.provider.force_flush() {
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
/// * `enable_fmt_layer` - Enable console output for debugging (default: false)
/// * `set_global_provider` - Set as global tracer provider (default: true)
/// * `resource` - Custom resource attributes (default: auto-detected from Lambda)
/// * `env_var_name` - Environment variable name for log level configuration
///
/// # Examples
///
/// Basic usage with default configuration:
///
/// ```no_run
/// use lambda_otel_lite::telemetry::TelemetryConfig;
///
/// let config = TelemetryConfig::default();
/// ```
///
/// Custom configuration with resource attributes:
///
/// ```no_run
/// use lambda_otel_lite::telemetry::TelemetryConfig;
/// use opentelemetry::KeyValue;
/// use opentelemetry_sdk::Resource;
///
/// let config = TelemetryConfig::builder()
///     .resource(Resource::builder()
///         .with_attributes(vec![KeyValue::new("version", "1.0.0")])
///         .build())
///     .build();
/// ```
///
/// Custom configuration with logging options:
///
/// ```no_run
/// use lambda_otel_lite::telemetry::TelemetryConfig;
///
/// let config = TelemetryConfig::builder()
///     .enable_fmt_layer(true)  // Enable console output for debugging
///     .env_var_name("MY_CUSTOM_LOG_LEVEL".to_string())  // Custom env var for log level
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

    /// Enable console output for debugging.
    ///
    /// When enabled, spans and events will be printed to the console in addition
    /// to being exported through the configured span processors. This is useful
    /// for debugging but adds overhead and should be disabled in production.
    ///
    /// Default: `false`
    #[builder(default = false)]
    pub enable_fmt_layer: bool,

    /// Set this provider as the global OpenTelemetry provider.
    ///
    /// When enabled, the provider will be registered as the global provider
    /// for the OpenTelemetry API. This allows using the global tracer API
    /// without explicitly passing around the provider.
    ///
    /// Default: `true`
    #[builder(default = true)]
    pub set_global_provider: bool,

    /// Custom resource attributes for all spans.
    ///
    /// If not provided, resource attributes will be automatically detected
    /// from the Lambda environment. Custom resources will override any
    /// automatically detected attributes with the same keys.
    ///
    /// Default: `None` (auto-detected from Lambda environment)
    pub resource: Option<Resource>,

    /// Environment variable name to use for log level configuration.
    ///
    /// This field specifies which environment variable should be used to configure
    /// the tracing subscriber's log level filter. If not specified, the system will
    /// first check for `RUST_LOG` and then fall back to `AWS_LAMBDA_LOG_LEVEL`.
    ///
    /// Default: `None` (uses `RUST_LOG` or `AWS_LAMBDA_LOG_LEVEL`)
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
/// Returns a tuple containing:
/// - A tracer instance for manual instrumentation
/// - A completion handler for managing span export timing
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
/// Basic usage with default configuration:
///
/// ```no_run
/// use lambda_otel_lite::telemetry::{init_telemetry, TelemetryConfig};
///
/// # async fn example() -> Result<(), lambda_runtime::Error> {
/// // Initialize with default configuration
/// let (_, telemetry) = init_telemetry(TelemetryConfig::default()).await?;
/// # Ok(())
/// # }
/// ```
///
/// Custom configuration:
///
/// ```no_run
/// use lambda_otel_lite::telemetry::{init_telemetry, TelemetryConfig};
/// use opentelemetry::KeyValue;
/// use opentelemetry_sdk::Resource;
///
/// # async fn example() -> Result<(), lambda_runtime::Error> {
/// // Create custom resource
/// let resource = Resource::builder()
///     .with_attributes(vec![
///         KeyValue::new("service.name", "payment-api"),
///         KeyValue::new("service.version", "1.2.3"),
///     ])
///     .build();
///
/// // Initialize with custom configuration
/// let (_, telemetry) = init_telemetry(
///     TelemetryConfig::builder()
///         .resource(resource)
///         .build()
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// Advanced usage with BatchSpanProcessor (required for async exporters):
///
/// ```no_run
/// use lambda_otel_lite::{init_telemetry, TelemetryConfig};
/// use opentelemetry_otlp::{WithExportConfig, WithHttpConfig, Protocol};
/// use opentelemetry_sdk::trace::BatchSpanProcessor;
/// use lambda_runtime::Error;
///
/// # async fn example() -> Result<(), Error> {
/// let batch_exporter = opentelemetry_otlp::SpanExporter::builder()
///     .with_http()
///     .with_http_client(reqwest::Client::new())
///     .with_protocol(Protocol::HttpBinary)
///     .build()?;
///
/// let (provider, completion) = init_telemetry(
///     TelemetryConfig::builder()
///         .with_span_processor(BatchSpanProcessor::builder(batch_exporter).build())
///         .build()
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// Using LambdaSpanProcessor with blocking http client:
///
/// ```no_run
/// use lambda_otel_lite::{init_telemetry, TelemetryConfig, LambdaSpanProcessor};
/// use opentelemetry_otlp::{WithExportConfig, WithHttpConfig, Protocol};
/// use lambda_runtime::Error;
///
/// # async fn example() -> Result<(), Error> {
/// let lambda_exporter = opentelemetry_otlp::SpanExporter::builder()
///     .with_http()
///     .with_http_client(reqwest::blocking::Client::new())
///     .with_protocol(Protocol::HttpBinary)
///     .build()?;
///
/// let (provider, completion) = init_telemetry(
///     TelemetryConfig::builder()
///         .with_span_processor(
///             LambdaSpanProcessor::builder()
///                 .exporter(lambda_exporter)
///                 .max_batch_size(512)
///                 .max_queue_size(2048)
///                 .build()
///         )
///         .build()
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
pub async fn init_telemetry(
    mut config: TelemetryConfig,
) -> Result<(opentelemetry_sdk::trace::Tracer, TelemetryCompletionHandler), Error> {
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
        let processor = LambdaSpanProcessor::builder()
            .exporter(OtlpStdoutSpanExporter::default())
            .build();
        config.provider_builder = config.provider_builder.with_span_processor(processor);
    }

    // Apply defaults and build the provider
    let resource = config.resource.unwrap_or_else(get_lambda_resource);
    let provider = Arc::new(config.provider_builder.with_resource(resource).build());

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

    let completion_handler = TelemetryCompletionHandler::new(provider.clone(), sender, mode);
    let tracer = completion_handler.get_tracer().clone();

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

    Ok((tracer, completion_handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_sdk::trace::SimpleSpanProcessor;
    use sealed_test::prelude::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    #[sealed_test]
    async fn test_init_telemetry_defaults() {
        let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await.unwrap();
        assert!(completion_handler.sender.is_none()); // Default mode is Sync
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_init_telemetry_custom() {
        let resource = Resource::builder().build();
        let config = TelemetryConfig::builder()
            .resource(resource)
            .enable_fmt_layer(true)
            .set_global_provider(false)
            .build();

        let (_, completion_handler) = init_telemetry(config).await.unwrap();
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
            SdkTracerProvider::builder()
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
            SdkTracerProvider::builder()
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
            SdkTracerProvider::builder()
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
            SdkTracerProvider::builder()
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
            SdkTracerProvider::builder()
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
            SdkTracerProvider::builder()
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
