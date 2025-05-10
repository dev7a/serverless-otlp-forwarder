#![doc = include_str!("../README.md")]
//! Core library for the `livetrace` CLI application.
//!
//! This crate provides the main functionality for discovering CloudWatch Log Groups,
//! tailing or polling them for OTLP (OpenTelemetry Protocol) trace data,
//! parsing this data, displaying it in a user-friendly console view, and
//! optionally forwarding it to an OTLP-compatible endpoint.

// Module declarations
pub mod aws_setup;
pub mod cli;
pub mod config;
pub mod console_display;
pub mod forwarder;
pub mod live_tail_adapter;
pub mod poller;
pub mod processing;

// Standard Library
use std::collections::HashMap;
use std::env;
use std::time::Duration;

// External Crates
use anyhow::{Context, Result};
use colored::*;
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use prost::Message;
use reqwest::Client as ReqwestClient;
use tokio::sync::mpsc;
use tokio::time::{interval, Instant};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

// Re-export CliArgs for easy use in main.rs, and other necessary items from modules.
// Specific functions and structs from submodules will be used via their module path,
// e.g., `aws_setup::setup_aws_resources`.
// Ensure these items are public in their respective modules.
use aws_setup::setup_aws_resources;
use cli::{parse_attr_globs, ColoringMode}; // Assuming these are pub in cli.rs
pub use cli::{CliArgs, Commands}; // Re-export Commands as well
use config::{
    load_and_resolve_config, load_or_default_config_file, merge_into_profile_config,
    save_profile_config, EffectiveConfig, ProfileConfig,
}; // Assuming these are pub in config.rs
use console_display::{display_console, get_terminal_width, Theme}; // Assuming these are pub in console_display.rs
use forwarder::{parse_otlp_headers_from_vec, send_batch}; // Assuming these are pub in forwarder.rs
use live_tail_adapter::start_live_tail_task; // Assuming this is pub in live_tail_adapter.rs
use poller::start_polling_task; // Assuming this is pub in poller.rs
use processing::{SpanCompactionConfig, TelemetryData}; // Assuming these are pub in processing.rs // Assuming this is pub in aws_setup.rs

// Constants for trace buffering timeouts
const IDLE_TIMEOUT: Duration = Duration::from_millis(500); // Time to wait after root is seen

// Structure to hold state for traces being buffered
#[derive(Debug)]
struct TraceBufferState {
    buffered_payloads: Vec<TelemetryData>,
    has_received_root: bool,
    first_message_received_at: Instant,
    last_message_received_at: Instant,
}

