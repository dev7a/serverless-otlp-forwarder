//! Defines the command-line interface structure and argument parsing for `livetrace`.
//!
//! This module uses the `clap` crate to define the CLI arguments, their types,
//! help messages, and any validation rules (like mutually exclusive groups).
//! It also includes custom parsers for specific argument types (e.g., themes)
//! and helper functions related to CLI argument processing (e.g., parsing glob patterns).

use crate::console_display::Theme;
use clap::{crate_authors, crate_description, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// Made public for use in lib.rs
pub struct ThemeCliInfo {
    pub name: &'static str,
    pub description: &'static str,
}

// Made public for use in lib.rs
pub const AVAILABLE_THEMES_INFO: &[ThemeCliInfo] = &[
    ThemeCliInfo {
        name: "default",
        description: "OpenTelemetry-inspired blue-purple palette",
    },
    ThemeCliInfo {
        name: "tableau",
        description: "Tableau 12 color palette with distinct hues",
    },
    ThemeCliInfo {
        name: "colorbrewer",
        description: "ColorBrewer Set3 palette (pastel colors)",
    },
    ThemeCliInfo {
        name: "material",
        description: "Material Design palette with bright, modern colors",
    },
    ThemeCliInfo {
        name: "solarized",
        description: "Solarized color scheme with muted tones",
    },
    ThemeCliInfo {
        name: "monochrome",
        description: "Grayscale palette for minimal distraction",
    },
];

// Public constants for default CLI argument values
pub const DEFAULT_SESSION_TIMEOUT_MS: u64 = 30 * 60 * 1000; // 30m
pub const DEFAULT_TRACE_TIMEOUT_MS: u64 = 5 * 1000; // 5s
pub const DEFAULT_TRACE_STRAGGLERS_WAIT_MS: u64 = 0; // 0ms
pub const DEFAULT_EVENT_SEVERITY_ATTRIBUTE: &str = "event.severity";
pub const DEFAULT_EVENTS_ONLY: bool = true;
pub const DEFAULT_COLOR_BY: ColoringMode = ColoringMode::Span;

/// Defines coloring strategies for the console output
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize, Default)]
pub enum ColoringMode {
    /// Color by span ID (default)
    #[default]
    Span,
    /// Color by service name
    Service,
}

const USAGE_EXAMPLES: &str = "\
EXAMPLES:
    # Tail logs from a CloudFormation stack (Live Tail mode)
    livetrace --stack-name my-api-stack

    # Tail logs matching a pattern and forward to a local OTLP collector
    livetrace --log-group-pattern \"/aws/lambda/my-service-\" -e http://localhost:4318

    # Poll logs every 30 seconds from a stack, showing only events, with a specific theme
    livetrace --stack-name my-data-processing --poll-interval 30s --events-only --theme solarized

    # Discover log groups by pattern and save the configuration to a profile named \"dev\"
    livetrace --log-group-pattern \"/aws/lambda/user-service-\" --save-profile dev

    # Load configuration from the \"dev\" profile and override the OTLP endpoint
    livetrace --config-profile dev -e http://localhost:4319";

/// livetrace: Tail CloudWatch Logs for OTLP/stdout traces and forward them.
#[derive(Parser, Debug, Clone)]
#[command(author = crate_authors!(", "), version, about = crate_description!(), long_about = None, after_help = USAGE_EXAMPLES)]
// Removed the `mode_selector` group as it no longer serves its original purpose.
pub struct CliArgs {
    /// Log group name pattern(s) for discovery (case-sensitive substring search). Can be specified multiple times.
    #[arg(short = 'g', long = "log-group-pattern", num_args(1..))]
    pub log_group_pattern: Option<Vec<String>>,

    /// CloudFormation stack name for log group discovery.
    #[arg(short = 's', long = "stack-name")]
    pub stack_name: Option<String>,

    /// The OTLP HTTP endpoint URL to send traces to (e.g., http://localhost:4318/v1/traces).
    #[arg(short = 'e', long)]
    pub otlp_endpoint: Option<String>,

    /// Add custom HTTP headers to the outgoing OTLP request (e.g., "Authorization=Bearer token"). Can be specified multiple times.
    #[arg(short = 'H', long = "otlp-header")]
    pub otlp_headers: Vec<String>,

    /// AWS Region to use. Defaults to environment/profile configuration.
    #[arg(short = 'r', long = "aws-region")]
    pub aws_region: Option<String>,

    /// AWS Profile to use. Defaults to environment/profile configuration.
    #[arg(short = 'p', long = "aws-profile")]
    pub aws_profile: Option<String>,

    /// Increase logging verbosity (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Only forward telemetry, do not display it in the console.
    #[arg(long)]
    pub forward_only: bool,

    /// Comma-separated list of glob patterns for attribute filtering (e.g., "http.*,db.*,aws.lambda.*").
    /// Applies to both span attributes and event attributes.
    #[arg(long = "attrs")]
    pub attrs: Option<String>,

    /// Optional polling interval in seconds. If set, uses FilterLogEvents API instead of StartLiveTail.
    #[arg(long, group = "mode_selector", value_parser = parse_duration_to_millis, help = "Polling interval (e.g., '10s', '1m'). Requires suffix: ms, s, m, h.")]
    pub poll_interval: Option<u64>, // Stores milliseconds

    /// Overall session duration after which livetrace will automatically exit.
    /// Applies to both LiveTail and Polling modes.
    #[arg(long, value_parser = parse_duration_to_millis, help = "Overall session duration (e.g., '30m', '1h'). Requires suffix: ms, s, m, h. [default: 30m]")]
    pub session_timeout: Option<u64>, // Changed to Option<u64>, removed default_value

