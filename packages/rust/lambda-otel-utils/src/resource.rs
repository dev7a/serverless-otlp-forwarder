use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use std::env;

/// Retrieves the Lambda resource with the service name.
///
/// This function attempts to retrieve the service name from the `OTEL_SERVICE_NAME` environment variable.
/// If that variable is not set, it falls back to the `AWS_LAMBDA_FUNCTION_NAME` environment variable.
/// If neither variable is set, it defaults to "unknown-service".
///
/// The function then creates a new `Resource` with the detected Lambda resource information
/// and merges it with a new `Resource` containing the service name key-value pair.
///
/// # Returns
///
/// A `Resource` representing the Lambda resource with the service name.
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

    // create resource with standard attributes and merge with custom attributes
    Resource::builder().with_attributes(attributes).build()
}
