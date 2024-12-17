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
- Renamed examples for better clarity (basic → stdout_export)
- Improved example documentation

## [0.2.1] - 2024-12-05

### Changed
- Updated OpenTelemetry dependencies to 0.27.0
- Migrated to async I/O with `tokio::io::AsyncWrite`
- Updated examples and documentation to use new OpenTelemetry builder pattern

### Fixed
- Resolved potential deadlocks in writer operations
- Fixed thread safety issues in test implementations

## [0.2.0] - 2024-11-20

### Changed
- Major refactoring of package structure
- Simplified API to focus on core stdout functionality
- Updated dependencies to latest stable versions

### Removed
- Removed unused configuration options
- Removed experimental features that were not core to the stdout functionality

### Added
- Improved documentation and examples
- Better error handling and logging

## [0.1.1] - 2024-09-29
### Fixed
- Corrected documentation references to use the correct crate name `otlp-stdout-client`

## [0.1.0] - 2024-09-29
### Added
- Initial release of `otlp-stdout-client`
- Support for exporting OpenTelemetry data to stdout in JSON format
- Designed to work with AWS Lambda environments
- Configurable through standard OpenTelemetry environment variables
- Support for both tracing and metrics (as optional features)
- Local implementation of `LambdaResourceDetector` (temporary solution)
