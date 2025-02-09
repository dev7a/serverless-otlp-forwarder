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
//! let resource = Resource::new(vec![
//!     KeyValue::new("service.name", "my-service"),
//!     KeyValue::new("service.version", "1.0.0"),
//! ]);
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

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use std::env;
use urlencoding::decode;

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
/// # Returns
///
/// Returns a [`Resource`] containing all detected and configured attributes.
///
/// # Examples
///
/// Basic usage with automatic detection:
///
/// ```no_run
/// use lambda_otel_lite::get_lambda_resource;
///
/// let resource = get_lambda_resource();
/// ```
///
/// Using with custom attributes:
///
/// ```no_run
/// use lambda_otel_lite::get_lambda_resource;
/// use std::env;
///
/// // Set custom attributes
/// env::set_var("OTEL_RESOURCE_ATTRIBUTES", "deployment.stage=prod,team=backend");
/// env::set_var("OTEL_SERVICE_NAME", "payment-processor");
///
/// let resource = get_lambda_resource();
/// ```
///
/// Merging with additional resource attributes:
///
/// ```no_run
/// use lambda_otel_lite::get_lambda_resource;
/// use opentelemetry::KeyValue;
/// use opentelemetry_sdk::Resource;
///
/// // Get base Lambda resource
/// let lambda_resource = get_lambda_resource();
///
/// // Create additional resource
/// let extra_resource = Resource::new(vec![
///     KeyValue::new("service.version", "1.0.0"),
///     KeyValue::new("deployment.environment", "staging"),
/// ]);
///
/// // Merge resources (Lambda attributes take precedence)
/// let final_resource = extra_resource.merge(&lambda_resource);
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

    if let Ok(function_name) = env::var("AWS_LAMBDA_FUNCTION_NAME") {
        attributes.push(KeyValue::new("faas.name", function_name.clone()));
        // Use function name as service name if not set
        if env::var("OTEL_SERVICE_NAME").is_err() {
            attributes.push(KeyValue::new("service.name", function_name));
        }
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

    // Add custom attributes from OTEL_RESOURCE_ATTRIBUTES
    if let Ok(attrs) = env::var("OTEL_RESOURCE_ATTRIBUTES") {
        for pair in attrs.split(',') {
            let parts: Vec<&str> = pair.split('=').collect();
            if parts.len() == 2 {
                let key = parts[0].trim().to_string();
                let value = parts[1].trim();
                if !value.is_empty() {
                    if let Ok(decoded_value) = decode(value) {
                        let owned_value = decoded_value.into_owned();
                        attributes.push(KeyValue::new(key, owned_value));
                    }
                }
            }
        }
    }

    // Create resource and merge with default resource
    let resource = Resource::new(attributes);
    Resource::default().merge(&resource)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::Value;
    use serial_test::serial;
    use std::env;

    fn cleanup_env() {
        env::remove_var("AWS_REGION");
        env::remove_var("AWS_LAMBDA_FUNCTION_NAME");
        env::remove_var("AWS_LAMBDA_FUNCTION_VERSION");
        env::remove_var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE");
        env::remove_var("AWS_LAMBDA_LOG_STREAM_NAME");
        env::remove_var("OTEL_SERVICE_NAME");
        env::remove_var("OTEL_RESOURCE_ATTRIBUTES");
    }

    #[test]
    #[serial]
    fn test_get_lambda_resource_with_standard_env() {
        cleanup_env();

        // Set up test environment
        env::set_var("AWS_REGION", "us-west-2");
        env::set_var("AWS_LAMBDA_FUNCTION_NAME", "test-function");
        env::set_var("AWS_LAMBDA_FUNCTION_VERSION", "$LATEST");
        env::set_var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE", "128");
        env::set_var("AWS_LAMBDA_LOG_STREAM_NAME", "2024/01/01/[$LATEST]abc123");

        let resource = get_lambda_resource();
        let schema = resource.schema_url().unwrap_or("");
        assert!(schema.is_empty()); // Default resource has no schema URL

        assert_eq!(
            resource.get("cloud.provider".into()),
            Some(Value::String("aws".into()))
        );
        assert_eq!(
            resource.get("cloud.region".into()),
            Some(Value::String("us-west-2".into()))
        );
        assert_eq!(
            resource.get("faas.name".into()),
            Some(Value::String("test-function".into()))
        );
        assert_eq!(
            resource.get("service.name".into()),
            Some(Value::String("test-function".into()))
        ); // Falls back to function name
        assert_eq!(
            resource.get("faas.version".into()),
            Some(Value::String("$LATEST".into()))
        );
        assert_eq!(
            resource.get("faas.max_memory".into()),
            Some(Value::I64(134217728)) // 128 * 1024 * 1024
        );
        assert_eq!(
            resource.get("faas.instance".into()),
            Some(Value::String("2024/01/01/[$LATEST]abc123".into()))
        );

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
        assert_eq!(
            resource.get("service.name".into()),
            Some(Value::String("custom-service".into()))
        ); // Uses OTEL_SERVICE_NAME
        assert_eq!(
            resource.get("faas.name".into()),
            Some(Value::String("test-function".into()))
        ); // Still sets faas.name

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
        assert_eq!(
            resource.get("custom.attr".into()),
            Some(Value::String("value".into()))
        );
        assert_eq!(
            resource.get("deployment.stage".into()),
            Some(Value::String("prod".into()))
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
        assert_eq!(
            resource.get("custom.attr".into()),
            Some(Value::String("hello world".into()))
        );
        assert_eq!(
            resource.get("tag".into()),
            Some(Value::String("value=test".into()))
        );

        cleanup_env();
    }

    #[test]
    #[serial]
    fn test_get_lambda_resource_with_empty_environment() {
        cleanup_env();

        let resource = get_lambda_resource();
        assert!(resource.schema_url().unwrap_or("").is_empty());
        assert!(resource.get("cloud.provider".into()).is_none());
        assert!(resource.get("cloud.region".into()).is_none());
        assert!(resource.get("faas.name".into()).is_none());

        cleanup_env();
    }
}
