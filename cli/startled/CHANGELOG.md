# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2025-06-23

### Added
- **Summary Page Implementation**: New comprehensive summary page accessible via memory size links in navigation
- **Response Latency Metric**: Added to all reports and prominently featured in summary pages
- **Enhanced Navigation**: Summary pages now accessible via clicking memory size links (e.g., "128MB", "512MB")
- **Improved Layout**: Single-column layout with better spacing, color-coded charts with legends for improved readability

### Fixed
- **Chart Title Bug**: Individual metric pages now correctly display metric-specific titles instead of custom report titles
- **Theme Switching**: Resolved JavaScript errors when switching between light/dark themes on summary pages
- **Naming Consistency**: All warm start metrics now use consistent `warm-start-` prefix in directory structure
- **Data Completeness**: Added missing cold start variants for memory usage and produced bytes metrics

### Changed
- **Enhanced Metric Descriptions**: Improved AWS-documentation-based descriptions with clear cold/warm start distinctions
- **Code Quality**: Improved enum memory efficiency by boxing large variant fields to resolve clippy warnings
- **Navigation Organization**: Streamlined navigation by removing redundant "Resources" category

## [0.6.0] - 2024-06-10

### Added
- **Cold Start Resource Metrics**: Added cold start variants for memory usage and produced bytes charts, providing visibility into resource consumption during Lambda initialization
- **Complete Set of Warm Start Platform Metrics**: Added remaining warm start platform metrics including response latency, response duration, runtime overhead, and runtime done duration for comprehensive performance analysis
- **Enhanced Metric Descriptions**: Updated and expanded AWS-documentation-based descriptions for all new metrics, including cold/warm start distinctions for resource metrics

### Changed
- **Consistent Directory Naming**: All warm start charts now use the `warm-start-` prefix for consistent external interface naming:
  - `client-duration/` → `warm-start-client-duration/`
  - `server-duration/` → `warm-start-server-duration/`
  - `extension-overhead/` → `warm-start-extension-overhead/`
  - `memory-usage/` → `warm-start-memory-usage/`
  - `produced-bytes/` → `warm-start-produced-bytes/`
- **Reorganized Navigation Layout**: Eliminated redundant "Resources" section, integrating memory and produced bytes metrics directly into their respective cold start and warm start sections for improved logical organization
- **Optimized CSS Layout**: Updated navigation styling to accommodate additional metrics with a clean 2-row desktop layout (width: `calc(100% / 14)`, min-width: `5.5rem`)
- **Improved Cold Start Ordering**: Moved "Total Cold Start Duration" to the end of the cold start navigation section for better logical flow

### Fixed
- **Complete Data Coverage**: Fixed missing cold start data for memory usage and produced bytes - these metrics are now available for both cold and warm start scenarios
- Updated test cases to reflect new consistent metric naming conventions

## [0.5.1] - 2025-06-21

### Added
- `--suffix` option to `report` command for customizing the file extension of generated reports (default: html)
  - Enables generating reports in different formats (Markdown, plain text, etc.) when combined with custom templates
  - Applies to both landing pages and individual chart pages
  - Fully backward compatible - defaults to `.html` for existing users

### Changed
- Updated README.md examples to demonstrate generating reports with different file formats

## [0.5.0] - 2025-06-21

### Added
- **AWS-Documentation-Based Metric Descriptions**: Each chart page now displays comprehensive descriptions explaining what each metric represents, based on official AWS Lambda documentation
  - Cold start metrics: init duration, server duration, extension overhead, total duration, response latency/duration, runtime overhead
  - Warm start metrics: client/server duration, extension overhead, response latency/duration, runtime overhead  
  - Resource metrics: memory usage, produced bytes
  - Descriptions explain AWS CloudWatch equivalents, platform.runtimeDone metrics, and performance implications
- Enhanced visual styling for metric description display with dedicated CSS styling

### Changed
- Improved color contrast in dark theme (text-secondary: `#565f89` → `#b1bfff`)
- Lightened background in light theme (secondary-bg: `#f5f5f5` → `#f7f7f7`)
- Simplified readme content styling for better readability

## [0.4.1] - 2025-06-21

### Fixed
- Fixed JavaScript file embedding issue that caused "Failed to copy default lib.js" error when using distributed binaries from GitHub releases. The JS file is now properly embedded at compile time using `include_str!()` instead of relying on `CARGO_MANIFEST_DIR`.

## [0.4.0] - 2025-06-21

### Added
- `--title` option to `report` command for customizing the report landing page title
- `--description` option to `report` command for adding descriptive text to the landing page

### Changed
- Removed duplicative items-grid section from landing page template in favor of cleaner sidebar navigation
- Updated CLI usage examples to demonstrate new `--title` and `--description` options

## [0.3.3] - 2025-05-18

### Changed
- Updated `startled` CLI version from 0.3.2 to 0.3.3 in `Cargo.toml`.
- Removed `serde_yaml` and `reqwest` from the `Cargo.toml` file.


## [0.3.2] - 2025-05-13

### Changed
- Minor updates to README.md and main.rs to consistently use named arguments.

## [0.3.1] - 2025-05-13

### Added
- Combined chart view that shows both bar charts (statistical aggregates) and line charts (time series data) on the same page

### Changed
- Improved chart layout and styling for better visualization
- Enhanced screenshot functionality with larger window size for capturing both chart types
- Added support for local browsing with proper link suffix handling with `--local-browsing`
- Improved numerical precision and formatting for chart values
- Updated color palette for better readability
- Various code improvements and refactoring of chart rendering logic

### Fixed
- Improved screenshot reliability with additional waits between rendering stages

## [0.3.0] - 2025-05-12

### Added
- `--parallel` option to `stack` command for concurrent benchmarking of selected Lambda functions. Includes an overall progress bar and a final summary for parallel runs, suppressing detailed individual console logs.

### Changed
- `--memory` option is now **required** for both `function` and `stack` commands. This simplifies result directory structures by removing the "default" memory path.

### Fixed
- Improved console output management for parallel `stack` benchmarks to ensure a cleaner progress bar display by serializing configuration printing and conditionally suppressing other verbose logs from individual function benchmark tasks.

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

