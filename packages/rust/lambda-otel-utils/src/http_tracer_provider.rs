//! This module provides utilities for configuring and building an OpenTelemetry TracerProvider
//! specifically tailored for use in AWS Lambda environments.
//!
//! It includes:
//! - `HttpTracerProviderBuilder`: A builder struct for configuring and initializing a TracerProvider.
//! - `get_lambda_resource`: A function to create a Resource with Lambda-specific attributes.
//!
//! The module supports various configuration options, including:
//! - Custom HTTP clients for exporting traces
//! - Enabling/disabling logging layers
//! - Setting custom tracer names
//! - Configuring propagators and ID generators
//! - Choosing between simple and batch exporters
//!
//! It also respects environment variables for certain configurations, such as the span processor type
//! and the OTLP exporter protocol.
//!
//! # Examples
//!
//! ```
//! use lambda_otel_utils::HttpTracerProviderBuilder;
//! use opentelemetry_sdk::trace::{TracerProvider, Tracer};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let tracer_provider: TracerProvider = HttpTracerProviderBuilder::default()
//!     .with_stdout_client()
//!     .with_default_text_map_propagator()
//!     .with_default_id_generator()
//!     .enable_global(true)
//!     .build()?;
//! # Ok(())
//! # }
//! ```

use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_aws::trace::{XrayIdGenerator, XrayPropagator};
use opentelemetry_http::HttpClient;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{IdGenerator, RandomIdGenerator, TracerProvider as SdkTracerProvider},
};
use otlp_stdout_client::StdoutClient;
use std::{env, fmt::Debug};
use thiserror::Error;

#[derive(Debug)]
enum ExporterType {
    Simple,
    Batch,
}

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("Failed to build exporter: {0}")]
    ExporterBuildError(#[from] opentelemetry::trace::TraceError),
}

/// A type-safe builder for configuring and initializing a TracerProvider.
pub struct HttpTracerProviderBuilder<
    C: HttpClient = StdoutClient,
    I: IdGenerator = RandomIdGenerator,
    P: TextMapPropagator = TraceContextPropagator,
> {
    client: C,
    install_global: bool,
    id_generator: I,
    propagators: Vec<Box<dyn TextMapPropagator + Send + Sync>>,
    exporter_type: ExporterType,
    _propagator_type: std::marker::PhantomData<P>,
}

/// Provides a default implementation for `HttpTracerProviderBuilder`.
///
/// This implementation creates a new `HttpTracerProviderBuilder` with default settings
/// by calling the `new()` method.
///
/// # Examples
///
/// ```
/// use lambda_otel_utils::HttpTracerProviderBuilder;
///
/// let default_builder = HttpTracerProviderBuilder::default();
/// ```
impl Default
    for HttpTracerProviderBuilder<StdoutClient, RandomIdGenerator, TraceContextPropagator>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C, I, P> HttpTracerProviderBuilder<C, I, P>
