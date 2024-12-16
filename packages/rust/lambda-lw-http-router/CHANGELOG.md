# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2024-12-14
### Added
- Added header capturing to OpenTelemetry spans (thanks to Gary Sassano, https://github.com/dev7a/lambda-otlp-forwarder/pull/27)
- Updated lambda-lw-http-router-core dependency to version 0.1.3
## [0.1.2] - 2024-12-13

### Fixed
- Fixed https://github.com/dev7a/serverless-otlp-forwarder/issues/22
- Updated dependency for lambda-lw-http-router-core dependency to version 0.1.2

## [0.1.1] - 2024-11-23

### Changed
- Updated lambda-lw-http-router-core dependency to version 0.1.1

## [0.1.0] - 2024-11-20

### Added
- Initial release
- HTTP router for AWS Lambda functions
- Support for API Gateway and Application Load Balancer events
