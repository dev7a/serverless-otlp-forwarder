//! Resource attribute management for Lambda functions.
//!
//! This module provides functionality for managing OpenTelemetry resource attributes
//! in a Lambda environment. It automatically detects and sets standard Lambda attributes
//! and allows for custom attribute configuration through environment variables.
//!
//! # Automatic FAAS Attributes
//!
//! The module automatically sets relevant FAAS attributes based on the Lambda context:
//!
//! ## Resource Attributes
//! - `cloud.provider`: Set to "aws"
//! - `cloud.region`: From AWS_REGION
//! - `faas.name`: From AWS_LAMBDA_FUNCTION_NAME
//! - `faas.version`: From AWS_LAMBDA_FUNCTION_VERSION
//! - `faas.instance`: From AWS_LAMBDA_LOG_STREAM_NAME
//! - `faas.max_memory`: From AWS_LAMBDA_FUNCTION_MEMORY_SIZE
//! - `service.name`: From OTEL_SERVICE_NAME or function name
//!
//! # Configuration
//!
//! ## Custom Attributes
//!
//! Additional attributes can be set via the `OTEL_RESOURCE_ATTRIBUTES` environment variable
//! in the format: `key=value,key2=value2`. Values can be URL-encoded if they contain
//! special characters:
//!
//! ```bash
//! # Setting custom attributes
//! OTEL_RESOURCE_ATTRIBUTES="deployment.stage=prod,custom.tag=value%20with%20spaces"
//! ```
//!
//! ## Service Name
//!
//! The service name can be configured in two ways:
//!
//! 1. Using `OTEL_SERVICE_NAME` environment variable:
//! ```bash
//! OTEL_SERVICE_NAME="my-custom-service"
//! ```
//!
//! 2. Through the [`TelemetryConfig`](crate::TelemetryConfig) builder:
//! ```no_run
//! use lambda_otel_lite::{TelemetryConfig, init_telemetry};
//! use opentelemetry::KeyValue;
//! use opentelemetry_sdk::Resource;
//!
//! # async fn example() -> Result<(), lambda_runtime::Error> {
//! let resource = Resource::builder()
//!     .with_attributes(vec![
//!         KeyValue::new("service.name", "my-service"),
//!         KeyValue::new("service.version", "1.0.0"),
//!     ])
//!     .build();
//!
//! let config = TelemetryConfig::builder()
//!     .resource(resource)
//!     .build();
//!
//! let _completion_handler = init_telemetry(config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Integration
//!
//! This module is primarily used by the [`init_telemetry`](crate::init_telemetry) function
//! to configure the OpenTelemetry tracer provider. The detected resource attributes are
//! automatically attached to all spans created by the tracer.
//!
//! See the [`telemetry`](crate::telemetry) module for more details on initialization
//! and configuration options.

use crate::constants::defaults;
use crate::constants::{env_vars, resource_attributes};
use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use std::env;

