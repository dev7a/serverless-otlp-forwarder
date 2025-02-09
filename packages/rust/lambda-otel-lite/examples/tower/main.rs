use aws_lambda_events::event::apigw::ApiGatewayV2httpRequest;
use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};
use lambda_runtime::{tower::ServiceBuilder, Error, LambdaEvent, Runtime};
use serde_json::Value;
use tracing::{info, instrument};

/// Simple Lambda function that logs the event and returns it.
///
/// This example demonstrates basic OpenTelemetry setup with lambda-otel-lite.
/// It creates spans for each invocation and logs the event payload.
async fn handler(event: LambdaEvent<ApiGatewayV2httpRequest>) -> Result<Value, Error> {
    // Extract request ID from the event for correlation
    let request_id = &event.context.request_id;
    info!(request_id, "handling request");

    nested_function().await;
    // Return a simple response
    Ok(serde_json::json!({
        "statusCode": 200,
        "body": format!("Hello from request {}", request_id)
    }))
}

/// Simple nested function that logs a message.
///
/// This function is used to demonstrate the nested span functionality of OpenTelemetry.
#[instrument(skip_all)]
async fn nested_function() {
    tracing::debug!("Nested function called");
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize telemetry with default configuration
    let completion_handler = init_telemetry(TelemetryConfig::default()).await?;

    // Build service with OpenTelemetry tracing middleware
    let service = ServiceBuilder::new()
        .layer(OtelTracingLayer::new(completion_handler).with_name("tower-handler"))
        .service_fn(handler);

    // Create and run the Lambda runtime
    let runtime = Runtime::new(service);
    runtime.run().await
}
