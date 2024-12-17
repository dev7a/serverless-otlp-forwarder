# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2024-12-16

### Changed
- Updated repository name from lambda-otlp-forwarder to serverless-otlp-forwarder
- Version aligned with other packages in the monorepo

## [0.1.3] - 2024-12-14
### Added
- Added header capturing to OpenTelemetry spans (thanks to Gary Sassano, https://github.com/dev7a/lambda-otlp-forwarder/pull/27)

## [0.1.2] - 2024-12-13

### Fixed
- Fixed https://github.com/dev7a/serverless-otlp-forwarder/issues/22

## [0.1.1] - 2024-11-23

### Added
- Implemented `Default` trait for `Router` and `RouterBuilder` types
- Added `#![allow(clippy::type_complexity)]` to acknowledge intentionally complex types

### Changed
- Fixed string comparison in route matching to avoid unnecessary allocation
- Updated doctest for `set_otel_attribute` to use `text` format
- Updated OpenTelemetry dependency to version 0.27.0
- Updated tracing-opentelemetry dependency to version 0.28.0

## [0.1.0] - 2024-11-20

### Added
- Initial release
- Core functionality for HTTP routing in AWS Lambda functions
- Support for API Gateway and Application Load Balancer events
- OpenTelemetry integration with version 0.26.0
