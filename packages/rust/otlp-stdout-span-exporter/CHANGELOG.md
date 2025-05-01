# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.15.0] - 2025-04-30

### Fixed
- Ensure named pipe output performs an open/close operation (generating EOF) even when exporting an empty batch of spans. This guarantees downstream pipe readers receive a signal after every flush, even if no spans were sampled.

### Added
- Added `BufferOutput` struct, an `Output` implementation useful for testing that captures lines to an internal buffer.
- Added `is_pipe()` and `touch_pipe()` methods to the public `Output` trait.

### Changed
- Made the `Output` trait public (`pub trait Output`).
- Updated `nix` dependency to use workspace version.
- Removed unused `futures-util` dependency.

## [0.14.0] - 2024-06-07

### Changed
- Upgraded OpenTelemetry dependencies from 0.28.0 to 0.29.0
- Updated API implementation to match OpenTelemetry SDK 0.29.0 changes
- Addressed breaking changes and deprecations introduced in OpenTelemetry 0.29.0
- Added separate example for named pipe and stdout output

## [0.13.0] - 2025-03-27

### Added
- Added optional `level` field in the output for easier filtering in log aggregation systems
- Added `LogLevel` enum with `Debug`, `Info`, `Warn`, and `Error` variants
- Added `OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL` environment variable to set the log level
- Added builder method to set the log level programmatically
- Added support for named pipe output as an alternative to stdout
- Added `OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE` environment variable to control output type ("pipe" or "stdout")
- Added builder method `pipe(bool)` to configure named pipe output programmatically

### Changed
- Named pipe output uses a fixed path at `/tmp/otlp-stdout-span-exporter.pipe` for consistency

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