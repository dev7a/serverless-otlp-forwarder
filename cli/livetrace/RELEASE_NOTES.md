## livetrace v0.2.0 - 2025-05-17

This release of `livetrace` introduces powerful new filtering and log fetching capabilities, enhances usability with improved CLI arguments and color palettes, and includes important bug fixes and code refactoring for better maintainability.

### New Features and Enhancements:

*   **CLI Options:**
    *   Added `--grep <REGEX>` to filter spans/events based on attribute values matching a Rust-compatible regex, with highlighted matches in the console.
    *   Introduced `--backtrace <DURATION>` to fetch logs from a specified duration ago in polling mode.
    *   Implemented shell completion for the `--theme` argument using `clap::ValueEnum`.
    *   Enhanced duration-based CLI arguments to accept units (e.g., `s`, `m`, `h`) for better flexibility.
*   **Color Palettes:**
    *   Introduced "red-safe" alternatives in `TABLEAU_12`, `MATERIAL_12`, and `SOLARIZED_12` palettes to avoid confusion with error indicators.

### Documentation Updates:

*   **README.md:**
    *   Added detailed examples and descriptions for new features like `--grep` and `--backtrace`.
    *   Improved shell completion instructions with escaped single quotes for compatibility.

### Code Refactoring:

*   **CLI Codebase:**
    *   Centralized default values for CLI arguments as constants for better maintainability.
    *   Refactored `cli.rs` to group related options and simplify imports.
    *   Updated `CliArgs` to use `Option<T>` for better handling of defaults and explicit CLI arguments.

### Bug Fixes:

*   **Configuration and Display:**
    *   Fixed an issue where clap default values for CLI arguments incorrectly overrode configuration profile settings.
    *   Resolved missing timestamp and status/level fields for `SpanStart` items in the timeline log.
*   **Dependency Updates:**
    *   Added `regex` crate to support the new `--grep` feature.

For a detailed list of all individual changes, bug fixes, and commits, please see the [CHANGELOG.md](https://github.com/dev7a/serverless-otlp-forwarder/blob/main/cli/livetrace/CHANGELOG.md).

