use aws_sdk_dynamodb::{
    types::{AttributeValue, ReturnValue}, // Corrected import for ReturnValue
    Client as DynamoDbClient,
};
use chrono::{Duration, Utc};
use serde_json::{json, Map as JsonMap, Value};
use std::env;
use std::sync::OnceLock;
use tracing::{field, instrument, Instrument};

use lambda_otel_lite::events::{event, EventLevel};
use opentelemetry::{Array, Value as OtelValue};
use serde_dynamo::{from_item, to_attribute_value, to_item}; // Added serde_dynamo imports
use tracing_opentelemetry::OpenTelemetrySpanExt;

// Cache the expiration time to avoid parsing the environment variable on every call
static EXPIRATION_SECONDS: OnceLock<i64> = OnceLock::new();

fn get_expiration_seconds() -> i64 {
    *EXPIRATION_SECONDS.get_or_init(|| {
        env::var("EXPIRATION_TIME")
            .unwrap_or_else(|_| "3600".to_string())
            .parse::<i64>()
            .unwrap_or(3600) // Default to 1 hour if parsing fails
    })
}

#[derive(Clone)]
pub struct AppState {
    pub dynamodb_client: DynamoDbClient,
    pub table_name: String,
    pub error_probability: f64,
}

/// Sets DynamoDB-specific attributes on a tracing span
///
/// # Arguments
/// * `span` - The span to set attributes on
/// * `table_name` - The DynamoDB table name (now passed as argument)
/// * `operation` - The DynamoDB operation name (e.g., "PutItem", "GetItem")
fn set_dynamodb_span_attributes(
    span: &tracing::Span,
    table_name: &str, // Changed from &'static str
    operation: &str,  // Changed from &'static str
) {
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let endpoint = format!("dynamodb.{region}.amazonaws.com");

    // Basic span attributes
    span.record("otel.kind", "client");
    span.record("otel.name", format!("DynamoDB.{operation}"));

    // Standard OpenTelemetry attributes
    span.set_attribute("db.system", "dynamodb");
    span.set_attribute("db.operation", operation.to_string());
    span.set_attribute("net.peer.name", endpoint);
    span.set_attribute("net.peer.port", 443);

    // AWS-specific attributes
    span.set_attribute("aws.region", region.clone());
    span.set_attribute("cloud.provider", "aws");
    span.set_attribute("cloud.region", region);
    // span.set_attribute("otel.name", format!("DynamoDB.{operation}")); // Already set above

    // RPC attributes
    span.set_attribute("rpc.system", "aws-api");
    span.set_attribute("rpc.service", "DynamoDB");
    span.set_attribute("rpc.method", operation.to_string());

    // AWS semantic conventions
    span.set_attribute("aws.remote.service", "AWS::DynamoDB");
    span.set_attribute("aws.remote.operation", operation.to_string());
    span.set_attribute("aws.remote.resource.type", "AWS::DynamoDB::Table");
    span.set_attribute("aws.remote.resource.identifier", table_name.to_string());
    span.set_attribute(
        "aws.remote.resource.cfn.primary.identifier",
        table_name.to_string(),
    );
    span.set_attribute("aws.span.kind", "CLIENT");

    // Set table names as array
    let table_name_array = OtelValue::Array(Array::String(vec![table_name.to_string().into()]));
    span.set_attribute("aws.dynamodb.table_names", table_name_array);
}

/// Helper function to convert a DynamoDB item (as a serde_json Map) to our API's JSON format
/// by renaming "pk" to "id".
fn dynamodb_json_to_api_json(mut item_map: JsonMap<String, Value>) -> Value {
    if let Some(pk_value) = item_map.remove("pk") {
        item_map.insert("id".to_string(), pk_value);
    }
    Value::Object(item_map)
}

