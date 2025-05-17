//! Manages configuration loading, parsing, and merging for `livetrace`.
//!
//! This module defines structures for representing the configuration file (`.livetrace.toml`),
//! individual profiles within that file, and the final `EffectiveConfig` that results
//! from merging CLI arguments, environment variables (handled in `lib.rs`/`main.rs` for OTLP),
//! and profile settings.
//!
//! Key functionalities include:
//! - Loading the `.livetrace.toml` file.
//! - Parsing TOML into Rust structs.
//! - Applying a precedence order: CLI arguments > Profile settings > Global settings.
//! - Saving CLI arguments to a named profile in the configuration file.

use crate::cli::{
    CliArgs, ColoringMode, DEFAULT_COLOR_BY, DEFAULT_EVENTS_ONLY, DEFAULT_EVENT_SEVERITY_ATTRIBUTE,
    DEFAULT_SESSION_TIMEOUT_MS, DEFAULT_TRACE_STRAGGLERS_WAIT_MS, DEFAULT_TRACE_TIMEOUT_MS,
};
use crate::console_display::Theme;
use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path, path::PathBuf};

// Default filename for the configuration
const LIVETRACE_TOML: &str = ".livetrace.toml";

// Helper function to format milliseconds into a human-readable duration string.
// Prefers largest whole unit (h, m, s, then ms). Does not produce decimals.
// Made pub(crate) to be accessible from lib.rs for preamble display.
pub(crate) fn format_millis_to_duration_string(millis: u64) -> String {
    if millis == 0 {
        return "0ms".to_string(); // Or "0s", depending on preference for zero.
    }
    if millis % (60 * 60 * 1000) == 0 {
        format!("{}h", millis / (60 * 60 * 1000))
    } else if millis % (60 * 1000) == 0 {
        format!("{}m", millis / (60 * 1000))
    } else if millis % 1000 == 0 {
        format!("{}s", millis / 1000)
    } else {
        format!("{}ms", millis)
    }
}

/// Represents the entire structure of the livetrace.toml file.
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    #[serde(default)]
    pub version: f32,

    #[serde(default)]
    pub global: Option<ProfileConfig>,

    #[serde(default)]
    pub profiles: IndexMap<String, ProfileConfig>,
}

/// Represents the configuration settings within a profile (or global section).
/// Fields should generally mirror CliArgs, using `Option<T>` and serde attributes.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct ProfileConfig {
    // Discovery (Mirroring CliArgs groups)
    #[serde(rename = "log-group-pattern")]
    pub log_group_pattern: Option<Vec<String>>,
    #[serde(rename = "stack-name")]
    pub stack_name: Option<String>,

    // Forwarding (Mirroring CliArgs)
    #[serde(rename = "otlp-endpoint")]
    pub otlp_endpoint: Option<String>,
    #[serde(rename = "otlp-header")]
    pub otlp_headers: Option<Vec<String>>,

    // AWS (Mirroring CliArgs)
    #[serde(rename = "aws-region")]
    pub aws_region: Option<String>,
    #[serde(rename = "aws-profile")]
    pub aws_profile: Option<String>,

    // Console Display (Mirroring CliArgs)
    #[serde(rename = "forward-only")]
    pub forward_only: Option<bool>,
    #[serde(rename = "attrs")]
    pub attrs: Option<String>,
    #[serde(rename = "event-severity-attribute")]
    pub event_severity_attribute: Option<String>,
    #[serde(rename = "theme")]
    pub theme: Option<Theme>,
    #[serde(rename = "color-by", skip_serializing_if = "Option::is_none")]
    pub color_by: Option<ColoringMode>,

    // Mode (Mirroring CliArgs groups)
    #[serde(rename = "poll-interval")]
    pub poll_interval: Option<String>, // Changed to Option<String>
    #[serde(rename = "session-timeout")]
    pub session_timeout: Option<String>, // Changed to Option<String>
    // Note: Verbosity (`verbose`) is generally not configured via file.
    #[serde(rename = "events-only", skip_serializing_if = "Option::is_none")]
    pub events_only: Option<bool>,

    #[serde(rename = "trace-timeout", skip_serializing_if = "Option::is_none")]
    pub trace_timeout: Option<String>, // Changed to Option<String>
    #[serde(
        rename = "trace-stragglers-wait",
        skip_serializing_if = "Option::is_none"
    )]
    pub trace_stragglers_wait: Option<String>, // New field

    // Filtering Options
    #[serde(rename = "grep", skip_serializing_if = "Option::is_none")]
    pub grep: Option<String>,
    #[serde(rename = "backtrace", skip_serializing_if = "Option::is_none")]
    pub backtrace: Option<String>, // Changed to Option<String>
}

