use aws_lambda_events::event::apigw::ApiGatewayV2httpRequest;
use lambda_otel_lite::{create_traced_handler, init_telemetry, TelemetryConfig};
use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
use opentelemetry::trace::Status;
use rand::Rng;
use serde_json::Value;
use std::borrow::Cow;
use std::fmt::{self, Display};
use tracing::{error, info, instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
// Define error type as a simple enum
#[derive(Debug)]
enum ErrorType {
    Expected,
    Unexpected,
}

impl Display for ErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Simple nested function that creates its own span.
#[instrument(skip(event), level = "info", err)]
async fn nested_function(event: &ApiGatewayV2httpRequest) -> Result<String, ErrorType> {
    info!("Nested function called");

    // Simulate random errors if the path is /error
    if event.raw_path.as_deref() == Some("/error") {
        let r: f64 = rand::rng().random();
        if r < 0.25 {
            return Err(ErrorType::Expected);
        } else if r < 0.5 {
            return Err(ErrorType::Unexpected);
        }
    }

    Ok("success".to_string())
}

/// Simple Hello World Lambda function using lambda-otel-lite.
///
/// This example demonstrates basic OpenTelemetry setup with lambda-otel-lite.
/// It creates spans for each invocation and logs the event payload using span events.
async fn handler(event: LambdaEvent<ApiGatewayV2httpRequest>) -> Result<Value, Error> {
    // Extract request ID from the event for correlation
    let request_id = &event.context.request_id;
    let current_span = tracing::Span::current();

    // Set request ID as span attribute
    current_span.set_attribute("request.id", request_id.to_string());

    // Log the full event payload like in Python version
    info!(
        event = serde_json::to_string(&event.payload).unwrap_or_default(),
        "handling request"
    );

    // Call the nested function and handle potential errors
    match nested_function(&event.payload).await {
        Ok(_) => {
            // Return a successful response
            Ok(serde_json::json!({
                "statusCode": 200,
                "body": format!("Hello from request {}", request_id)
            }))
        }
        Err(ErrorType::Expected) => {
            // Log the error and return a 400 Bad Request
            error!("Expected error occurred");

            // Return a 400 Bad Request for expected errors
            Ok(serde_json::json!({
                "statusCode": 400,
                "body": format!("{{\"message\": \"This is an expected error\"}}")
            }))
        }
        Err(ErrorType::Unexpected) => {
            // For other errors, propagate them up
            error!("Unexpected error occurred");

            // Set span status to ERROR like in Python version
            current_span.set_status(Status::Error {
                description: Cow::Borrowed("Unexpected error occurred"),
            });

            Err(Error::from("Unexpected error occurred"))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize telemetry with default config
    let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;

    // Create the traced handler
    let handler = create_traced_handler("simple-handler", completion_handler, handler);

    // Use it directly with the runtime
    Runtime::new(service_fn(handler)).run().await
}
