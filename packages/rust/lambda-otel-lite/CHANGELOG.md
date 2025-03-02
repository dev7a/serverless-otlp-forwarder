# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


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

### Fixed
- Fixed resource attribute access in tests to properly handle the tuple structure returned by the resource iterator

### Documentation
- Updated examples to use the new Resource builder pattern
- Updated examples to handle the new tuple return type from `init_telemetry()`
- Improved module-level documentation with clearer examples
- Enhanced API documentation to reflect the updated methods

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

