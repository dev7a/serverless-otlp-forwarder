// demo/rust/src/backend/db.rs

use crate::AppState; // Import AppState from main (assuming it stays there)
use aws_sdk_dynamodb::types::AttributeValue;
use lambda_runtime::Error as LambdaError;
use opentelemetry::{Array, Value as OtelValue};
use serde_dynamo::{from_item, to_attribute_value};
use serde_json::Value;
use tracing::{field, instrument, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

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
    let endpoint = format!("dynamodb.{}.amazonaws.com", &region);

    // Basic span attributes
    span.record("otel.kind", "client");
    span.record("otel.name", format!("DynamoDB.{}", operation));

    // Standard OpenTelemetry attributes
    span.set_attribute("db.system", "dynamodb");
    span.set_attribute("db.operation", operation.to_string());
    span.set_attribute("net.peer.name", endpoint);
    span.set_attribute("net.peer.port", 443);

    // AWS-specific attributes
    span.set_attribute("aws.region", region.clone());
    span.set_attribute("cloud.provider", "aws");
    span.set_attribute("cloud.region", region);
    // span.set_attribute("otel.name", format!("DynamoDB.{}", operation)); // Already set above

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

// Make functions public and adjust error types if necessary
#[instrument(skip_all)]
pub async fn write_item(
    pk: &str,
    timestamp: &str,
    payload: &Option<Value>,
    state: &AppState, // Pass state containing the client and table_name
) -> Result<(), anyhow::Error> {
    // Using anyhow::Error for broader compatibility

    let mut request = state
        .dynamodb_client
        .put_item()
        .table_name(state.table_name.clone()) // Use state.table_name
        .item("pk", AttributeValue::S(pk.to_string()))
        .item(
            "expiry",
            AttributeValue::N((chrono::Utc::now().timestamp() + 86400).to_string()),
        )
        .item("timestamp", AttributeValue::S(timestamp.to_string()));

    if let Some(payload_value) = payload {
        let attribute_value = to_attribute_value(payload_value)?;
        request = request.item("payload", attribute_value);
    }

    // Manually create span and set attributes
    let span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&span, state.table_name.as_str(), "PutItem");

    request
        .send()
        .instrument(span) // Instrument with the manually created span
        .await
        .map(|_| ())
        .map_err(anyhow::Error::from)
}

#[instrument(skip(state))]
pub async fn read_item(pk: &str, state: &AppState) -> Result<Option<Value>, LambdaError> {
    // Manually create span and set attributes
    let span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&span, state.table_name.as_str(), "GetItem");

    let response = state
        .dynamodb_client
        .get_item()
        .table_name(state.table_name.clone()) // Use state.table_name
        .key("pk", AttributeValue::S(pk.to_string()))
        .send()
        .instrument(span) // Instrument with the manually created span
        .await
        .map_err(|e| LambdaError::from(format!("Failed to get item from DynamoDB: {}", e)))?;

    if let Some(item) = response.item {
        from_item(item)
            .map_err(|e| LambdaError::from(format!("Failed to deserialize DynamoDB item: {}", e)))
            .map(Some)
    } else {
        // Use state.table_name in logging
        tracing::info!(name = "item not found", pk = %pk, table = state.table_name.as_str());
        Ok(None)
    }
}

#[instrument(skip(state))]
pub async fn list_items(state: &AppState) -> Result<Vec<Value>, LambdaError> {
    // Manually create span and set attributes
    let span = tracing::info_span!(
        "dynamodb_operation",
        otel.name = field::Empty,
        otel.kind = "client"
    );
    set_dynamodb_span_attributes(&span, state.table_name.as_str(), "Scan");

    let scan_output = state
        .dynamodb_client
        .scan()
        .table_name(state.table_name.clone()) // Use state.table_name
        .limit(10) // Limit the scan to 10 items
        .send()
        .instrument(span) // Instrument with the manually created span
        .await
        .map_err(|e| LambdaError::from(format!("Failed to scan DynamoDB table: {}", e)))?;

    let items = scan_output
        .items
        .unwrap_or_default()
        .into_iter()
        .map(|item| {
            from_item(item).map_err(|e| {
                LambdaError::from(format!("Failed to deserialize DynamoDB item: {}", e))
            })
        })
        .collect::<Result<Vec<Value>, LambdaError>>()?;

    let item_count = items.len();
    tracing::info!(
        name = "retrieved items from dynamo",
        item_count = item_count,
        table = state.table_name.as_str() // Use state.table_name
    );

    Ok(items)
}
