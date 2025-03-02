//! Utilities for configuring and managing OpenTelemetry tracing subscribers.
//!
//! This module provides a builder pattern for configuring tracing subscribers with
//! OpenTelemetry support, along with utility functions for creating tracing and metrics layers.
//!
//! # Examples
//!
//! Basic usage with default configuration:
//!
//! ```rust,no_run
//! use lambda_otel_utils::{
//!     HttpTracerProviderBuilder,
//!     HttpMeterProviderBuilder,
//!     init_otel_subscriber
//! };
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//! let tracer_provider = HttpTracerProviderBuilder::default()
//!     .with_stdout_client()
//!     .build()?;
//!
//! let meter_provider = HttpMeterProviderBuilder::default()
//!     .with_stdout_client()
//!     .with_meter_name("my-service")
//!     .with_export_interval(Duration::from_secs(30))
//!     .build()?;
//!
//! // Initialize with default settings
//! init_otel_subscriber(tracer_provider, meter_provider, "my-service")?;
//! # Ok(())
//! # }
//! ```
//!
//! Custom configuration using the builder:
//!
//! ```rust,no_run
//! use lambda_otel_utils::{
//!     HttpTracerProviderBuilder,
//!     HttpMeterProviderBuilder,
//!     OpenTelemetrySubscriberBuilder
//! };
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
//! let tracer_provider = HttpTracerProviderBuilder::default()
//!     .with_stdout_client()
//!     .build()?;
//!
//! let meter_provider = HttpMeterProviderBuilder::default()
//!     .with_stdout_client()
//!     .with_meter_name("my-service")
//!     .with_export_interval(Duration::from_secs(30))
//!     .build()?;
//!
//! // Custom configuration
//! OpenTelemetrySubscriberBuilder::new()
//!     .with_tracer_provider(tracer_provider)
//!     .with_meter_provider(meter_provider)
//!     .with_service_name("my-service")
//!     .with_env_filter(true)
//!     .with_json_format(true)
//!     .init()?;
//! # Ok(())
//! # }
//! ```

use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::{metrics::SdkMeterProvider, trace::SdkTracerProvider};
use tracing_subscriber::prelude::*;

/// Returns a tracing layer configured with the given tracer provider and tracer name.
///
/// This function creates an OpenTelemetry tracing layer that can be used with a
/// tracing subscriber to enable OpenTelemetry integration.
///
/// # Type Parameters
///
/// * `S` - The type of the subscriber that this layer will be applied to.
///
/// # Arguments
///
/// * `tracer_provider` - A reference to the tracer provider.
/// * `tracer_name` - The name of the tracer to be used (must have static lifetime).
///
/// # Returns
///
/// A `tracing_opentelemetry::OpenTelemetryLayer` configured with the specified tracer.
///
/// # Examples
///
/// ```rust,no_run
/// use tracing_subscriber::Registry;
/// use lambda_otel_utils::{HttpTracerProviderBuilder, create_otel_tracing_layer};
/// use tracing_subscriber::prelude::*;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
/// let tracer_provider = HttpTracerProviderBuilder::default()
///     .with_stdout_client()
///     .build()?;
///
/// let subscriber = Registry::default()
///     .with(create_otel_tracing_layer(&tracer_provider, "my-service"));
/// # Ok(())
/// # }
/// ```
pub fn create_otel_tracing_layer<S>(
    tracer_provider: &SdkTracerProvider,
    tracer_name: &'static str,
) -> tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    tracing_opentelemetry::OpenTelemetryLayer::new(tracer_provider.tracer(tracer_name))
}

/// Returns a metrics layer configured with the given meter provider.
///
/// This function creates an OpenTelemetry metrics layer that can be used with a
/// tracing subscriber to enable metrics collection.
///
/// # Type Parameters
///
/// * `S` - The type of the subscriber that this layer will be applied to.
///
/// # Arguments
///
/// * `meter_provider` - A reference to the meter provider.
///
/// # Returns
///
/// A `tracing_opentelemetry::MetricsLayer` configured with the specified meter provider.
///
/// # Examples
///
/// ```rust,no_run
/// use tracing_subscriber::Registry;
/// use tracing_subscriber::prelude::*;
/// use lambda_otel_utils::{HttpMeterProviderBuilder, create_otel_metrics_layer};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
/// let meter_provider = HttpMeterProviderBuilder::default()
///     .with_stdout_client()
///     .with_meter_name("my-service")
///     .with_export_interval(Duration::from_secs(30))
///     .build()?;
///
/// let subscriber = Registry::default()
///     .with(create_otel_metrics_layer(&meter_provider));
/// # Ok(())
/// # }
/// ```
pub fn create_otel_metrics_layer<S>(
    meter_provider: &SdkMeterProvider,
) -> tracing_opentelemetry::MetricsLayer<S>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    tracing_opentelemetry::MetricsLayer::new(meter_provider.clone())
}

