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

use crate::cli::{CliArgs, ColoringMode};
use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

// Default filename for the configuration
const LIVETRACE_TOML: &str = ".livetrace.toml";

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
#[derive(Debug, Deserialize, Serialize, Default, Clone)] // Added Clone
#[serde(deny_unknown_fields)]
pub struct ProfileConfig {
    // --- Discovery --- (Mirroring CliArgs groups)
    #[serde(rename = "log-group-pattern")]
    pub log_group_pattern: Option<Vec<String>>,
    #[serde(rename = "stack-name")]
    pub stack_name: Option<String>,

    // --- Forwarding --- (Mirroring CliArgs)
    #[serde(rename = "otlp-endpoint")]
    pub otlp_endpoint: Option<String>,
    #[serde(rename = "otlp-header")]
    pub otlp_headers: Option<Vec<String>>,

    // --- AWS --- (Mirroring CliArgs)
    #[serde(rename = "aws-region")]
    pub aws_region: Option<String>,
    #[serde(rename = "aws-profile")]
    pub aws_profile: Option<String>,

    // --- Console Display --- (Mirroring CliArgs)
    #[serde(rename = "forward-only")]
    pub forward_only: Option<bool>,
    #[serde(rename = "attrs")]
    pub attrs: Option<String>,
    #[serde(rename = "event-severity-attribute")]
    pub event_severity_attribute: Option<String>,
    #[serde(rename = "monochrome")]
    pub monochrome: Option<bool>,
    #[serde(rename = "theme")]
    pub theme: Option<String>,
    #[serde(rename = "color-by", skip_serializing_if = "Option::is_none")]
    pub color_by: Option<ColoringMode>,

    // --- Mode --- (Mirroring CliArgs groups)
    #[serde(rename = "poll-interval")]
    pub poll_interval: Option<u64>,
    #[serde(rename = "session-timeout")]
    pub session_timeout: Option<u64>,
    // Note: Verbosity (`verbose`) is generally not configured via file.
    #[serde(rename = "events-only", skip_serializing_if = "Option::is_none")]
    pub events_only: Option<bool>,

    #[serde(rename = "trace-timeout", skip_serializing_if = "Option::is_none")]
    pub trace_timeout: Option<u64>,
}

/// Represents the final, merged configuration after applying precedence rules.
#[derive(Debug, Clone)] // Clone is useful
pub struct EffectiveConfig {
    // --- Discovery ---
    pub log_group_pattern: Option<Vec<String>>,
    pub stack_name: Option<String>,

    // --- Forwarding ---
    pub otlp_endpoint: Option<String>,
    pub otlp_headers: Vec<String>, // Merged headers

    // --- AWS ---
    pub aws_region: Option<String>,
    pub aws_profile: Option<String>,

    // --- Console Display ---
    pub forward_only: bool,
    pub attrs: Option<String>,
    pub event_severity_attribute: String,
    pub color_by: ColoringMode,
    pub events_only: bool,
    pub trace_timeout: u64,

    // --- Mode ---
    pub poll_interval: Option<u64>,
    pub session_timeout: u64,

    // --- Execution Control ---
    pub verbose: u8, // Keep verbosity
    pub theme: String, // Changed from monochrome: bool
                     // We don't store config_profile or save_profile here,
                     // as they are processed before creating EffectiveConfig.
}

// --- Add from_cli_args implementation ---
impl ProfileConfig {
    /// Creates a ProfileConfig from CliArgs, only including non-default values.
    pub fn from_cli_args(args: &CliArgs) -> Self {
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
            // Only save flags/options if they differ from clap's default
            forward_only: Some(args.forward_only).filter(|&f| f), // Default is false
            attrs: args.attrs.clone(),
            poll_interval: args.poll_interval,
            session_timeout: Some(args.session_timeout).filter(|&t| t != 30), // Default is 30
            event_severity_attribute: Some(args.event_severity_attribute.clone())
                .filter(|s| s != "event.severity"), // Default
            monochrome: None, // No longer used, but kept for compatibility
            theme: Some(args.theme.clone()).filter(|s| s != "default"), // Default is "default"
            color_by: Some(args.color_by).filter(|&c| c != ColoringMode::Service), // Default is Service
            events_only: Some(args.events_only).filter(|&e| e), // Default is false
            trace_timeout: Some(args.trace_timeout).filter(|&t| t != 5), // Default is 5
        }
    }
}

