# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.6.0] - 2024-02-07
### Added
- New `extractors` module for attribute extraction from Lambda events
- New `resource` module for Lambda resource attribute management
- Automatic extraction of AWS Lambda resource attributes
- Better support for custom event types through `SpanAttributesExtractor` trait
- Comprehensive documentation for all modules and features
- Detailed examples for common use cases and integration patterns

### Changed
- Simplified handler API by removing `TracedHandlerOptions` in favor of direct string names
- Made all modules public for better extensibility
- Moved span attributes functionality from `layer` to dedicated `extractors` module
- Improved module organization and public exports
- Enhanced error handling and logging

### Documentation
- Added comprehensive module-level documentation with clear examples
- Improved architecture documentation with module responsibilities
- Added detailed processing modes documentation
- Enhanced FAAS attributes documentation
- Added integration patterns comparison (Tower Layer vs Handler Wrapper)
- Added best practices for configuration and usage
- Improved API documentation with more examples