/// A builder for configuring and creating a tracing subscriber with OpenTelemetry support.
///
/// This builder provides a flexible way to configure various aspects of the tracing
/// subscriber, including OpenTelemetry integration, environment filters, and JSON formatting.
///
/// # Examples
///
/// ```rust,no_run
/// use lambda_otel_utils::{
///     OpenTelemetrySubscriberBuilder,
///     HttpTracerProviderBuilder,
///     HttpMeterProviderBuilder
/// };
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
/// let tracer_provider = HttpTracerProviderBuilder::default()
///     .with_stdout_client()
///     .build()?;
///
/// let meter_provider = HttpMeterProviderBuilder::default()
///     .with_stdout_client()
///     .with_meter_name("my-service")
///     .with_export_interval(Duration::from_secs(30))
///     .build()?;
///
/// OpenTelemetrySubscriberBuilder::new()
///     .with_tracer_provider(tracer_provider)
///     .with_meter_provider(meter_provider)
///     .with_service_name("my-service")
///     .with_env_filter(true)
///     .with_json_format(true)
///     .init()?;
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct OpenTelemetrySubscriberBuilder {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    service_name: Option<&'static str>,
    with_env_filter: bool,
    env_filter_string: Option<String>,
    with_json_format: bool,
}

impl OpenTelemetrySubscriberBuilder {
    /// Creates a new builder instance with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the tracer provider for the subscriber.
    pub fn with_tracer_provider(mut self, provider: SdkTracerProvider) -> Self {
        self.tracer_provider = Some(provider);
        self
    }

    /// Sets the meter provider for the subscriber.
    pub fn with_meter_provider(mut self, provider: SdkMeterProvider) -> Self {
        self.meter_provider = Some(provider);
        self
    }

    /// Sets the service name for the subscriber.
    pub fn with_service_name(mut self, name: &'static str) -> Self {
        self.service_name = Some(name);
        self
    }

    /// Enables or disables the environment filter.
    ///
    /// When enabled, the subscriber will use the `RUST_LOG` environment variable
    /// to configure logging levels.
    pub fn with_env_filter(mut self, enabled: bool) -> Self {
        self.with_env_filter = enabled;
        self
    }

    /// Sets a custom string for the environment filter.
    ///
    /// This allows specifying a filter directive string directly instead of using
    /// the `RUST_LOG` environment variable.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lambda_otel_utils::OpenTelemetrySubscriberBuilder;
    ///
    /// OpenTelemetrySubscriberBuilder::new()
    ///     .with_env_filter_string("info,my_crate=debug")
    ///     .init().unwrap();
    /// ```
    pub fn with_env_filter_string(mut self, filter: impl Into<String>) -> Self {
        self.env_filter_string = Some(filter.into());
        self
    }

    /// Enables or disables JSON formatting for log output.
    pub fn with_json_format(mut self, enabled: bool) -> Self {
        self.with_json_format = enabled;
        self
    }

    /// Builds and sets the global default subscriber.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the subscriber was successfully set as the global default,
    /// or an error if something went wrong.
    pub fn init(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let subscriber = tracing_subscriber::registry::Registry::default();

        // Create optional layers
        let otel_layer = self
            .tracer_provider
            .as_ref()
            .zip(self.service_name)
            .map(|(provider, name)| create_otel_tracing_layer(provider, name));

        let metrics_layer = self.meter_provider.as_ref().map(create_otel_metrics_layer);

        let env_filter = if self.with_env_filter {
            if let Some(filter_string) = self.env_filter_string {
                Some(tracing_subscriber::EnvFilter::new(filter_string))
            } else {
                Some(tracing_subscriber::EnvFilter::from_default_env())
            }
        } else {
            None
        };

        let fmt_layer = if self.with_json_format {
            Some(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_target(false)
                    .without_time(),
            )
        } else {
            None
        };

        // Build the subscriber by conditionally adding layers
        let subscriber = subscriber
            .with(otel_layer)
            .with(metrics_layer)
            .with(env_filter)
            .with(fmt_layer);

        // Set the subscriber as the default
        tracing::subscriber::set_global_default(subscriber)?;

        Ok(())
    }
}

/// Convenience function to create and initialize a default OpenTelemetry subscriber.
///
/// This function provides a simple way to set up a tracing subscriber with OpenTelemetry
/// support using sensible defaults.
///
/// # Arguments
///
/// * `tracer_provider` - The tracer provider to use.
/// * `meter_provider` - The meter provider to use.
/// * `service_name` - The name of the service (must have static lifetime).
///
/// # Returns
///
/// Returns `Ok(())` if the subscriber was successfully initialized, or an error if
/// something went wrong.
///
/// # Examples
///
/// ```rust,no_run
/// use lambda_otel_utils::{
///     init_otel_subscriber,
///     HttpTracerProviderBuilder,
///     HttpMeterProviderBuilder
/// };
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
/// let tracer_provider = HttpTracerProviderBuilder::default()
///     .with_stdout_client()
///     .build()?;
///
/// let meter_provider = HttpMeterProviderBuilder::default()
///     .with_stdout_client()
///     .with_meter_name("my-service")
///     .with_export_interval(Duration::from_secs(30))
///     .build()?;
///
/// init_otel_subscriber(tracer_provider, meter_provider, "my-service")?;
/// # Ok(())
/// # }
/// ```
pub fn init_otel_subscriber(
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    service_name: &'static str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    OpenTelemetrySubscriberBuilder::new()
        .with_tracer_provider(tracer_provider)
        .with_meter_provider(meter_provider)
        .with_service_name(service_name)
        .init()
}
