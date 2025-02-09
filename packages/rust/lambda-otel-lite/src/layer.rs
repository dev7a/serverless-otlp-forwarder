//! Tower middleware for OpenTelemetry tracing in AWS Lambda functions.
//!
//! This module provides a Tower middleware layer that automatically creates OpenTelemetry spans
//! for Lambda invocations. It supports automatic extraction of span attributes from common AWS
//! event types and allows for custom attribute extraction through a flexible trait system.
//!
//! # When to Use the Tower Layer
//!
//! The Tower layer approach is recommended when:
//! - You need middleware composition (e.g., combining with other Tower layers)
//! - You want a more idiomatic Rust approach using traits
//! - Your application has complex middleware requirements
//! - You're already using Tower in your application
//!
//! For simpler use cases, consider using the handler wrapper approach instead.
//!
//! # Architecture
//!
//! The layer operates by wrapping a Lambda service with OpenTelemetry instrumentation:
//! 1. Creates a span for each Lambda invocation
//! 2. Extracts attributes from the event using either:
//!    - Built-in implementations of `SpanAttributesExtractor` for supported event types
//!    - Custom implementations of `SpanAttributesExtractor` for user-defined types
//!    - A closure-based extractor for one-off customizations
//! 3. Propagates context from incoming requests via headers
//! 4. Tracks response status codes
//! 5. Signals completion for span export through the `TelemetryCompletionHandler`
//!
//! # Features
//!
//! - Automatic span creation for Lambda invocations
//! - Built-in support for common AWS event types:
//!   - API Gateway v1/v2 (HTTP method, path, route, protocol)
//!   - Application Load Balancer (HTTP method, path, target group ARN)
//! - Extensible attribute extraction through the `SpanAttributesExtractor` trait
//! - Custom attribute extraction through closure-based extractors
//! - Automatic context propagation from HTTP headers
//! - Response status code tracking
//!
//! # Basic Usage
//!
//! ```no_run
//! use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};
//! use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
//! use aws_lambda_events::event::apigw::ApiGatewayV2httpRequest;
//! use tower::ServiceBuilder;
//!
//! async fn function_handler(event: LambdaEvent<ApiGatewayV2httpRequest>) -> Result<serde_json::Value, Error> {
//!     Ok(serde_json::json!({"statusCode": 200}))
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
//!     
//!     let service = ServiceBuilder::new()
//!         .layer(OtelTracingLayer::new(completion_handler)
//!             .with_name("my-handler"))
//!         .service_fn(function_handler);
//!
//!     Runtime::new(service).run().await
//! }
//! ```
//!
//! # Custom Attribute Extraction
//!
//! You can implement the `SpanAttributesExtractor` trait for your own event types:
//!
//! ```rust,no_run
//! use lambda_otel_lite::{SpanAttributes, SpanAttributesExtractor};
//! use std::collections::HashMap;
//! use opentelemetry::Value;
//! struct MyEvent {
//!     user_id: String,
//! }
//!
//! impl SpanAttributesExtractor for MyEvent {
//!     fn extract_span_attributes(&self) -> SpanAttributes {
//!         let mut attributes = HashMap::new();
//!         attributes.insert("user.id".to_string(), Value::String(self.user_id.clone().into()));
//!         SpanAttributes {
//!             attributes,
//!             ..SpanAttributes::default()
//!         }
//!     }
//! }
//! ```
//!
//! # Context Propagation
//!
//! The layer automatically extracts and propagates tracing context from HTTP headers
//! in supported event types. This enables distributed tracing across service boundaries.
//! The W3C Trace Context format is used for propagation.
//!
//! # Response Tracking
//!
//! For HTTP responses, the layer automatically:
//! - Sets `http.status_code` from the response statusCode
//! - Sets span status to ERROR for 5xx responses
//! - Sets span status to OK for all other responses

