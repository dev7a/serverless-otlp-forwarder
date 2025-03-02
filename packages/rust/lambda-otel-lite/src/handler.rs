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
///
/// ```rust,no_run
/// use std::result::Result;
/// use lambda_runtime::{Error, LambdaEvent};
/// use serde_json::Value;
/// use lambda_otel_lite::{init_telemetry, create_traced_handler, TelemetryConfig};
///
/// async fn my_handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
///     let prefix = event.payload.get("prefix").and_then(|p| p.as_str()).unwrap_or("default");
///     Ok::<Value, Error>(serde_json::json!({ "prefix": prefix }))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Error> {
///     let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;
///     let handler = create_traced_handler(
///         "my-handler",
///         completion_handler,
///         my_handler
///     );
///     // ... use handler with Runtime ...
/// #   Ok(())
/// # }
/// ```
use crate::extractors::{set_common_attributes, set_response_attributes, SpanAttributesExtractor};
use crate::TelemetryCompletionHandler;
use futures_util::future::BoxFuture;
use lambda_runtime::{Error, LambdaEvent};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::field::Empty;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

static IS_COLD_START: AtomicBool = AtomicBool::new(true);

/// Type representing a traced Lambda handler function.
/// Takes a `LambdaEvent<T>` and returns a `Future` that resolves to `Result<R, Error>`.
pub type TracedHandler<T, R> =
    Box<dyn Fn(LambdaEvent<T>) -> BoxFuture<'static, Result<R, Error>> + Send + Sync>;