/// Write a TODO item to DynamoDB
#[instrument(name = "database/storage/write", skip_all)]
pub async fn write_item(
    id: &str,
    timestamp: &str,
    todo_payload: &Value, // Renamed for clarity
    state: &AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut item_json_map = if let Some(payload_obj) = todo_payload.as_object() {
        payload_obj.clone() // Start with the fields from the input todo_payload
    } else {
        JsonMap::new() // Should ideally be an object, handle if not.
    };

    // Add/overwrite primary key, timestamp, and expiry
    item_json_map.insert("pk".to_string(), json!(id));
    item_json_map.insert("timestamp".to_string(), json!(timestamp));
    let now = Utc::now();
    let expiry_time = now + Duration::seconds(get_expiration_seconds());
    item_json_map.insert("expiry".to_string(), json!(expiry_time.timestamp()));

    // If the original payload had 'completed' as true and 'completed_at' was missing,
    // add 'completed_at'. serde_dynamo will handle boolean 'completed' correctly.
    if item_json_map
        .get("completed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !item_json_map.contains_key("completed_at")
    {
        item_json_map.insert("completed_at".to_string(), json!(Utc::now().to_rfc3339()));
    }

    let dynamodb_item = to_item(Value::Object(item_json_map)).map_err(Box::new)?;

    let span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&span, state.table_name.as_str(), "PutItem");

    state
        .dynamodb_client
        .put_item()
        .table_name(&state.table_name)
        .set_item(Some(dynamodb_item))
        .return_values(ReturnValue::None)
        .send()
        .instrument(span)
        .await?;

    event()
        .level(EventLevel::Info)
        .message(format!("Successfully wrote TODO {id} to DynamoDB"))
        .attribute("event.name", "backend.db.write_item.success")
        .attribute("todo.id", id.to_string())
        .attribute(
            "todo.completed",
            todo_payload
                .get("completed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .call();

    Ok(())
}

/// Read a TODO item from DynamoDB
#[instrument(name = "database/storage/read", skip(state))]
pub async fn read_item(
    id: &str,
    state: &AppState,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&span, state.table_name.as_str(), "GetItem");

    let result = state
        .dynamodb_client
        .get_item()
        .table_name(&state.table_name)
        .key("pk", AttributeValue::S(id.to_string())) // Key remains AttributeValue::S for query
        .send()
        .instrument(span)
        .await?;

    if let Some(item_attributes) = result.item {
        // Convert DynamoDB attributes to serde_json::Value
        let json_value: Value = from_item(item_attributes).map_err(Box::new)?;

        let final_json = match json_value {
            Value::Object(map) => dynamodb_json_to_api_json(map),
            _ => {
                return Err(Box::from(
                    "Failed to deserialize DynamoDB item into a JSON object",
                ))
            }
        };

        event()
            .level(EventLevel::Info)
            .message(format!("Successfully read TODO {id} from DynamoDB"))
            .attribute("event.name", "backend.db.read_item.success")
            .attribute("todo.id", id.to_string())
            .call();
        Ok(Some(final_json))
    } else {
        event()
            .level(EventLevel::Info)
            .message(format!("TODO {id} not found in DynamoDB"))
            .attribute("event.name", "backend.db.read_item.not_found")
            .attribute("todo.id", id.to_string())
            .call();
        Ok(None)
    }
}

/// Unified function to list TODO items with optional filtering and pagination.
#[instrument(name = "database/storage/list", skip(state), fields(
    filter.completed = field::Empty,
    pagination.limit = field::Empty,
    pagination.offset = field::Empty
))]
pub async fn list_items(
    state: &AppState,
    filter_completed: Option<bool>,
    page_limit: Option<usize>,
    page_offset: Option<usize>,
) -> Result<(Vec<Value>, usize), Box<dyn std::error::Error>> {
    let span = tracing::Span::current();
    if let Some(status) = filter_completed {
        span.record("filter.completed", status);
    }
    if let Some(limit) = page_limit {
        span.record("pagination.limit", limit as i64);
    }
    if let Some(offset) = page_offset {
        // Or page_limit.is_some() and record offset.unwrap_or(0)
        span.record("pagination.offset", offset as i64);
    }

    event()
        .level(EventLevel::Info)
        .message(format!(
            "Listing items. Filter: {filter_completed:?}, Limit: {page_limit:?}, Offset: {page_offset:?}"
        ))
        .attribute("event.name", "backend.db.list_items.start")
        .attribute("filter.completed", filter_completed.map(|b| b.to_string()).unwrap_or_else(|| "None".to_string()))
        .attribute("pagination.limit", page_limit.map(|u| u as i64).unwrap_or(-1))
        .attribute("pagination.offset", page_offset.map(|u| u as i64).unwrap_or(-1))
        .call();

    let scan_span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&scan_span, state.table_name.as_str(), "Scan");

    let scan_result = state
        .dynamodb_client
        .scan()
        .table_name(&state.table_name)
        .send()
        .instrument(scan_span)
        .await?;

    let mut all_api_items = Vec::new();
    if let Some(db_items) = scan_result.items {
        let db_items_len = db_items.len();
        event()
            .level(EventLevel::Info)
            .message(format!("Scan returned {db_items_len} items from DynamoDB"))
            .attribute("event.name", "backend.db.list_items.scan_result")
            .attribute("dynamodb.item_count", db_items.len() as i64)
            .call();
        for item_attrs in db_items {
            let json_value: Value = from_item(item_attrs).map_err(Box::new)?;
            let final_json = match json_value {
                Value::Object(map) => dynamodb_json_to_api_json(map),
                _ => continue, // Or handle error
            };
            all_api_items.push(final_json);
        }
    }

    // Apply filtering
    let filtered_items: Vec<Value> = if let Some(status_filter) = filter_completed {
        all_api_items
            .into_iter()
            .filter(|todo| {
                todo.get("completed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    == status_filter
            })
            .collect()
    } else {
        all_api_items
    };

    let total_relevant_count = filtered_items.len();

    // Apply sorting and pagination if limit is specified
    let final_items: Vec<Value>;

    // Always sort items by created_at (descending - newest first) after filtering
    let mut sorted_items = filtered_items;
    sorted_items.sort_by(|a, b| {
        let a_date = a
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let b_date = b
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        b_date.cmp(a_date) // Descending order
    });

    if let Some(limit_val) = page_limit {
        // Pagination is requested
        let offset_val = page_offset.unwrap_or(0);
        if offset_val >= total_relevant_count {
            // total_relevant_count is based on count before sort, but length is same
            final_items = Vec::new(); // Offset is beyond the total number of items
        } else {
            let end_index = std::cmp::min(offset_val + limit_val, total_relevant_count);
            // Paginate from the already sorted list
            final_items = sorted_items[offset_val..end_index].to_vec();
        }
    } else {
        // No pagination, return all sorted (and filtered) items
        final_items = sorted_items;
    }

    let final_items_len = final_items.len();
    event()
        .level(EventLevel::Info)
        .message(format!(
            "Successfully processed items. Returned: {final_items_len}, Total relevant: {total_relevant_count}"
        ))
        .attribute("event.name", "backend.db.list_items.success")
        .attribute("items.returned_count", final_items.len() as i64)
        .attribute("items.total_relevant_count", total_relevant_count as i64)
        .call();

    Ok((final_items, total_relevant_count))
}

