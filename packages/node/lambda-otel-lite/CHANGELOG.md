# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.16.1] - 2025-06-20

### Fixed
- **Critical**: Fixed timeout issue in Lambda Extensions API long-polling that caused extensions to fail with "HTTP request timeout" errors
- Made HTTP timeout optional in `syncHttpRequest` function - long-polling requests (event polling) no longer have timeouts while admin operations (registration) retain 5-second timeouts
- Improved error handling and documentation for different types of HTTP requests in the extension

## [0.16.0] - 2025-06-20

### Changed
- **BREAKING**: Upgraded OpenTelemetry dependencies to 2.x versions:
  - `@opentelemetry/core`: ^1.30.1 → ^2.0.0
  - `@opentelemetry/resources`: ^1.30.1 → ^2.0.0
  - `@opentelemetry/sdk-trace-base`: ^1.30.1 → ^2.0.0
  - `@opentelemetry/sdk-trace-node`: ^1.30.1 → ^2.0.0
  - `@opentelemetry/propagator-aws-xray`: ^1.3.1 → ^2.1.0
- **BREAKING**: Updated minimum Node.js version requirement to `^18.19.0 || >=20.6.0` (aligned with OpenTelemetry 2.x requirements)
- Updated `@dev7a/otlp-stdout-span-exporter` to ^0.17.3 (with OpenTelemetry 2.x support)
- Converted Lambda extension from JavaScript to TypeScript for better maintainability:
  - Enhanced type safety with proper TypeScript interfaces
  - Improved error handling and HTTP response processing
  - Added comprehensive JSDoc documentation
  - Better integration with the overall TypeScript codebase
- Refactored extension HTTP client to use native `fetch` API instead of Node.js `http` module:
  - Simplified code with ~60 fewer lines of manual stream handling
  - Better timeout management using `AbortController`
  - Cleaner error handling and more maintainable code
  - Leverages modern web standard APIs available in Node.js 18+

### Fixed
- Updated Resource API usage to be compatible with OpenTelemetry 2.x:
  - Replaced `new Resource()` with `resourceFromAttributes()`
  - Replaced `Resource.default()` with `defaultResource()`
  - Replaced `new Resource({})` with `emptyResource()`
- Fixed TypeScript compilation issues related to OpenTelemetry 2.x changes
- Removed outdated manual type definition files in favor of auto-generated ones

### Migration Notes
- This version requires OpenTelemetry SDK 2.x. If you're using OpenTelemetry 1.x, please use version 0.15.0
- Minimum Node.js version is now 18.19.0 or 20.6.0+ (previously 18.0.0+)
- No API changes required in user code - the upgrade is compatible at the usage level
- The Lambda extension is now a TypeScript module but maintains full backward compatibility

## [0.15.0] - 2025-04-30

### Changed
- Removed batching logic from `LambdaSpanProcessor.forceFlush` - all spans are now exported in a single batch regardless of size
- Modified `forceFlush` to always call the exporter's `export` method, even when the span buffer is empty
- Updated dependency on `@dev7a/otlp-stdout-span-exporter` to 0.15.0 or greater
- Fixed issue where extension could hang waiting for EOF when no spans were sampled
- Removed `LAMBDA_SPAN_PROCESSOR_BATCH_SIZE` environment variable which is no longer needed

## [0.13.0] - 2025-04-16

### Added
- Support for configuring processor mode programmatically via the `processorMode` option in `initTelemetry`. Environment variable `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE` still takes precedence.
- Support for configuring context propagation via the `OTEL_PROPAGATORS` environment variable (comma-separated list). **Supported values:** `tracecontext`, `xray`, `xray-lambda`, `none`. This takes precedence over the `propagators` option in `initTelemetry`.
- Added `LambdaXRayPropagator` which correctly extracts trace context from both incoming headers and the `_X_AMZN_TRACE_ID` environment variable, respecting the `Sampled=0` flag.

### Changed
- **Configuration Precedence:** Updated configuration loading for processor mode, queue size, batch size, and compression level to consistently follow the precedence: Environment Variable > Programmatic Configuration > Default Value. Invalid environment variable values now log a warning and use the fallback instead of raising an error.
- **Default Propagator:** Changed the default propagator (used when `OTEL_PROPAGATORS` env var and `propagators` option are not set) to `[LambdaXRayPropagator(), W3CTraceContextPropagator()]`.
- **HTTP Header Handling:** Improved header normalization to standardize on lowercase while preserving the canonical form of X-Amzn-Trace-Id for compatibility with AWS X-Ray propagation.
- **Enhanced Type Safety:** Improved generic type system for `createTracedHandler` and `AttributesExtractor` to provide better type inference and validation when using specific event types.
- **Exported Types:** Added export for `AttributesExtractor` type to enable better type checking in custom extractors.
- Improved code formatting and organization throughout the codebase, with dedicated modules for configuration and propagation.

## [0.11.3] - 2025-03-25

### Fixed
- Package release fix to ensure all required files are included in the release
- No functional changes from 0.11.2

## [0.11.2] - 2025-03-25

### Added
- Support for custom ID generators via the `idGenerator` option in `initTelemetry`
- Added AWS X-Ray compatible ID generator documentation and examples
- Comprehensive test coverage for ID generator functionality

### Changed
- Updated documentation to include X-Ray integration examples
- Removed deprecated environment variable reference from examples documentation

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