/// Internal implementation that wraps a Lambda handler function with OpenTelemetry tracing.
///
/// This is an implementation detail. Users should use `create_traced_handler` instead.
pub(crate) async fn traced_handler<T, R, F, Fut>(
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

/// Creates a traced handler function that can be used directly with `service_fn`.
///
/// This is a convenience wrapper around `traced_handler` that returns a function suitable
/// for direct use with the Lambda runtime. It provides a more ergonomic interface by
/// allowing handler creation to be separated from usage.
///
/// # Type Parameters
///
/// * `T` - The event payload type that must be deserializable and serializable
/// * `R` - The response type that must be serializable
/// * `F` - The handler function type, must be `Clone` to allow reuse across invocations
/// * `Fut` - The future returned by the handler function
///
/// # Handler Requirements
///
/// The handler function must implement `Clone`. This is automatically satisfied by:
/// - Regular functions (e.g., `fn(LambdaEvent<T>) -> Future<...>`)
/// - Closures that capture only `Clone` types
///
/// For example:
/// ```ignore
/// // Regular function - automatically implements Clone
/// async fn my_handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
///     Ok(serde_json::json!({}))
/// }
///
/// // Closure capturing Clone types - implements Clone
/// let prefix = "my-prefix".to_string();
/// let handler = |event: LambdaEvent<Value>| async move {
///     let prefix = prefix.clone();
///     Ok::<Value, Error>(serde_json::json!({ "prefix": prefix }))
/// };
/// ```
///
/// # Arguments
///
/// * `name` - Name of the handler/span
/// * `completion_handler` - Handler for managing span export
/// * `handler_fn` - The actual Lambda handler function to wrap
///
/// # Returns
///
/// Returns a boxed function that can be used directly with `service_fn`
///
/// # Examples
///
/// ```rust
/// use lambda_runtime::{Error, LambdaEvent};
/// use serde_json::Value;
/// use lambda_otel_lite::{init_telemetry, create_traced_handler, TelemetryConfig};
///
/// async fn my_handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
///     let prefix = event.payload.get("prefix").and_then(|p| p.as_str()).unwrap_or("default");
///     Ok::<Value, Error>(serde_json::json!({ "prefix": prefix }))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Error> {
///     let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;
///     let handler = create_traced_handler(
///         "my-handler",
///         completion_handler,
///         my_handler
///     );
///     // ... use handler with Runtime ...
/// #   Ok(())
/// # }
/// ```
pub fn create_traced_handler<T, R, F, Fut>(
    name: &'static str,
    completion_handler: TelemetryCompletionHandler,
    handler_fn: F,
) -> TracedHandler<T, R>
where
    T: SpanAttributesExtractor + DeserializeOwned + Serialize + Send + 'static,
    R: Serialize + Send + 'static,
    F: Fn(LambdaEvent<T>) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = Result<R, Error>> + Send + 'static,
{
    Box::new(move |event: LambdaEvent<T>| {
        let completion_handler = completion_handler.clone();
        let handler_fn = handler_fn.clone();
        Box::pin(traced_handler(name, event, completion_handler, handler_fn))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::ProcessorMode;
    use futures_util::future::BoxFuture;
    use lambda_runtime::Context;
    use opentelemetry::trace::Status;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::{
        trace::{SdkTracerProvider, SpanData, SpanExporter},
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

    // Test infrastructure
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
        fn export(
            &mut self,
            spans: Vec<SpanData>,
        ) -> BoxFuture<'static, opentelemetry_sdk::error::OTelSdkResult> {
            self.export_count.fetch_add(spans.len(), Ordering::SeqCst);
            self.spans.lock().unwrap().extend(spans);
            Box::pin(futures_util::future::ready(Ok(())))
        }
    }

    fn setup_test_provider() -> (
        Arc<SdkTracerProvider>,
        Arc<TestExporter>,
        tracing::dispatcher::DefaultGuard,
    ) {
        let exporter = Arc::new(TestExporter::new());
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.as_ref().clone())
            .with_resource(Resource::builder().build())
            .build();
        let subscriber = tracing_subscriber::registry::Registry::default()
            .with(tracing_opentelemetry::OpenTelemetryLayer::new(
                provider.tracer("test"),
            ))
            .set_default();
        (Arc::new(provider), exporter, subscriber)
    }

    async fn wait_for_spans(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    // Basic functionality tests
    #[tokio::test]
    #[serial]
    async fn test_successful_response() -> Result<(), Error> {
        let (provider, exporter, _guard) = setup_test_provider();
        let completion_handler =
            TelemetryCompletionHandler::new(provider, None, ProcessorMode::Sync);

        async fn handler(_: LambdaEvent<Value>) -> Result<Value, Error> {
            Ok(serde_json::json!({ "statusCode": 200, "body": "Success" }))
        }

        let traced_handler = create_traced_handler("test-handler", completion_handler, handler);
        let event = LambdaEvent::new(serde_json::json!({}), Context::default());

        let result = traced_handler(event).await?;

        wait_for_spans(Duration::from_millis(100)).await;

        let spans = exporter.get_spans();
        assert!(!spans.is_empty(), "No spans were exported");

        let span = &spans[0];
        assert_eq!(span.name, "test-handler", "Unexpected span name");
        assert_eq!(
            TestExporter::find_attribute(span, "http.status_code"),
            Some("200".to_string())
        );
        assert_eq!(result["statusCode"], 200);

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_error_response() -> Result<(), Error> {
        let (provider, exporter, _guard) = setup_test_provider();
        let completion_handler =
            TelemetryCompletionHandler::new(provider, None, ProcessorMode::Sync);

        async fn handler(_: LambdaEvent<Value>) -> Result<Value, Error> {
            Ok(serde_json::json!({
                "statusCode": 500,
                "body": "Internal Server Error"
            }))
        }

        let traced_handler = create_traced_handler("test-handler", completion_handler, handler);
        let event = LambdaEvent::new(serde_json::json!({}), Context::default());

        let result = traced_handler(event).await?;

        wait_for_spans(Duration::from_millis(100)).await;

        let spans = exporter.get_spans();
        assert!(!spans.is_empty(), "No spans were exported");

        let span = &spans[0];
        assert_eq!(span.name, "test-handler", "Unexpected span name");
        assert_eq!(
            TestExporter::find_attribute(span, "http.status_code"),
            Some("500".to_string())
        );
        assert!(matches!(span.status, Status::Error { .. }));
        assert_eq!(result["statusCode"], 500);

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_handler_reuse() -> Result<(), Error> {
        let (provider, exporter, _guard) = setup_test_provider();
        let completion_handler =
            TelemetryCompletionHandler::new(provider, None, ProcessorMode::Sync);

        async fn handler(_: LambdaEvent<Value>) -> Result<Value, Error> {
            Ok(serde_json::json!({ "status": "ok" }))
        }

        let traced_handler = create_traced_handler("test-handler", completion_handler, handler);

        // Call the handler multiple times to verify it can be reused
        for _ in 0..3 {
            let event = LambdaEvent::new(serde_json::json!({}), Context::default());
            let _ = traced_handler(event).await?;
        }

        wait_for_spans(Duration::from_millis(100)).await;

        let spans = exporter.get_spans();
        assert_eq!(spans.len(), 3, "Expected exactly 3 spans");

        // Verify all spans have the correct name
        for span in spans {
            assert_eq!(span.name, "test-handler", "Unexpected span name");
        }

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_handler_with_closure() -> Result<(), Error> {
        let (provider, exporter, _guard) = setup_test_provider();
        let completion_handler =
            TelemetryCompletionHandler::new(provider, None, ProcessorMode::Sync);

        let prefix = "test-prefix".to_string();
        let handler = move |_event: LambdaEvent<Value>| {
            let prefix = prefix.clone();
            async move {
                Ok(serde_json::json!({
                    "statusCode": 200,
                    "prefix": prefix
                }))
            }
        };

        let traced_handler = create_traced_handler("test-handler", completion_handler, handler);
        let event = LambdaEvent::new(serde_json::json!({}), Context::default());

        let result = traced_handler(event).await?;

        wait_for_spans(Duration::from_millis(100)).await;

        let spans = exporter.get_spans();
        assert!(!spans.is_empty(), "No spans were exported");

        assert_eq!(result["prefix"], "test-prefix");
        assert_eq!(spans[0].name, "test-handler", "Unexpected span name");

        Ok(())
    }
}
