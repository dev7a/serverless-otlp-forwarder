# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.2] - 2025-02-22

### Changed
- Added ARM architecture support in CI/CD pipeline
- Enhanced test coverage with multi-architecture testing

## [0.8.1] - 2025-02-22

### Fixed
- Fixed API Gateway v2 event extraction to use `rawPath` as `http.route` when `routeKey` is `$default`
- Aligned Python and Node.js implementations for consistent attribute extraction behavior

## [0.8.0] - 2025-02-21

### Breaking Changes
- Removed direct span access from handler function signature
  - Old: `handler(async (event, context, span) => { ... })`
  - New: `handler(async (event, context) => { ... })`
- Changed handler creation API to match Python implementation
  - Old: `createTracedHandler(completionHandler, { name, attributesExtractor })`
  - New: `createTracedHandler(name, completionHandler, { attributesExtractor })`

### Changed
- Simplified handler interface to use OpenTelemetry API for span access
- Updated examples to use `trace.getActiveSpan()` for span access
- Improved alignment with Python implementation
- Enhanced documentation with updated examples
- Simplified configuration interface

### Fixed
- Improved attribute extraction logic in event extractors:
  - Fixed API Gateway v1 extractor to use `Host` header for `server.address` instead of `requestContext.domainName`
  - Updated API Gateway v2 extractor to use `requestContext.http.userAgent` for user agent
  - Ensured consistent header normalization across all extractors
  - Aligned Python implementation with Node.js for consistent behavior
  - Guaranteed span completion by moving span.end() to finally block in handler

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
  - Changed span name precedence: extractor's `spanName`