/// Main entry point for the livetrace application logic.
///
/// This function takes the parsed command-line arguments and executes the
/// primary operations of the tool: setting up AWS resources, starting
/// log data acquisition (either live tail or polling), processing telemetry,
/// displaying it, and forwarding it if configured.
pub async fn run_livetrace(args: CliArgs) -> Result<()> {
    // (Logic from original main.rs, starting after CliArgs::parse())

    // Check if list-themes was specified
    if args.list_themes {
        println!("\nAvailable themes:");
        println!("  * default - OpenTelemetry-inspired blue-purple palette");
        println!("  * tableau - Tableau 12 color palette with distinct hues");
        println!("  * colorbrewer - ColorBrewer Set3 palette (pastel colors)");
        println!("  * material - Material Design palette with bright, modern colors");
        println!("  * solarized - Solarized color scheme with muted tones");
        println!("  * monochrome - Grayscale palette for minimal distraction");
        println!("\nUsage: livetrace --theme <THEME>");
        return Ok(());
    }

    // Save Profile Check
    if let Some(profile_name) = args.save_profile.as_ref() {
        let config_file = load_or_default_config_file()?;
        let existing_profile_config = config_file
            .profiles
            .get(profile_name)
            .cloned()
            .unwrap_or_default();
        let cli_profile_config = ProfileConfig::from_cli_args(&args);
        let merged_profile_config =
            merge_into_profile_config(&existing_profile_config, &cli_profile_config);
        save_profile_config(profile_name, &merged_profile_config)?;
        println!(
            "Configuration profile '{}' updated in {}.",
            profile_name,
            config::get_config_path()?.display()
        );
    }

    // Load Configuration Profile if Specified
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
            attrs: args.attrs.clone(),
            event_severity_attribute: args.event_severity_attribute.clone(),
            poll_interval: args.poll_interval,
            session_timeout: args.session_timeout,
            verbose: args.verbose,
            theme: args.theme.clone(),
            color_by: args.color_by,
            events_only: args.events_only,
            trace_timeout: args.trace_timeout,
        }
    };

    // Validate discovery parameters
    if config.log_group_pattern.is_none() && config.stack_name.is_none() {
        return Err(anyhow::anyhow!(
            "Either --log-group-pattern or --stack-name must be provided on the command line or in the configuration profile"
        ));
    }

    // Initialize Logging
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

    // Resolve OTLP Endpoint
    let resolved_endpoint: Option<String> = config.otlp_endpoint.clone().or_else(|| {
        env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
            .ok()
            .or_else(|| env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok())
    });
    let endpoint_opt = resolved_endpoint.as_deref();
    tracing::debug!(config_endpoint = ?config.otlp_endpoint, env_traces = ?env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT").ok(), env_general = ?env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(), resolved = ?resolved_endpoint, "Resolved OTLP endpoint");

    // Resolve OTLP Headers
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
        Vec::new()
    };

    // Post-Resolution Validation
    if config.forward_only && endpoint_opt.is_none() {
        return Err(anyhow::anyhow!("
            --forward-only requires --otlp-endpoint argument or OTEL_EXPORTER_OTLP_TRACES_ENDPOINT/OTEL_EXPORTER_OTLP_ENDPOINT env var to be set"));
    }
    if !config.forward_only && endpoint_opt.is_none() {
        tracing::debug!("Running in console-only mode. No OTLP endpoint configured.");
    }

    // AWS Setup
    let aws_result = setup_aws_resources(
        &config.log_group_pattern,
        &config.stack_name,
        &config.aws_region,
        &config.aws_profile,
    )
    .await?;
    let cwl_client = aws_result.cwl_client;
    let account_id = aws_result.account_id;
    let region_str = aws_result.region_str;
    let resolved_log_group_arns = aws_result.resolved_arns;

    // Setup HTTP Client & Parse Resolved OTLP Headers
    let http_client = ReqwestClient::builder()
        .build()
        .context("Failed to build Reqwest client")?;
    tracing::debug!("Reqwest HTTP client created.");
    let otlp_header_map = parse_otlp_headers_from_vec(&resolved_headers_vec)?;
    let compaction_config = SpanCompactionConfig::default();

    // Prepare Console Display
    let console_enabled = !config.forward_only;
    let attr_globs = parse_attr_globs(&config.attrs);

    // Preamble Output
    let preamble_width: usize = get_terminal_width(80);
    let config_heading = "Livetrace Configuration";
    let config_padding = preamble_width.saturating_sub(config_heading.len() + 3);

    println!("\n");
    println!(
        "{} {} {}\n",
        "─".dimmed(),
        config_heading.bold(),
        "─".repeat(config_padding).dimmed()
    );
    println!("  {:<18}: {}", "AWS Account ID".dimmed(), account_id);
    println!("  {:<18}: {}", "AWS Region".dimmed(), region_str);
    if let Some(profile) = &config.aws_profile {
        println!("  {:<18}: {}", "AWS Profile".dimmed(), profile);
    }
    if let Some(patterns) = &config.log_group_pattern {
        println!("  {:<18}: {:?}", "Pattern".dimmed(), patterns);
    }
    if let Some(stack) = &config.stack_name {
        println!("  {:<18}: {}", "CloudFormation".dimmed(), stack);
    }
    println!();
    if let Some(poll_secs) = config.poll_interval {
        println!("  {:<18}: Polling", "Mode".dimmed());
        println!("  {:<18}: {} seconds", "Poll Interval".dimmed(), poll_secs);
    } else {
        println!("  {:<18}: Live Tail", "Mode".dimmed());
        println!(
            "  {:<18}: {} minutes",
            "Session Timeout".dimmed(),
            config.session_timeout
        );
    }
    println!(
        "  {:<18}: {}",
        "Forward Only".dimmed(),
        if config.forward_only { "Yes" } else { "No" }
    );
    if let Some(endpoint) = &resolved_endpoint {
        println!("  {:<18}: {}", "OTLP Endpoint".dimmed(), endpoint);
    } else {
        println!("  {:<18}: Not configured", "OTLP Endpoint".dimmed());
    }
    if !resolved_headers_vec.is_empty() {
        println!(
            "  {:<18}: {} headers",
            "OTLP Headers".dimmed(),
            resolved_headers_vec.len()
        );
    }
    println!("  {:<18}: {}", "Theme".dimmed(), config.theme);
    println!(
        "  {:<18}: {}",
        "Color By".dimmed(),
        match config.color_by {
            ColoringMode::Service => "Service",
            ColoringMode::Span => "Span ID",
        }
    );
    if let Some(attrs) = &config.attrs {
        println!("  {:<18}: {}", "Attributes".dimmed(), attrs);
    } else {
        println!("  {:<18}: All", "Attributes".dimmed());
    }
    println!(
        "  {:<18}: {}",
        "Severity Attr".dimmed(),
        config.event_severity_attribute
    );
    println!(
        "  {:<18}: {}",
        "Events Only".dimmed(),
        if config.events_only { "Yes" } else { "No" }
    );
    println!(
        "  {:<18}: {} seconds",
        "Trace Timeout".dimmed(),
        config.trace_timeout
    );
    if let Some(profile) = &args.config_profile {
        // Use args here as config doesn't store it
        println!("  {:<18}: {}", "Config Profile".dimmed(), profile);
    }
    let verbosity_str = match config.verbose {
        0 => "Normal",
        1 => "Debug (-v)",
        _ => {
            let v_str = format!("Trace (-v{})", "v".repeat(config.verbose as usize - 1));
            Box::leak(v_str.into_boxed_str())
        }
    };
    println!("  {:<18}: {}", "Verbosity".dimmed(), verbosity_str);
    println!();
    let validated_log_group_names_for_display: Vec<String> = resolved_log_group_arns
        .iter()
        .map(|arn| {
            arn.split(':')
                .next_back()
                .unwrap_or("unknown-name")
                .to_string()
        })
        .collect();
    print!("  {:<18}: ", "Log Groups".dimmed());
    if let Some((first, rest)) = validated_log_group_names_for_display.split_first() {
        println!("{}", first);
        for name in rest {
            println!("{:<22}{}", "", name);
        }
    } else {
        println!("None");
    }
    println!("\n");

    // Create MPSC Channel and Spawn Event Source Task
    let (tx, mut rx) = mpsc::channel::<Result<TelemetryData>>(100);

    if let Some(interval_secs) = config.poll_interval {
        tracing::debug!(
            interval = interval_secs,
            "Using FilterLogEvents polling mode."
        );
        start_polling_task(cwl_client, resolved_log_group_arns, interval_secs, tx);
    } else {
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

    // Main Event Processing Loop
    tracing::debug!("Waiting for telemetry events...");
    let mut trace_buffers: HashMap<String, TraceBufferState> = HashMap::new();
    let mut ticker = interval(Duration::from_secs(1));

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message("Waiting for telemetry events...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    loop {
        tokio::select! {
            received = rx.recv() => {
                match received {
                    Some(Ok(telemetry)) => {
                        match ExportTraceServiceRequest::decode(telemetry.payload.as_slice()) {
                            Ok(request) => {
                                spinner.set_message("Processing telemetry data...");
                                let mut trace_id_hex_opt: Option<String> = None;
                                let mut is_root_present_in_req = false;

                                for resource_span in &request.resource_spans {
                                    for scope_span in &resource_span.scope_spans {
                                        for span in &scope_span.spans {
                                            if trace_id_hex_opt.is_none() {
                                                trace_id_hex_opt = Some(hex::encode(&span.trace_id));
                                            }
                                            if span.parent_span_id.is_empty() {
                                                is_root_present_in_req = true;
                                            }
                                        }
                                    }
                                }

                                if let Some(tid) = trace_id_hex_opt {
                                    let now = Instant::now();
                                    let state = trace_buffers
                                        .entry(tid)
                                        .or_insert_with(|| TraceBufferState {
                                            buffered_payloads: Vec::new(),
                                            has_received_root: false,
                                            first_message_received_at: now,
                                            last_message_received_at: now,
                                        });
                                    state.buffered_payloads.push(telemetry);
                                    state.has_received_root |= is_root_present_in_req;
                                    state.last_message_received_at = now;
                                } else {
                                    tracing::warn!("Received OTLP request with no spans, cannot determine trace ID.");
                                }
                                spinner.set_message("Waiting for telemetry events...");
                            }
                            Err(e) => {
                                spinner.set_message(format!("Error: {}", e));
                                tracing::warn!(error = %e, "Failed to decode OTLP protobuf payload, skipping item.");
                            }
                        }
                    }
                    Some(Err(e)) => {
                        spinner.set_message(format!("Error: {}", e));
                        tracing::error!(error = %e, "Error received from event source task");
                    }
                    None => {
                        spinner.finish_with_message("Event source channel closed");
                        tracing::info!("Event source channel closed. Exiting.");
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                let now = Instant::now();
                let mut trace_ids_to_flush: Vec<String> = Vec::new();

                for (trace_id, state) in trace_buffers.iter() {
                    let time_since_last = now.duration_since(state.last_message_received_at);
                    let time_since_first = now.duration_since(state.first_message_received_at);
                    let should_flush =
                        (state.has_received_root && time_since_last > IDLE_TIMEOUT)
                        || (time_since_first > Duration::from_secs(config.trace_timeout));
                    if should_flush {
                        trace_ids_to_flush.push(trace_id.clone());
                    }
                }

                if !trace_ids_to_flush.is_empty() {
                    let mut batches_to_process: Vec<(String, Vec<TelemetryData>, bool)> = Vec::new();
                    for trace_id in &trace_ids_to_flush {
                        if let Some(state) = trace_buffers.get(trace_id) {
                            batches_to_process.push((
                                trace_id.clone(),
                                state.buffered_payloads.clone(),
                                state.has_received_root,
                            ));
                        }
                    }
                    // Remove flushed traces from buffer *after* collecting data
                    for trace_id in &trace_ids_to_flush {
                        trace_buffers.remove(trace_id);
                    }

                    let mut futures_vec = Vec::new();
                    for (trace_id, payloads_to_process, root_seen) in batches_to_process {
                        spinner.set_message(format!("Flushing trace {}...", &trace_id[..8]));
                        if console_enabled {
                            display_console(
                                // &trace_id, // trace_id is not a direct parameter
                                &payloads_to_process,
                                &attr_globs,
                                &config.event_severity_attribute,
                                config.theme.parse::<Theme>().unwrap_or(Theme::Default),
                                config.color_by,
                                config.events_only,
                                root_seen,
                                // &compaction_config, // compaction_config is not a parameter for display_console
                            )?;
                        }

                        if let Some(endpoint_url) = endpoint_opt {
                            let client_clone = http_client.clone();
                            let endpoint_clone = endpoint_url.to_string();
                            let headers_clone = otlp_header_map.clone();
                            let payloads_clone = payloads_to_process.clone(); // Clone for async task
                            let compaction_config_clone = compaction_config.clone(); // Clone for the async task

                            futures_vec.push(tokio::spawn(async move {
                                send_batch(
                                    &client_clone,
                                    &endpoint_clone,
                                    payloads_clone, // Use cloned payloads
                                    &compaction_config_clone, // Use the cloned config
                                    headers_clone,
                                )
                                .await
                            }));
                        }
                        spinner.set_message("Waiting for telemetry events...");
                    }
                    join_all(futures_vec).await; // Wait for all forwarding tasks to complete
                }
            }
        }
    }
    spinner.finish_and_clear();
    Ok(())
}
