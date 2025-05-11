# Release Notes - startled v0.2.0

**Release Date:** 2025-05-11

This release of `startled` (v0.2.0) introduces significant enhancements to metrics reporting and report usability.

## Key Highlights

*   **Enhanced Metrics Collection & Reporting**: `startled` now captures a richer set of platform metrics from AWS Lambda, including:
    *   Response Latency
    *   Response Duration
    *   Runtime Overhead
    *   Produced Bytes
    *   Runtime Done Duration
    These new metrics are available in both the raw JSON output and the generated HTML reports, providing deeper insights into function performance.

*   **Standard Deviation in Reports**: All statistical summaries and bar charts in the HTML report now include Standard Deviation (StdDev), offering a better understanding of the variability in your benchmark results alongside averages and percentiles.

*   **Improved HTML Report Layout**: The navigation for metric groups (Cold Start, Warm Start, Resources) within the HTML report has been redesigned. Groups are now stacked vertically, and the links within each group are arranged in a responsive grid, making it easier to navigate and view on various screen sizes.

*   **Accurate Sub-Millisecond Reporting**: Values in HTML charts, especially for short durations, are now rounded to 3 decimal places, ensuring that sub-millisecond timings are accurately represented instead of being rounded to zero.

*   **Full-Length Descriptions**: Link labels and page titles in the HTML report now use their full, descriptive names for clarity, leveraging the new flexible layout.

## Detailed Changes

### Added
- Collection and reporting (JSON & HTML) for new platform metrics: Response Latency, Response Duration, Runtime Overhead, Produced Bytes, and Runtime Done Duration.
- Calculation and display of Standard Deviation (StdDev) for all relevant metrics in HTML reports.
- `PUBLISHING.md`: A comprehensive guide for the internal release process.

### Changed
- **HTML Report UI**: 
    - Navigation groups (Cold Start, Warm Start, Resources) are now stacked vertically.
    - Links within these groups wrap into a grid layout, improving usability on different screen widths.
    - Link labels and page H1 titles reverted to full descriptive text.
- **Data Precision**: Statistical values in HTML bar charts are now rounded to 3 decimal places.
- **Telemetry**: `telemetry.rs` updated for conditional console tracing via the `TRACING_STDOUT` environment variable.
- Minor updates to `testbed/Makefile` and `testbed/testbed.md`.

### Fixed
- Addressed various test failures and linter warnings that arose during the development of these new features.
- Resolved CSS issues to ensure correct chart display and navigation link layout.
- Corrected test data in `benchmark.rs` and `stats.rs` to properly initialize all metric fields in test structs.

---
We believe these enhancements make `startled` an even more powerful tool for Lambda performance analysis. As always, feedback and contributions are welcome!
