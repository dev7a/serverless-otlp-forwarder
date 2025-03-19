//! OpenTelemetry instrumentation optimized for AWS Lambda functions.
//!
//! This crate provides a lightweight, efficient implementation of OpenTelemetry tracing
//! specifically designed for AWS Lambda functions. It offers flexible processing modes,
//! automatic resource detection, and integration with the Lambda Extensions API.
//!
//! # Features
//!
//! - **Flexible Processing Modes**: Support for synchronous, asynchronous, and custom export strategies
//! - **Automatic Resource Detection**: Automatic extraction of Lambda environment attributes
//! - **Lambda Extension Integration**: Built-in extension for efficient telemetry export
//! - **Efficient Memory Usage**: Fixed-size ring buffer to prevent memory growth
//! - **AWS Event Support**: Automatic extraction of attributes from common AWS event types
//! - **Flexible Context Propagation**: Support for W3C Trace Context and custom propagators
//!
//! # Architecture
//!
//! The crate is organized into several modules, each handling a specific aspect of telemetry:
//!
//! - [`telemetry`]: Core initialization and configuration
//!   - Main entry point via `init_telemetry`
//!   - Configures global tracer and span processors
//!   - Returns a `TelemetryCompletionHandler` for span lifecycle management
//!
//! - [`processor`]: Lambda-optimized span processor
//!   - Fixed-size ring buffer implementation
//!   - Multiple processing modes
//!   - Coordinates with extension for async export
//!
//! - [`extension`]: Lambda Extension implementation
//!   - Manages extension lifecycle and registration
//!   - Handles span export coordination
//!   - Implements graceful shutdown
//!
//! - [`resource`]: Resource attribute management
//!   - Automatic Lambda attribute detection
//!   - Environment-based configuration
//!   - Custom attribute support
//!
//! - [`extractors`]: Event processing
//!   - Built-in support for API Gateway and ALB events
//!   - Extensible trait system for custom events
//!   - W3C Trace Context propagation
//!
//! The crate provides two integration patterns:
//!
//! - [`layer`]: Tower middleware integration
//!   - Best for complex services with middleware chains
//!   - Integrates with Tower's service ecosystem
//!   - Standardized instrumentation across services
//!
//! - [`handler`]: Direct function wrapper
//!   - Best for simple Lambda functions
//!   - Lower overhead for basic use cases
//!   - Quick integration with existing handlers
//!
//! # Processing Modes
//!
//! The crate supports three processing modes for telemetry data:
//!
//! - **Sync Mode**: Direct export in handler thread
//!   - Simple execution path with no IPC overhead
//!   - Efficient for small payloads and low resource environments
//!   - Guarantees span delivery before response
//!
//! - **Async Mode**: Export via Lambda extension
//!   - Requires coordination with extension process
//!   - Additional overhead from IPC
//!   - Best when advanced export features are needed
//!   - Provides retry capabilities through extension
//!
//! - **Finalize Mode**: Custom export strategy
//!   - Full control over export timing and behavior
//!   - Compatible with BatchSpanProcessor
//!   - Best for specialized export requirements
//!
//! See [`processor`] module for detailed documentation on processing modes.
//!
//! # Configuration
//!
//! Configuration is handled through environment variables:
//!
//! - `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Controls processing mode
//!   - "sync" for Sync mode (default)
//!   - "async" for Async mode
//!   - "finalize" for Finalize mode
//!
//! - `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Controls buffer size
//!   - Defaults to 2048 spans
//!   - Should be tuned based on span volume
//!
//! - `OTEL_SERVICE_NAME`: Service name for spans
//!   - Falls back to AWS_LAMBDA_FUNCTION_NAME
//!   - Required for proper service identification
//!
//! See [`constants`] module for centralized constants and [`telemetry`] module for detailed configuration options.
//!
//! # Examples
//!
//! ## Using the Tower Layer
//!
//! ```no_run
//! use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};
//! use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
//! use aws_lambda_events::event::apigw::ApiGatewayV2httpRequest;
//! use tower::ServiceBuilder;
//! use serde_json::Value;
//!
//! async fn function_handler(event: LambdaEvent<ApiGatewayV2httpRequest>) -> Result<Value, Error> {
//!     Ok(serde_json::json!({ "statusCode": 200 }))
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let (tracer, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;
//!     
//!     let service = ServiceBuilder::new()
//!         .layer(OtelTracingLayer::new(completion_handler).with_name("tower-handler"))
//!         .service_fn(function_handler);
//!
//!     Runtime::new(service).run().await
//! }
//! ```
//!
//! See [`layer`] module for more examples of automatic instrumentation.
//!
//! ## Using the Handler Wrapper
//!
//! ```no_run
//! use lambda_otel_lite::{init_telemetry, create_traced_handler, TelemetryConfig};
//! use lambda_runtime::{service_fn, Error, LambdaEvent, Runtime};
//! use serde_json::Value;
//!
//! async fn handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
//!     Ok(serde_json::json!({ "statusCode": 200 }))
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;
//!     
//!     // Create the traced handler
//!     let handler = create_traced_handler(
//!         "my-handler",
//!         completion_handler,
//!         handler
//!     );
//!
//!     // Use it directly with the runtime
//!     Runtime::new(service_fn(handler)).run().await
//! }
//! ```

pub use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;

pub mod constants;
pub mod extension;
pub mod extractors;
pub mod handler;
pub mod layer;
pub mod logger;
pub mod mode;
pub mod processor;
pub mod propagation;
pub mod resource;
pub mod telemetry;

pub use extension::OtelInternalExtension;
pub use extractors::{SpanAttributes, SpanAttributesExtractor, TriggerType};
pub use handler::create_traced_handler;
pub use layer::OtelTracingLayer;
pub use mode::ProcessorMode;
pub use processor::LambdaSpanProcessor;
pub use propagation::LambdaXrayPropagator;
pub use resource::get_lambda_resource;
pub use telemetry::{
    init_telemetry, TelemetryCompletionHandler, TelemetryConfig, TelemetryConfigBuilder,
};

#[cfg(doctest)]
#[macro_use]
extern crate doc_comment;

#[cfg(doctest)]
use doc_comment::doctest;

#[cfg(doctest)]
doctest!("../README.md", readme);