/// Represents the final, merged configuration after applying precedence rules.
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    // Discovery
    pub log_group_pattern: Option<Vec<String>>,
    pub stack_name: Option<String>,

    // Forwarding
    pub otlp_endpoint: Option<String>,
    pub otlp_headers: Vec<String>, // Merged headers

    // AWS
    pub aws_region: Option<String>,
    pub aws_profile: Option<String>,

    // Console Display
    pub forward_only: bool,
    pub attrs: Option<String>,
    pub event_severity_attribute: String,
    pub color_by: ColoringMode,
    pub events_only: bool,
    pub trace_timeout_ms: u64,
    pub trace_stragglers_wait_ms: u64,

    // Mode
    pub poll_interval_ms: Option<u64>,
    pub session_timeout_ms: u64,

    // Execution Control
    pub verbose: u8, // Keep verbosity
    pub theme: Theme,

    // Filtering Options
    pub grep: Option<String>,
    pub backtrace_ms: Option<u64>,
}

impl ProfileConfig {
    /// Creates a ProfileConfig from CliArgs, only including non-default values.
    pub fn from_cli_args(args: &CliArgs) -> Self {
        // Default values from CliArgs definitions, converted to milliseconds
        const DEFAULT_SESSION_TIMEOUT_STR: &str = "30m";
        const DEFAULT_TRACE_TIMEOUT_STR: &str = "5s";
        const DEFAULT_TRACE_STRAGGLERS_WAIT_STR: &str = "0ms";

        ProfileConfig {
            log_group_pattern: args.log_group_pattern.clone().filter(|v| !v.is_empty()),
            stack_name: args.stack_name.clone(),
            otlp_endpoint: args.otlp_endpoint.clone(),
            otlp_headers: if args.otlp_headers.is_empty() {
                None
            } else {
                Some(args.otlp_headers.clone())
            },
            aws_region: args.aws_region.clone(),
            aws_profile: args.aws_profile.clone(),
            forward_only: Some(args.forward_only).filter(|&f| f),
            attrs: args.attrs.clone(),
            poll_interval: args.poll_interval.map(format_millis_to_duration_string),
            session_timeout: args
                .session_timeout
                .map(format_millis_to_duration_string)
                .filter(|s| s != DEFAULT_SESSION_TIMEOUT_STR),
            event_severity_attribute: args
                .event_severity_attribute
                .clone()
                .filter(|s| s != DEFAULT_EVENT_SEVERITY_ATTRIBUTE),
            theme: args.theme.filter(|&t| t != Theme::Default),
            color_by: args.color_by.filter(|&c| c != DEFAULT_COLOR_BY),
            events_only: args.events_only.filter(|&e| e != DEFAULT_EVENTS_ONLY),
            trace_timeout: args
                .trace_timeout
                .map(format_millis_to_duration_string)
                .filter(|s| s != DEFAULT_TRACE_TIMEOUT_STR),
            trace_stragglers_wait: args
                .trace_stragglers_wait
                .map(format_millis_to_duration_string)
                .filter(|s| s != DEFAULT_TRACE_STRAGGLERS_WAIT_STR),
            grep: args.grep.clone(),
            backtrace: args.backtrace.map(format_millis_to_duration_string),
        }
    }
}

pub fn load_and_resolve_config(
    config_profile_name: Option<String>,
    cli_args: &CliArgs,
) -> Result<EffectiveConfig> {
    // Default values are now primarily from cli::constants or direct enum variants
    // const DEFAULT_SESSION_TIMEOUT_MS: u64 = 30 * 60 * 1000; // No longer needed here
    // const DEFAULT_TRACE_TIMEOUT_MS: u64 = 5 * 1000; // No longer needed here
    // const DEFAULT_TRACE_STRAGGLERS_WAIT_MS: u64 = 0; // No longer needed here

    let mut effective = EffectiveConfig {
        log_group_pattern: None,
        stack_name: None,
        otlp_endpoint: None,
        otlp_headers: Vec::new(),
        aws_region: None,
        aws_profile: None,
        forward_only: false,
        attrs: None,
        event_severity_attribute: DEFAULT_EVENT_SEVERITY_ATTRIBUTE.to_string(),
        poll_interval_ms: None,
        session_timeout_ms: DEFAULT_SESSION_TIMEOUT_MS,
        verbose: 0,
        theme: Theme::Default,
        color_by: DEFAULT_COLOR_BY,
        events_only: DEFAULT_EVENTS_ONLY,
        trace_timeout_ms: DEFAULT_TRACE_TIMEOUT_MS,
        trace_stragglers_wait_ms: DEFAULT_TRACE_STRAGGLERS_WAIT_MS,
        grep: None,
        backtrace_ms: None,
    };

    if config_profile_name.is_none() {
        // If no profile, apply CLI args (which might be None or Some)
        apply_cli_args_to_effective(cli_args, &mut effective);
        return Ok(effective);
    }

    let config_path = get_config_path()?;
    if !config_path.exists() {
        tracing::warn!(
            path = %config_path.display(),
            "Config file not found while trying to load profile. Using CLI arguments only."
        );
        apply_cli_args_to_effective(cli_args, &mut effective);
        return Ok(effective);
    }

    let config_file = load_config_file(&config_path)?;

    if let Some(global_config) = &config_file.global {
        apply_profile_to_effective(global_config, &mut effective);
    }

    let profile_name = config_profile_name.unwrap();
    if let Some(profile_config) = config_file.profiles.get(&profile_name) {
        apply_profile_to_effective(profile_config, &mut effective);
        tracing::info!(profile = %profile_name, "Loaded configuration from profile");
    } else {
        return Err(anyhow::anyhow!(
            "Configuration profile '{}' not found in config file '{}'",
            profile_name,
            config_path.display()
        ));
    }

    apply_cli_args_to_effective(cli_args, &mut effective);
    Ok(effective)
}

