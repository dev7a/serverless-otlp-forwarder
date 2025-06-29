# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-01-26

### Added
- Initial release of the serverless-otlp-forwarder-core crate
- Core `TelemetryData` struct for standardized telemetry representation
- `EventParser` trait for pluggable event parsing from different AWS sources
- Span compaction functionality to merge multiple OTLP messages into single batches
- HTTP sender with configurable OTLP export (supports standard OpenTelemetry environment variables)
- Generic `process_event_batch` orchestrator function
- Zero-boilerplate HTTP client implementations (simple, with timeout, instrumented)
- Optional instrumented client with request tracing and middleware support
- Support for standard OTLP environment variables (`OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, `OTEL_EXPORTER_OTLP_TRACES_HEADERS`, etc.)
- Configurable compression with environment variable support (`OTEL_EXPORTER_OTLP_COMPRESSION`, `OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL`)
- Comprehensive test suite with 83+ tests
- Complete API documentation with usage examples
- Support for both JSON and protobuf payload conversion
- Built-in gzip compression and decompression utilities 