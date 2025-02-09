//! Lambda function handler wrapper with OpenTelemetry tracing.
//!
//! This module provides a wrapper function that automatically creates OpenTelemetry spans
//! for Lambda function invocations. It offers an alternative to the Tower middleware layer
//! when more direct control over span creation is needed.
//!
//! # When to Use the Handler Wrapper
//!
//! The handler wrapper approach is recommended when:
//! - You have a simple Lambda function without complex middleware needs
//! - You want minimal setup and configuration
//! - You need direct control over span creation and attributes
//! - You don't need Tower's middleware composition features
//!
//! For more complex applications, consider using the Tower layer approach instead.
//!
//! # Features
//!
//! - Automatic span creation with configurable names and attributes
//! - Built-in support for common AWS event types (API Gateway v1/v2)
//! - Automatic context propagation from HTTP headers
//! - Response status code tracking
//! - Custom attribute extraction
//!
//! # Architecture
//!
//! The handler wrapper operates by:
//! 1. Creating a span for each invocation
//! 2. Extracting attributes from the event
//! 3. Running the handler function within the span
//! 4. Capturing response attributes (e.g., status code)
//! 5. Signaling completion for span export
//!
//! # Performance Considerations
//!
//! The wrapper is designed to minimize overhead:
//! - Lazy attribute extraction
//! - Efficient downcasting for type detection
//! - Minimal allocations for span attributes
//! - No blocking operations in the critical path
//!
//! # Comparison with Tower Layer
//!
//! This wrapper provides an alternative to the `OtelTracingLayer`:
//! - More direct control over span creation
//! - Simpler integration (no middleware stack)
//! - Easier to customize span attributes
//! - Better suited for simple Lambda functions
//!
//! Use this wrapper when:
//! - You have a simple Lambda function
//! - You don't need other Tower middleware
//! - You want direct control over spans
//!
//! Use the Tower layer when:
//! - You're building a complex service
//! - You need other Tower middleware
//! - You want standardized instrumentation
//!
//! # Examples
//!
//! Basic usage with JSON events:
//!
//! ```rust,no_run
//! use lambda_otel_lite::{init_telemetry, traced_handler, TelemetryConfig};
//! use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
//! use aws_lambda_events::event::apigw::ApiGatewayV2httpRequest;
//!
//! async fn function_handler(event: LambdaEvent<ApiGatewayV2httpRequest>) -> Result<serde_json::Value, Error> {
//!     Ok(serde_json::json!({
//!         "statusCode": 200,
//!         "body": format!("Hello from request {}", event.context.request_id)
//!     }))
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
//!     
//!     let runtime = Runtime::new(service_fn(|event| {
//!         traced_handler("my-handler", event, completion_handler.clone(), function_handler)
//!     }));
//!
//!     runtime.run().await
//! }
//! ```
//!
//! Using with API Gateway events:
//!
//! ```rust,no_run
//! use lambda_otel_lite::{init_telemetry, traced_handler, TelemetryConfig};
//! use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
//! use aws_lambda_events::event::apigw::ApiGatewayV2httpRequest;
//!
//! async fn api_handler(
//!     event: LambdaEvent<ApiGatewayV2httpRequest>
//! ) -> Result<serde_json::Value, Error> {
//!     // HTTP attributes will be automatically extracted
//!     Ok(serde_json::json!({
//!         "statusCode": 200,
//!         "body": format!("Hello from request {}", event.context.request_id)
//!     }))
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
//!     
//!     let runtime = Runtime::new(service_fn(|event| {
//!         traced_handler("api-handler", event, completion_handler.clone(), api_handler)
//!     }));
//!
//!     runtime.run().await
//! }
//! ```

use crate::extractors::{set_common_attributes, set_response_attributes, SpanAttributesExtractor};
use crate::TelemetryCompletionHandler;
use lambda_runtime::{Error, LambdaEvent};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::field::Empty;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