fn apply_cli_args_to_effective(cli_args: &CliArgs, effective: &mut EffectiveConfig) {
    // Default values from CliArgs definitions, converted to milliseconds, for comparison
    // These constants are now only for reference if needed, actual default application is changing.
    // const DEFAULT_SESSION_TIMEOUT_MS: u64 = 30 * 60 * 1000; // 30m
    // const DEFAULT_TRACE_TIMEOUT_MS: u64 = 5 * 1000; // 5s
    // const DEFAULT_TRACE_STRAGGLERS_WAIT_MS: u64 = 0; // 0ms

    if cli_args
        .log_group_pattern
        .as_ref()
        .is_some_and(|v| !v.is_empty())
    {
        effective.log_group_pattern = cli_args.log_group_pattern.clone();
    }
    if cli_args.stack_name.is_some() {
        effective.stack_name = cli_args.stack_name.clone();
    }
    if cli_args.otlp_endpoint.is_some() {
        effective.otlp_endpoint = cli_args.otlp_endpoint.clone();
    }
    if !cli_args.otlp_headers.is_empty() {
        effective.otlp_headers = cli_args.otlp_headers.clone();
    }
    if cli_args.aws_region.is_some() {
        effective.aws_region = cli_args.aws_region.clone();
    }
    if cli_args.aws_profile.is_some() {
        effective.aws_profile = cli_args.aws_profile.clone();
    }
    if cli_args.attrs.is_some() {
        effective.attrs = cli_args.attrs.clone();
    }
    if cli_args.grep.is_some() {
        effective.grep = cli_args.grep.clone();
    }
    if cli_args.backtrace.is_some() {
        effective.backtrace_ms = cli_args.backtrace;
    }
    if cli_args.poll_interval.is_some() {
        effective.poll_interval_ms = cli_args.poll_interval;
    }
    if cli_args.forward_only {
        effective.forward_only = true;
    }
    if let Some(val) = &cli_args.event_severity_attribute {
        effective.event_severity_attribute = val.clone();
    }
    if let Some(val) = cli_args.session_timeout {
        effective.session_timeout_ms = val;
    }
    if let Some(val) = cli_args.color_by {
        effective.color_by = val;
    }
    if let Some(val) = cli_args.events_only {
        effective.events_only = val;
    }
    if let Some(val) = cli_args.trace_timeout {
        effective.trace_timeout_ms = val;
    }
    if let Some(val) = cli_args.trace_stragglers_wait {
        effective.trace_stragglers_wait_ms = val;
    }
    if let Some(val) = cli_args.theme {
        effective.theme = val;
    }
    effective.verbose = cli_args.verbose;
}

pub fn get_config_path() -> Result<PathBuf> {
    Ok(PathBuf::from(LIVETRACE_TOML))
}

pub fn load_or_default_config_file() -> Result<ConfigFile> {
    let config_path = get_config_path()?;
    if config_path.exists() {
        load_config_file(&config_path)
    } else {
        Ok(ConfigFile::default())
    }
}

pub fn save_profile_config(profile_name: &str, profile_data: &ProfileConfig) -> Result<()> {
    let config_path = get_config_path()?;
    let mut config = load_or_default_config_file()?;
    config
        .profiles
        .insert(profile_name.to_string(), profile_data.clone());
    let toml_string =
        toml::to_string_pretty(&config).context("Failed to serialize configuration to TOML")?;
    let mut file = fs::File::create(&config_path).with_context(|| {
        format!(
            "Failed to create or open config file for writing: {}",
            config_path.display()
        )
    })?;
    file.write_all(toml_string.as_bytes())
        .with_context(|| format!("Failed to write to config file: {}", config_path.display()))?;
    Ok(())
}

fn load_config_file(path: &Path) -> Result<ConfigFile> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config: ConfigFile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML from config file: {}", path.display()))?;
    Ok(config)
}

