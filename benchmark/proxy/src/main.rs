use aws_sdk_lambda::primitives::Blob;
use aws_sdk_lambda::Client as LambdaClient;
use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

/// Request payload for the proxy function
#[derive(Deserialize)]
struct ProxyRequest {
    /// Target Lambda function to invoke
    target: String,
    /// Payload to send to the target function
    payload: Value,
}

/// Response with timing measurements
#[derive(Serialize)]
struct ProxyResponse {
    /// Time taken for the invocation in milliseconds
    invocation_time_ms: f64,
    /// Response from the target function
    response: Value,
}

/// Main handler for the proxy function
async fn function_handler(
    event: LambdaEvent<ProxyRequest>,
    lambda_client: &LambdaClient,
) -> Result<ProxyResponse, Error> {
    let request = event.payload;

    // Extract X-Ray header from payload if it exists
    let xray_header_value = if let Some(headers) = request.payload.get("headers") {
        if let Some(xray) = headers.get("X-Amzn-Trace-Id") {
            xray.as_str().map(|s| s.to_string())
        } else {
            None
        }
    } else {
        // Also check root level
        request
            .payload
            .get("X-Amzn-Trace-Id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    // Prepare the request builder
    let req_builder = lambda_client
        .invoke()
        .function_name(&request.target)
        .payload(Blob::new(serde_json::to_vec(&request.payload)?));

    // Start timing
    let start: Instant = Instant::now();

    // Invoke target function with X-Ray header if available
    let invoke_result = if let Some(header_value) = xray_header_value {
        req_builder
            .customize()
            .mutate_request(move |http_req| {
                http_req
                    .headers_mut()
                    .insert("X-Amzn-Trace-Id", header_value.clone());
            })
            .send()
            .await?
    } else {
        req_builder.send().await?
    };

    // Calculate elapsed time
    let elapsed = start.elapsed();
    let invocation_time_ms = elapsed.as_secs_f64() * 1000.0;

    // Parse response payload
    let response = if let Some(payload) = invoke_result.payload() {
        serde_json::from_slice::<Value>(payload.as_ref())?
    } else {
        json!({
            "status": "no_payload",
            "message": "The Lambda function did not return a payload"
        })
    };
    tracing::info!(response = serde_json::to_string(&response).unwrap_or_default());

    Ok(ProxyResponse {
        invocation_time_ms,
        response,
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    // Initialize AWS Lambda client
    let config = aws_config::load_from_env().await;
    let lambda_client = LambdaClient::new(&config);

    // Create a closure that clones the Lambda client
    let handler_func = move |event| {
        let client = lambda_client.clone();
        async move { function_handler(event, &client).await }
    };

    run(service_fn(handler_func)).await
}