use crate::extractors::{set_common_attributes, set_response_attributes, SpanAttributesExtractor};
use crate::TelemetryCompletionHandler;
use futures_util::ready;
use lambda_runtime::{Error, LambdaEvent};
use opentelemetry::trace::Status;
use pin_project::pin_project;
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use std::{
    future::Future,
    pin::Pin,
    task::{self, Poll},
};
use tower::{Layer, Service};
use tracing::{field::Empty, instrument::Instrumented, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Future that calls complete() on the completion handler when the inner future completes.
///
/// This future wraps the inner service future to ensure that spans are properly completed
/// and exported. It:
/// 1. Polls the inner future to completion
/// 2. Extracts response attributes (e.g., status code)
/// 3. Sets span status based on response
/// 4. Signals completion through the completion handler
///
/// This type is created automatically by `OtelTracingService` - you shouldn't need to
/// construct it directly.
#[pin_project]
pub struct CompletionFuture<Fut> {
    #[pin]
    future: Option<Fut>,
    completion_handler: Option<TelemetryCompletionHandler>,
    span: Option<tracing::Span>,
}

impl<Fut, R> Future for CompletionFuture<Fut>
where
    Fut: Future<Output = Result<R, Error>>,
    R: Serialize + Send + 'static,
{
    type Output = Result<R, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let ready = ready!(self
            .as_mut()
            .project()
            .future
            .as_pin_mut()
            .expect("future polled after completion")
            .poll(cx));

        // Extract response attributes if it's a successful response
        if let Ok(response) = &ready {
            if let Ok(value) = serde_json::to_value(response) {
                if let Some(span) = self.span.as_ref() {
                    set_response_attributes(span, &value);
                }
            }
        } else if let Err(error) = &ready {
            if let Some(span) = self.span.as_ref() {
                // Set error status according to OpenTelemetry spec
                span.set_status(Status::error(error.to_string()));
            }
        }

        // Drop the future and span before calling complete
        Pin::set(&mut self.as_mut().project().future, None);
        let this = self.project();
        this.span.take(); // Take ownership and drop the span

        // Now that the span is closed, complete telemetry
        if let Some(handler) = this.completion_handler.take() {
            handler.complete();
        }

        Poll::Ready(ready)
    }
}

/// Tower middleware to create an OpenTelemetry tracing span for Lambda invocations.
///
/// This layer wraps a Lambda service to automatically create and configure OpenTelemetry
/// spans for each invocation. It supports:
/// - Automatic span creation with configurable names
/// - Automatic attribute extraction from supported event types
/// - Context propagation from HTTP headers
/// - Response status tracking
///
/// # Example
///
/// ```no_run
/// use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig, SpanAttributes};
/// use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
/// use aws_lambda_events::event::apigw::ApiGatewayV2httpRequest;
/// use tower::ServiceBuilder;
///
/// async fn handler(event: LambdaEvent<ApiGatewayV2httpRequest>) -> Result<serde_json::Value, Error> {
///     Ok(serde_json::json!({ "statusCode": 200 }))
/// }
///
/// # async fn example() -> Result<(), Error> {
/// let completion_handler = init_telemetry(TelemetryConfig::default()).await?;
///
/// // Create a layer with custom name
/// let layer = OtelTracingLayer::new(completion_handler)
///     .with_name("api-handler");
///
/// // Apply the layer to your handler
/// let service = ServiceBuilder::new()
///     .layer(layer)
///     .service_fn(handler);
///
/// Runtime::new(service).run().await
/// # }
/// ```
#[derive(Clone)]
pub struct OtelTracingLayer<T: SpanAttributesExtractor> {
    completion_handler: TelemetryCompletionHandler,
    name: String,
    _phantom: PhantomData<T>,
}

impl<T: SpanAttributesExtractor> OtelTracingLayer<T> {
    /// Create a new OpenTelemetry tracing layer with the required completion handler.
    ///
    /// The completion handler is used to signal when spans should be exported. It's typically
    /// obtained from [`init_telemetry`](crate::init_telemetry).
    ///
    /// # Arguments
    ///
    /// * `completion_handler` - Handler for managing span export timing
    pub fn new(completion_handler: TelemetryCompletionHandler) -> Self {
        Self {
            completion_handler,
            name: "lambda-invocation".to_string(),
            _phantom: PhantomData,
        }
    }

    /// Set the span name.
    ///
    /// This name will be used for all spans created by this layer. It should describe
    /// the purpose of the Lambda function (e.g., "process-order", "api-handler").
    ///
    /// # Arguments
    ///
    /// * `name` - The name to use for spans
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl<S, T> Layer<S> for OtelTracingLayer<T>
where
    T: SpanAttributesExtractor + Clone,
{
    type Service = OtelTracingService<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        OtelTracingService::<S, T> {
            inner,
            completion_handler: self.completion_handler.clone(),
            name: self.name.clone(),
            is_cold_start: true,
            _phantom: PhantomData,
        }
    }
}