    /// Event attribute name to use for determining event severity level.
    #[arg(
        long,
        help = "Event attribute name for severity. [default: event.severity]"
    )]
    pub event_severity_attribute: Option<String>, // Changed to Option<String>, removed default_value

    /// Load configuration from a specific profile in .livetrace.toml.
    #[arg(long)]
    pub config_profile: Option<String>,

    /// Save the current effective command-line arguments to the specified profile in .livetrace.toml.
    #[arg(long, value_name = "PROFILE_NAME")]
    pub save_profile: Option<String>,

    /// Color theme for console output.
    /// Use --list-themes for all available options and their descriptions.
    #[arg(
        long,
        value_enum,
        value_name = "THEME",
        help_heading = "Display Options",
        help = "Color theme for console output. Use --list-themes for options. [default: default]"
    )]
    pub theme: Option<Theme>, // Changed to Option<Theme>

    /// List available color themes and exit.
    #[arg(
        long = "list-themes",
        help_heading = "Display Options",
        conflicts_with = "theme"
    )]
    pub list_themes: bool,

    /// Color output by service name or span ID
    #[arg(
        long = "color-by",
        value_enum,
        help_heading = "Display Options",
        help = "Color output by service name or span ID. [default: span]"
    )]
    pub color_by: Option<ColoringMode>, // Changed to Option<ColoringMode>

    /// Only display events, hiding span start information in the timeline log view.
    #[arg(
        long,
        value_name = "true|false", // Explicit values
        num_args = 0..=1, // Allows --events-only or --events-only=value
        default_missing_value = "true", // If --events-only is specified without a value, it's true
        help_heading = "Display Options",
        help = "Only display events, hiding span start information. [default: true]"
    )]
    pub events_only: Option<bool>, // Changed to Option<bool>

    /// Maximum time to wait for spans belonging to a trace before displaying/forwarding it.
    #[arg(long, value_parser = parse_duration_to_millis, help_heading = "Processing Options", help = "Max trace buffering time (e.g., '5s', '500ms'). Requires suffix: ms, s, m, h. [default: 5s]")]
    pub trace_timeout: Option<u64>, // Changed to Option<u64>, removed default_value

    /// Time to wait for late-arriving (straggler) spans after the last observed activity on a trace (if its root span has been received) before flushing.
    #[arg(long, value_parser = parse_duration_to_millis, help_heading = "Processing Options", help = "Time to wait for straggler spans after last trace activity (if root is present). (e.g., '500ms', '1s'). Requires suffix: ms, s, m, h. [default: 0ms]")]
    pub trace_stragglers_wait: Option<u64>, // Changed to Option<u64>, removed default_value

    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Filter spans/events by regex matching attribute values
    #[arg(long, help_heading = "Filtering Options")]
    pub grep: Option<String>,

    /// Go back in time for initial log poll (e.g., 30, 120s, 3m)
    #[arg(long, value_parser = parse_duration_to_millis, help_heading = "Filtering Options")]
    pub backtrace: Option<u64>, // Stores milliseconds
}

// Custom parser for duration strings into milliseconds
// Made pub(crate) so it can be called from config.rs
pub(crate) fn parse_duration_to_millis(s: &str) -> Result<u64, String> {
    let s_lower = s.to_lowercase();
    if let Some(stripped) = s_lower.strip_suffix("ms") {
        u64::from_str(stripped)
            .map_err(|e| format!("Invalid milliseconds value '{}': {}", stripped, e))
    } else if let Some(stripped) = s_lower.strip_suffix('s') {
        u64::from_str(stripped)
            .map(|n| n * 1000)
            .map_err(|e| format!("Invalid seconds value '{}': {}", stripped, e))
    } else if let Some(stripped) = s_lower.strip_suffix('m') {
        u64::from_str(stripped)
            .map(|n| n * 60 * 1000)
            .map_err(|e| format!("Invalid minutes value '{}': {}", stripped, e))
    } else if let Some(stripped) = s_lower.strip_suffix('h') {
        u64::from_str(stripped)
            .map(|n| n * 60 * 60 * 1000)
            .map_err(|e| format!("Invalid hours value '{}': {}", stripped, e))
    } else {
        Err(format!(
            "Invalid duration string '{}'. Must end with 'ms', 's', 'm', or 'h'. Bare numbers are not allowed.",
            s
        ))
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Generate shell completion script
    #[command(name = "generate-completions", hide = true)]
    GenerateCompletions {
        /// Shell for which to generate completions
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Parses attribute glob patterns from a string pattern.
/// This is a more general function that doesn't depend on CliArgs or EffectiveConfig directly.
pub fn parse_attr_globs(patterns_opt: &Option<String>) -> Option<GlobSet> {
    match patterns_opt.as_deref() {
        Some(patterns_str) if !patterns_str.is_empty() => {
            let mut builder = GlobSetBuilder::new();
            for pattern in patterns_str.split(',') {
                let trimmed_pattern = pattern.trim();
                if !trimmed_pattern.is_empty() {
                    match Glob::new(trimmed_pattern) {
                        Ok(glob) => {
                            builder.add(glob);
                        }
                        Err(e) => {
                            tracing::warn!(pattern = trimmed_pattern, error = %e, "Invalid glob pattern for attribute filtering, skipping.");
                        }
                    }
                }
            }
            match builder.build() {
                Ok(glob_set) => Some(glob_set),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to build glob set for attributes");
                    None // Treat as no filter if build fails
                }
            }
        }
        _ => None, // No patterns provided
    }
}
