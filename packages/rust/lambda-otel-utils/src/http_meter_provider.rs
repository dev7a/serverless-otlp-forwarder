//! This module provides utilities for configuring and building an OpenTelemetry MeterProvider
//! specifically tailored for use in AWS Lambda environments.
//!
//! It includes:
//! - `HttpMeterProviderBuilder`: A builder struct for configuring and initializing a MeterProvider.
//!
//! The module supports various configuration options, including:
//! - Custom HTTP clients for exporting metrics
//! - Setting custom meter names
//! - Configuring periodic exporters
//! - Integration with Lambda resource attributes

use std::time::Duration;

use opentelemetry_http::HttpClient;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    error::OTelSdkError,
    metrics::{PeriodicReader, SdkMeterProvider},
};
use otlp_stdout_client::StdoutClient;

/// Builder for configuring and initializing a MeterProvider.
///
/// This struct provides a fluent interface for configuring various aspects of the
/// OpenTelemetry metrics setup, including the exporter configuration and meter names.
///
/// # Examples
///
/// ```
/// use lambda_otel_utils::HttpMeterProviderBuilder;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), opentelemetry_sdk::error::Error> {
///     let meter_provider = HttpMeterProviderBuilder::default()
///         .with_stdout_client()
///         .with_meter_name("my-service")
///         .with_export_interval(Duration::from_secs(60))
///         .build()?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct HttpMeterProviderBuilder<C: HttpClient + 'static = StdoutClient> {
    client: Option<C>,
    meter_name: Option<&'static str>,
    export_interval: Duration,
    export_timeout: Duration,
    install_global: bool,
}

impl Default for HttpMeterProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpMeterProviderBuilder {
    /// Creates a new `HttpMeterProviderBuilder` with default settings.
    pub fn new() -> Self {
        Self {
            client: None,
            meter_name: None,
            export_interval: Duration::from_secs(60),
            export_timeout: Duration::from_secs(10),
            install_global: false,
        }
    }

    /// Configures the builder to use a stdout client for exporting metrics.
    pub fn with_stdout_client(mut self) -> Self {
        self.client = Some(StdoutClient::new());
        self
    }

    /// Sets the meter name.
    ///
    /// # Arguments
    ///
    /// * `meter_name` - A static string reference (string literal)
    pub fn with_meter_name(mut self, meter_name: &'static str) -> Self {
        self.meter_name = Some(meter_name);
        self
    }

    /// Sets the export interval for periodic metric collection.
    pub fn with_export_interval(mut self, interval: Duration) -> Self {
        self.export_interval = interval;
        self
    }

    /// Sets the export timeout for metric collection.
    pub fn with_export_timeout(mut self, timeout: Duration) -> Self {
        self.export_timeout = timeout;
        self
    }

    /// Enables or disables global installation of the meter provider.
    ///
    /// # Examples
    ///
    /// ```
    /// use lambda_otel_utils::HttpMeterProviderBuilder;
    ///
    /// let builder = HttpMeterProviderBuilder::default().enable_global(true);
    /// ```
    pub fn enable_global(mut self, set_global: bool) -> Self {
        self.install_global = set_global;
        self
    }

    /// Builds the `MeterProvider` with the configured settings.
    pub fn build(self) -> Result<SdkMeterProvider, OTelSdkError> {
        let mut exporter_builder = opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_protocol(crate::protocol::get_protocol())
            .with_timeout(self.export_timeout);

        if let Some(client) = self.client {
            exporter_builder = exporter_builder.with_http_client(client);
        }

        let exporter = exporter_builder
            .build()
            .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;

        let reader = PeriodicReader::builder(exporter)
            .with_interval(self.export_interval)
            .build();

        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(crate::resource::get_lambda_resource())
            .build();

        // Optionally install global provider
        if self.install_global {
            opentelemetry::global::set_meter_provider(provider.clone());
        }

        Ok(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_meter_provider_builder_default() {
        let builder = HttpMeterProviderBuilder::default();
        assert!(builder.client.is_none());
        assert!(builder.meter_name.is_none());
        assert_eq!(builder.export_interval, Duration::from_secs(60));
        assert_eq!(builder.export_timeout, Duration::from_secs(10));
        assert!(!builder.install_global);
    }

    #[test]
    fn test_http_meter_provider_builder_customization() {
        let builder = HttpMeterProviderBuilder::new()
            .with_stdout_client()
            .with_meter_name("test-meter")
            .with_export_interval(Duration::from_secs(30))
            .with_export_timeout(Duration::from_secs(5))
            .enable_global(true);

        assert!(builder.client.is_some());
        assert_eq!(builder.meter_name, Some("test-meter"));
        assert_eq!(builder.export_interval, Duration::from_secs(30));
        assert_eq!(builder.export_timeout, Duration::from_secs(5));
        assert!(builder.install_global);
    }
}
