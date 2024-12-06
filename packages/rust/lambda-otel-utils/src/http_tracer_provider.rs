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
//!
//! This example demonstrates how to use the `HttpTracerProviderBuilder` to configure and build
//! a TracerProvider with custom settings for use in a Lambda function.

use delegate::delegate;
use opentelemetry::propagation::text_map_propagator::FieldIter;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry::{
    trace::{SpanId, TraceError, TraceId},
    Context,
};

use opentelemetry_aws::trace::{XrayIdGenerator, XrayPropagator};
use opentelemetry_http::HttpClient;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{IdGenerator, RandomIdGenerator, TracerProvider as SdkTracerProvider},
};
use otlp_stdout_client::StdoutClient;
use std::{env, fmt::Debug};

#[derive(Debug)]
enum ExporterType {
    Simple,
    Batch,
}

#[derive(Debug)]
struct PropagatorWrapper(Box<dyn TextMapPropagator + Send + Sync>);

impl TextMapPropagator for PropagatorWrapper {
    delegate! {
        to self.0 {
            fn inject_context(&self, cx: &Context, carrier: &mut dyn Injector);
            fn extract_with_context(&self, cx: &Context, carrier: &dyn Extractor) -> Context;
            fn fields(&self) -> FieldIter<'_>;
        }
    }
}

#[derive(Debug)]
struct IdGeneratorWrapper(Box<dyn IdGenerator + Send + Sync>);

impl IdGenerator for IdGeneratorWrapper {
    delegate! {
        to self.0 {
            fn new_trace_id(&self) -> TraceId;
            fn new_span_id(&self) -> SpanId;
        }
    }
}

/// Builder for configuring and initializing a TracerProvider.
///
/// This struct provides a fluent interface for configuring various aspects of the
/// OpenTelemetry tracing setup, including the exporter type, propagators, and ID generators.
///
/// # Examples
///
/// ```
/// use lambda_otel_utils::HttpTracerProviderBuilder;
/// use opentelemetry_sdk::trace::{TracerProvider, Tracer};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let tracer_provider: TracerProvider = HttpTracerProviderBuilder::default()
///     .with_stdout_client()
///     .with_default_text_map_propagator()
///     .with_default_id_generator()
///     .enable_global(true)
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct HttpTracerProviderBuilder<C: HttpClient + 'static = StdoutClient> {
    client: Option<C>,
    install_global: bool,
    propagators: Vec<PropagatorWrapper>,
    id_generator: Option<IdGeneratorWrapper>,
    exporter_type: ExporterType,
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
impl Default for HttpTracerProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpTracerProviderBuilder {
    /// Creates a new `HttpTracerProviderBuilder` with default settings.
    ///
    /// The default exporter type is determined by the `LAMBDA_OTEL_SPAN_PROCESSOR` environment variable:
    /// - "batch" - Uses batch span processor
    /// - "simple" - Uses simple span processor (default)
    /// - Any other value will default to simple span processor with a warning
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::new();
    /// ```
    pub fn new() -> Self {
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

        Self {
            client: None,
            install_global: false,
            propagators: Vec::new(),
            id_generator: None,
            exporter_type,
        }
    }