/// Update a TODO item in DynamoDB
#[instrument(name = "database/storage/update", skip(state))]
pub async fn update_item(
    id: &str,
    todo_payload: &Value, // Expected to be like {"completed": true/false}
    state: &AppState,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    // Check if the item exists first.
    // If not, we can't update it, so return Ok(None) like before.
    let existing_item = read_item(id, state).await?;
    if existing_item.is_none() {
        return Ok(None);
    }

    let mut update_expression_parts = Vec::new();
    let mut expression_attribute_values = std::collections::HashMap::new();
    let mut expression_attribute_names = std::collections::HashMap::new();

    // Only process if "completed" field is present in the payload
    if let Some(completed_bool) = todo_payload.get("completed").and_then(Value::as_bool) {
        update_expression_parts.push("#C = :completed".to_string());
        expression_attribute_names.insert("#C".to_string(), "completed".to_string());
        expression_attribute_values.insert(
            ":completed".to_string(),
            to_attribute_value(json!(completed_bool)).map_err(Box::new)?,
        );

        if completed_bool {
            // If marking as completed, set/update completed_at,
            // unless completed_at is explicitly provided in the payload and is a valid string.
            let set_completed_at = todo_payload
                .get("completed_at")
                .and_then(Value::as_str)
                .map_or(true, str::is_empty); // Set if not present or if present but empty

            if set_completed_at {
                update_expression_parts.push("#CA = :completed_at".to_string());
                expression_attribute_names.insert("#CA".to_string(), "completed_at".to_string());
                expression_attribute_values.insert(
                    ":completed_at".to_string(),
                    to_attribute_value(json!(Utc::now().to_rfc3339())).map_err(Box::new)?,
                );
            }
        } else {
            // If marking as not completed, explicitly remove completed_at
            // This assumes that if a task is marked incomplete, its completion time is no longer relevant.
            update_expression_parts.push("REMOVE #CA".to_string());
            expression_attribute_names.insert("#CA".to_string(), "completed_at".to_string());
            // No value needed for REMOVE in expression_attribute_values for this key
        }
    } else {
        // If "completed" is not in the payload or not a boolean,
        // we assume no update is intended based on this simplified logic.
        // We return the item as it was before the update attempt.
        return Ok(existing_item);
    }

    // If, for some reason (e.g. payload was empty or malformed earlier), no parts were added.
    if update_expression_parts.is_empty() {
        return Ok(existing_item);
    }

    // Construct the update expression.
    let mut set_statements = Vec::new();
    let mut remove_statements = Vec::new();

    for part in update_expression_parts {
        if part.starts_with("REMOVE") {
            remove_statements.push(part.trim_start_matches("REMOVE ").to_string());
        } else {
            // Assumes it's a SET part like "#C = :completed"
            set_statements.push(part);
        }
    }

    let mut final_update_expression = String::new();
    if !set_statements.is_empty() {
        let set_statement = set_statements.join(", ");
        final_update_expression.push_str(&format!("SET {set_statement}"));
    }
    if !remove_statements.is_empty() {
        if !final_update_expression.is_empty() {
            final_update_expression.push(' ');
        }
        let remove_statement = remove_statements.join(", ");
        final_update_expression.push_str(&format!("REMOVE {remove_statement}"));
    }

    if final_update_expression.is_empty() {
        return Ok(existing_item); // Should be caught by update_expression_parts.is_empty() but as a safeguard.
    }

    let span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&span, state.table_name.as_str(), "UpdateItem");

    let result = state
        .dynamodb_client
        .update_item()
        .table_name(&state.table_name)
        .key("pk", AttributeValue::S(id.to_string()))
        .update_expression(final_update_expression)
        .set_expression_attribute_names(Some(expression_attribute_names).filter(|m| !m.is_empty()))
        .set_expression_attribute_values(
            Some(expression_attribute_values).filter(|m| !m.is_empty()),
        )
        .return_values(ReturnValue::AllNew)
        .send()
        .instrument(span)
        .await?;

    if let Some(item_attributes) = result.attributes {
        let updated_json_value: Value = from_item(item_attributes).map_err(Box::new)?;
        let final_json = match updated_json_value {
            Value::Object(map) => dynamodb_json_to_api_json(map),
            _ => {
                return Err(Box::from(
                    "Failed to deserialize updated DynamoDB item into a JSON object",
                ))
            }
        };

        event()
            .level(EventLevel::Info)
            .message(format!("Successfully updated TODO {id} in DynamoDB"))
            .attribute("event.name", "backend.db.update_item.success")
            .attribute("todo.id", id.to_string())
            .call();
        Ok(Some(final_json))
    } else {
        Ok(None) // Should not happen with ReturnValue::AllNew if item exists
    }
}

/// Delete a TODO item from DynamoDB
#[instrument(name = "database/storage/delete", skip(state))]
pub async fn delete_item(id: &str, state: &AppState) -> Result<bool, Box<dyn std::error::Error>> {
    let span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&span, state.table_name.as_str(), "DeleteItem");

    // Delete the item from DynamoDB
    let result = state
        .dynamodb_client
        .delete_item()
        .table_name(&state.table_name)
        .key("pk", AttributeValue::S(id.to_string()))
        .return_values(ReturnValue::AllOld)
        .send()
        .instrument(span) // Instrument with the manually created span
        .await?;

    // Check if the item was deleted
    let deleted = result.attributes.is_some();

    event()
        .level(EventLevel::Info)
        .message(format!("Successfully deleted TODO {id} from DynamoDB"))
        .attribute("event.name", "backend.db.delete_item.success")
        .attribute("todo.id", id.to_string())
        .attribute("todo.deleted", deleted)
        .call();

    Ok(deleted)
}