/// Get default Lambda resource attributes.
///
/// This function automatically detects and sets standard Lambda attributes from environment
/// variables and allows for custom attribute configuration through `OTEL_RESOURCE_ATTRIBUTES`.
///
/// # Environment Variables
///
/// - `AWS_REGION`: Sets `cloud.region`
/// - `AWS_LAMBDA_FUNCTION_NAME`: Sets `faas.name` and default `service.name`
/// - `AWS_LAMBDA_FUNCTION_VERSION`: Sets `faas.version`
/// - `AWS_LAMBDA_FUNCTION_MEMORY_SIZE`: Sets `faas.max_memory`
/// - `AWS_LAMBDA_LOG_STREAM_NAME`: Sets `faas.instance`
/// - `OTEL_SERVICE_NAME`: Overrides default service name
/// - `OTEL_RESOURCE_ATTRIBUTES`: Additional attributes in key=value format
///
/// # Configuration Attributes
///
/// The following configuration attributes are set in the resource **only when**
/// the corresponding environment variables are explicitly set:
///
/// - `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Sets `lambda_otel_lite.extension.span_processor_mode`
/// - `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Sets `lambda_otel_lite.lambda_span_processor.queue_size`
/// - `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: Sets `lambda_otel_lite.otlp_stdout_span_exporter.compression_level`
///
/// # Returns
///
/// Returns a [`Resource`] containing all detected and configured attributes.
///
/// # Examples
///
/// Basic usage with environment variables:
///
/// ```no_run
/// use lambda_otel_lite::resource::get_lambda_resource;
/// use opentelemetry::KeyValue;
///
/// // Get resource with Lambda environment attributes
/// let resource = get_lambda_resource();
/// ```
///
/// Adding custom attributes:
///
/// ```no_run
/// use lambda_otel_lite::resource::get_lambda_resource;
/// use opentelemetry::KeyValue;
/// use opentelemetry_sdk::Resource;
///
/// // Get Lambda resource
/// let lambda_resource = get_lambda_resource();
///
/// // Create custom resource
/// let extra_resource = Resource::builder()
///     .with_attributes(vec![
///         KeyValue::new("deployment.stage", "prod"),
///         KeyValue::new("team", "backend"),
///     ])
///     .build();
///
/// // Combine resources (custom attributes take precedence)
/// // Create a new resource with all attributes
/// let mut all_attributes = vec![
///     KeyValue::new("deployment.stage", "prod"),
///     KeyValue::new("team", "backend"),
/// ];
///
/// // Add lambda attributes (could be done more programmatically in real code)
/// all_attributes.push(KeyValue::new("cloud.provider", "aws"));
/// all_attributes.push(KeyValue::new("faas.name", "my-function"));
///
/// let final_resource = Resource::builder()
///     .with_attributes(all_attributes)
///     .build();
/// ```
///
/// # Integration with Telemetry Config
///
/// This function is automatically called by [`init_telemetry`](crate::init_telemetry)
/// when no custom resource is provided. To override or extend these attributes, use
/// the [`TelemetryConfig`](crate::TelemetryConfig) builder:
///
/// ```no_run
/// use lambda_otel_lite::{TelemetryConfig, init_telemetry};
/// use opentelemetry_sdk::Resource;
///
/// # async fn example() -> Result<(), lambda_runtime::Error> {
/// // Get base Lambda resource
/// let base_resource = lambda_otel_lite::get_lambda_resource();
///
/// // Configure telemetry with the resource
/// let config = TelemetryConfig::builder()
///     .resource(base_resource)
///     .build();
///
/// let _completion_handler = init_telemetry(config).await?;
/// # Ok(())
/// # }
/// ```
pub fn get_lambda_resource() -> Resource {
    let mut attributes = Vec::new();

    // Add standard Lambda attributes
    if let Ok(region) = env::var("AWS_REGION") {
        attributes.push(KeyValue::new("cloud.provider", "aws"));
        attributes.push(KeyValue::new("cloud.region", region));
    }

    if let Ok(function_name) = env::var(env_vars::AWS_LAMBDA_FUNCTION_NAME) {
        attributes.push(KeyValue::new("faas.name", function_name.clone()));
    }

    if let Ok(version) = env::var("AWS_LAMBDA_FUNCTION_VERSION") {
        attributes.push(KeyValue::new("faas.version", version));
    }

    if let Ok(memory) = env::var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE") {
        if let Ok(memory_mb) = memory.parse::<i64>() {
            let memory_bytes = memory_mb * 1024 * 1024;
            attributes.push(KeyValue::new("faas.max_memory", memory_bytes));
        }
    }

    if let Ok(log_stream) = env::var("AWS_LAMBDA_LOG_STREAM_NAME") {
        attributes.push(KeyValue::new("faas.instance", log_stream));
    }

    // Set service name with fallback logic:
    // 1. Use OTEL_SERVICE_NAME if defined
    // 2. Fall back to AWS_LAMBDA_FUNCTION_NAME if available
    // 3. Fall back to "unknown_service" if neither is available
    let service_name = env::var(env_vars::SERVICE_NAME)
        .or_else(|_| env::var(env_vars::AWS_LAMBDA_FUNCTION_NAME))
        .unwrap_or_else(|_| defaults::SERVICE_NAME.to_string());

    attributes.push(KeyValue::new("service.name", service_name));

    // Add configuration attributes only when environment variables are explicitly set
    if let Ok(mode) = env::var(env_vars::PROCESSOR_MODE) {
        attributes.push(KeyValue::new(resource_attributes::PROCESSOR_MODE, mode));
    }

    if let Ok(queue_size) = env::var(env_vars::QUEUE_SIZE) {
        if let Ok(size) = queue_size.parse::<i64>() {
            attributes.push(KeyValue::new(resource_attributes::QUEUE_SIZE, size));
        }
    }

    if let Ok(compression_level) = env::var(env_vars::COMPRESSION_LEVEL) {
        if let Ok(level) = compression_level.parse::<i64>() {
            attributes.push(KeyValue::new(resource_attributes::COMPRESSION_LEVEL, level));
        }
    }

    // create resource with standard attributes and merge with custom attributes
    Resource::builder().with_attributes(attributes).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    fn cleanup_env() {
        env::remove_var("AWS_REGION");
        env::remove_var(env_vars::AWS_LAMBDA_FUNCTION_NAME);
        env::remove_var("AWS_LAMBDA_FUNCTION_VERSION");
        env::remove_var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE");
        env::remove_var("AWS_LAMBDA_LOG_STREAM_NAME");
        env::remove_var(env_vars::SERVICE_NAME);
        env::remove_var(env_vars::RESOURCE_ATTRIBUTES);
        env::remove_var(env_vars::QUEUE_SIZE);
        env::remove_var(env_vars::PROCESSOR_MODE);
        env::remove_var(env_vars::COMPRESSION_LEVEL);
    }

    // Helper function to find an attribute by key
    fn find_attr<'a>(
        attrs: &'a [(&'a str, &'a opentelemetry::Value)],
        key: &str,
    ) -> Option<&'a opentelemetry::Value> {
        attrs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    #[test]
    #[serial]
    fn test_get_lambda_resource_with_standard_env() {
        cleanup_env();

        // Set up test environment
        env::set_var("AWS_REGION", "us-west-2");
        env::set_var(env_vars::AWS_LAMBDA_FUNCTION_NAME, "test-function");
        env::set_var("AWS_LAMBDA_FUNCTION_VERSION", "$LATEST");
        env::set_var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE", "128");
        env::set_var("AWS_LAMBDA_LOG_STREAM_NAME", "2024/01/01/[$LATEST]abc123");

        let resource = get_lambda_resource();
        let schema = resource.schema_url().unwrap_or("");
        assert!(schema.is_empty()); // Default resource has no schema URL

        // Check attributes using the resource's attribute iterator
        let attrs: Vec<_> = resource.iter().map(|(k, v)| (k.as_str(), v)).collect();

        assert_eq!(
            find_attr(&attrs, "cloud.provider"),
            Some(&opentelemetry::Value::String("aws".into()))
        );
        assert_eq!(
            find_attr(&attrs, "cloud.region"),
            Some(&opentelemetry::Value::String("us-west-2".into()))
        );
        assert_eq!(
            find_attr(&attrs, "faas.name"),
            Some(&opentelemetry::Value::String("test-function".into()))
        );
        assert_eq!(
            find_attr(&attrs, "faas.version"),
            Some(&opentelemetry::Value::String("$LATEST".into()))
        );

        // Verify memory is converted to bytes
        assert_eq!(
            find_attr(&attrs, "faas.max_memory"),
            Some(&opentelemetry::Value::I64(128 * 1024 * 1024))
        );
        assert_eq!(
            find_attr(&attrs, "faas.instance"),
            Some(&opentelemetry::Value::String(
                "2024/01/01/[$LATEST]abc123".into()
            ))
        );

        cleanup_env();
    }

    #[test]
    #[serial]
    fn test_get_lambda_resource_with_no_env() {
        cleanup_env();

        let resource = get_lambda_resource();
        let attrs: Vec<_> = resource.iter().map(|(k, v)| (k.as_str(), v)).collect();

        // No attributes should be set
        assert!(find_attr(&attrs, "cloud.provider").is_none());
        assert!(find_attr(&attrs, "cloud.region").is_none());
        assert!(find_attr(&attrs, "faas.name").is_none());

        cleanup_env();
    }

    #[test]
    #[serial]
    fn test_get_lambda_resource_with_custom_service_name() {
        cleanup_env();

        // Set up test environment
        env::set_var("AWS_LAMBDA_FUNCTION_NAME", "test-function");
        env::set_var("OTEL_SERVICE_NAME", "custom-service");

        let resource = get_lambda_resource();
        let attrs: Vec<_> = resource.iter().collect();

        let find_attr = |key: &str| -> Option<&opentelemetry::Value> {
            attrs.iter().find(|kv| kv.0.as_str() == key).map(|kv| kv.1)
        };

        assert_eq!(
            find_attr("service.name"),
            Some(&opentelemetry::Value::String("custom-service".into()))
        );
        assert_eq!(
            find_attr("faas.name"),
            Some(&opentelemetry::Value::String("test-function".into()))
        );

        cleanup_env();
    }

    #[test]
    #[serial]
    fn test_get_lambda_resource_with_custom_attributes() {
        cleanup_env();

        // Set up test environment
        env::set_var(
            "OTEL_RESOURCE_ATTRIBUTES",
            "custom.attr=value,deployment.stage=prod",
        );

        let resource = get_lambda_resource();
        let attrs: Vec<_> = resource.iter().collect();

        let find_attr = |key: &str| -> Option<&opentelemetry::Value> {
            attrs.iter().find(|kv| kv.0.as_str() == key).map(|kv| kv.1)
        };

        assert_eq!(
            find_attr("custom.attr"),
            Some(&opentelemetry::Value::String("value".into()))
        );
        assert_eq!(
            find_attr("deployment.stage"),
            Some(&opentelemetry::Value::String("prod".into()))
        );

        cleanup_env();
    }

    #[test]
    #[serial]
    fn test_get_lambda_resource_with_encoded_attributes() {
        cleanup_env();

        // Set up test environment
        env::set_var(
            "OTEL_RESOURCE_ATTRIBUTES",
            "custom.attr=hello%20world,tag=value%3Dtest",
        );

        let resource = get_lambda_resource();
        let attrs: Vec<_> = resource.iter().collect();

        let find_attr = |key: &str| -> Option<&opentelemetry::Value> {
            attrs.iter().find(|kv| kv.0.as_str() == key).map(|kv| kv.1)
        };

        assert_eq!(
            find_attr("custom.attr"),
            Some(&opentelemetry::Value::String("hello%20world".into()))
        );
        assert_eq!(
            find_attr("tag"),
            Some(&opentelemetry::Value::String("value%3Dtest".into()))
        );

        cleanup_env();
    }

    #[test]
    #[serial]
    fn test_resource_attributes_only_set_when_env_vars_present() {
        cleanup_env();

        // Create resource with no environment variables set
        let resource = get_lambda_resource();
        let attrs: Vec<_> = resource.iter().map(|(k, v)| (k.as_str(), v)).collect();

        // Verify that configuration attributes are not set
        assert!(find_attr(&attrs, resource_attributes::QUEUE_SIZE).is_none());
        assert!(find_attr(&attrs, resource_attributes::PROCESSOR_MODE).is_none());
        assert!(find_attr(&attrs, resource_attributes::COMPRESSION_LEVEL).is_none());

        // Set environment variables
        env::set_var(env_vars::QUEUE_SIZE, "4096");
        env::set_var(env_vars::PROCESSOR_MODE, "async");
        env::set_var(env_vars::COMPRESSION_LEVEL, "9");

        // Create resource with environment variables set
        let resource_with_env = get_lambda_resource();
        let attrs_with_env: Vec<_> = resource_with_env
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect();

        // Verify that configuration attributes are set with correct values
        assert_eq!(
            find_attr(&attrs_with_env, resource_attributes::QUEUE_SIZE),
            Some(&opentelemetry::Value::I64(4096))
        );
        assert_eq!(
            find_attr(&attrs_with_env, resource_attributes::PROCESSOR_MODE),
            Some(&opentelemetry::Value::String("async".into()))
        );
        assert_eq!(
            find_attr(&attrs_with_env, resource_attributes::COMPRESSION_LEVEL),
            Some(&opentelemetry::Value::I64(9))
        );

        cleanup_env();
    }

    #[test]
    #[serial]
    fn test_resource_attributes_not_set_with_invalid_env_vars() {
        cleanup_env();

        // Set invalid environment variables
        env::set_var(env_vars::QUEUE_SIZE, "not_a_number");
        env::set_var(env_vars::COMPRESSION_LEVEL, "high");

        // Create resource with invalid environment variables
        let resource = get_lambda_resource();
        let attrs: Vec<_> = resource.iter().map(|(k, v)| (k.as_str(), v)).collect();

        // Verify that configuration attributes with invalid values are not set
        assert!(find_attr(&attrs, resource_attributes::QUEUE_SIZE).is_none());
        assert!(find_attr(&attrs, resource_attributes::COMPRESSION_LEVEL).is_none());

        // But the mode attribute should be set since it's a string
        env::set_var(env_vars::PROCESSOR_MODE, "custom_mode");
        let resource_with_mode = get_lambda_resource();
        let attrs_with_mode: Vec<_> = resource_with_mode
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect();

        assert_eq!(
            find_attr(&attrs_with_mode, resource_attributes::PROCESSOR_MODE),
            Some(&opentelemetry::Value::String("custom_mode".into()))
        );

        cleanup_env();
    }
}
