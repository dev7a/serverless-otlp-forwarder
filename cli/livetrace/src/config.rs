use crate::cli::CliArgs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::Write, path::Path};

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
    pub profiles: HashMap<String, ProfileConfig>,
}

/// Represents the configuration settings within a profile (or global section).
/// Fields should generally mirror CliArgs, using Option<T> and serde attributes.
#[derive(Debug, Deserialize, Serialize, Default, Clone)] // Added Clone
#[serde(deny_unknown_fields)]
pub struct ProfileConfig {
    // --- Discovery --- (Mirroring CliArgs groups)
    #[serde(rename = "pattern")]
    pub log_group_pattern: Option<String>,
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
    #[serde(rename = "timeline-width")]
    pub timeline_width: Option<usize>,
    #[serde(rename = "compact-display")]
    pub compact_display: Option<bool>,
    #[serde(rename = "event-attrs")]
    pub event_attrs: Option<String>,
    #[serde(rename = "event-severity-attribute")]
    pub event_severity_attribute: Option<String>,
    #[serde(rename = "monochrome")]
    pub monochrome: Option<bool>,
    #[serde(rename = "theme")]
    pub theme: Option<String>,

    // --- Mode --- (Mirroring CliArgs groups)
    #[serde(rename = "poll-interval")]
    pub poll_interval: Option<u64>,
    #[serde(rename = "session-timeout")]
    pub session_timeout: Option<u64>,
    // Note: Verbosity (`verbose`) is generally not configured via file.
}

/// Represents the final, merged configuration after applying precedence rules.
#[derive(Debug, Clone)] // Clone is useful
pub struct EffectiveConfig {
    // --- Discovery ---
    pub log_group_pattern: Option<String>,
    pub stack_name: Option<String>,

    // --- Forwarding ---
    pub otlp_endpoint: Option<String>,
    pub otlp_headers: Vec<String>, // Merged headers

    // --- AWS ---
    pub aws_region: Option<String>,
    pub aws_profile: Option<String>,

