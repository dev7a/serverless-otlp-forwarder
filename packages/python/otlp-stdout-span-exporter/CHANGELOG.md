# Changelog

All notable changes to this project will be documented in this file.

## [0.11.0] - 2025-03-18

### Added
- New constants module that exports `EnvVars`, `Defaults`, and `ResourceAttributes` classes
- Improved documentation for configuration precedence rules
- Added type hints for all constants

### Changed
- Environment variables now always take precedence over constructor parameters
- Improved error handling for invalid configuration values
- Enhanced input validation for environment variables
- Updated README to document the new constants module and precedence rules

## [0.10.0] - 2025-03-05

### Changed
- Version standardization across language implementations
- Updated documentation with consistent structure
- Auto-generated version file during build process using Hatch's built-in version hook
- Updated to MIT license
- Updated OpenTelemetry dependencies to latest versions:
  - opentelemetry-sdk>=1.30.0 (from 1.29.0)
  - opentelemetry-exporter-otlp-proto-common>=1.30.0 (from 1.29.0)

### Added
- Publishing script and CI workflow
- Enhanced error handling

## [0.1.2] - 2025-01-12

### Fixed
- Fixed packaging configuration to correctly install the package in site-packages

## [0.1.1] - 2025-01-12 [YANKED]

### Added
- Type hints support with py.typed marker

### Note
- This version was yanked due to incorrect packaging configuration that caused the package to be installed in the wrong location

## [0.1.0] - 2025-01-12

### Added
- Initial release
- Support for exporting spans to stdout in OTLP format
- GZIP compression support
- Custom headers support
- Service name detection from environment variables