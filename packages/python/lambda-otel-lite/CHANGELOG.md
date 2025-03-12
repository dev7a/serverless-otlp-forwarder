# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.1] - 2025-03-11

### Added
- Support for custom context propagators via the `propagators` parameter in `init_telemetry`
- Added tests for custom propagator functionality

## [0.10.0] - 2025-03-08

### Changed
- Updated build system to use Hatch instead of setuptools
- Implemented automatic version management using Hatch's version hook
- Version is now managed in a single place (pyproject.toml)
- Removed hardcoded version from __init__.py

## [0.9.0] - 2025-02-23

### Changed
- Version bump to align with Node.js package version
- All functionality remains the same as 0.8.2

## [0.8.2] - 2025-02-22

### Changed
- Added ARM architecture support in CI/CD pipeline
- Improved code formatting and type safety
- Fixed mypy configuration for consistent type checking

## [0.8.1] - 2025-02-22

### Fixed
- Fixed API Gateway v2 event extraction to use `rawPath` as `http.route` when `routeKey` is `$default`
- Aligned Python and Node.js implementations for consistent attribute extraction behavior
- Improved handling of missing attributes and case sensitivity in headers

## [0.8.0] - 2025-02-20

### Breaking Changes
- Changed `init_telemetry` return type to `tuple[Tracer, TelemetryCompletionHandler]`
- Removed `traced_handler` interface and replaced with `create_traced_handler`
- Removed direct span processor configuration in favor of the `span_processors` parameter
- Removed `urllib3` dependency in favor of standard library `http.client` to reduce package size
- Removed `opentelemetry-exporter-otlp-proto-http` from core dependencies (now optional for advanced usage)

### Added
- New `TelemetryCompletionHandler` class for coordinating span flushing
- Event-specific extractors for span attributes and context propagation:
  - `api_gateway_v1_extractor` for REST API
  - `api_gateway_v2_extractor` for HTTP API
  - `alb_extractor` for Application Load Balancer
  - `default_extractor` for basic Lambda attributes
- Standardized trigger types via `TriggerType` constants
- Improved connection handling and error management in extension
- Comprehensive docstrings and type hints

### Changed
- Simplified handler interface with context manager pattern
- Improved extension implementation with better connection reuse
- Enhanced error handling and logging throughout
- Updated documentation with new interfaces and examples
- Renamed environment variable to `OTEL_LAMBDA_PROCESSOR_MODE` for consistency
- Updated documentation to clarify that OTLP HTTP exporter is an optional dependency for advanced usage

### Fixed
- Connection reuse in extension with proper response body reading
- Error propagation and status code validation
- Detailed error logging in extension and processor

## [0.5.1] - 2025-01-18

### Changed
- Consolidated ProcessorMode into a single source of truth in __init__.py
- Ensure consistent default mode SYNC across all modules
- Improved documentation clarity and accuracy:
  - Clarified that FAAS attributes are HTTP-only
  - Improved installation instructions with venv creation
  - Simplified examples and removed redundant ones
  - Improved docstrings for name parameter in init_telemetry
  - Fixed invocation_id source documentation
- Code improvements:
  - Simplified processor implementations in examples
  - Added function name to example scripts for local testing
  - Simplified test assertions

## [0.5.0] - 2025-01-17

### Breaking Changes
- Simplified the telemetry initialization API
  - Removed separate `span_processor` and `exporter` parameters from `init_telemetry`
  - Added `span_processors` parameter that accepts a list of processors
  - If no processors are provided, defaults to `LambdaSpanProcessor` with `OTLPStdoutSpanExporter`

### Added
- New examples demonstrating different usage patterns:
  - Basic "Hello World" example showing default processor setup
  - Custom processor example showing how to chain multiple processors
- Comprehensive test coverage for the new telemetry initialization API

### Changed
- Updated README.md with clearer examples and documentation
- Improved resource attribute handling in `get_lambda_resource`
- Enhanced type hints and docstrings

## [0.4.0] - 2025-01-13

### Changed
- Removed `LAMBDA_EXTENSION_SPAN_PROCESSOR_FREQUENCY` environment variable and related functionality
- Spans are now flushed after every request in async mode

## [0.3.1] - 2025-01-13

### Fixed
- Fixed missing code block closing in README.md

## [0.3.0] - 2025-01-13

### Breaking Changes
- Replaced `otlp_stdout_adapter` dependency with `otlp-stdout-span-exporter`
- Changed default exporter to use `OTLPStdoutSpanExporter` instead of `OTLPSpanExporter`

### Added
- Support for `OTEL_SERVICE_NAME` environment variable to override service name
- Support for `OTEL_RESOURCE_ATTRIBUTES` environment variable for custom resource attributes
- Support for configurable compression level via `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`

### Enhanced
- Improved resource attribute handling with proper URL decoding
- Enhanced type safety with `Final` type annotations
- More robust resource merging strategy

## [0.2.0] - 2025-01-04

### Added
- Automatic context propagation from HTTP headers
- Support for custom carrier extraction via `get_carrier` parameter
- Automatic FAAS attributes from Lambda context and events
- Cold start detection and tracking
- Optimizations for cold start performance
- HTTP status code tracking and span status updates (5xx only)
- API Gateway v1 and v2 attribute detection
- Proper HTTP route, method, target, and scheme attributes

### Changed
- Moved `traced_handler` to its own module for better organization
- Moved telemetry initialization to dedicated module
- Improved error handling in context propagation
- Removed dependency on `typing` module (requires Python 3.12+)
- Using string literals for attribute names instead of constants
- Improved trigger detection to match AWS conventions
- Only set span status to error for 5xx responses

### Fixed
- Extraction of cloud account ID from Lambda context ARN
- HTTP trigger detection to use requestContext

## [0.1.1] - 2024-12-28

### Added
- Project URLs in package metadata

## [0.1.0] - 2024-12-28

### Added
- Initial release of lambda-otel-lite
- Core `LambdaSpanProcessor` implementation for efficient span processing in AWS Lambda
- Support for synchronous, asynchronous, and finalize processing modes
- Integration with OpenTelemetry SDK and OTLP exporters
- Lambda-specific resource detection and attributes
- Comprehensive test suite and documentation 