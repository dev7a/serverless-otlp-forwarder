// Crate Modules
mod aws_setup;
mod cli;
mod config;
mod console_display;
mod forwarder;
mod live_tail_adapter;
mod poller;
mod processing;

// Standard Library
use std::env;
use std::time::Duration;

// External Crates
use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use reqwest::Client as ReqwestClient;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

// Internal Crate Imports
use crate::aws_setup::setup_aws_resources;
use crate::cli::{parse_event_attr_globs, CliArgs};
use crate::config::{load_and_resolve_config, save_profile_config, EffectiveConfig, ProfileConfig};
use crate::console_display::{display_console, Theme};
use crate::forwarder::{parse_otlp_headers_from_vec, send_batch};
use crate::live_tail_adapter::start_live_tail_task;
use crate::poller::start_polling_task;
use crate::processing::{SpanCompactionConfig, TelemetryData};

/// livetrace: Tail CloudWatch Logs for OTLP/stdout traces and forward them.
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Parse Args fully now (clap handles defaults)
    let args = CliArgs::parse();

    // --- Save Profile Check ---
    if let Some(profile_name) = args.save_profile.as_ref() {
        // Convert CliArgs to the savable ProfileConfig format
        let profile_to_save = ProfileConfig::from_cli_args(&args);
        // Call the actual save function
        save_profile_config(profile_name, &profile_to_save)?;
        println!(
            "Configuration saved to profile '{}'. Exiting.",
            profile_name
        );
        return Ok(());
    }
    // --- End Save Profile Check ---

    // --- Load Configuration Profile if Specified ---
    let config = if args.config_profile.is_some() {
        load_and_resolve_config(args.config_profile.clone(), &args)?
    } else {
        EffectiveConfig {
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
        }
    };

    // Validate discovery parameters - either log_group_pattern or stack_name must be set
    if config.log_group_pattern.is_none() && config.stack_name.is_none() {
        return Err(anyhow::anyhow!(
            "Either --pattern or --stack-name must be provided on the command line or in the configuration profile"
        ));
    }

    // 2. Initialize Logging
    let log_level = match config.verbose {
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .parse_lossy(format!("{}={}", env!("CARGO_PKG_NAME"), log_level)),
        )
        .init();
    tracing::debug!("Starting livetrace with configuration: {:?}", config);

    // 3. Resolve OTLP Endpoint (CONFIG > TRACES_ENV > GENERAL_ENV)
    let resolved_endpoint: Option<String> = config.otlp_endpoint.clone().or_else(|| {
        env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
            .ok()
            .or_else(|| env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok())
    });
    let endpoint_opt = resolved_endpoint.as_deref();
    tracing::debug!(config_endpoint = ?config.otlp_endpoint, env_traces = ?env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT").ok(), env_general = ?env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(), resolved = ?resolved_endpoint, "Resolved OTLP endpoint");

    // 4. Resolve OTLP Headers (CONFIG > TRACES_ENV > GENERAL_ENV)
    let resolved_headers_vec: Vec<String> = if !config.otlp_headers.is_empty() {
        tracing::debug!(source="config", headers=?config.otlp_headers, "Using headers from configuration");
        config.otlp_headers.clone()
    } else if let Ok(hdr_str) = env::var("OTEL_EXPORTER_OTLP_TRACES_HEADERS") {
        let headers: Vec<String> = hdr_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        tracing::debug!(source="env_traces", headers=?headers, "Using headers from OTEL_EXPORTER_OTLP_TRACES_HEADERS");
        headers
    } else if let Ok(hdr_str) = env::var("OTEL_EXPORTER_OTLP_HEADERS") {
        let headers: Vec<String> = hdr_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        tracing::debug!(source="env_general", headers=?headers, "Using headers from OTEL_EXPORTER_OTLP_HEADERS");
        headers
    } else {
        Vec::new() // No headers specified anywhere
    };

    // 5. Post-Resolution Validation
    if config.forward_only && endpoint_opt.is_none() {
        return Err(anyhow::anyhow!("
            --forward-only requires --otlp-endpoint argument or OTEL_EXPORTER_OTLP_TRACES_ENDPOINT/OTEL_EXPORTER_OTLP_ENDPOINT env var to be set"));
    }
    if !config.forward_only && endpoint_opt.is_none() {
        tracing::debug!("Running in console-only mode. No OTLP endpoint configured.");
    }

    // 6. AWS Setup (Config, Clients, Discovery, Validation, ARN Construction)
    // Create a temporary CliArgs-like structure for AWS setup since it expects CliArgs
    let aws_setup_args = CliArgs {
        log_group_pattern: config.log_group_pattern.clone(),
        stack_name: config.stack_name.clone(),
        aws_region: config.aws_region.clone(),
        aws_profile: config.aws_profile.clone(),
        // Other fields don't matter for AWS setup, use defaults
        otlp_endpoint: None,
        otlp_headers: Vec::new(),
        verbose: 0,
        forward_only: false,
        timeline_width: 80,
        compact_display: false,
        event_attrs: None,
        poll_interval: None,
        session_timeout: 30,
        event_severity_attribute: "event.severity".to_string(),
        config_profile: None,
        save_profile: None,
        theme: "default".to_string(),
    };
    let aws_result = setup_aws_resources(&aws_setup_args).await?;
    let cwl_client = aws_result.cwl_client;
    let account_id = aws_result.account_id;
    let region_str = aws_result.region_str;
    let resolved_log_group_arns = aws_result.resolved_arns;

    // 7. Setup HTTP Client & Parse Resolved OTLP Headers
    let http_client = ReqwestClient::builder()
        .build()
        .context("Failed to build Reqwest client")?;
    tracing::debug!("Reqwest HTTP client created.");
    let otlp_header_map = parse_otlp_headers_from_vec(&resolved_headers_vec)?;
    let compaction_config = SpanCompactionConfig::default();

    // 8. Prepare Console Display
    let console_enabled = !config.forward_only;
    let event_attr_globs = parse_event_attr_globs(&config.event_attrs); // Now passes the Option<String> directly

    // --- Preamble Output (List Style) ---
    let preamble_width: usize = 80; // Explicitly usize
    let config_heading = " Livetrace Configuration";
    let config_padding = preamble_width.saturating_sub(config_heading.len() + 3);

    println!("\n");
    println!(
        " {} {} {}",
        "─".dimmed(),
        config_heading.bold(),
        "─".repeat(config_padding).dimmed()
    );
    println!("  {:<18}: {}", "Account ID".dimmed(), account_id);
    println!("  {:<18}: {}", "Region".dimmed(), region_str);
    // Need validated names for the count/list - let's re-get them from ARNs for simplicity here
    // In a real scenario, might pass validated_names through AwsSetupResult
    let validated_log_group_names_for_display: Vec<String> = resolved_log_group_arns
        .iter()
        .map(|arn| arn.split(':').last().unwrap_or("unknown-name").to_string())
        .collect();
    println!(
        "  {:<18}: ({})",
        "Log Groups".dimmed(),
        validated_log_group_names_for_display.len()
    );
    for name in &validated_log_group_names_for_display {
        println!("{:<20}  - {}", "", name.bright_black());
    }
    if let Some(profile) = &args.config_profile {
        println!("  {:<18}: {}", "Config Profile".dimmed(), profile);
    }
    println!("\n");
    // --- End Preamble ---

    // 9. Create MPSC Channel and Spawn Event Source Task
    let (tx, mut rx) = mpsc::channel::<Result<TelemetryData>>(100); // Channel for TelemetryData or errors

    if let Some(interval_secs) = config.poll_interval {
        // --- Polling Mode ---
        tracing::debug!(
            interval = interval_secs,
            "Using FilterLogEvents polling mode."
        );
        start_polling_task(cwl_client, resolved_log_group_arns, interval_secs, tx);
    } else {
        // --- Live Tail Mode ---
        tracing::debug!(
            timeout_minutes = config.session_timeout,
            "Using StartLiveTail streaming mode with timeout."
        );
        start_live_tail_task(
            cwl_client,
            resolved_log_group_arns,
            tx,
            config.session_timeout,
        );
    }

    // 10. Main Event Processing Loop
    tracing::debug!("Waiting for telemetry events...");
    let mut telemetry_buffer: Vec<TelemetryData> = Vec::new();
    let mut ticker = interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            // Receive from either the poller or live tail adapter task
            received = rx.recv() => {
                match received {
                    Some(Ok(telemetry)) => {
                        // Successfully received telemetry data
                        if console_enabled || endpoint_opt.is_some() {
                            telemetry_buffer.push(telemetry);
                        }
                    }
                    Some(Err(e)) => {
                        // An error occurred in the source task (polling or live tail)
                        tracing::error!(error = %e, "Error received from event source task");
                        // Depending on the error, we might want to break or continue
                        // For now, let's break if the source task reports a fatal error
                        // (like channel closure implicitly does via None)
                    }
                    None => {
                        // Channel closed by the sender task
                        tracing::info!("Event source channel closed. Exiting.");
                        break;
                    }
                }
            }
            // Ticker Branch (unchanged)
            _ = ticker.tick() => {
                if !telemetry_buffer.is_empty() {
                    tracing::debug!("Timer tick: Processing buffer with {} items.", telemetry_buffer.len());
                    let batch_to_send = std::mem::take(&mut telemetry_buffer);

                    if console_enabled {
                        // Convert string theme to Theme enum
                        let theme = Theme::from_str(&config.theme);
                        display_console(
                            &batch_to_send,
                            config.timeline_width,
                            config.compact_display,
                            &event_attr_globs,
                            config.event_severity_attribute.as_str(),
                            theme,
                        )?;
                    }

                    if let Some(endpoint) = endpoint_opt {
                        send_batch(
                            &http_client,
                            endpoint,
                            batch_to_send,
                            &compaction_config,
                            otlp_header_map.clone(),
                        ).await?;
                    }
                }
            }
            // Removed Live Tail stream recv() branch
            // Removed Timeout branch
        }
    }

    // 11. Final Flush
    if !telemetry_buffer.is_empty() {
        tracing::debug!(
            "Flushing remaining {} items from buffer before exiting.",
            telemetry_buffer.len()
        );
        let final_batch = std::mem::take(&mut telemetry_buffer);

        if console_enabled {
            // Convert string theme to Theme enum
            let theme = Theme::from_str(&config.theme);
            display_console(
                &final_batch,
                config.timeline_width,
                config.compact_display,
                &event_attr_globs,
                &config.event_severity_attribute,
                theme,
            )?;
        }

        if let Some(endpoint) = endpoint_opt {
            send_batch(
                &http_client,
                endpoint,
                final_batch,
                &compaction_config,
                otlp_header_map.clone(),
            )
            .await?;
        }
    }

    tracing::info!("livetrace finished successfully.");
    Ok(())
}
