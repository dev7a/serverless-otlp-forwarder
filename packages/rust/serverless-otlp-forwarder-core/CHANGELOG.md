# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-11-01

### Fixed
- Updated `async-trait` to `0.1.92` to avoid a redundant generated `#[must_use]` attribute rejected by Clippy 1.99.

### Changed
- **Breaking**: Raised the workspace minimum supported Rust version to `1.85`. The platform TLS verifier uses an edition-2024 macOS dependency.
- **Breaking**: Upgraded the public HTTP client types from reqwest 0.12 to 0.13 and the OpenTelemetry protobuf types to 0.32.
- Unified simple and instrumented clients on reqwest 0.13, removing the separate `reqwest13` dependency.
- The default Rustls client now verifies certificates with `rustls-platform-verifier` and the operating system trust store instead of bundled webpki roots. Minimal Linux containers and Lambda custom runtimes must provide a usable CA bundle.
- Updated `otlp-stdout-span-exporter` to `0.18.0`, requiring `opentelemetry_sdk >= 0.32.1` with the fix for GHSA-w9wp-h8wv-79jx.

## [0.2.1] - 2026-04-23

### Changed
- Removed sensitive values from emitted tracing and logging across the HTTP sender, processor, telemetry conversion, and span compaction paths
- Replaced high-cardinality log messages with static event names plus allowlisted fields such as counts, status codes, sizes, compression settings, and timeout metadata
- Sanitized non-success OTLP export errors so they report the HTTP status without including raw response bodies

## [0.2.0] - 2026-03-30

### Changed
- Updated the crate to the OpenTelemetry 0.31 line
- Updated the optional instrumented HTTP client path to use the current `reqwest-tracing` middleware stack
- Reworked the forwarder client response contract to support the mixed HTTP client versions required by the newer tracing stack
- Made `HttpForwarderResponse` constructible and readable by external crates implementing `HttpOtlpForwarderClient`

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
