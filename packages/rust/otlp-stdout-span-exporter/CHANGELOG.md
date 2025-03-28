# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.12.0] - 2025-03-28

### Added
- Added optional `level` field in the output for easier filtering in log aggregation systems
- Added `LogLevel` enum with `Debug`, `Info`, `Warn`, and `Error` variants
- Added `OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL` environment variable to set the log level
- Added builder method to set the log level programmatically

## [0.11.1] - 2025-03-26

### Changed
- Updated dependencies to use workspace references for better consistency
- Made `ExporterOutput` struct and fields public for improved API access
- Improved test implementation using builder pattern

## [0.11.0] - 2024-03-18

### Added
- Added centralized constants module for configuration values
- Added builder pattern using the `bon` crate for more idiomatic configuration
- Added comprehensive documentation about configuration precedence rules

### Changed
- [Breaking] Changed configuration precedence to follow cloud-native best practices:
  - Environment variables now always take precedence over constructor parameters
  - Constructor parameters take precedence over default values
- [Breaking] Removed internal `get_compression_level_from_env` method
- [Breaking] Replaced `with_options` method with a more idiomatic builder pattern
- Improved error handling with better logging for invalid configuration values
- Updated tests to verify the new precedence rules and builder pattern

## [0.10.1] - 2025-03-04

### Changed
- Updated Readme

## [0.10.0] - 2025-03-02

### Changed
- Improved stdout serialization
- Improved documentation


## [0.9.0] - 2025-02-24

### Added
- Support for configuring GZIP compression level via `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL` environment variable
- Comprehensive tests for GZIP compression level functionality

### Changed
- Upgraded OpenTelemetry dependencies from 0.27.0 to 0.28.0
- Updated API implementation to match OpenTelemetry SDK 0.28.0 changes
- Improved error handling using `OTelSdkError` instead of `TraceError`
- Enhanced documentation with examples for the new environment variable

## [0.6.0] - 2025-02-09

### Changed
- Version bump to align with lambda-otel-utils and other packages

## [0.1.2] - 2025-01-17

### Changed
- Modified export implementation to perform work synchronously and return a resolved future, making the behavior more explicit

## [0.1.1] - 2025-01-16

### Fixed
- Fixed resource attributes not being properly set in the exporter by moving `set_resource` implementation into the `SpanExporter` trait implementation block 

## [0.1.0] - 2025-01-15

### Added
- Initial release of the otlp-stdout-span-exporter
- Support for exporting OpenTelemetry spans to stdout in OTLP format
- GZIP compression with configurable levels (0-9)
- Environment variable support for service name and headers
- Both simple and batch export modes
- Comprehensive test suite
- Example code demonstrating usage
- Documentation and README 