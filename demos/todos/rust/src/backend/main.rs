use aws_config::meta::region::RegionProviderChain;
use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use lambda_lw_http_router::{define_router, route}; // Added route, removed RouterBuilder
use lambda_otel_lite::events::{event, EventLevel};
use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};
use lambda_runtime::{tower::ServiceBuilder, Error as LambdaError, LambdaEvent, Runtime}; // Removed service_fn
use rand::Rng;
use serde_json::{json, Value};
use std::env;
use std::sync::Arc; // Added Arc
use tracing::error;
use uuid::Uuid;

use todo_app::response::{api_empty_response, api_error_response, api_json_response};

mod db;
use db::{delete_item, list_items, read_item, update_item, write_item, AppState};

// Define the router with ApiGatewayProxyRequest event and AppState
define_router!(event = ApiGatewayProxyRequest, state = AppState); // RouteContext will be generated

/// Create the router for the API
fn create_router() -> Router {
    // Returns Router type from define_router!
    RouterBuilder::from_registry().build() // Use from_registry
}

// Handler for creating a new TODO
#[tracing::instrument(name = "api/backend/create", skip_all)]
#[route(method = "POST", path = "/todos")]
async fn create_todo_handler(ctx: RouteContext) -> Result<Value, LambdaError> {
    // Maybe inject an error
    if should_inject_error(&ctx.state) {
        return inject_error("Error creating TODO");
    }

    // Parse the request body
    let body_str = ctx.event.body.as_deref().unwrap_or_default();

    // Parse the JSON
    let mut todo_payload: Value = match serde_json::from_str(body_str) {
        Ok(todo) => todo,
        Err(err) => {
            return Ok(api_error_response(400, &format!("Invalid JSON: {err}")));
        }
    };

    // Generate an ID if not provided
    let id = match todo_payload.get("id").and_then(Value::as_str) {
        Some(id_str) => id_str.to_string(),
        None => {
            let new_id = Uuid::new_v4().to_string();
            todo_payload["id"] = json!(new_id.clone());
            new_id
        }
    };

    // Generate a timestamp and add it as created_at to the payload for the response
    let creation_timestamp = chrono::Utc::now().to_rfc3339();
    todo_payload["created_at"] = json!(creation_timestamp.clone());

    // Write the item to DynamoDB, passing the original id and the creation_timestamp
    // The write_item function will use the creation_timestamp for DynamoDB's 'timestamp' attribute
    // and also for 'created_at' if the payload doesn't already have it (which it now will).
    match write_item(&id, &creation_timestamp, &todo_payload, &ctx.state).await {
        Ok(_) => {
            event()
                .level(EventLevel::Info)
                .message(format!("Successfully created TODO {id}"))
                .attribute("event.name", "backend.api.create_todo.success")
                .attribute("todo.id", id.clone())
                .call();
            Ok(api_json_response(201, todo_payload))
        }
        Err(err) => {
            error!("Error creating TODO: {}", err);
            event()
                .level(EventLevel::Error)
                .message(format!("Error creating TODO: {err}"))
                .attribute("event.name", "backend.api.create_todo.error")
                .attribute("todo.id", id)
                .call();
            Ok(api_error_response(
                500,
                &format!("Error creating TODO: {err}"),
            ))
        }
    }
}

// Handler for getting a TODO by ID
#[tracing::instrument(name = "api/backend/get", skip_all)]
#[route(method = "GET", path = "/todos/{id}")]
async fn get_todo(ctx: RouteContext) -> Result<Value, LambdaError> {
    if should_inject_error(&ctx.state) {
        return inject_error("Error getting TODO");
    }

    let id = ctx
        .params
        .get("id")
        .ok_or_else(|| LambdaError::from("Missing 'id' path parameter"))?;

    match read_item(id, &ctx.state).await {
        Ok(Some(todo)) => {
            event()
                .level(EventLevel::Info)
                .message(format!("Successfully retrieved TODO {id}"))
                .attribute("event.name", "backend.api.get_todo.success")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_json_response(200, todo))
        }
        Ok(None) => {
            event()
                .level(EventLevel::Info)
                .message(format!("TODO {id} not found"))
                .attribute("event.name", "backend.api.get_todo.not_found")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_error_response(404, &format!("TODO {id} not found")))
        }
        Err(err) => {
            error!("Error getting TODO: {}", err);
            event()
                .level(EventLevel::Error)
                .message(format!("Error getting TODO {id}: {err}"))
                .attribute("event.name", "backend.api.get_todo.error")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_error_response(
                500,
                &format!("Error getting TODO: {err}"),
            ))
        }
    }
}

