# Changelog

All notable changes to the serverless-otlp-forwarder project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Span compaction feature in the Lambda OTLP forwarder
  - Aggregates multiple OTLP payloads into a single request
  - Reduces the number of HTTP requests to collectors
  - Improves efficiency and reduces costs
- Payload normalization in the Lambda OTLP forwarder
  - Converts different telemetry formats (JSON, protobuf) to a standardized format
  - Ensures consistent handling of all telemetry data
  - Improves compaction effectiveness and compatibility
- Improved experimental processors
  - Enhanced AWS span processor with direct protobuf conversion
  - Updated Kinesis processor with span compaction and streamlined processing
  - Consistent approach across all processors for better maintainability

### Changed
- Updated `TelemetryData` in the forwarder to implement the `Clone` trait
- Streamlined telemetry processing to optimize performance:
  - Payloads are now converted to uncompressed protobuf format first
  - Compaction is performed on uncompressed protobuf payloads
  - Compression is only applied once at the end of the process
  - Eliminates unnecessary compression/decompression cycles

### Fixed
- N/A

### Removed
- N/A

## [0.9.0] - Initial Release

### Added
- Initial implementation of the serverless-otlp-forwarder
- Lambda OTLP forwarder for CloudWatch logs
- Lambda OTLP Lite for efficient OpenTelemetry instrumentation in Lambda functions
- OTLP stdout span exporter for Lambda functions
- OTLP SigV4 client for AWS authentication
- Support for multiple collectors with different endpoints
- Comprehensive documentation and examples 