    /// Configures the builder to use a stdout client for exporting traces.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default().with_stdout_client();
    /// ```
    pub fn with_stdout_client(mut self) -> Self {
        self.client = Some(StdoutClient::new());
        self
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
    pub fn with_text_map_propagator<P>(mut self, propagator: P) -> Self
    where
        P: TextMapPropagator + Send + Sync + 'static,
    {
        self.propagators
            .push(PropagatorWrapper(Box::new(propagator)));
        self
    }

    /// Adds the default text map propagator (TraceContextPropagator).
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default().with_default_text_map_propagator();
    /// ```
    pub fn with_default_text_map_propagator(mut self) -> Self {
        self.propagators
            .push(PropagatorWrapper(Box::new(TraceContextPropagator::new())));
        self
    }

    /// Adds the XRay text map propagator.
    pub fn with_xray_text_map_propagator(mut self) -> Self {
        self.propagators
            .push(PropagatorWrapper(Box::new(XrayPropagator::new())));
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
    pub fn with_id_generator<I>(mut self, id_generator: I) -> Self
    where
        I: IdGenerator + Send + Sync + 'static,
    {
        self.id_generator = Some(IdGeneratorWrapper(Box::new(id_generator)));
        self
    }

    /// Sets the default ID generator (RandomIdGenerator).
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default().with_default_id_generator();
    /// ```
    pub fn with_default_id_generator(mut self) -> Self {
        self.id_generator = Some(IdGeneratorWrapper(Box::new(RandomIdGenerator::default())));
        self
    }

    /// Adds the XRay ID generator.
    pub fn with_xray_id_generator(mut self) -> Self {
        self.id_generator = Some(IdGeneratorWrapper(Box::new(XrayIdGenerator::default())));
        self
    }

    /// Enables or disables global installation of the tracer provider.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default().enable_global(true);
    /// ```
    pub fn enable_global(mut self, set_global: bool) -> Self {
        self.install_global = set_global;
        self
    }

    /// Configures the builder to use a simple exporter.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    ///
    /// let builder = HttpTracerProviderBuilder::default().with_simple_exporter();
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
    /// let builder = HttpTracerProviderBuilder::default().with_batch_exporter();
    /// ```
    pub fn with_batch_exporter(mut self) -> Self {
        self.exporter_type = ExporterType::Batch;
        self
    }

    /// Builds the `TracerProvider` with the configured settings.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpTracerProviderBuilder;
    /// use opentelemetry_sdk::trace::{TracerProvider, Tracer};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let tracer_provider: TracerProvider = HttpTracerProviderBuilder::default()
    ///     .with_stdout_client()
    ///     .with_default_text_map_propagator()
    ///     .with_default_id_generator()
    ///     .enable_global(true)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> Result<SdkTracerProvider, TraceError> {
        // Build exporter
        fn build_exporter(
            client: Option<impl HttpClient + 'static>,
        ) -> Result<opentelemetry_otlp::SpanExporter, TraceError> {
            // Build exporter
            let mut builder = opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_protocol(crate::protocol::get_protocol());

            if let Some(client) = client {
                builder = builder.with_http_client(client);
            }
            builder.build()
        }

        // Get tracer provider builder
        fn get_tracer_provider_builder(
            exporter_type: ExporterType,
            exporter: opentelemetry_otlp::SpanExporter,
        ) -> opentelemetry_sdk::trace::Builder {
            match exporter_type {
                ExporterType::Simple => SdkTracerProvider::builder().with_simple_exporter(exporter),
                ExporterType::Batch => SdkTracerProvider::builder()
                    .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio),
            }
        }

        // Main build logic
        let id_generator = self
            .id_generator
            .unwrap_or_else(|| IdGeneratorWrapper(Box::new(RandomIdGenerator::default())));
        let tracer_provider = get_tracer_provider_builder(
            self.exporter_type,
            build_exporter(self.client).expect("Failed to build exporter"),
        )
        .with_resource(crate::resource::get_lambda_resource())
        .with_id_generator(id_generator)
        .build();

        // Setup propagators
        if !self.propagators.is_empty() {
            let composite_propagator = opentelemetry::propagation::TextMapCompositePropagator::new(
                self.propagators.into_iter().map(|p| p.0).collect(),
            );
            opentelemetry::global::set_text_map_propagator(composite_propagator);
        }

        // Optionally install global provider
        if self.install_global {
            opentelemetry::global::set_tracer_provider(tracer_provider.clone());
        }
        Ok(tracer_provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_tracer_provider_builder_default() {
        let builder = HttpTracerProviderBuilder::default();
        assert!(matches!(builder.exporter_type, ExporterType::Simple));
        assert!(builder.client.is_none());
        assert!(builder.propagators.is_empty());
        assert!(builder.id_generator.is_none());
    }

    #[test]
    fn test_http_tracer_provider_builder_customization() {
        let builder = HttpTracerProviderBuilder::new()
            .with_stdout_client()
            .with_default_text_map_propagator()
            .with_default_id_generator()
            .enable_global(true)
            .with_batch_exporter();

        assert!(builder.client.is_some());
        assert_eq!(builder.propagators.len(), 1);
        assert!(builder.id_generator.is_some());
        assert!(matches!(builder.exporter_type, ExporterType::Batch));
    }
}