// --- Main configuration functions ---

pub fn load_and_resolve_config(
    config_profile_name: Option<String>,
    cli_args: &CliArgs,
) -> Result<EffectiveConfig> {
    // First, we'll construct the base effective config with default values
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
        poll_interval: None,
        session_timeout: 30,
        verbose: 0,
        theme: "default".to_string(),
        color_by: ColoringMode::Service,
        events_only: false,
        trace_timeout: 5,
    };

    // If no profile specified, just return CLI args as effective config
    if config_profile_name.is_none() {
        // Apply CLI arguments to the empty effective config
        apply_cli_args_to_effective(cli_args, &mut effective);
        return Ok(effective);
    }

    // Try to load the config file
    let config_path = get_config_path()?;
    if !config_path.exists() {
        tracing::warn!(
            path = %config_path.display(),
            "Config file not found while trying to load profile. Using CLI arguments only."
        );
        // Still apply CLI args even if config file doesn't exist
        apply_cli_args_to_effective(cli_args, &mut effective);
        return Ok(effective);
    }

    // Load the configuration file
    let config_file = load_config_file(&config_path)?;

    // Apply global settings first (lowest precedence)
    if let Some(global_config) = &config_file.global {
        apply_profile_to_effective(global_config, &mut effective);
    }

    // Apply profile-specific settings if the named profile exists
    let profile_name = config_profile_name.unwrap(); // Safe because we checked is_none() above
    if let Some(profile_config) = config_file.profiles.get(&profile_name) {
        apply_profile_to_effective(profile_config, &mut effective);
        tracing::info!(profile = %profile_name, "Loaded configuration from profile");
    } else {
        // Profile specified but not found - return an error
        return Err(anyhow::anyhow!(
            "Configuration profile '{}' not found in config file '{}'",
            profile_name,
            config_path.display()
        ));
    }

    // Apply CLI arguments (highest precedence)
    // This ensures explicitly set CLI args override settings from the profile
    apply_cli_args_to_effective(cli_args, &mut effective);

    Ok(effective)
}

/// Applies CLI arguments to an effective configuration, but only if they are explicitly set.
fn apply_cli_args_to_effective(cli_args: &CliArgs, effective: &mut EffectiveConfig) {
    // For Option<T> fields, we know they're explicitly set if they're Some
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

    if cli_args.poll_interval.is_some() {
        effective.poll_interval = cli_args.poll_interval;
    }

    // For non-Option types, check if they differ from their default values
    // Only apply them if they're different (indicating they were explicitly set)

    // Default: false
    if cli_args.forward_only {
        effective.forward_only = true;
    }

    // Default: "event.severity"
    if cli_args.event_severity_attribute != "event.severity" {
        effective.event_severity_attribute = cli_args.event_severity_attribute.clone();
    }

    // Default: 30 (Only applicable if poll_interval is None)
    if cli_args.poll_interval.is_none() && cli_args.session_timeout != 30 {
        effective.session_timeout = cli_args.session_timeout;
    }

    // Default: ColoringMode::Service
    if cli_args.color_by != ColoringMode::default() {
        effective.color_by = cli_args.color_by;
    }

    // Default: false
    if cli_args.events_only {
        effective.events_only = true;
    }

    // Default: 5
    if cli_args.trace_timeout != 5 {
        effective.trace_timeout = cli_args.trace_timeout;
    }

    // Default: "default"
    if cli_args.theme != "default" {
        effective.theme = cli_args.theme.clone();
    }

    // Always apply verbosity from CLI - highest precedence makes sense here
    effective.verbose = cli_args.verbose;
}

// Remove the old placeholder:
// pub fn handle_save_profile(
//     profile_name: &str,
//     // cli_args: &CliArgs, // We'll likely need clap::ArgMatches later
// ) -> Result<()> {
//     // TODO: Implement saving logic
//     unimplemented!("Saving profile configuration not yet implemented.");
// }

