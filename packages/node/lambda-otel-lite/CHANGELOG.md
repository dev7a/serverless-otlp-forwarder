# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.1] - 2025-03-22

### Fixed
- Added missing `./extractors` subpath to package exports, fixing errors when importing from `@dev7a/lambda-otel-lite/extractors`
- Created dedicated extractors directory for cleaner imports
- Updated documentation with examples of both import patterns

## [0.11.0] - 2025-03-18

### Changed
- **Breaking Change**: Changed configuration precedence to ensure environment variables always take precedence over constructor parameters
- Resource attributes for configuration values are now only recorded when the corresponding environment variables are explicitly set
- Updated `LambdaSpanProcessor` to use a consistent approach for handling environment variables
- Added proper error handling and logging for invalid environment variable values
- Exported `LambdaSpanProcessorConfig` interface for improved TypeScript type safety
- Refactored environment variable constants to a centralized `constants.ts` file 
- Exported `ENV_VARS`, `DEFAULTS`, and `RESOURCE_ATTRIBUTES` constants for users of the package
- Moved `getLambdaResource` function from `init.ts` to its own dedicated `resource.ts` file to improve code organization

## [0.10.2] - 2025-03-15

### Added
- Enhanced context propagation by extracting carrier headers from event headers in the `defaultExtractor` function

### Changed
- Reorganized test files to follow a consistent naming pattern (`test_*.ts`)
- Updated Jest configuration to match the new test file naming pattern
- Improved example application to properly serialize event objects in span events
- Enhanced documentation with more detailed examples and explanations
- Removed unnecessary ARN outputs from example template.yaml

## [0.10.1] - 2025-03-11

### Added
- Support for custom context propagators via the `propagators` option in `initTelemetry`
- Added documentation and examples for using custom propagators

### Changed
- Updated dependencies:
  - `@dev7a/otlp-stdout-span-exporter` from ^0.1.0 to ^0.10.1
  - `@opentelemetry/core` from ^1.19.0 to ^1.30.1
  - `@opentelemetry/resources` from ^1.19.0 to ^1.30.1

## [0.10.0] - 2025-03-08

### Changed
- Updated versioning approach to use auto-generated version.ts file
- Version is now managed in a single place (package.json)
- Updated publishing process to use CI/CD pipeline for tagging and publishing

## [0.9.1] - 2025-02-24

### Fixed
- Fixed version mismatch in package.json and src/version.ts

### Changed
- Updated publishing workflow to validate version consistency

## [0.9.0] - 2025-02-22

### Breaking Changes
- Simplified handler creation API by removing configuration object wrapper:
  - Old: `createTracedHandler(name, completionHandler, { attributesExtractor })`
  - New: `createTracedHandler(name, completionHandler, attributesExtractor)`
- Removed `TracerConfig` interface as it's no longer needed

### Changed
- Fixed `faas.max_memory` attribute to be in bytes instead of the raw MB value
- Ensured all numeric attributes are set as numbers instead of strings:
  - `lambda_otel_lite.lambda_span_processor.queue_size`
  - `lambda_otel_lite.lambda_span_processor.batch_size`
  - `lambda_otel_lite.otlp_stdout_span_exporter.compression_level`
- Added Prettier for code formatting:
  - Added `.prettierrc.json` configuration
  - Added `.prettierignore` file
  - Added format scripts to package.json
  - Formatted all code according to style guide
- Updated examples to use the new direct extractor passing style
- Improved alignment with Python and Rust implementations

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