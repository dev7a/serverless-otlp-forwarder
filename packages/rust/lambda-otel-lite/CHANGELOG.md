# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.15.0] - 2025-04-30

### Changed
- Modified `LambdaSpanProcessor::force_flush` to export all buffered spans in a single batch, ensuring atomicity and supporting the pipe touch mechanism in the exporter.
- Removed the `max_batch_size` configuration option and `LAMBDA_SPAN_PROCESSOR_BATCH_SIZE` environment variable, as batching during flush is no longer performed.

### Dependency Updates
- Updated `otlp-stdout-span-exporter` dependency to `0.15.0`.

## [0.14.0] - 2025-04-21
### Changed
- Updated otlp-stdout-span-exporter dependency to version 0.14.0
- Removed need to use Box::new() when creating OtlpStdoutSpanExporter instances
- Updated SpanExporter trait implementation in testing code to match newer SDK
- Simplified function names in example templates
- Enhanced examples with better event handling and explicit attribute usage

## [0.13.1] - 2025-03-29
### Fixed
- Changed propagator registration order to prioritize W3C TraceContext over X-Ray when both are present
- Fixed handling of Sampled=0 in X-Ray trace headers to allow proper root span sampling
- Improved context extraction logic from the _X_AMZN_TRACE_ID environment variable
- Enhanced X-Ray propagation with better validity and sampling checks

## [0.13.0] - 2025-03-28
### Added
- Added ability to programmatically set the processor mode via `TelemetryConfig.processor_mode`
- Added `ProcessorMode::resolve()` method to handle both environment variable and programmatic configuration
- Environment variable `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE` still takes precedence over programmatic setting

## [0.11.4] - 2025-03-26
### Changed
- Updated dependencies to use workspace references for better consistency and maintainability
- Fixed a missing comma in the Tower example JSON response

## [0.11.3] - 2025-03-24
### Enhanced
- Improved `LAMBDA_TRACING_ENABLE_FMT_LAYER` environment variable handling:
  - Now properly validates exact "true" or "false" values (case-insensitive)
  - Added explicit precedence: environment variable overrides code configuration when present
  - Added warning for invalid values with fallback to code configuration
  - Updated documentation to reflect the new behavior

## [0.11.2] - 2025-03-23
### Added
- Refactored support for the `LAMBDA_TRACING_ENABLE_FMT_LAYER` environment variable to control console logging output without code changes

## [0.11.1] - 2025-03-22
### Fixed
- Fixed service name fallback logic in resource.rs to properly use AWS_LAMBDA_FUNCTION_NAME when OTEL_SERVICE_NAME is not defined, and fall back to "unknown_service" if neither is available

## [0.11.0] - 2025-03-19
### Added
- Added module for centralized constants to ensure consistency across the codebase
- Added environment variable precedence in configuration (env vars > constructor params > defaults)
- Added resource attributes based on explicitly set environment variables only
- Added support for AWS X-Ray propagation through the OTEL_PROPAGATORS environment variable
- Added LambdaXrayPropagator for enhanced X-Ray propagation with Lambda environment detection
- Added with_named_propagator() method to TelemetryConfigBuilder for simpler propagator configuration
- Added default combined propagator setup with both W3C TraceContext and X-Ray
- Improved context propagation by extracting both W3C and X-Ray trace headers

### Changed
- Improved environment variable handling in LambdaSpanProcessor for batch size and queue size
- Updated resource attribute handling logic for better consistency and efficiency
- Fixed potential issues with default values and improved error handling
- Enhanced documentation with more details on configuration options
- Updated dependency versions
- Modified SpanAttributes to support both W3C Trace Context and AWS X-Ray headers

### Documentation
- Added documentation for X-Ray propagation setup and configuration
- Added examples for using multiple propagators together
- Updated documentation on context propagation in HTTP events

## [0.10.2] - 2025-03-15
### Added
- Support for controlling the fmt layer through the `LAMBDA_TRACING_ENABLE_FMT_LAYER` environment variable
- Added tests to verify environment variable handling for the fmt layer configuration