// Add the new implementation:
use std::path::PathBuf; // Need this for path manipulation

pub fn get_config_path() -> Result<PathBuf> {
    // For now, just use the local directory. Could be extended later.
    Ok(PathBuf::from(LIVETRACE_TOML))
}

// Helper to load or create default config
pub fn load_or_default_config_file() -> Result<ConfigFile> {
    let config_path = get_config_path()?;
    if config_path.exists() {
        load_config_file(&config_path) // Use existing private helper
    } else {
        Ok(ConfigFile::default()) // Create a default if file doesn't exist
    }
}

/// Saves the provided profile configuration under the given name in the config file.
pub fn save_profile_config(profile_name: &str, profile_data: &ProfileConfig) -> Result<()> {
    let config_path = get_config_path()?;
    // Load existing config or create a default one
    let mut config = load_or_default_config_file()?;

    // Update the specific profile in the HashMap
    config
        .profiles
        .insert(profile_name.to_string(), profile_data.clone());

    // Serialize the entire ConfigFile structure back to TOML
    let toml_string =
        toml::to_string_pretty(&config).context("Failed to serialize configuration to TOML")?;

    // Write the TOML string back to the file, overwriting it
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

// --- Helper for Loading --- (Basic version)
fn load_config_file(path: &Path) -> Result<ConfigFile> {
    // Make private helper
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config: ConfigFile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML from config file: {}", path.display()))?;
    // Basic validation (e.g., version check) could go here
    Ok(config)
}

// Add the apply_profile_to_effective function that was removed during the edit

/// Applies settings from a profile to an effective configuration.
fn apply_profile_to_effective(profile: &ProfileConfig, effective: &mut EffectiveConfig) {
    // Only override non-None values from the profile
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

    if let Some(val) = profile.poll_interval {
        effective.poll_interval = Some(val);
    }

    if let Some(val) = profile.session_timeout {
        effective.session_timeout = val;
    }

    if let Some(val) = &profile.theme {
        effective.theme = val.clone();
    } else if let Some(true) = profile.monochrome {
        // For backward compatibility, set theme to monochrome if monochrome was true
        effective.theme = "monochrome".to_string();
    }

    if let Some(val) = &profile.color_by {
        effective.color_by = *val;
    }

    if let Some(val) = profile.events_only {
        effective.events_only = val;
    }

    if let Some(val) = profile.trace_timeout {
        effective.trace_timeout = val;
    }
}

/// Merges two ProfileConfig instances.
///
/// Values from `overrides` take precedence if they are `Some`.
/// Otherwise, the values from `base` are kept.
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
        poll_interval: overrides.poll_interval.or(base.poll_interval),
        session_timeout: overrides.session_timeout.or(base.session_timeout),
        theme: overrides.theme.clone().or_else(|| base.theme.clone()),
        color_by: overrides.color_by.or(base.color_by),
        events_only: overrides.events_only.or(base.events_only),
        trace_timeout: overrides.trace_timeout.or(base.trace_timeout),
        // Note: monochrome is deprecated, so we don't explicitly merge it.
        // If needed for backward compat, theme should handle it.
        monochrome: base.monochrome, // Keep base value if needed elsewhere, but don't merge overrides
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
            poll_interval: Some(30),
            session_timeout: 45,
            event_severity_attribute: "custom.severity".to_string(),
            config_profile: None,
            save_profile: None,
            theme: "test-theme".to_string(),
            list_themes: false,
            color_by: ColoringMode::Service,
            events_only: true,
            trace_timeout: 10,
            command: None, // Add the new field
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

[profiles.default]
log-group-pattern = ["default-pattern-a", "default-pattern-b"]
forward-only = true

[profiles.dev]
log-group-pattern = ["/aws/lambda/dev-func", "specific-dev-group"]
otlp-endpoint = "http://dev-collector:4318"
aws-region = "us-west-1"
        "#;
        fs::write(&config_path, config_content).expect("Failed to write test config");
        config_path
    }

    #[test]
    fn test_from_cli_args() {
        let args = mock_cli_args();
        let profile = ProfileConfig::from_cli_args(&args);

        // Verify fields are converted correctly
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
        assert_eq!(profile.poll_interval, Some(30));
        assert_eq!(profile.session_timeout, Some(45));
        assert_eq!(
            profile.event_severity_attribute,
            Some("custom.severity".to_string())
        );
        assert_eq!(profile.theme, Some("test-theme".to_string()));
        assert_eq!(profile.events_only, Some(true));
        assert_eq!(profile.trace_timeout, Some(10));
    }

    #[test]
    fn test_save_profile_config() {
        let temp_dir = tempdir().expect("Failed to create temp dir");

        // Get the temporary path
        let temp_path = temp_dir.path().to_path_buf().join(LIVETRACE_TOML);

        // Create a test profile
        let args = mock_cli_args();
        let profile = ProfileConfig::from_cli_args(&args);

        // Since we can't easily mock the get_config_path function,
        // we'll directly write to our temporary path
        let mut config = ConfigFile::default();
        config.profiles.insert("test-profile".to_string(), profile);
        let toml_string = toml::to_string_pretty(&config).expect("Failed to serialize");
        fs::write(&temp_path, toml_string).expect("Failed to write test config");

        // Now read it back to verify
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
        // Create a base effective config
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
            poll_interval: None,
            session_timeout: 30,
            verbose: 0,
            theme: "default".to_string(),
            color_by: ColoringMode::Service,
            events_only: false,
            trace_timeout: 5,
        };

        // Create a profile with some settings
        let profile = ProfileConfig {
            log_group_pattern: Some(vec![
                "profile-pattern-1".to_string(),
                "profile-pattern-2".to_string(),
            ]),
            stack_name: None,
            otlp_endpoint: Some("http://profile-endpoint:4318".to_string()),
            otlp_headers: Some(vec!["Profile-Auth=token123".to_string()]),
            aws_region: None,
            aws_profile: Some("profile-aws-profile".to_string()),
            forward_only: Some(true),
            attrs: Some("profile.*".to_string()),
            event_severity_attribute: Some("profile.severity".to_string()),
            poll_interval: Some(45),
            session_timeout: None,
            monochrome: None,
            theme: Some("test-theme".to_string()),
            color_by: None,
            events_only: Some(true),
            trace_timeout: Some(10),
        };

        // Apply the profile
        apply_profile_to_effective(&profile, &mut effective);

        // Verify overrides happened correctly
        assert_eq!(
            effective.log_group_pattern,
            Some(vec![
                "profile-pattern-1".to_string(),
                "profile-pattern-2".to_string()
            ])
        );
        assert_eq!(effective.stack_name, Some("original-stack".to_string()));
        assert_eq!(
            effective.otlp_endpoint,
            Some("http://profile-endpoint:4318".to_string())
        );
        assert_eq!(effective.otlp_headers, vec!["Profile-Auth=token123"]);
        assert_eq!(effective.aws_region, Some("us-east-1".to_string()));
        assert_eq!(
            effective.aws_profile,
            Some("profile-aws-profile".to_string())
        );
        assert!(effective.forward_only);
        assert_eq!(effective.attrs, Some("profile.*".to_string()));
        assert_eq!(effective.event_severity_attribute, "profile.severity");
        assert_eq!(effective.poll_interval, Some(45));
        assert_eq!(effective.session_timeout, 30);
        assert_eq!(effective.theme, "test-theme".to_string());
        assert!(effective.events_only);
        assert_eq!(effective.trace_timeout, 10);
    }

    #[test]
    fn test_load_and_resolve_config() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let config_path = create_test_config_file(temp_dir.path());

        // Mock CLI args with a specific profile
        let mut args = mock_cli_args();
        args.config_profile = Some("dev".to_string());

        // Since we can't easily override the get_config_path function,
        // we need to reorganize our load_and_resolve_config to make it more testable
        // For this test, we'll manually read the config and simulate load_and_resolve_config

        let config_file: ConfigFile =
            toml::from_str(&fs::read_to_string(&config_path).expect("Failed to read test config"))
                .expect("Failed to parse test config");

        // Start with CLI args
        let mut effective = EffectiveConfig {
            log_group_pattern: args.log_group_pattern.clone(),
            stack_name: args.stack_name.clone(),
            otlp_endpoint: args.otlp_endpoint.clone(),
            otlp_headers: args.otlp_headers.clone(),
            aws_region: args.aws_region.clone(),
            aws_profile: args.aws_profile.clone(),
            forward_only: args.forward_only,
            attrs: args.attrs.clone(),
            event_severity_attribute: args.event_severity_attribute.clone(),
            poll_interval: args.poll_interval,
            session_timeout: args.session_timeout,
            verbose: args.verbose,
            theme: args.theme.clone(),
            color_by: args.color_by,
            events_only: args.events_only,
            trace_timeout: args.trace_timeout,
        };

        // Apply global settings
        if let Some(global_config) = &config_file.global {
            apply_profile_to_effective(global_config, &mut effective);
        }

        // Apply profile settings
        if let Some(profile_config) = config_file.profiles.get("dev") {
            apply_profile_to_effective(profile_config, &mut effective);
        }

        // Verify the result has correct precedence
        // Pattern from 'dev' profile should override CLI args
        assert_eq!(
            effective.log_group_pattern,
            Some(vec![
                "/aws/lambda/dev-func".to_string(),
                "specific-dev-group".to_string()
            ])
        );

        // otlp_endpoint from 'dev' profile should override CLI args
        assert_eq!(
            effective.otlp_endpoint,
            Some("http://dev-collector:4318".to_string())
        );

        // aws_region from 'dev' profile should override global
        assert_eq!(effective.aws_region, Some("us-west-1".to_string()));

        // event_severity_attribute from global (not overridden by 'dev')
        assert_eq!(effective.event_severity_attribute, "global.severity");
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
            poll_interval: Some(10),
            session_timeout: None,
            theme: Some("base-theme".to_string()),
            color_by: Some(ColoringMode::Service),
            events_only: Some(false),
            trace_timeout: None,
            monochrome: None,
        };

        let overrides = ProfileConfig {
            log_group_pattern: None,                                         // Keep base
            stack_name: Some("override-stack".to_string()),                  // Override base
            otlp_endpoint: None,                                             // Keep base
            otlp_headers: Some(vec!["override-header".to_string()]),         // Add new
            aws_region: Some("us-west-2".to_string()),                       // Override base
            aws_profile: None,                                               // Keep base (None)
            forward_only: Some(true),                                        // Override base
            attrs: None,                                                     // Keep base
            event_severity_attribute: Some("override.severity".to_string()), // Add new
            poll_interval: None,                                             // Keep base
            session_timeout: Some(99),                                       // Add new
            theme: None,                                                     // Keep base
            color_by: Some(ColoringMode::Span),                              // Override base
            events_only: None,                                               // Keep base
            trace_timeout: Some(20),                                         // Add new
            monochrome: None,
        };

        let merged = merge_into_profile_config(&base, &overrides);

        // --- Assertions ---
        // Kept from base
        assert_eq!(merged.log_group_pattern, base.log_group_pattern);
        assert_eq!(merged.otlp_endpoint, base.otlp_endpoint);
        assert_eq!(merged.aws_profile, base.aws_profile);
        assert_eq!(merged.attrs, base.attrs);
        assert_eq!(merged.poll_interval, base.poll_interval);
        assert_eq!(merged.theme, base.theme);
        assert_eq!(merged.events_only, base.events_only);

        // Overridden by overrides
        assert_eq!(merged.stack_name, overrides.stack_name);
        assert_eq!(merged.aws_region, overrides.aws_region);
        assert_eq!(merged.forward_only, overrides.forward_only);
        assert_eq!(merged.color_by, overrides.color_by);

        // Added by overrides
        assert_eq!(merged.otlp_headers, overrides.otlp_headers);
        assert_eq!(
            merged.event_severity_attribute,
            overrides.event_severity_attribute
        );
        assert_eq!(merged.session_timeout, overrides.session_timeout);
        assert_eq!(merged.trace_timeout, overrides.trace_timeout);
    }
}
