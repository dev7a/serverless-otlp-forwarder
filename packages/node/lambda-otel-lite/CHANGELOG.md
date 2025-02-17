# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2025-02-16

### Breaking Changes
- Changed `initTelemetry()` to return both `tracer` and `completionHandler` in a single object
- Removed `name` parameter from `initTelemetry()` function
- Changed `getTracer()` to no longer require a name parameter
- Renamed `tracedHandler` to `createTracedHandler` for better clarity
- Updated handler interface to use a more functional approach
- Removed index signature from `LambdaContext` interface for better type safety

### Added
- New `version.ts` module to centralize package version information
- Added library instrumentation scope attributes
- Added telemetry configuration resource attributes:
  - `lambda_otel_lite.extension.span_processor_mode`
  - `lambda_otel_lite.lambda_span_processor.queue_size`
  - `lambda_otel_lite.lambda_span_processor.batch_size`
  - `lambda_otel_lite.otlp_stdout_span_exporter.compression_level`
- Exported `getLambdaResource` function for custom resource creation
- Added comprehensive test coverage for resource attributes and completion handler
- Enhanced TypeScript type definitions for better developer experience

### Changed
- Improved documentation with more detailed examples and explanations
- Simplified handler creation with a more intuitive API
- Optimized tracer creation by caching instance in TelemetryCompletionHandler
- Removed unused dependencies
- Updated all OpenTelemetry dependencies to latest versions

### Fixed
- Improved error handling in context extraction
- Better type safety in Lambda context handling

## [0.6.1] - 2025-02-15

### Added
- Package metadata improvements:
  - Added `engines` field specifying Node.js version requirement
  - Added comprehensive publishing checklist
  - Added package.json linting configuration
- Example package improvements:
  - Added proper package metadata
  - Added correct dependency versions
  - Added build and start scripts

### Fixed
- Fixed dependency version formats to use caret (^) instead of tilde (~)
- Fixed scripts ordering in package.json
- Added missing license and repository information

## [0.6.0] - 2025-02-15

### Breaking Changes
- Complete overhaul of the handler interface:
  - Removed direct tracer/provider parameters from `TracedHandlerOptions`
  - Introduced `completionHandler` from `initTelemetry` as the main configuration point
  - Changed function signature to `tracedHandler(options, event, context, fn)`
  - Removed legacy interface with `fn` in options object
- Moved all span configuration to extractors:
  - Moved `links` from handler options to extractor attributes
  - Removed `startTime` parameter
  - Removed `parentContext` parameter (now handled via carrier in extractors)
  - Changed span name precedence: extractor's `spanName` takes priority over handler's `name`
- Simplified attribute extraction:
  - Removed built-in HTTP attribute extraction from handler
  - All attribute extraction now happens through extractors
  - Removed `getCarrier` option (now part of extractor interface)
- Restructured project layout:
  - Moved internal modules to `internal` directory
  - Removed `telemetry` directory
  - Consolidated types into respective modules

### Added
- New attribute extractor interface with comprehensive documentation
- Built-in extractors for common AWS event types:
  - API Gateway v1/v2
  - Application Load Balancer (ALB)
- Type definitions for AWS Lambda events
- Improved TypeScript type safety throughout

### Changed
- Improved logging with more specific prefix `[lambda-otel-lite]` instead of `[runtime]`
- Better error handling and logging in extension
- Simplified extension initialization
- Improved documentation and examples
- Better alignment with Rust implementation

### Fixed
- Documentation improvements and corrections
- More consistent error handling
- Better type safety in internal APIs

## [0.5.0] - 2025-01-18

### Added
- Support for chaining multiple span processors in order
  - Changed `spanProcessor` option to `spanProcessors` array
  - Old: `initTelemetry('name', { spanProcessor: new MyProcessor() })`
  - New: `initTelemetry('name', { spanProcessors: [new MyProcessor()] })`
- Added ability to run examples locally
- Added detailed SAM template examples for async mode configuration
- Added comprehensive environment variables documentation

### Changed
- Updated documentation to use JavaScript examples throughout

### Fixed
- Fixed documentation for processor chaining order

## [0.4.0] - 2025-01-13

### Added
- New handler interface that takes the handler function as a separate parameter for better ergonomics
  ```typescript
  tracedHandler(options, async (span) => { ... })
  ```

### Changed
- Deprecated the old handler interface with `fn` in options object
  - Old interface still works but will show deprecation notice
  - Will be removed in a future major version

### Removed
- Removed flush frequency behavior in async mode
  - Spans are now flushed after every handler completion
  - Removed `LAMBDA_EXTENSION_SPAN_PROCESSOR_FREQUENCY` environment variable

## [0.3.1] - 2025-01-13

### Added
- Export `OTLPStdoutSpanExporter` from main package for easier access
- Improved extension initialization sequence with proper Lambda runtime synchronization

### Changed
- Updated to use `@dev7a/otlp-stdout-span-exporter` instead of `@dev7a/otlp-stdout-exporter`
- Unified logging approach across runtime and extension
- Standardized code formatting and indentation
- Improved error reporting and context in log messages

### Fixed
- Extension initialization and event handling sequence
- Removed unused `processDetectorSync` from resource detectors

## [0.1.1] - 2025-01-05

### Fixed
- Aligned `faas.trigger` attribute behavior with Python implementation:
  - Set default value to 'other' unconditionally
  - Simplified HTTP request detection logic
  - Fixed attribute setting order

## [0.1.0] - 2025-01-05

### Added
- Initial release with basic Lambda instrumentation support
- Automatic FAAS attribute detection
- Support for API Gateway events
- Distributed tracing capabilities
- Three processing modes: sync, async, and finalize 