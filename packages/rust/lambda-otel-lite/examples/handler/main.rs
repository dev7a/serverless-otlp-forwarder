use aws_lambda_events::event::apigw::{ApiGatewayV2httpRequest, ApiGatewayV2httpResponse};
use lambda_otel_lite::{create_traced_handler, init_telemetry, TelemetryConfig};
use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
use opentelemetry::trace::Status;
use rand::Rng;
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
    tracing::event!(
        name: "example.info",
        tracing::Level::INFO,
        "event.body" = "Nested function called",
        "event.severity_text" = "info",
        "event.severity_number" = 9
    );

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
async fn handler(
    event: LambdaEvent<ApiGatewayV2httpRequest>,
) -> Result<ApiGatewayV2httpResponse, Error> {
    // Extract request ID from the event for correlation
    let request_id = &event.context.request_id;
    let current_span = tracing::Span::current();

    // Set request ID as span attribute
    current_span.set_attribute("request.id", request_id.to_string());

    // Log the full event payload like in Python version
    tracing::event!(
        name: "example.info",
        tracing::Level::INFO,
        "event.body" = serde_json::to_string(&event.payload).unwrap_or_default(),
        "event.severity_text" = "info",
        "event.severity_number" = 9
    );

    // Call the nested function and handle potential errors
    match nested_function(&event.payload).await {
        Ok(_) => {
            // Return a successful response
            Ok(ApiGatewayV2httpResponse {
                status_code: 200,
                body: Some(format!("Hello from request {}", request_id).into()),
                ..Default::default()
            })
        }
        Err(ErrorType::Expected) => {
            // Log the error and return a 400 Bad Request
            tracing::event!(
              name:"example.error",
              tracing::Level::ERROR,
              "event.body" = "This is an expected error",
              "event.severity_text" = "error",
              "event.severity_number" = 10,
            );

            // Return a 400 Bad Request for expected errors
            Ok(ApiGatewayV2httpResponse {
                status_code: 400,
                body: Some("{{\"message\": \"This is an expected error\"}}".into()),
                ..Default::default()
            })
        }
        Err(ErrorType::Unexpected) => {
            // For other errors, propagate them up
            tracing::event!(
              name:"example.error",
              tracing::Level::ERROR,
              "event.body" = "This is an unexpected error",
              "event.severity_text" = "error",
              "event.severity_number" = 10,
            );

            // Set span status to ERROR like in Python version
            current_span.set_status(Status::Error {
                description: Cow::Borrowed("Unexpected error occurred"),
            });

            // propagate the error
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
