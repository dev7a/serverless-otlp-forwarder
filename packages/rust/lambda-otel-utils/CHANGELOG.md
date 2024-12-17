# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2024-12-16

### Changed
- Updated repository name from lambda-otlp-forwarder to serverless-otlp-forwarder
- Version aligned with other packages in the monorepo

## [0.2.2] - 2024-12-08

### Changed
- Moved to Cargo workspace configuration
- Updated dependencies to use workspace-level versions
- Renamed examples for better clarity (basic → tracing_example)
- Added missing AWS and OpenTelemetry related dependencies
- Fixed test dependencies

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
