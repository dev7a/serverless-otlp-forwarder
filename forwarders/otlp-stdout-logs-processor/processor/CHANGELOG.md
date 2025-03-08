# Changelog

All notable changes to the `lambda-otlp-forwarder` crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Span compaction feature that aggregates multiple OTLP payloads into a single request
  - Added `span_compactor` module with payload encoding/decoding functions
  - Implemented `compact_telemetry_payloads` function to merge multiple payloads
  - Added configuration options for controlling compaction behavior
  - Added comprehensive unit tests for the span compactor
- Payload normalization feature that converts different telemetry formats to a standard format
  - Added `convert_to_protobuf` method to `TelemetryData` to standardize on protobuf format
  - Updated `from_log_record` and `from_raw_span` methods to use normalization
  - Added JSON to protobuf conversion using the OTLP schema
  - Added unit tests for payload normalization

### Changed
- Modified `TelemetryData` to implement the `Clone` trait
- Updated `function_handler` in `log_processor.rs` to use the span compactor
- Streamlined telemetry processing to avoid unnecessary compression/decompression cycles:
  - Payloads are now converted to uncompressed protobuf format first
  - Compaction is performed on uncompressed protobuf payloads
  - Compression is only applied once at the end of the process
  - Added a `compress` method to `TelemetryData` for final compression

### Fixed
- N/A

### Removed
- N/A

## [0.1.0] - Initial Release

### Added
- Initial implementation of the Lambda OTLP forwarder
- Support for forwarding CloudWatch log-wrapped OTLP records to collectors
- Support for multiple collectors with different endpoints
- Support for custom headers and authentication
- Support for AWS SigV4 authentication
- OpenTelemetry instrumentation for request tracking 