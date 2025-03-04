# Changelog

## [0.10.0] - 2024-03-04

### Added
- Added comprehensive test suite for request signing functionality
- Added documentation about OpenTelemetry SDK 0.28.0+ compatibility issues and workarounds
- Added example of thread isolation pattern for client creation

### Changed
- Improved error handling throughout the codebase
- Enhanced documentation for feature flags and service defaults
- Updated signing implementation to properly handle different HTTP methods
- Fixed AWS service name documentation to focus on X-Ray usage
- Version aligned with other packages in the monorepo
- Updated example to demonstrate best practices with OpenTelemetry SDK 0.28.0

### Fixed
- Fixed typo in example README (changed "Span are" to "Spans are")
- Replaced unsafe unwrap() calls with proper error handling
- Fixed potential issues with header conversions

## [0.3.0] - 2024-12-16

### Changed
- Updated repository name from lambda-otlp-forwarder to serverless-otlp-forwarder
- Version aligned with other packages in the monorepo

## [0.1.0] - 2024-12-08

### Added
- Initial release
- AWS SigV4 authentication for OpenTelemetry OTLP exporters
- Support for both reqwest and hyper HTTP clients
- Automatic AWS region detection from environment
- Configurable AWS service name
- Compatible with AWS credentials provider chain
- Implements OpenTelemetry's HttpClient trait
- Comprehensive documentation and examples 