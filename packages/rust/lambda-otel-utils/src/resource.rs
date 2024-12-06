use opentelemetry::KeyValue;
use opentelemetry_aws::detector::LambdaResourceDetector;
use opentelemetry_sdk::{resource::ResourceDetector, Resource};
use opentelemetry_semantic_conventions as semconv;
use std::env;
use std::time::Duration;

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
    let service_name =
        match env::var("OTEL_SERVICE_NAME").or_else(|_| env::var("AWS_LAMBDA_FUNCTION_NAME")) {
            Ok(name) => name,
            Err(_) => "unknown-service".to_string(),
        };
    Resource::default()
        .merge(&LambdaResourceDetector.detect(Duration::default()))
        .merge(&Resource::new(vec![
            KeyValue::new(semconv::resource::SERVICE_NAME, service_name),
            KeyValue::new(semconv::resource::PROCESS_RUNTIME_NAME, "rust"),
            KeyValue::new(
                semconv::resource::PROCESS_RUNTIME_VERSION,
                rustc_version_runtime::version().to_string(),
            ),
        ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sealed_test::prelude::*;

    #[sealed_test(env = [
        ("OTEL_SERVICE_NAME", "test-service"),
        ("AWS_LAMBDA_FUNCTION_NAME", "test-function"),
    ])]
    fn test_get_lambda_resource_with_otel_service_name() {
        let resource = get_lambda_resource();
        assert_eq!(
            resource.get(opentelemetry_semantic_conventions::resource::SERVICE_NAME.into()),
            Some("test-service".into())
        );
    }

    #[sealed_test(env = [
        ("AWS_LAMBDA_FUNCTION_NAME", "test-function"),
    ])]
    fn test_get_lambda_resource_with_aws_lambda_function_name() {
        let resource = get_lambda_resource();
        assert_eq!(
            resource.get(opentelemetry_semantic_conventions::resource::SERVICE_NAME.into()),
            Some("test-function".into())
        );
    }

    #[sealed_test]
    fn test_get_lambda_resource_without_env_vars() {
        let resource = get_lambda_resource();
        assert_eq!(
            resource.get(opentelemetry_semantic_conventions::resource::SERVICE_NAME.into()),
            Some("unknown-service".into())
        );
    }

    #[test]
    fn test_runtime_attributes() {
        let resource = get_lambda_resource();

        // Test process.runtime.name
        assert_eq!(
            resource.get(opentelemetry_semantic_conventions::resource::PROCESS_RUNTIME_NAME.into()),
            Some("rust".into())
        );

        // Test process.runtime.version is present and follows semver format
        let version = resource
            .get(opentelemetry_semantic_conventions::resource::PROCESS_RUNTIME_VERSION.into())
            .expect("Runtime version should be present");
        assert!(
            version.to_string().contains('.'),
            "Version should be in semver format"
        );
    }
}
