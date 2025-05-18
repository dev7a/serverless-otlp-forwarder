# Changelog for livetrace

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-05-14

### Added
- Added `--grep <REGEX>` option to filter spans/events based on attribute values matching a Rust-compatible regular expression. Matching text is highlighted in the console.
- Added `--backtrace <DURATION>` option to fetch logs from a specified duration ago for the initial poll in polling mode. Supports parsing durations in seconds (`s`) or minutes (`m`).
- Added `regex` dependency for implementing the `--grep` feature.
- Implemented shell completion for the `--theme` argument using `clap::ValueEnum` for a better user experience.
- Introduced "red-safe" alternatives in `TABLEAU_12`, `MATERIAL_12`, and `SOLARIZED_12` color palettes to prevent them from being mistaken for error indicators.
### Changed
- Updated `livetrace` version to `0.2.0` in `Cargo.toml`.
- Enhanced `README.md` to include examples and descriptions for the new `--grep` and `--backtrace` options.
- Updated shell completion instructions in `README.md` to escape single quotes for better compatibility.
- Refactored `cli.rs` to improve readability and maintainability, including grouping related options and simplifying imports.
- Centralized most CLI argument default values as constants in `cli.rs`.
- Modified `Theme` enum in `console_display.rs` to derive `serde::Serialize`, `serde::Deserialize` and use `#[serde(rename_all = "kebab-case")]` for consistent theme naming in configuration files.
- Updated `config.rs` to handle the new `--grep` and `--backtrace` options in both `ProfileConfig` and `EffectiveConfig`.
- Improved color selection for services and spans in `console_display.rs` by using FNV-1a hashing for a more even distribution of palette colors.
- Refactored the `console_display::display_console` function into several smaller, private helper functions (`prepare_trace_data_from_batch`, `calculate_layout_widths`, `print_trace_header`, `collect_and_filter_timeline_items_for_trace`, `build_waterfall_hierarchy_and_meta`, `render_waterfall_table`, `print_timeline_log`) to enhance modularity and readability.
- Updated the handling of CLI argument default values (e.g., for `theme`, `events_only`, `color_by`, and duration-based fields) to correctly prioritize explicit CLI arguments over profile settings, even when the CLI argument matches a `clap` default. This involved changing these fields to `Option<T>` in `CliArgs` and removing `default_value_t` attributes.

### Fixed
- Resolved a bug where default values provided by `clap` for CLI arguments (e.g., `--theme default`) would incorrectly override settings specified in a loaded configuration profile.
- Addressed a console display issue in `console_display.rs` where timestamp and status/level fields were sometimes missing for `SpanStart` items in the timeline log output.
- Corrected the display of the selected theme in the startup preamble in `lib.rs`.

### Removed
- Removed unused `monochrome` field from `config.rs` for cleaner configuration handling.

## [0.1.0] - 2025-05-10

### Added
- Shell completion script generation for Bash, Zsh, Fish, and other shells via `livetrace generate-completions` subcommand.
- Enhanced README with detailed usage examples, installation instructions, and a new section for shell completions.

### Changed
- Updated several dependencies, including `chrono`, `nix`, and `indicatif`.
- Improved code structure, module-level documentation, and refactored parts of the AWS setup for better maintainability.
- Updated project description and keywords in `Cargo.toml`.
- Updated copyright year in LICENSE to 2025.

## [Unreleased]

### Added
- Initial project setup.