where
    C: HttpClient + 'static,
    I: IdGenerator + Send + Sync + 'static,
    P: TextMapPropagator + Send + Sync + 'static,
{
    /// Creates a new `HttpTracerProviderBuilder` with default settings.
    ///
    /// The default exporter type is determined by the `LAMBDA_OTEL_SPAN_PROCESSOR` environment variable:
    /// - "batch" - Uses batch span processor
    /// - "simple" - Uses simple span processor (default)
    /// - Any other value will default to simple span processor with a warning
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    /// use otlp_stdout_client::StdoutClient;
    /// use opentelemetry_sdk::propagation::TraceContextPropagator;
    /// use opentelemetry_sdk::trace::RandomIdGenerator;
    ///
    /// let builder = HttpTracerProviderBuilder::default();
    /// ```
    pub fn new(
    ) -> HttpTracerProviderBuilder<StdoutClient, RandomIdGenerator, TraceContextPropagator> {
        let exporter_type = match env::var("LAMBDA_OTEL_SPAN_PROCESSOR")
            .unwrap_or_else(|_| "simple".to_string())
            .to_lowercase()
            .as_str()
        {
            "batch" => ExporterType::Batch,
            "simple" => ExporterType::Simple,
            invalid => {
                eprintln!(
                    "Warning: Invalid LAMBDA_OTEL_SPAN_PROCESSOR value '{}'. Defaulting to Simple.",
                    invalid
                );
                ExporterType::Simple
            }
        };

        HttpTracerProviderBuilder {
            client: StdoutClient::new(),
            install_global: false,
            id_generator: RandomIdGenerator::default(),
            propagators: Vec::new(),
            exporter_type,
            _propagator_type: std::marker::PhantomData,
        }
    }

    /// Configures the builder to use a stdout client for exporting traces.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_stdout_client();
    /// ```
    pub fn with_stdout_client(self) -> HttpTracerProviderBuilder<StdoutClient, I, P> {
        HttpTracerProviderBuilder {
            client: StdoutClient::new(),
            install_global: self.install_global,
            id_generator: self.id_generator,
            propagators: self.propagators,
            exporter_type: self.exporter_type,
            _propagator_type: std::marker::PhantomData,
        }
    }

    /// Sets a custom HTTP client for exporting traces.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    /// use reqwest::Client;
    /// use opentelemetry_sdk::propagation::TraceContextPropagator;
    /// use opentelemetry_sdk::trace::RandomIdGenerator;
    ///
    /// let client = Client::new();
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_http_client(client);
    /// ```
    pub fn with_http_client<NewC>(self, client: NewC) -> HttpTracerProviderBuilder<NewC, I, P>
    where
        NewC: HttpClient + 'static,
    {
        HttpTracerProviderBuilder {
            client,
            install_global: self.install_global,
            id_generator: self.id_generator,
            propagators: self.propagators,
            exporter_type: self.exporter_type,
            _propagator_type: std::marker::PhantomData,
        }
    }

    /// Adds a custom text map propagator.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    /// use opentelemetry_sdk::propagation::TraceContextPropagator;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_text_map_propagator(TraceContextPropagator::new());
    /// ```
    pub fn with_text_map_propagator<NewP>(
        mut self,
        propagator: NewP,
    ) -> HttpTracerProviderBuilder<C, I, NewP>
    where
        NewP: TextMapPropagator + Send + Sync + 'static,
    {
        self.propagators.push(Box::new(propagator));
        HttpTracerProviderBuilder {
            client: self.client,
            install_global: self.install_global,
            id_generator: self.id_generator,
            propagators: self.propagators,
            exporter_type: self.exporter_type,
            _propagator_type: std::marker::PhantomData,
        }
    }

    /// Adds the default text map propagator (TraceContextPropagator).
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_default_text_map_propagator();
    /// ```
    pub fn with_default_text_map_propagator(mut self) -> Self {
        self.propagators
            .push(Box::new(TraceContextPropagator::new()));
        self
    }

    /// Adds the XRay text map propagator.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_xray_text_map_propagator();
    /// ```
    pub fn with_xray_text_map_propagator(mut self) -> Self {
        self.propagators.push(Box::new(XrayPropagator::new()));
        self
    }

    /// Sets a custom ID generator.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    /// use opentelemetry_sdk::trace::RandomIdGenerator;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_id_generator(RandomIdGenerator::default());
    /// ```
    pub fn with_id_generator<NewI>(
        self,
        id_generator: NewI,
    ) -> HttpTracerProviderBuilder<C, NewI, P>
    where
        NewI: IdGenerator + Send + Sync + 'static,
    {
        HttpTracerProviderBuilder {
            client: self.client,
            install_global: self.install_global,
            id_generator,
            propagators: self.propagators,
            exporter_type: self.exporter_type,
            _propagator_type: std::marker::PhantomData,
        }
    }

    /// Sets the default ID generator (RandomIdGenerator).
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_default_id_generator();
    /// ```
    pub fn with_default_id_generator(self) -> HttpTracerProviderBuilder<C, RandomIdGenerator, P> {
        self.with_id_generator(RandomIdGenerator::default())
    }

    /// Sets the XRay ID generator.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_xray_id_generator();
    /// ```
    pub fn with_xray_id_generator(self) -> HttpTracerProviderBuilder<C, XrayIdGenerator, P> {
        self.with_id_generator(XrayIdGenerator::default())
    }

    /// Configures the builder to use a simple exporter.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_simple_exporter();
    /// ```
    pub fn with_simple_exporter(mut self) -> Self {
        self.exporter_type = ExporterType::Simple;
        self
    }

    /// Configures the builder to use a batch exporter.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .with_batch_exporter();
    /// ```
    pub fn with_batch_exporter(mut self) -> Self {
        self.exporter_type = ExporterType::Batch;
        self
    }

    /// Enables or disables global installation of the tracer provider.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default()
    ///     .enable_global(true);
    /// ```
    pub fn enable_global(mut self, set_global: bool) -> Self {
        self.install_global = set_global;
        self
    }

    /// Builds the TracerProvider with the configured settings.
    ///
    /// # Errors
    ///
    /// Returns a `BuilderError` if the exporter fails to build
    pub fn build(self) -> Result<SdkTracerProvider, BuilderError> {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_protocol(crate::protocol::get_protocol())
            .with_http_client(self.client)
            .build()
            .map_err(BuilderError::ExporterBuildError)?;

        let builder = match self.exporter_type {
            ExporterType::Simple => SdkTracerProvider::builder().with_simple_exporter(exporter),
            ExporterType::Batch => SdkTracerProvider::builder()
                .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio),
        };

        let tracer_provider = builder
            .with_resource(crate::resource::get_lambda_resource())
            .with_id_generator(self.id_generator)
            .build();

        if !self.propagators.is_empty() {
            let composite_propagator =
                opentelemetry::propagation::TextMapCompositePropagator::new(self.propagators);
            opentelemetry::global::set_text_map_propagator(composite_propagator);
        }

        if self.install_global {
            opentelemetry::global::set_tracer_provider(tracer_provider.clone());
        }

        Ok(tracer_provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::Span;
    use opentelemetry::trace::Tracer;
    use opentelemetry::trace::TracerProvider;

    #[test]
    fn test_default_builder() {
        let builder = HttpTracerProviderBuilder::default();
        assert!(!builder.install_global);
        assert!(matches!(builder.exporter_type, ExporterType::Simple));
    }

    #[tokio::test]
    async fn test_successful_build() -> Result<(), BuilderError> {
        let provider = HttpTracerProviderBuilder::default().build()?;

        let tracer = provider.tracer("test");
        let span = tracer.span_builder("test_span").start(&tracer);
        assert!(span.is_recording());
        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_propagators() -> Result<(), BuilderError> {
        let provider = HttpTracerProviderBuilder::default()
            .with_text_map_propagator(TraceContextPropagator::new())
            .with_text_map_propagator(XrayPropagator::new())
            .build()?;

        let tracer = provider.tracer("test");
        let span = tracer.span_builder("test_span").start(&tracer);
        assert!(span.is_recording());
        Ok(())
    }

    #[tokio::test]
    async fn test_custom_id_generator() -> Result<(), BuilderError> {
        let provider = HttpTracerProviderBuilder::default()
            .with_id_generator(XrayIdGenerator::default())
            .build()?;

        let tracer = provider.tracer("test");
        let span = tracer.span_builder("test_span").start(&tracer);
        assert!(span.is_recording());
        Ok(())
    }
}