// Handler for listing all TODOs
#[tracing::instrument(name = "api/backend/list", skip_all)]
#[route(method = "GET", path = "/todos")]
async fn list_todo(ctx: RouteContext) -> Result<Value, LambdaError> {
    if should_inject_error(&ctx.state) {
        return inject_error("Error listing TODOs");
    }

    // Parse pagination parameters
    let limit: usize = ctx
        .event
        .query_string_parameters
        .first("limit")
        .and_then(|val_str| val_str.parse().ok())
        .unwrap_or(50); // Default limit of 50 items

    let offset: usize = ctx
        .event
        .query_string_parameters
        .first("offset")
        .and_then(|val_str| val_str.parse().ok())
        .unwrap_or(0); // Default offset of 0

    // Parse completion filter
    let completed_filter: Option<bool> = ctx
        .event
        .query_string_parameters
        .first("completed")
        .and_then(|val_str| val_str.parse().ok());

    // Get todos with pagination using the consolidated list_items function
    match list_items(&ctx.state, completed_filter, Some(limit), Some(offset)).await {
        Ok((todos, total_count)) => {
            // Create response with metadata
            let response_body = json!({
                "metadata": {
                    "total": total_count,
                    "limit": limit,
                    "offset": offset
                },
                "items": todos
            });

            let todos_len = todos.len();
            event()
                .level(EventLevel::Info)
                .message(format!(
                    "Successfully listed {todos_len} TODOs (total: {total_count})"
                ))
                .attribute("event.name", "backend.api.list_todos.success")
                .attribute("todo.count", todos.len() as i64)
                .attribute("todo.total", total_count as i64)
                .attribute("pagination.limit", limit as i64)
                .attribute("pagination.offset", offset as i64)
                .call();

            Ok(api_json_response(200, response_body))
        }
        Err(err) => {
            error!("Error listing TODOs: {}", err);
            event()
                .level(EventLevel::Error)
                .message(format!("Error listing TODOs: {err}"))
                .attribute("event.name", "backend.api.list_todos.error")
                .call();
            Ok(api_error_response(
                500,
                &format!("Error listing TODOs: {err}"),
            ))
        }
    }
}

// Handler for updating a TODO
#[tracing::instrument(name = "api/backend/update", skip_all)]
#[route(method = "PUT", path = "/todos/{id}")]
async fn update_todo(ctx: RouteContext) -> Result<Value, LambdaError> {
    if should_inject_error(&ctx.state) {
        return inject_error("Error updating TODO");
    }

    let id = ctx
        .params
        .get("id")
        .ok_or_else(|| LambdaError::from("Missing 'id' path parameter"))?;
    let body_str = ctx.event.body.as_deref().unwrap_or_default();
    let todo_payload: Value = match serde_json::from_str(body_str) {
        Ok(todo) => todo,
        Err(err) => {
            return Ok(api_error_response(400, &format!("Invalid JSON: {err}")));
        }
    };

    match update_item(id, &todo_payload, &ctx.state).await {
        Ok(Some(updated_todo)) => {
            event()
                .level(EventLevel::Info)
                .message(format!("Successfully updated TODO {id}"))
                .attribute("event.name", "backend.api.update_todo.success")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_json_response(200, updated_todo))
        }
        Ok(None) => {
            event()
                .level(EventLevel::Info)
                .message(format!("TODO {id} not found for update"))
                .attribute("event.name", "backend.api.update_todo.not_found")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_error_response(404, &format!("TODO {id} not found")))
        }
        Err(err) => {
            error!("Error updating TODO: {}", err);
            event()
                .level(EventLevel::Error)
                .message(format!("Error updating TODO {id}: {err}"))
                .attribute("event.name", "backend.api.update_todo.error")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_error_response(
                500,
                &format!("Error updating TODO: {err}"),
            ))
        }
    }
}

