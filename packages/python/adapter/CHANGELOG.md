# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2024-12-16

### Changed
- Updated repository name from lambda-otlp-forwarder to serverless-otlp-forwarder
- Version aligned with other packages in the monorepo

## [0.1.1] - 2024-11-10

### Fixed
- Updated dependency specifications to comply with PEP 508 format

## [0.1.0] - 2024-10-30

### Added
- Initial release of the OTLP Stdout Adapter
- Custom HTTP adapter for OpenTelemetry OTLP exporters
- Support for JSON and Protobuf payloads
- GZIP compression support
- Base64 encoding for binary payloads
- AWS Lambda resource attributes
- Environment variable configuration
- Comprehensive test suite
- MIT License
- Full documentation in README.md

### Notes
- While the OpenTelemetry specification supports both JSON and Protobuf over HTTP, the Python SDK currently only supports Protobuf (see [opentelemetry-python#1003](https://github.com/open-telemetry/opentelemetry-python/issues/1003)) 