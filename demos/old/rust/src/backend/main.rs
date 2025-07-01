use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::sync::Arc;

use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use lambda_lw_http_router::{define_router, route};
use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};
use lambda_runtime::Error as LambdaError;
use lambda_runtime::{tower::ServiceBuilder, LambdaEvent, Runtime};
use rand::Rng; // Import Rng trait for random number generation
use demo_lambda::spanevent::{span_event, Level};
mod db;

#[derive(Clone, Debug)]
struct AppState {
    dynamodb_client: DynamoDbClient,
    table_name: String,
    error_probability: f64,
}

define_router!(event = ApiGatewayProxyRequest, state = AppState);

#[tracing::instrument(skip_all)]
#[route(path = "/quotes", method = "POST")]
async fn handle_post_quotes(ctx: RouteContext) -> Result<Value, LambdaError> {
    let timestamp = chrono::Utc::now().to_rfc3339();
    let post_body = match ctx.event.body.as_ref() {
        Some(body) => match serde_json::from_str(body) {
            Ok(parsed_body) => parsed_body,
            Err(e) => {
                return Ok(json!({
                    "statusCode": 400,
                    "headers": {"Content-Type": "application/json"},
                    "body": json!({
                        "error": "Invalid JSON in request body",
                        "details": e.to_string()
                    }).to_string()
                }))
            }
        },
        None => {
            return Ok(json!({
                "statusCode": 400,
                "headers": {"Content-Type": "application/json"},
                "body": json!({
                    "error": "Missing request body"
                }).to_string()
            }))
        }
    };
    let id = {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_string(&post_body)?.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let random_value: f64 = rand::rng().random();

    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let span = tracing::Span::current();
    span.set_attribute("test-attribute", "test-value");

    if random_value < ctx.state.error_probability {
        let error_msg = format!(
            "Intentional error triggered for write_item (probability: {}, random: {})",
            ctx.state.error_probability, random_value
        );
        // tracing::event!(
        //     name: "backend.write_item.error",
        //     tracing::Level::ERROR,
        //     "event.body" = error_msg.clone(),
        //     "event.severity_text" = "error",
        //     "event.severity_number" = 17,
        //     "error_probability" = ctx.state.error_probability,
        //     "random_value" = random_value
        // );
        use demo_lambda::spanevent::{span_event, Level};
        use opentelemetry::KeyValue;

        span_event(
            "backend.write_item.error",
            error_msg.clone(),
            Level::Error,
            vec![
                KeyValue::new("error_probability", ctx.state.error_probability),
                KeyValue::new("random_value", random_value),
            ],
            None,
        );
        return Ok(json!({
            "statusCode": 500,
            "headers": {"Content-Type": "application/json"},
            "body": json!({ "message": error_msg.clone() }).to_string()
        }));
    }

    // Use db::write_item
    db::write_item(&id, &timestamp, &post_body, &ctx.state).await?;
    // tracing::event!(
    //     name: "backend.write_item.success",
    //     tracing::Level::INFO,
    //     "event.body" = "Item successfully written to DynamoDB",
    //     "event.severity_text" = "info",
    //     "event.severity_number" = 9,
    // );
    span_event(
        "backend.write_item.success",
        "Item successfully written to DynamoDB",
        Level::Info,
        vec![],
        None,
    );

    Ok(json!({
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": serde_json::to_string(&json!({
            "message": "Item successfully written to DynamoDB",
            "received": post_body,
            "timestamp": timestamp
        }))?
    }))
}

#[route(path = "/quotes", method = "GET")]
async fn handle_get_quotes(ctx: RouteContext) -> Result<Value, LambdaError> {
    // Use db::list_items
    let items = db::list_items(&ctx.state).await?;
    Ok(json!({
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": serde_json::to_string(&items)?
    }))
}

#[route(path = "/quotes/{id}", method = "GET")]
async fn handle_get_quote(ctx: RouteContext) -> Result<Value, LambdaError> {
    let id = ctx.params.get("id").expect("id parameter is required");
    // Use db::read_item
    let item = db::read_item(id, &ctx.state).await?;

    match item {
        Some(value) => Ok(json!({
            "statusCode": 200,
            "headers": {"Content-Type": "application/json"},
            "body": serde_json::to_string(&value)?
        })),
        None => Ok(json!({
            "statusCode": 404,
            "headers": {"Content-Type": "application/json"},
            "body": json!({"message": "Item not found"}).to_string()
        })),
    }
}
async fn handler(
    event: LambdaEvent<ApiGatewayProxyRequest>,
    router: Arc<Router>,
    state: Arc<AppState>,
) -> Result<Value, LambdaError> {
    use opentelemetry::trace::get_active_span;

    get_active_span(|span_ref| {
        println!("Span Context in handler: {:?}", span_ref.span_context());
        println!("Is Recording in handler: {:?}", span_ref.is_recording());
    });
    router.handle_request(event, state).await
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    // Initialize telemetry
    let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;

    // Read error probability from environment, default to 0.0
    let error_probability = env::var("ERROR_PROBABILITY")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    let aws_config = aws_config::load_from_env().await;
    let state = Arc::new(AppState {
        dynamodb_client: DynamoDbClient::new(&aws_config),
        table_name: env::var("TABLE_NAME").expect("TABLE_NAME must be set"),
        error_probability,
    });

    let router = Arc::new(RouterBuilder::from_registry().build());

    // Define the handler function

    // Create a traced handler with the captured router and state
    // let traced_handler =
    //     create_traced_handler("backend-handler", completion_handler, move |event| {
    //         let router_clone = router.clone();
    //         let state_clone = state.clone();
    //         handler(event, router_clone, state_clone)
    //     });
    let service = ServiceBuilder::new()
        .layer(OtelTracingLayer::new(completion_handler).with_name("tower-handler"))
        .service_fn(move |event| {
            let router_clone = router.clone();
            let state_clone = state.clone();
            handler(event, router_clone, state_clone)
        });

    // Run the Lambda runtime with our traced handler
    Runtime::new(service).run().await
}