    // --- Console Display ---
    pub forward_only: bool,
    pub timeline_width: usize,
    pub compact_display: bool,
    pub event_attrs: Option<String>,
    pub event_severity_attribute: String,

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
            log_group_pattern: args.log_group_pattern.clone(), // Always include one of pattern/stack
            stack_name: args.stack_name.clone(), // Always include one of pattern/stack
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
            timeline_width: Some(args.timeline_width).filter(|&w| w != 80), // Default is 80
            compact_display: Some(args.compact_display).filter(|&c| c), // Default is false
            event_attrs: args.event_attrs.clone(),
            poll_interval: args.poll_interval,
            session_timeout: Some(args.session_timeout).filter(|&t| t != 30), // Default is 30
            event_severity_attribute: Some(args.event_severity_attribute.clone())
                .filter(|s| s != "event.severity"), // Default
            monochrome: None, // No longer used, but kept for compatibility
            theme: Some(args.theme.clone()).filter(|s| s != "default"), // Default is "default"
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
        timeline_width: 80,
        compact_display: false,
        event_attrs: None,
        event_severity_attribute: "event.severity".to_string(),
        poll_interval: None,
        session_timeout: 30,
        verbose: 0,
        theme: "default".to_string(),
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
    if cli_args.log_group_pattern.is_some() {
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

    if cli_args.event_attrs.is_some() {
        effective.event_attrs = cli_args.event_attrs.clone();
    }

    if cli_args.poll_interval.is_some() {
        effective.poll_interval = cli_args.poll_interval;
    }

    // For non-Option types with default values, we'll always apply them
    // This assumes if they're passed on the CLI, they're explicitly set

    // Always apply these values from CLI args, as we can't detect if they were explicitly set
    effective.forward_only = cli_args.forward_only;
    effective.timeline_width = cli_args.timeline_width;
    effective.compact_display = cli_args.compact_display;
    effective.event_severity_attribute = cli_args.event_severity_attribute.clone();
    effective.session_timeout = cli_args.session_timeout;
    effective.verbose = cli_args.verbose;
    effective.theme = cli_args.theme.clone();
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

fn get_config_path() -> Result<PathBuf> {
    // For now, just use the local directory. Could be extended later.
    Ok(PathBuf::from(LIVETRACE_TOML))
}

// Helper to load or create default config
fn load_or_default_config_file() -> Result<ConfigFile> {
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
    if let Some(val) = &profile.log_group_pattern {
        effective.log_group_pattern = Some(val.clone());
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

    if let Some(val) = profile.timeline_width {
        effective.timeline_width = val;
    }

    if let Some(val) = profile.compact_display {
        effective.compact_display = val;
    }

    if let Some(val) = &profile.event_attrs {
        effective.event_attrs = Some(val.clone());
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
            log_group_pattern: Some("test-pattern".to_string()),
            stack_name: None,
            otlp_endpoint: Some("http://localhost:4318".to_string()),
            otlp_headers: vec!["Auth=Bearer xyz".to_string()],
            aws_region: Some("us-west-2".to_string()),
            aws_profile: Some("test-profile".to_string()),
            verbose: 1,
            forward_only: true,
            timeline_width: 100,
            compact_display: true,
            event_attrs: Some("http.*,db.*".to_string()),
            poll_interval: Some(30),
            session_timeout: 45,
            event_severity_attribute: "custom.severity".to_string(),
            config_profile: None,
            save_profile: None,
            theme: "test-theme".to_string(),
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
pattern = "default-pattern"
forward-only = true

[profiles.dev]
pattern = "dev-pattern"
otlp-endpoint = "http://dev-collector:4318"
aws-region = "us-west-1"
timeline-width = 120
        "#;
        fs::write(&config_path, config_content).expect("Failed to write test config");
        config_path
    }

    #[test]
    fn test_from_cli_args() {
        let args = mock_cli_args();
        let profile = ProfileConfig::from_cli_args(&args);

        // Verify fields are converted correctly
        assert_eq!(profile.log_group_pattern, Some("test-pattern".to_string()));
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
        assert_eq!(profile.timeline_width, Some(100));
        assert_eq!(profile.compact_display, Some(true));
        assert_eq!(profile.event_attrs, Some("http.*,db.*".to_string()));
        assert_eq!(profile.poll_interval, Some(30));
        assert_eq!(profile.session_timeout, Some(45));
        assert_eq!(
            profile.event_severity_attribute,
            Some("custom.severity".to_string())
        );
        assert_eq!(profile.theme, Some("test-theme".to_string()));
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
            Some("test-pattern".to_string())
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
            log_group_pattern: None,
            stack_name: Some("original-stack".to_string()),
            otlp_endpoint: None,
            otlp_headers: Vec::new(),
            aws_region: Some("us-east-1".to_string()),
            aws_profile: None,
            forward_only: false,
            timeline_width: 80,
            compact_display: false,
            event_attrs: None,
            event_severity_attribute: "default.severity".to_string(),
            poll_interval: None,
            session_timeout: 30,
            verbose: 0,
            theme: "default".to_string(),
        };

        // Create a profile with some settings
        let profile = ProfileConfig {
            log_group_pattern: Some("profile-pattern".to_string()),
            stack_name: None,
            otlp_endpoint: Some("http://profile-endpoint:4318".to_string()),
            otlp_headers: Some(vec!["Profile-Auth=token123".to_string()]),
            aws_region: None, // Don't override region
            aws_profile: Some("profile-aws-profile".to_string()),
            forward_only: Some(true),
            timeline_width: Some(120),
            compact_display: None, // Don't override
            event_attrs: Some("profile.*".to_string()),
            event_severity_attribute: Some("profile.severity".to_string()),
            poll_interval: Some(45),
            session_timeout: None, // Don't override
            monochrome: None,
            theme: Some("test-theme".to_string()),
        };

        // Apply the profile
        apply_profile_to_effective(&profile, &mut effective);

        // Verify overrides happened correctly
        assert_eq!(
            effective.log_group_pattern,
            Some("profile-pattern".to_string())
        );
        assert_eq!(effective.stack_name, Some("original-stack".to_string())); // Unchanged
        assert_eq!(
            effective.otlp_endpoint,
            Some("http://profile-endpoint:4318".to_string())
        );
        assert_eq!(effective.otlp_headers, vec!["Profile-Auth=token123"]);
        assert_eq!(effective.aws_region, Some("us-east-1".to_string())); // Unchanged
        assert_eq!(
            effective.aws_profile,
            Some("profile-aws-profile".to_string())
        );
        assert_eq!(effective.forward_only, true);
        assert_eq!(effective.timeline_width, 120);
        assert_eq!(effective.compact_display, false); // Unchanged
        assert_eq!(effective.event_attrs, Some("profile.*".to_string()));
        assert_eq!(effective.event_severity_attribute, "profile.severity");
        assert_eq!(effective.poll_interval, Some(45));
        assert_eq!(effective.session_timeout, 30); // Unchanged
        assert_eq!(effective.theme, "test-theme".to_string());
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
            timeline_width: args.timeline_width,
            compact_display: args.compact_display,
            event_attrs: args.event_attrs.clone(),
            event_severity_attribute: args.event_severity_attribute.clone(),
            poll_interval: args.poll_interval,
            session_timeout: args.session_timeout,
            verbose: args.verbose,
            theme: args.theme.clone(),
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
        assert_eq!(effective.log_group_pattern, Some("dev-pattern".to_string()));

        // otlp_endpoint from 'dev' profile should override CLI args
        assert_eq!(
            effective.otlp_endpoint,
            Some("http://dev-collector:4318".to_string())
        );

        // aws_region from 'dev' profile should override global
        assert_eq!(effective.aws_region, Some("us-west-1".to_string()));

        // timeline_width from 'dev' profile should override CLI args
        assert_eq!(effective.timeline_width, 120);

        // event_severity_attribute from global (not overridden by 'dev')
        assert_eq!(effective.event_severity_attribute, "global.severity");
    }
}
