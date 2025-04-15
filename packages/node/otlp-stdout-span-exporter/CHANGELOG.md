# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.0] - 2025-04-14

### Added
- Added optional `level` field in the output for easier filtering in log aggregation systems
- Added `LogLevel` enum with `Debug`, `Info`, `Warn`, and `Error` variants
- Added `OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL` environment variable to set the log level
- Added support for named pipe output as an alternative to stdout
- Added `OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE` environment variable to control output type ("pipe" or "stdout")
- Added comprehensive tests for log level and named pipe output features

### Changed
- Named pipe output uses a fixed path at `/tmp/otlp-stdout-span-exporter.pipe` for consistency
- Improved error handling with fallback to stdout when pipe is unavailable

## [0.11.0] - 2024-11-17

### Changed
- Modified configuration value precedence to ensure environment variables always take precedence over constructor parameters
- Improved error handling for invalid environment variable values
- Enhanced documentation to clearly explain configuration precedence rules

## [0.10.1] - 2025-03-05

### Added
- Support for OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL environment variable to configure compression level
- Improved documentation to align with Python and Rust implementations
- Added unit tests for environment variable compression level support
- Updated code example to use current OpenTelemetry API patterns

## [0.10.0] - 2025-03-05

### Changed
- Version standardization across language implementations
- Added example for simple usage
- Updated dependencies

## [0.1.0] - 2024-01-13

### Added
- Initial release of the OpenTelemetry OTLP Span Exporter
- Support for exporting spans in OTLP format to stdout
- TypeScript type definitions
- Full test coverage
- ESLint configuration
- MIT License
- Comprehensive documentation