fn apply_profile_to_effective(profile: &ProfileConfig, effective: &mut EffectiveConfig) {
    if profile
        .log_group_pattern
        .as_ref()
        .is_some_and(|v| !v.is_empty())
    {
        effective.log_group_pattern = profile.log_group_pattern.clone();
    }
    if let Some(val) = &profile.stack_name {
        effective.stack_name = Some(val.clone());
    }
    if let Some(val) = &profile.otlp_endpoint {
        effective.otlp_endpoint = Some(val.clone());
    }
    if let Some(val) = &profile.otlp_headers {
        effective.otlp_headers = val.clone();
    }
    if let Some(val) = &profile.aws_region {
        effective.aws_region = Some(val.clone());
    }
    if let Some(val) = &profile.aws_profile {
        effective.aws_profile = Some(val.clone());
    }
    if let Some(val) = profile.forward_only {
        effective.forward_only = val;
    }
    if let Some(val) = &profile.attrs {
        effective.attrs = Some(val.clone());
    }
    if let Some(val) = &profile.event_severity_attribute {
        effective.event_severity_attribute = val.clone();
    }

    if let Some(s_val) = &profile.poll_interval {
        match crate::cli::parse_duration_to_millis(s_val.as_str()) {
            Ok(ms_val) => effective.poll_interval_ms = Some(ms_val),
            Err(e) => tracing::warn!(
                profile_key = "poll-interval", value = %s_val, error = %e,
                "Failed to parse duration from profile for poll-interval. Effective value: {}", effective.poll_interval_ms.map_or_else(|| "None".to_string(), format_millis_to_duration_string)
            ),
        }
    }
    if let Some(s_val) = &profile.session_timeout {
        match crate::cli::parse_duration_to_millis(s_val.as_str()) {
            Ok(ms_val) => effective.session_timeout_ms = ms_val,
            Err(e) => tracing::warn!(
                profile_key = "session-timeout", value = %s_val, error = %e,
                "Failed to parse duration from profile for session-timeout. Effective value: {}", format_millis_to_duration_string(effective.session_timeout_ms)
            ),
        }
    }

    if let Some(val) = &profile.theme {
        effective.theme = *val;
    }
    if let Some(val) = &profile.color_by {
        effective.color_by = *val;
    }
    if let Some(val) = profile.events_only {
        effective.events_only = val;
    }

    if let Some(s_val) = &profile.trace_timeout {
        match crate::cli::parse_duration_to_millis(s_val.as_str()) {
            Ok(ms_val) => effective.trace_timeout_ms = ms_val,
            Err(e) => tracing::warn!(
                profile_key = "trace-timeout", value = %s_val, error = %e,
                "Failed to parse duration from profile for trace-timeout. Effective value: {}", format_millis_to_duration_string(effective.trace_timeout_ms)
            ),
        }
    }

    if let Some(s_val) = &profile.trace_stragglers_wait {
        match crate::cli::parse_duration_to_millis(s_val.as_str()) {
            Ok(ms_val) => effective.trace_stragglers_wait_ms = ms_val,
            Err(e) => tracing::warn!(
                profile_key = "trace-stragglers-wait", value = %s_val, error = %e,
                "Failed to parse duration from profile for trace-stragglers-wait. Effective value: {}", format_millis_to_duration_string(effective.trace_stragglers_wait_ms)
            ),
        }
    }

    if let Some(val) = &profile.grep {
        effective.grep = Some(val.clone());
    }
    if let Some(s_val) = &profile.backtrace {
        match crate::cli::parse_duration_to_millis(s_val.as_str()) {
            Ok(ms_val) => effective.backtrace_ms = Some(ms_val),
            Err(e) => tracing::warn!(
                profile_key = "backtrace", value = %s_val, error = %e,
                "Failed to parse duration from profile for backtrace. Effective value: {}", effective.backtrace_ms.map_or_else(|| "None".to_string(), format_millis_to_duration_string)
            ),
        }
    }
}