/// Tower service returned by [OtelTracingLayer].
///
/// This service wraps the inner Lambda service to:
/// 1. Create a span for each invocation
/// 2. Extract and set span attributes
/// 3. Propagate context from headers
/// 4. Track response status
/// 5. Signal completion for span export
///
/// The service is created automatically by the layer - you shouldn't need to
/// construct it directly.
#[derive(Clone)]
pub struct OtelTracingService<S, T: SpanAttributesExtractor> {
    inner: S,
    completion_handler: TelemetryCompletionHandler,
    name: String,
    is_cold_start: bool,
    _phantom: PhantomData<T>,
}

impl<S, F, T, R> Service<LambdaEvent<T>> for OtelTracingService<S, T>
where
    S: Service<LambdaEvent<T>, Response = R, Error = Error, Future = F> + Send,
    F: Future<Output = Result<R, Error>> + Send + 'static,
    T: SpanAttributesExtractor + DeserializeOwned + Serialize + Send + 'static,
    R: Serialize + Send + 'static,
{
    type Response = R;
    type Error = Error;
    type Future = CompletionFuture<Instrumented<S::Future>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, event: LambdaEvent<T>) -> Self::Future {
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
        span.record("otel.name", self.name.clone());
        span.record("otel.kind", "SERVER");

        // Set common Lambda attributes with cold start tracking
        set_common_attributes(&span, &event.context, self.is_cold_start);
        if self.is_cold_start {
            self.is_cold_start = false;
        }

        // Extract attributes directly using the trait
        let attrs = event.payload.extract_span_attributes();

        // Apply extracted attributes
        if let Some(span_name) = attrs.span_name {
            span.record("otel.name", span_name);
        }

        if let Some(kind) = &attrs.kind {
            span.record("otel.kind", kind.to_string());
        }

        for (key, value) in &attrs.attributes {
            span.set_attribute(key.to_string(), value.to_string());
        }

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

        let future = {
            let _guard = span.enter();
            self.inner.call(event)
        };

        CompletionFuture {
            future: Some(future.instrument(span.clone())),
            completion_handler: Some(self.completion_handler.clone()),
            span: Some(span),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProcessorMode;
    use futures_util::future::BoxFuture;
    use lambda_runtime::Context;
    use opentelemetry::trace::TraceResult;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::{
        export::trace::{SpanData, SpanExporter},
        trace::TracerProvider,
        Resource,
    };
    use serial_test::serial;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tower::ServiceExt;
    use tracing_subscriber::prelude::*;

    // Mock exporter that counts exports
    #[derive(Debug)]
    struct CountingExporter {
        export_count: Arc<AtomicUsize>,
    }

    impl CountingExporter {
        fn new() -> Self {
            Self {
                export_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl SpanExporter for CountingExporter {
        fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, TraceResult<()>> {
            self.export_count.fetch_add(batch.len(), Ordering::SeqCst);
            Box::pin(futures_util::future::ready(Ok(())))
        }

        fn shutdown(&mut self) {}
    }

    #[tokio::test]
    #[serial]
    async fn test_basic_layer() -> Result<(), Error> {
        let exporter = CountingExporter::new();
        let export_count = exporter.export_count.clone();

        let provider = TracerProvider::builder()
            .with_simple_exporter(exporter)
            .with_resource(Resource::empty())
            .build();
        let provider = Arc::new(provider);

        // Set up tracing subscriber
        let _subscriber = tracing_subscriber::registry::Registry::default()
            .with(tracing_opentelemetry::OpenTelemetryLayer::new(
                provider.tracer("test"),
            ))
            .set_default();

        let completion_handler =
            TelemetryCompletionHandler::new(provider.clone(), None, ProcessorMode::Sync);

        let handler = |_: LambdaEvent<serde_json::Value>| async {
            // Create a span to ensure it's captured
            let _span = tracing::info_span!("test_span");
            Ok::<_, Error>(serde_json::json!({"status": "ok"}))
        };

        let layer = OtelTracingLayer::new(completion_handler).with_name("test-handler");

        let mut svc = tower::ServiceBuilder::new()
            .layer(layer)
            .service_fn(handler);

        let event = LambdaEvent::new(serde_json::json!({}), Context::default());

        let _ = svc.ready().await?.call(event).await?;

        // Wait a bit longer for spans to be exported
        tokio::time::sleep(Duration::from_millis(500)).await;

        assert!(export_count.load(Ordering::SeqCst) > 0);

        Ok(())
    }
}