static IS_COLD_START: AtomicBool = AtomicBool::new(true);
/// Wraps a Lambda handler function with OpenTelemetry tracing.
///
/// This function wraps a Lambda handler function to automatically create and configure
/// OpenTelemetry spans for each invocation. It provides automatic instrumentation
/// with minimal code changes required.
///
/// # Features
///
/// - Creates spans for each invocation with configurable names
/// - Extracts attributes from events implementing `SpanAttributesExtractor`
/// - Propagates context from incoming requests via headers
/// - Tracks response status codes for HTTP responses
/// - Supports both sync and async span processing modes
///
/// # Span Attributes
///
/// The following attributes are automatically set:
///
/// - `otel.name`: The handler name provided
/// - `otel.kind`: "SERVER" by default, can be overridden by extractor
/// - `requestId`: The Lambda request ID
/// - `faas.invocation_id`: The Lambda request ID
/// - `cloud.resource_id`: The Lambda function ARN
/// - `cloud.account.id`: The AWS account ID (extracted from ARN)
/// - `faas.trigger`: "http" or "other" based on event type
/// - `http.status_code`: For HTTP responses
///
/// Additional attributes can be added through the `SpanAttributesExtractor` trait.
///
/// # Error Handling
///
/// The wrapper handles errors gracefully:
/// - Extraction failures don't fail the function
/// - Invalid headers are skipped
/// - Export errors are logged but don't fail the function
/// - 5xx status codes set the span status to ERROR
///
/// # Type Parameters
///
/// * `T` - The event payload type that must be deserializable and serializable
/// * `R` - The response type that must be serializable
/// * `F` - The handler function type
/// * `Fut` - The future returned by the handler function
///
/// # Arguments
///
/// * `name` - Name of the handler/span
/// * `event` - Lambda event containing both payload and context
/// * `completion_handler` - Handler for managing span export
/// * `handler_fn` - The actual Lambda handler function to wrap
///
/// # Returns
///
/// Returns the result from the handler function
///
/// # Examples
///
/// Basic usage:
///
/// ```rust,no_run
/// use lambda_otel_lite::{init_telemetry, traced_handler, TelemetryConfig};
/// use lambda_runtime::{service_fn, Error, LambdaEvent};
/// use serde_json::Value;
///
/// async fn function_handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
///     Ok(serde_json::json!({ "statusCode": 200 }))
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
///     
///     let func = service_fn(|event| {
///         traced_handler(
///             "my-handler",
///             event,
///             completion_handler.clone(),
///             function_handler,
///         )
///     });
///
///     lambda_runtime::run(func).await
/// }
/// ```
///
/// With custom event type implementing `SpanAttributesExtractor`:
///
/// ```rust,no_run
/// use lambda_otel_lite::{init_telemetry, traced_handler, TelemetryConfig};
/// use lambda_otel_lite::{SpanAttributes, SpanAttributesExtractor};
/// use lambda_runtime::{service_fn, Error, LambdaEvent};
/// use std::collections::HashMap;
/// use serde::{Serialize, Deserialize};
/// use opentelemetry::Value;
/// #[derive(Serialize, Deserialize)]
/// struct CustomEvent {
///     operation: String,
/// }
///
/// impl SpanAttributesExtractor for CustomEvent {
///     fn extract_span_attributes(&self) -> SpanAttributes {
///         let mut attributes = HashMap::new();
///         attributes.insert("operation".to_string(), Value::String(self.operation.clone().into()));
///         SpanAttributes::builder()
///             .attributes(attributes)
///             .build()
///     }
/// }
///
/// async fn handler(event: LambdaEvent<CustomEvent>) -> Result<serde_json::Value, Error> {
///     Ok(serde_json::json!({ "statusCode": 200 }))
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
///     
///     let func = service_fn(|event| {
///         traced_handler(
///             "custom-handler",
///             event,
///             completion_handler.clone(),
///             handler,
///         )
///     });
///
///     lambda_runtime::run(func).await
/// }
/// ```
pub async fn traced_handler<T, R, F, Fut>(
    name: &'static str,
    event: LambdaEvent<T>,
    completion_handler: TelemetryCompletionHandler,
    handler_fn: F,
) -> Result<R, Error>
where
    T: SpanAttributesExtractor + DeserializeOwned + Serialize + Send + 'static,
    R: Serialize + Send + 'static,
    F: FnOnce(LambdaEvent<T>) -> Fut,
    Fut: Future<Output = Result<R, Error>> + Send,
{
    let result = {
        // Create the base span
        let span = tracing::info_span!(
            parent: None,
            "handler",
            otel.name=Empty,
            otel.kind=Empty,
            otel.status_code=Empty,
            otel.status_message=Empty,
            requestId=%event.context.request_id,
        );

        // Set the span name and default kind
        span.record("otel.name", name.to_string());
        span.record("otel.kind", "SERVER");

        // Set common Lambda attributes with cold start tracking
        let is_cold = IS_COLD_START.swap(false, Ordering::Relaxed);
        set_common_attributes(&span, &event.context, is_cold);

        // Extract attributes directly using the trait
        let attrs = event.payload.extract_span_attributes();

        // Apply extracted attributes
        if let Some(span_name) = attrs.span_name {
            span.record("otel.name", span_name);
        }

        if let Some(kind) = &attrs.kind {
            span.record("otel.kind", kind.to_string());
        }

        // Set custom attributes
        for (key, value) in &attrs.attributes {
            span.set_attribute(key.to_string(), value.to_string());
        }

        // Add span links
        for link in attrs.links {
            span.add_link_with_attributes(link.span_context, link.attributes);
        }

        // Propagate context from headers
        if let Some(carrier) = attrs.carrier {
            let parent_context = opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.extract(&carrier)
            });
            span.set_parent(parent_context);
        }

        // Set trigger type
        span.set_attribute("faas.trigger", attrs.trigger.to_string());

        // Run the handler with the span
        let result = handler_fn(event).instrument(span.clone()).await;

        // Set response attributes if successful
        if let Ok(response) = &result {
            if let Ok(value) = serde_json::to_value(response) {
                set_response_attributes(&span, &value);
            }
        } else if let Err(error) = &result {
            // Set error status according to OpenTelemetry spec
            span.set_status(opentelemetry::trace::Status::error(error.to_string()));
        }

        result
    };

    // Signal completion
    completion_handler.complete();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::ProcessorMode;
    use futures_util::future::BoxFuture;
    use lambda_runtime::Context;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry::trace::{Status, TraceResult};
    use opentelemetry_sdk::{
        export::trace::{SpanData, SpanExporter},
        trace::TracerProvider,
        Resource,
    };
    use serde_json::Value;
    use serial_test::serial;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;
    use tracing_subscriber::prelude::*;
    // Test exporter that captures spans and their attributes
    #[derive(Debug, Default, Clone)]
    struct TestExporter {
        export_count: Arc<AtomicUsize>,
        spans: Arc<Mutex<Vec<SpanData>>>,
    }

    impl TestExporter {
        fn new() -> Self {
            Self {
                export_count: Arc::new(AtomicUsize::new(0)),
                spans: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_spans(&self) -> Vec<SpanData> {
            self.spans.lock().unwrap().clone()
        }

        fn find_attribute(span: &SpanData, key: &str) -> Option<String> {
            span.attributes
                .iter()
                .find(|kv| kv.key.as_str() == key)
                .map(|kv| kv.value.to_string())
        }
    }

    impl SpanExporter for TestExporter {
        fn export(&mut self, spans: Vec<SpanData>) -> BoxFuture<'static, TraceResult<()>> {
            self.export_count.fetch_add(spans.len(), Ordering::SeqCst);
            self.spans.lock().unwrap().extend(spans);
            Box::pin(futures_util::future::ready(Ok(())))
        }
    }

    fn setup_test_provider() -> (
        Arc<TracerProvider>,
        Arc<TestExporter>,
        tracing::dispatcher::DefaultGuard,
    ) {
        let exporter = Arc::new(TestExporter::new());
        let provider = TracerProvider::builder()
            .with_simple_exporter(exporter.as_ref().clone())
            .with_resource(Resource::empty())
            .build();
        let subscriber = tracing_subscriber::registry::Registry::default()
            .with(tracing_opentelemetry::OpenTelemetryLayer::new(
                provider.tracer("test"),
            ))
            .set_default();
        (Arc::new(provider), exporter, subscriber)
    }

    #[tokio::test]
    #[serial]
    async fn test_basic_handler_wrapping() -> Result<(), Error> {
        let (provider, exporter, _subscriber_guard) = setup_test_provider();

        let completion_handler =
            TelemetryCompletionHandler::new(provider.clone(), None, ProcessorMode::Sync);

        let handler_fn =
            |_event: LambdaEvent<Value>| async move { Ok(serde_json::json!({"statusCode": 200})) };

        let event = LambdaEvent::new(serde_json::json!({}), Context::default());

        let result = traced_handler("test-handler", event, completion_handler, handler_fn).await?;

        // Wait a bit longer for spans to be exported
        tokio::time::sleep(Duration::from_millis(500)).await;

        let spans = exporter.get_spans();
        assert!(!spans.is_empty());
        assert_eq!(result["statusCode"], 200);

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_error_response() -> Result<(), Error> {
        let (provider, exporter, _subscriber_guard) = setup_test_provider();

        let completion_handler =
            TelemetryCompletionHandler::new(provider.clone(), None, ProcessorMode::Sync);

        let event = LambdaEvent::new(serde_json::json!({}), Context::default());

        let handler_fn = |_event: LambdaEvent<Value>| async move {
            Ok(serde_json::json!({
                "statusCode": 500,
                "body": "Internal Server Error"
            }))
        };

        let result = traced_handler("test-handler", event, completion_handler, handler_fn).await?;

        assert_eq!(result["statusCode"], 500);

        // Wait a bit for spans to be exported
        tokio::time::sleep(Duration::from_millis(100)).await;

        let spans = exporter.get_spans();
        assert!(!spans.is_empty());
        let span = &spans[0];

        assert_eq!(
            TestExporter::find_attribute(span, "http.status_code"),
            Some("500".to_string())
        );
        assert!(matches!(span.status, Status::Error { .. }));

        Ok(())
    }
}