pub fn merge_into_profile_config(base: &ProfileConfig, overrides: &ProfileConfig) -> ProfileConfig {
    ProfileConfig {
        log_group_pattern: overrides
            .log_group_pattern
            .clone()
            .or_else(|| base.log_group_pattern.clone()),
        stack_name: overrides
            .stack_name
            .clone()
            .or_else(|| base.stack_name.clone()),
        otlp_endpoint: overrides
            .otlp_endpoint
            .clone()
            .or_else(|| base.otlp_endpoint.clone()),
        otlp_headers: overrides
            .otlp_headers
            .clone()
            .or_else(|| base.otlp_headers.clone()),
        aws_region: overrides
            .aws_region
            .clone()
            .or_else(|| base.aws_region.clone()),
        aws_profile: overrides
            .aws_profile
            .clone()
            .or_else(|| base.aws_profile.clone()),
        forward_only: overrides.forward_only.or(base.forward_only),
        attrs: overrides.attrs.clone().or_else(|| base.attrs.clone()),
        event_severity_attribute: overrides
            .event_severity_attribute
            .clone()
            .or_else(|| base.event_severity_attribute.clone()),
        poll_interval: overrides
            .poll_interval
            .clone()
            .or_else(|| base.poll_interval.clone()),
        session_timeout: overrides
            .session_timeout
            .clone()
            .or_else(|| base.session_timeout.clone()),
        theme: overrides.theme.or(base.theme),
        color_by: overrides.color_by.or(base.color_by),
        events_only: overrides.events_only.or(base.events_only),
        trace_timeout: overrides
            .trace_timeout
            .clone()
            .or_else(|| base.trace_timeout.clone()),
        trace_stragglers_wait: overrides
            .trace_stragglers_wait
            .clone()
            .or_else(|| base.trace_stragglers_wait.clone()),
        grep: overrides.grep.clone().or_else(|| base.grep.clone()),
        backtrace: overrides
            .backtrace
            .clone()
            .or_else(|| base.backtrace.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // Helper to create a mock CliArgs for testing
    fn mock_cli_args() -> CliArgs {
        CliArgs {
            log_group_pattern: Some(vec![
                "test-pattern-1".to_string(),
                "test-pattern-2".to_string(),
            ]),
            stack_name: None,
            otlp_endpoint: Some("http://localhost:4318".to_string()),
            otlp_headers: vec!["Auth=Bearer xyz".to_string()],
            aws_region: Some("us-west-2".to_string()),
            aws_profile: Some("test-profile".to_string()),
            verbose: 1,
            forward_only: true,
            attrs: Some("http.*,db.*".to_string()),
            poll_interval: Some(30 * 1000),        // 30s in ms
            session_timeout: Some(45 * 60 * 1000), // 45m in ms
            event_severity_attribute: Some("custom.severity".to_string()),
            config_profile: None,
            save_profile: None,
            theme: Some(Theme::Solarized),
            list_themes: false,
            color_by: Some(ColoringMode::Service),
            events_only: Some(false),
            trace_timeout: Some(10 * 1000),   // 10s in ms
            trace_stragglers_wait: Some(500), // 500ms
            command: None,
            grep: None,
            backtrace: Some(60 * 1000), // 60s or 1m in ms
        }
    }

    // Helper to create a temporary config file
    fn create_test_config_file(dir: &std::path::Path) -> PathBuf {
        let config_path = dir.join(LIVETRACE_TOML);
        let config_content = r#"
version = 1.0

[global]
aws-region = "us-east-1"
event-severity-attribute = "global.severity"
trace-timeout = "7s" # Example global duration as string

[profiles.default]
log-group-pattern = ["default-pattern-a", "default-pattern-b"]
forward-only = true
session-timeout = "1h" # Example profile duration as string

[profiles.dev]
log-group-pattern = ["/aws/lambda/dev-func", "specific-dev-group"]
otlp-endpoint = "http://dev-collector:4318"
aws-region = "us-west-1"
poll-interval = "20s"  # Example profile duration as string
backtrace = "5m"       # Example profile duration as string
        "#;
        fs::write(&config_path, config_content).expect("Failed to write test config");
        config_path
    }

    #[test]
    fn test_from_cli_args() {
        let args = mock_cli_args();
        let profile = ProfileConfig::from_cli_args(&args);

        assert_eq!(
            profile.log_group_pattern,
            Some(vec![
                "test-pattern-1".to_string(),
                "test-pattern-2".to_string()
            ])
        );
        assert_eq!(profile.stack_name, None);
        assert_eq!(
            profile.otlp_endpoint,
            Some("http://localhost:4318".to_string())
        );
        assert_eq!(
            profile.otlp_headers,
            Some(vec!["Auth=Bearer xyz".to_string()])
        );
        assert_eq!(profile.aws_region, Some("us-west-2".to_string()));
        assert_eq!(profile.aws_profile, Some("test-profile".to_string()));
        assert_eq!(profile.forward_only, Some(true));
        assert_eq!(profile.attrs, Some("http.*,db.*".to_string()));
        assert_eq!(profile.poll_interval, Some("30s".to_string()));
        assert_eq!(profile.session_timeout, Some("45m".to_string()));
        assert_eq!(
            profile.event_severity_attribute,
            Some("custom.severity".to_string())
        );
        assert_eq!(profile.theme, Some(Theme::Solarized));
        assert_eq!(profile.color_by, Some(ColoringMode::Service));
        assert_eq!(profile.events_only, Some(false));
        assert_eq!(profile.trace_timeout, Some("10s".to_string()));
        assert_eq!(profile.trace_stragglers_wait, Some("500ms".to_string()));
        assert_eq!(profile.backtrace, Some("1m".to_string()));

        // Test case where session_timeout and trace_timeout are default
        let mut args_with_defaults = mock_cli_args();
        args_with_defaults.session_timeout = Some(DEFAULT_SESSION_TIMEOUT_MS); // Use const
        args_with_defaults.trace_timeout = Some(DEFAULT_TRACE_TIMEOUT_MS); // Use const
        args_with_defaults.trace_stragglers_wait = Some(DEFAULT_TRACE_STRAGGLERS_WAIT_MS); // Use const
        args_with_defaults.theme = Some(Theme::Default); // Default theme
        args_with_defaults.color_by = Some(DEFAULT_COLOR_BY); // Use const for default color_by
        args_with_defaults.event_severity_attribute =
            Some(DEFAULT_EVENT_SEVERITY_ATTRIBUTE.to_string()); // Use const
        args_with_defaults.events_only = Some(DEFAULT_EVENTS_ONLY); // Use const

        let profile_with_defaults = ProfileConfig::from_cli_args(&args_with_defaults);
        assert_eq!(profile_with_defaults.session_timeout, None);
        assert_eq!(profile_with_defaults.trace_timeout, None);
        assert_eq!(profile_with_defaults.trace_stragglers_wait, None);
        assert_eq!(profile_with_defaults.theme, None);
        assert_eq!(profile_with_defaults.color_by, None); // Should now pass
        assert_eq!(profile_with_defaults.event_severity_attribute, None);
        assert_eq!(profile_with_defaults.events_only, None); // Added assertion for events_only
    }

    #[test]
    fn test_save_profile_config() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path().to_path_buf().join(LIVETRACE_TOML);
        let args = mock_cli_args();
        let profile = ProfileConfig::from_cli_args(&args);
        let mut config = ConfigFile::default();
        config.profiles.insert("test-profile".to_string(), profile);
        let toml_string = toml::to_string_pretty(&config).expect("Failed to serialize");
        fs::write(&temp_path, toml_string).expect("Failed to write test config");
        let read_config: ConfigFile =
            toml::from_str(&fs::read_to_string(&temp_path).expect("Failed to read test config"))
                .expect("Failed to parse test config");
        assert!(read_config.profiles.contains_key("test-profile"));
        let saved_profile = &read_config.profiles["test-profile"];
        assert_eq!(
            saved_profile.log_group_pattern,
            Some(vec![
                "test-pattern-1".to_string(),
                "test-pattern-2".to_string()
            ])
        );
        assert_eq!(
            saved_profile.otlp_endpoint,
            Some("http://localhost:4318".to_string())
        );
    }

    #[test]
    fn test_apply_profile_to_effective() {
        // Default values in milliseconds for comparison
        const DEFAULT_SESSION_TIMEOUT_MS: u64 = 30 * 60 * 1000;
        const DEFAULT_TRACE_TIMEOUT_MS: u64 = 5 * 1000;
        const DEFAULT_TRACE_STRAGGLERS_WAIT_MS: u64 = 0; // 0ms

        let mut effective = EffectiveConfig {
            log_group_pattern: Some(vec!["initial-pattern".to_string()]),
            stack_name: Some("original-stack".to_string()),
            otlp_endpoint: None,
            otlp_headers: Vec::new(),
            aws_region: Some("us-east-1".to_string()),
            aws_profile: None,
            forward_only: false,
            attrs: None,
            event_severity_attribute: "default.severity".to_string(),
            poll_interval_ms: None,
            session_timeout_ms: DEFAULT_SESSION_TIMEOUT_MS, // Default in ms
            verbose: 0,
            theme: Theme::Default,
            color_by: ColoringMode::Service,
            events_only: false,
            trace_timeout_ms: DEFAULT_TRACE_TIMEOUT_MS, // Default in ms
            trace_stragglers_wait_ms: DEFAULT_TRACE_STRAGGLERS_WAIT_MS, // Default in ms
            grep: None,
            backtrace_ms: None,
        };
        let profile = ProfileConfig {
            // Durations as Option<String>
            log_group_pattern: Some(vec![
                "profile-pattern-1".to_string(),
                "profile-pattern-2".to_string(),
            ]),
            stack_name: None, // Will keep effective.stack_name
            otlp_endpoint: Some("http://profile-endpoint:4318".to_string()),
            otlp_headers: Some(vec!["Profile-Auth=token123".to_string()]),
            aws_region: None, // Will keep effective.aws_region
            aws_profile: Some("profile-aws-profile".to_string()),
            forward_only: Some(true),
            attrs: Some("profile.*".to_string()),
            event_severity_attribute: Some("profile.severity".to_string()),
            poll_interval: Some("45s".to_string()), // String duration
            session_timeout: Some("1h".to_string()), // String duration, different from effective default
            theme: Some(Theme::Solarized),
            color_by: None, // Will keep effective.color_by
            events_only: Some(true),
            trace_timeout: Some("10000ms".to_string()), // String duration (10s), different from effective default
            trace_stragglers_wait: Some("2s".to_string()), // String duration (2s)
            grep: Some("test-grep".to_string()),
            backtrace: Some("60s".to_string()), // String duration
        };
        apply_profile_to_effective(&profile, &mut effective);
        assert_eq!(
            effective.log_group_pattern,
            Some(vec![
                "profile-pattern-1".to_string(),
                "profile-pattern-2".to_string()
            ])
        );
        assert_eq!(effective.stack_name, Some("original-stack".to_string())); // Unchanged by profile
        assert_eq!(
            effective.otlp_endpoint,
            Some("http://profile-endpoint:4318".to_string())
        );
        assert_eq!(effective.otlp_headers, vec!["Profile-Auth=token123"]);
        assert_eq!(effective.aws_region, Some("us-east-1".to_string())); // Unchanged by profile
        assert_eq!(
            effective.aws_profile,
            Some("profile-aws-profile".to_string())
        );
        assert!(effective.forward_only);
        assert_eq!(effective.attrs, Some("profile.*".to_string()));
        assert_eq!(effective.event_severity_attribute, "profile.severity");
        assert_eq!(effective.poll_interval_ms, Some(45 * 1000)); // Check for ms
        assert_eq!(effective.session_timeout_ms, 60 * 60 * 1000); // Check for ms (1h)
        assert_eq!(effective.theme, Theme::Solarized);
        assert_eq!(effective.color_by, ColoringMode::Service); // Unchanged by profile
        assert!(effective.events_only);
        assert_eq!(effective.trace_timeout_ms, 10_000); // Check for ms (10s)
        assert_eq!(effective.trace_stragglers_wait_ms, 2_000); // Check for ms (2s)
        assert_eq!(effective.grep, Some("test-grep".to_string()));
        assert_eq!(effective.backtrace_ms, Some(60 * 1000)); // Check for ms
    }

    #[test]
    fn test_load_and_resolve_config() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let config_path = create_test_config_file(temp_dir.path()); // Writes to temp_dir/.livetrace.toml

        let cli_args_mock = mock_cli_args(); // CliArgs with durations in u64 ms

        // Manually replicate the core logic of load_and_resolve_config, but using our specific config_path
        const DEFAULT_EFFECTIVE_SESSION_TIMEOUT_MS: u64 = 30 * 60 * 1000;
        const DEFAULT_EFFECTIVE_TRACE_TIMEOUT_MS: u64 = 5 * 1000;
        const DEFAULT_EFFECTIVE_TRACE_STRAGGLERS_WAIT_MS: u64 = 0; // 0ms

        let mut effective = EffectiveConfig {
            log_group_pattern: None,
            stack_name: None,
            otlp_endpoint: None,
            otlp_headers: Vec::new(),
            aws_region: None,
            aws_profile: None,
            forward_only: false,
            attrs: None,
            event_severity_attribute: "event.severity".to_string(),
            poll_interval_ms: None,
            session_timeout_ms: DEFAULT_EFFECTIVE_SESSION_TIMEOUT_MS,
            verbose: 0,
            theme: Theme::Default,
            color_by: ColoringMode::Service,
            events_only: false,
            trace_timeout_ms: DEFAULT_EFFECTIVE_TRACE_TIMEOUT_MS,
            trace_stragglers_wait_ms: DEFAULT_EFFECTIVE_TRACE_STRAGGLERS_WAIT_MS, // Initialize new field
            grep: None,
            backtrace_ms: None,
        };

        // Load the specific test config file
        let config_file =
            load_config_file(&config_path).expect("Test: Failed to load test config file");

        // Apply global config from the test file
        if let Some(global_config) = &config_file.global {
            apply_profile_to_effective(global_config, &mut effective);
        }

        // Apply 'dev' profile from the test file
        if let Some(profile_config) = config_file.profiles.get("dev") {
            apply_profile_to_effective(profile_config, &mut effective);
        } else {
            panic!("Test: 'dev' profile not found in test config file");
        }

        // Apply CLI arguments last
        apply_cli_args_to_effective(&cli_args_mock, &mut effective);

        // Assertions based on precedence: CLI > Profile > Global > EffectiveConfig_default
        assert_eq!(effective.log_group_pattern, cli_args_mock.log_group_pattern); // CLI
        assert_eq!(effective.otlp_endpoint, cli_args_mock.otlp_endpoint); // CLI
        assert_eq!(effective.aws_region, cli_args_mock.aws_region); // CLI overrides profile 'dev' ("us-west-1") and global ("us-east-1")

        // Define constants for programmatic defaults to use in assertions
        const DEFAULT_SESSION_TIMEOUT_MS_TEST: u64 = 30 * 60 * 1000;
        const DEFAULT_TRACE_TIMEOUT_MS_TEST: u64 = 5 * 1000;
        const DEFAULT_TRACE_STRAGGLERS_WAIT_MS_TEST: u64 = 0;

        assert_eq!(
            effective.session_timeout_ms,
            cli_args_mock
                .session_timeout
                .unwrap_or(DEFAULT_SESSION_TIMEOUT_MS_TEST)
        );
        assert_eq!(
            effective.trace_timeout_ms,
            cli_args_mock
                .trace_timeout
                .unwrap_or(DEFAULT_TRACE_TIMEOUT_MS_TEST)
        );
        assert_eq!(
            effective.trace_stragglers_wait_ms,
            cli_args_mock
                .trace_stragglers_wait
                .unwrap_or(DEFAULT_TRACE_STRAGGLERS_WAIT_MS_TEST)
        );
        assert_eq!(effective.poll_interval_ms, cli_args_mock.poll_interval); // Some(30000ms) - This is Option<u64> on both sides
        assert_eq!(effective.backtrace_ms, cli_args_mock.backtrace); // Some(60000ms) - This is Option<u64> on both sides
        assert_eq!(
            effective.event_severity_attribute,
            cli_args_mock
                .event_severity_attribute
                .unwrap_or_else(|| "event.severity".to_string())
        );
        assert_eq!(
            effective.color_by,
            cli_args_mock.color_by.unwrap_or(ColoringMode::Service)
        );
        assert_eq!(
            effective.theme,
            cli_args_mock.theme.unwrap_or(Theme::Default)
        );
    }

    #[test]
    fn test_merge_into_profile_config() {
        let base = ProfileConfig {
            log_group_pattern: Some(vec!["base-pattern".to_string()]),
            stack_name: Some("base-stack".to_string()),
            otlp_endpoint: Some("http://base:4318".to_string()),
            otlp_headers: None,
            aws_region: Some("us-east-1".to_string()),
            aws_profile: None,
            forward_only: Some(false),
            attrs: Some("base.*".to_string()),
            event_severity_attribute: None,
            poll_interval: Some("10s".to_string()), // String duration
            session_timeout: None,                  // String duration (None)
            theme: Some(Theme::Material),
            color_by: Some(ColoringMode::Service),
            events_only: Some(false),
            trace_timeout: Some("7000ms".to_string()), // String duration
            trace_stragglers_wait: Some("1s".to_string()), // String duration
            grep: None,
            backtrace: Some("2m".to_string()), // String duration
        };
        let overrides = ProfileConfig {
            log_group_pattern: None,
            stack_name: Some("override-stack".to_string()),
            otlp_endpoint: None,
            otlp_headers: Some(vec!["override-header".to_string()]),
            aws_region: Some("us-west-2".to_string()),
            aws_profile: None,
            forward_only: Some(true),
            attrs: None,
            event_severity_attribute: Some("override.severity".to_string()),
            poll_interval: Some("15s".to_string()), // Override string duration
            session_timeout: Some("90m".to_string()), // Override string duration
            theme: None,
            color_by: Some(ColoringMode::Span),
            events_only: None,
            trace_timeout: Some("20s".to_string()), // Override string duration
            trace_stragglers_wait: Some("0ms".to_string()), // Override string duration (to default)
            grep: Some("override-grep".to_string()),
            backtrace: None, // Override with None
        };
        let merged = merge_into_profile_config(&base, &overrides);

        // Assertions check that overrides take precedence, or base is used if override is None.
        // Durations are asserted as Option<String>.
        assert_eq!(merged.log_group_pattern, base.log_group_pattern); // Override is None
        assert_eq!(merged.stack_name, overrides.stack_name);
        assert_eq!(merged.otlp_endpoint, base.otlp_endpoint); // Override is None
        assert_eq!(merged.otlp_headers, overrides.otlp_headers);
        assert_eq!(merged.aws_region, overrides.aws_region);
        assert_eq!(merged.aws_profile, base.aws_profile); // Override is None
        assert_eq!(merged.forward_only, overrides.forward_only);
        assert_eq!(merged.attrs, base.attrs); // Override is None
        assert_eq!(
            merged.event_severity_attribute,
            overrides.event_severity_attribute
        );
        assert_eq!(merged.poll_interval, overrides.poll_interval);
        assert_eq!(merged.session_timeout, overrides.session_timeout);
        assert_eq!(merged.theme, base.theme); // Override is None
        assert_eq!(merged.color_by, overrides.color_by);
        assert_eq!(merged.events_only, base.events_only); // Override is None
        assert_eq!(merged.trace_timeout, overrides.trace_timeout);
        assert_eq!(
            merged.trace_stragglers_wait,
            overrides.trace_stragglers_wait
        );
        assert_eq!(merged.grep, overrides.grep);
        assert_eq!(merged.backtrace, base.backtrace); // Override is None
    }
}
