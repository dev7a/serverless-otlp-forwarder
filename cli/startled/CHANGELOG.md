# Changelog for startled

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-05-11

### Added
- New platform metrics (Response Latency, Response Duration, Runtime Overhead, Produced Bytes, Runtime Done Duration) to data collection, JSON reports, and HTML reports.
- Standard Deviation (StdDev) to all statistical calculations and as a new category in HTML bar chart reports.
- `PUBLISHING.md` guide for release process.

### Changed
- HTML report navigation layout: metric groups are now stacked vertically, and links within groups wrap into a grid for improved readability.
- Reverted link labels and page titles in HTML reports to their full, more descriptive versions.
- Improved rounding for sub-millisecond values in HTML report charts to ensure accurate display (up to 3 decimal places).
- Refined telemetry initialization in `telemetry.rs` for conditional console tracing based on `TRACING_STDOUT` environment variable.
- Updated `testbed/Makefile` and `testbed/testbed.md`.

### Fixed
- Various test failures and linter warnings encountered during the addition of new metrics and report enhancements.
- CSS issues related to chart display and navigation link layout.
- Ensured test data in `benchmark.rs` and `stats.rs` correctly initializes new metric fields.

## [0.1.1] - 2025-05-10

### Added
- Initial project setup for startled CLI.
- Basic benchmarking functionality.
- Screenshot capture feature (optional).
- `tempfile` as a development dependency for managing temporary files in tests.
- New test module `cli/startled/src/benchmark.rs` for `FunctionBenchmarkConfig` creation and `save_report` functionality, covering default memory configurations and successful report saving.
- New test module `cli/startled/src/report.rs` validating utility functions:
    - `snake_to_kebab` string conversion.
    - `calculate_base_path` logic, including scenarios with and without a base URL.
    - Data preparation for bar and line chart rendering, handling edge cases such as empty measurements.

### Changed
- Updated `startled` CLI version from 0.1.0 to 0.1.1 in `Cargo.toml`.

## [0.3.0] - 2025-05-12

### Added
- `--parallel` option to `stack` command for concurrent benchmarking of selected Lambda functions. Includes an overall progress bar and a final summary for parallel runs, suppressing detailed individual console logs.

### Changed
- `--memory` option is now **required** for both `function` and `stack` commands. This simplifies result directory structures by removing the "default" memory path.

### Fixed
- Improved console output management for parallel `stack` benchmarks to ensure a cleaner progress bar display by serializing configuration printing and conditionally suppressing other verbose logs from individual function benchmark tasks.
