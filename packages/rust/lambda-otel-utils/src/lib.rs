//! # lambda-otel-utils
//!
//! `lambda-otel-utils` is a Rust library that provides utilities for integrating
//! OpenTelemetry tracing and metrics with AWS Lambda functions. It simplifies the process of
//! setting up and configuring OpenTelemetry for use in serverless environments.
//!
//! ## Main Components
//!
//! This crate consists of these main modules:
//!
//! - `http_tracer_provider`: Offers the `HttpTracerProviderBuilder` for configuring and
//!   building a custom TracerProvider tailored for Lambda environments.
//! - `http_meter_provider`: Provides the `HttpMeterProviderBuilder` for configuring and
//!   building a custom MeterProvider tailored for Lambda environments.
//! - `otel`: Provides utilities for configuring and building tracing subscribers
//!   with support for OpenTelemetry and other layers.
//! - `subscriber`: Provides utilities for configuring and building tracing subscribers
//!   with support for OpenTelemetry and other layers.
//!
//! ## Example
//!
//! ```rust,no_run
//! use lambda_otel_utils::{HttpTracerProviderBuilder, HttpMeterProviderBuilder};
//! use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent, Runtime};
//! use lambda_runtime::layers::{OpenTelemetryLayer, OpenTelemetryFaasTrigger};
//! use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;
//! use serde_json::Value;
//! use std::time::Duration;
//!
//! async fn function_handler(event: LambdaEvent<ApiGatewayProxyRequest>) -> Result<Value, LambdaError> {
//!     Ok(serde_json::json!({"message": "Hello, World!"}))
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), LambdaError> {
//!     let tracer_provider = HttpTracerProviderBuilder::default()
//!         .with_stdout_client()
//!         .enable_global(true)
//!         .build()?;
//!
//!     let meter_provider = HttpMeterProviderBuilder::default()
//!         .with_stdout_client()
//!         .with_meter_name("my-lambda-function")
//!         .with_export_interval(Duration::from_secs(60))
//!         .build()?;
//!
//!     let service = service_fn(function_handler);
//!
//!     let runtime = Runtime::new(service_fn(function_handler))
//!     .layer(
//!         OpenTelemetryLayer::new(|| {
//!             tracer_provider.force_flush();
//!             meter_provider.force_flush();
//!         }
//!      )
//!      .with_trigger(OpenTelemetryFaasTrigger::Http)
//!     );
//!
//!     runtime.run().await
//! }
//! ```
//!
//! This example demonstrates how to set up a comprehensive tracing and metrics configuration using
//! `lambda-otel-utils` in a Lambda function, including both the TracerProvider, MeterProvider, and
//! the OpenTelemetry layer.

pub mod http_meter_provider;
pub mod http_tracer_provider;
pub mod protocol;
pub mod resource;
pub mod subscriber;
pub mod vended;

pub use http_meter_provider::HttpMeterProviderBuilder;
pub use http_tracer_provider::HttpTracerProviderBuilder;
pub use lambda_runtime::tower::Layer;
pub use opentelemetry::metrics::MeterProvider;
pub use opentelemetry::trace::TracerProvider;
pub use subscriber::{
    create_otel_metrics_layer, create_otel_tracing_layer, init_otel_subscriber,
    OpenTelemetrySubscriberBuilder,
};
pub use vended::lambda_runtime_otel::{OpenTelemetryFaasTrigger, OpenTelemetryLayer};

#[cfg(doctest)]
extern crate doc_comment;

#[cfg(doctest)]
use doc_comment::doctest;

#[cfg(doctest)]
doctest!("../README.md", readme);
