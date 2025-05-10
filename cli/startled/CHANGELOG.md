# Changelog for startled

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
