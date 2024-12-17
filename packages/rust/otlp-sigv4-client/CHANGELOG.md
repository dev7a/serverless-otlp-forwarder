# Changelog

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