### Changed
- Updated the `TelemetryConfig::default()` implementation to read from environment variables
- Changed the `ApplicationLogLevel` from DEBUG to INFO in the example template.yaml
- Enhanced documentation with more detailed examples and explanations
- Improved example template configuration with better descriptions and properties

## [0.10.1] - 2025-03-11
### Fixed
- Removed debug `println!` statement from `TelemetryCompletionHandler::complete` method
- Fixed documentation examples in README.md:
  - Updated Quick Start example to use proper span access with `tracing::Span::current()`
  - Fixed API Gateway response body type to use `Body::Text` or `.into()`
  - Added feature flag note for Kinesis event example
  - Improved code examples with proper return types

### Documentation
- Enhanced README.md with more detailed examples and explanations
- Added Table of Contents for better navigation
- Improved formatting and structure of code examples
- Updated PUBLISHING.md with correct release branch naming pattern

## [0.10.0] - 2025-03-03
### Changed
- Updated `otlp-stdout-span-exporter` dependency to version 0.10.0

## [0.9.0] - 2025-03-01
### Breaking Changes
- **API Change**: Modified the return type of `init_telemetry()` to return a tuple `(tracer, completion_handler)` instead of just the completion handler
- **Removed**: The `library_name` parameter has been removed from `TelemetryConfig`
- **Removed**: The `ProcessorConfig` struct has been removed, with its functionality integrated directly into the `LambdaSpanProcessor` builder pattern
- **SDK Update**: Updated to OpenTelemetry SDK v0.28.0, which changes how resources and providers are created

### Changed
- **Resource Creation**: Changed from `Resource::new()` to `Resource::builder().with_attributes().build()` pattern
- **Provider Creation**: Changed from `TracerProviderBuilder::default()` to `SdkTracerProvider::builder()`
- **Batch Export**: Removed explicit runtime specification (e.g., `Tokio`) from batch exporter creation
- **HTTP Client**: Updated HTTP client implementation to use `send_bytes` instead of `send`
- **Shutdown Mechanism**: Changed from `global::shutdown_tracer_provider()` to `tracer_provider.shutdown()`
- Added `max_batch_size` parameter directly to `LambdaSpanProcessor` for better control over batch processing
- Added `get_tracer()` method to `TelemetryCompletionHandler` to retrieve the tracer
- Updated tracing-opentelemetry from v0.28.0 to v0.29.0
- Updated error handling to use `OTelSdkError` and `OTelSdkResult` instead of `TraceError` and `TraceResult`

### Improved
- **Logging System**: Enhanced the `Logger` implementation with compile-time initialization support through `const_new`
- **Module Loggers**: Added static module-specific loggers for improved performance and code readability

### Fixed
- Fixed resource attribute access in tests to properly handle the tuple structure returned by the resource iterator

### Documentation
- Updated examples to use the new Resource builder pattern
- Updated examples to handle the new tuple return type from `init_telemetry()`
- Improved module-level documentation with clearer examples
- Enhanced API documentation to reflect the updated methods
- Added documentation for using static loggers with the new `const_new` constructor

## [0.6.0] - 2025-02-07
### Added
- New `extractors` module for attribute extraction from Lambda events
- New `resource` module for Lambda resource attribute management
- Automatic extraction of AWS Lambda resource attributes
- Better support for custom event types through `SpanAttributesExtractor` trait
- Comprehensive documentation for all modules and features
- Detailed examples for common use cases and integration patterns

### Changed
- Simplified handler API by removing `TracedHandlerOptions` in favor of direct string names
- Made all modules public for better extensibility
- Moved span attributes functionality from `layer` to dedicated `extractors` module
- Improved module organization and public exports
- Enhanced error handling and logging

### Documentation
- Added comprehensive module-level documentation with clear examples
- Improved architecture documentation with module responsibilities
- Added detailed processing modes documentation
- Enhanced FAAS attributes documentation
- Added integration patterns comparison (Tower Layer vs Handler Wrapper)
- Added best practices for configuration and usage
- Improved API documentation with more examples

