use crate::console_display::Theme;
use clap::{builder::TypedValueParser, error::ErrorKind, ArgGroup, Parser};
use globset::{Glob, GlobSet, GlobSetBuilder};

/// livetrace: Tail CloudWatch Logs for OTLP/stdout traces and forward them.
#[derive(Parser, Debug, Clone)] // Added Clone
#[command(author = "Dev7A", version, about, long_about = None)]
#[clap(group(
    ArgGroup::new("discovery")
        .required(false) // Changed from true to false to allow loading from config
        .args(["log_group_pattern", "stack_name"]),
))]
#[clap(group( // Add group to make poll/timeout mutually exclusive
    ArgGroup::new("mode")
        .required(false) // One or neither can be specified
        .args(["poll_interval", "session_timeout"]),
))]
pub struct CliArgs {
    /// Log group name pattern for discovery (case-sensitive substring search).
    #[arg(long = "pattern", group = "discovery")]
    pub log_group_pattern: Option<String>,

    /// CloudFormation stack name for log group discovery.
    #[arg(long = "stack-name", group = "discovery")]
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

    /// Width of the timeline bar in characters.
    #[arg(long, default_value_t = 80)]
    pub timeline_width: usize,

    /// Use a compact display format (omits Span ID).
    #[arg(long)]
    pub compact_display: bool,

    /// Comma-separated list of glob patterns for event attributes to display (e.g., "http.*,db.*,aws.lambda.*").
    #[arg(long)]
    pub event_attrs: Option<String>,

    /// Optional polling interval in seconds. If set, uses FilterLogEvents API instead of StartLiveTail.
    #[arg(long, group = "mode")] // Add to group
    pub poll_interval: Option<u64>,

    /// Session duration in minutes after which livetrace will automatically exit (LiveTail mode only).
    #[arg(long, default_value_t = 30, group = "mode")] // Re-add, add to group
    pub session_timeout: u64,

    /// Event attribute name to use for determining event severity level.
    #[arg(long, default_value = "event.severity")]
    pub event_severity_attribute: String,

    /// Load configuration from a specific profile in livetrace.toml.
    #[arg(long)]
    pub config_profile: Option<String>,

    /// Save the current effective command-line arguments to the specified profile in livetrace.toml and exit.
    #[arg(long, value_name = "PROFILE_NAME")]
    pub save_profile: Option<String>,

    /// Color theme for console output (default, tableau, colorbrewer, material, solarized, monochrome).
    #[arg(long, default_value = "default", value_parser = ThemeValueParser)]
    pub theme: String,
}

// Create a custom value parser for themes
#[derive(Clone)]
struct ThemeValueParser;

impl TypedValueParser for ThemeValueParser {
    type Value = String;

    /// This method is called by Clap when validating the theme argument.
    /// It receives the command, argument definition, and user-provided value.
    /// We only use the value parameter to validate the theme name.
    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        // Convert OsStr to a regular string for validation
        let theme_str = value.to_string_lossy().to_string();

        // Check if the theme is valid
        if !Theme::is_valid_theme(&theme_str) {
            // Create a helpful error message with valid themes
            let valid_themes = [
                "default",
                "tableau",
                "colorbrewer",
                "material",
                "solarized",
                "monochrome",
            ]
            .join(", ");

            let err = format!(
                "Invalid theme '{}'. Valid themes are: {}",
                theme_str, valid_themes
            );

            return Err(clap::Error::raw(ErrorKind::InvalidValue, err));
        }

        // Valid theme, return it
        Ok(theme_str)
    }
}

/// Parses event attribute glob patterns from a string pattern.
/// This is a more general function that doesn't depend on CliArgs or EffectiveConfig directly.
pub fn parse_event_attr_globs(patterns_opt: &Option<String>) -> Option<GlobSet> {
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
                            tracing::warn!(pattern = trimmed_pattern, error = %e, "Invalid glob pattern for event attribute filtering, skipping.");
                        }
                    }
                }
            }
            match builder.build() {
                Ok(glob_set) => Some(glob_set),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to build glob set for event attributes");
                    None // Treat as no filter if build fails
                }
            }
        }
        _ => None, // No patterns provided
    }
}