// Handler for deleting a TODO
#[tracing::instrument(name = "api/backend/delete", skip_all)]
#[route(method = "DELETE", path = "/todos/{id}")]
async fn delete_todo(ctx: RouteContext) -> Result<Value, LambdaError> {
    if should_inject_error(&ctx.state) {
        return inject_error("Error deleting TODO");
    }

    let id = ctx
        .params
        .get("id")
        .ok_or_else(|| LambdaError::from("Missing 'id' path parameter"))?;

    match delete_item(id, &ctx.state).await {
        Ok(true) => {
            event()
                .level(EventLevel::Info)
                .message(format!("Successfully deleted TODO {id}"))
                .attribute("event.name", "backend.api.delete_todo.success")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_empty_response(204))
        }
        Ok(false) => {
            event()
                .level(EventLevel::Info)
                .message(format!("TODO {id} not found for deletion"))
                .attribute("event.name", "backend.api.delete_todo.not_found")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_error_response(404, &format!("TODO {id} not found")))
        }
        Err(err) => {
            error!("Error deleting TODO: {}", err);
            event()
                .level(EventLevel::Error)
                .message(format!("Error deleting TODO {id}: {err}"))
                .attribute("event.name", "backend.api.delete_todo.error")
                .attribute("todo.id", id.to_string())
                .call();
            Ok(api_error_response(
                500,
                &format!("Error deleting TODO: {err}"),
            ))
        }
    }
}

/// Check if we should inject an error
fn should_inject_error(state: &AppState) -> bool {
    if state.error_probability > 0.0 {
        let mut rng = rand::rng();
        return rng.random::<f64>() < state.error_probability;
    }
    false
}

/// Inject an error response
fn inject_error(message: &str) -> Result<Value, LambdaError> {
    event()
        .level(EventLevel::Warn)
        .message(message)
        .attribute("event.name", "backend.api.injected_error")
        .attribute("error.injected", true)
        .call();
    Ok(api_error_response(
        500,
        &json!({
            "error": message,
            "injected": true
        })
        .to_string(),
    ))
}

// Actual handler function called by the service_fn
async fn handler(
    event: LambdaEvent<ApiGatewayProxyRequest>, // Keep the full LambdaEvent
    router: Arc<Router>,
    state: Arc<AppState>,
) -> Result<Value, LambdaError> {
    // Pass the full event to handle_request, as it expects LambdaEvent<ApiGatewayProxyRequest>
    router.handle_request(event, state).await
}

/// Main function
#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    // Initialize telemetry
    let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;

    // Get environment variables
    let table_name = env::var("TABLE_NAME").unwrap_or_else(|_| "todo-table".to_string());
    let error_probability: f64 = env::var("ERROR_PROBABILITY")
        .unwrap_or_else(|_| "0.0".to_string())
        .parse()
        .unwrap_or(0.0);

    // Initialize AWS SDK
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    let dynamodb_client = DynamoDbClient::new(&config);

    // Create app state
    let state = Arc::new(AppState {
        dynamodb_client,
        table_name,
        error_probability,
    });

    // Create router
    let router = Arc::new(create_router());

    // Build the service
    let service = ServiceBuilder::new()
        .layer(OtelTracingLayer::new(completion_handler).with_name("todo-backend-api-tower"))
        .service_fn(move |event: LambdaEvent<ApiGatewayProxyRequest>| {
            let router_clone = router.clone();
            let state_clone = state.clone(); // Correctly clone the state
            handler(event, router_clone, state_clone) // Call the actual handler
        });

    // Run the Lambda runtime
    Runtime::new(service).run().await?;

    Ok(())
}
