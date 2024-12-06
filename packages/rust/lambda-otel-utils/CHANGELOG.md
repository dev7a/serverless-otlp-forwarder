# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.2.1] - 2024-12-05

### Added
- Temporarily added OpenTelemetryLayer from aws-lambda-rust-runtime to support specifying the span kind
- Added a tracing opentelemetry subscriber builder (OpenTelemetrySubscriberBuilder)

### Changed
- Updated OpenTelemetry dependencies to version 0.27.0
- Separated the tracing subscriber and layers integration into their own modules
- Updated the README.md
- Updated the example

### Removed
- Removed HttpPropagationLayer in favor of using just the OpenTelemetryLayer for Lambda

## [0.1.0] - 2024-11-20

### Added
- Initial release
- OpenTelemetry integration for AWS Lambda functions
- Support for OpenTelemetry 0.26.0
