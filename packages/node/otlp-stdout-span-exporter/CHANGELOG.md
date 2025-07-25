# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.17.3] - 2025-06-19

### Fixed
- Fixed missing ESM wrapper in published package by integrating build-esm-wrapper into main build process
- This ensures the `/esm` subpath export actually works in the published npm package

## [0.17.2] - 2025-06-19

### Fixed
- Fixed webpack bundling issue by changing export strategy
- Main export now always provides CommonJS for compatibility
- ESM support moved to `/esm` subpath export for native Node.js environments only

### Changed
- ESM is no longer available via the main export due to webpack incompatibility
- Added error detection for webpack bundling in ESM wrapper

## [0.17.1] - 2025-06-19

### Fixed
- Fixed webpack bundling compatibility by using Node.js-specific export condition
- ESM wrapper now only loads in native Node.js environments, preventing `MODULE_NOT_FOUND` errors in bundled applications
- Bundlers like webpack will now use CommonJS version by default for better compatibility

## [0.17.0] - 2025-06-19

### Added
- ESM (ES Modules) support via lightweight wrapper (adds ~400 bytes)
- Support for both CommonJS (`require`) and ESM (`import`) syntax

### Changed
- **BREAKING**: Moved OpenTelemetry packages from `dependencies` to `peerDependencies` for better version management
- Users now need to install OpenTelemetry packages separately (see README for instructions)
- Updated peer dependency requirements to align with OpenTelemetry SDK 2.x (which uses API 1.3.0+)

## [0.16.0] - 2025-06-14

### Changed
- **BREAKING**: Upgraded OpenTelemetry dependencies to 2.x versions:
  - `@opentelemetry/core`: ^1.30.1 → ^2.0.0
  - `@opentelemetry/sdk-trace-base`: ^1.30.1 → ^2.0.0
  - `@opentelemetry/otlp-transformer`: ^0.57.2 → ^0.200.0
  - `@opentelemetry/api`: ^1.9.0 → ^1.1.0 (peer dependency)
  - `@opentelemetry/sdk-trace-node`: ^1.30.1 → ^2.0.0 (dev dependency)
- **BREAKING**: Updated minimum Node.js version requirement to `^18.19.0 || >=20.6.0` (aligned with OpenTelemetry 2.x requirements)

### Migration Notes
- This version requires OpenTelemetry SDK 2.x. If you're using OpenTelemetry 1.x, please use version 0.15.0
- Minimum Node.js version is now 18.19.0 or 20.6.0+ (previously 18.0.0+)
- No API changes required in user code - the upgrade is compatible at the usage level

## [0.15.0] - 2025-04-30

### Added
- Added support for generating EOF signals on named pipes when exporting empty span batches
- Implemented "pipe touch" operation (open and immediately close the pipe) when export is called with empty spans

### Fixed
- Fixed issue where downstream extensions would hang waiting for EOF when no spans were sampled
- Resolved edge case where named pipes wouldn't receive EOF signals during empty span flushes

### Changed
- Version bump to align with other packages in the monorepo

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