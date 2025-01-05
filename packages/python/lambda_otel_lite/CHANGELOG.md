# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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