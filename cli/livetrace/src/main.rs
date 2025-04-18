// Crate Modules
mod aws_setup;
mod cli;
mod console_display;
mod forwarder;
mod processing;

// Standard Library
use std::time::Duration;
use std::env;

// External Crates
use anyhow::{Context, Result};
use aws_sdk_cloudwatchlogs::types::StartLiveTailResponseStream;
use clap::{Parser, ArgGroup};
use colored::*;
use reqwest::Client as ReqwestClient;
use tokio::time::{interval, sleep};
use tokio::pin;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

// Internal Crate Imports
use crate::aws_setup::setup_aws_resources;
use crate::cli::{parse_args, parse_event_attr_globs};
use crate::console_display::display_console;
use crate::forwarder::{parse_otlp_headers_from_vec, send_batch};
use crate::processing::{
    process_log_event_message,
    SpanCompactionConfig,
    TelemetryData,
};

/// livetrace: Tail CloudWatch Logs for OTLP/stdout traces and forward them.
#[derive(Parser, Debug)]
#[command(author = "Dev7A", version, about, long_about = None)]
#[clap(group(
    ArgGroup::new("discovery")
        .required(true)
        .args(["log_group_pattern", "stack_name"]),
))]
struct CliArgs {
    /// Log group name pattern for discovery (case-sensitive substring search).
    #[arg(long = "pattern", group = "discovery")]
    log_group_pattern: Option<String>,

    /// CloudFormation stack name for log group discovery.
    #[arg(long = "stack-name", group = "discovery")]
    stack_name: Option<String>,

    /// The OTLP HTTP endpoint URL to send traces to (e.g., http://localhost:4318/v1/traces).
    #[arg(short = 'e', long)]
    otlp_endpoint: Option<String>,

    /// Add custom HTTP headers to the outgoing OTLP request (e.g., "Authorization=Bearer token"). Can be specified multiple times.
    #[arg(short = 'H', long = "otlp-header")]
    otlp_headers: Vec<String>,

    /// AWS Region to use. Defaults to environment/profile configuration.
    #[arg(short, long)]
    region: Option<String>,

    /// AWS Profile to use. Defaults to environment/profile configuration.
    #[arg(short, long)]
    profile: Option<String>,

    /// Increase logging verbosity (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Only forward telemetry, do not display it in the console.
    #[arg(long)]
    forward_only: bool,

    /// Width of the timeline bar in characters.
    #[arg(long, default_value_t = 80)]
    timeline_width: usize,

    /// Use a compact display format (omits Span ID).
    #[arg(long)]
    compact_display: bool,

    /// Comma-separated list of glob patterns for event attributes to display (e.g., "http.*,db.*,aws.lambda.*").
    #[arg(long)]
    event_attrs: Option<String>,

    /// Session timeout in minutes.
    #[arg(short, long, default_value_t = 10)]
    session_timeout: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Parse Args
    let args = parse_args();

    // 2. Initialize Logging
    let log_level = match args.verbose {
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
    tracing::debug!("Starting livetrace with args: {:?}", args);

    // 3. Resolve OTLP Endpoint (CLI > TRACES_ENV > GENERAL_ENV)
    let resolved_endpoint: Option<String> = args.otlp_endpoint.clone().or_else(|| {
        env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT").ok().or_else(|| {
            env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()
        })
    });
    let endpoint_opt = resolved_endpoint.as_deref();
    tracing::debug!(cli_arg = ?args.otlp_endpoint, env_traces = ?env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT").ok(), env_general = ?env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(), resolved = ?resolved_endpoint, "Resolved OTLP endpoint");

    // 4. Resolve OTLP Headers (CLI > TRACES_ENV > GENERAL_ENV)
    let resolved_headers_vec: Vec<String> = if !args.otlp_headers.is_empty() {
        tracing::debug!(source="cli", headers=?args.otlp_headers, "Using headers from --otlp-header args");
        args.otlp_headers.clone()
    } else if let Ok(hdr_str) = env::var("OTEL_EXPORTER_OTLP_TRACES_HEADERS") {
        let headers: Vec<String> = hdr_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        tracing::debug!(source="env_traces", headers=?headers, "Using headers from OTEL_EXPORTER_OTLP_TRACES_HEADERS");
        headers
    } else if let Ok(hdr_str) = env::var("OTEL_EXPORTER_OTLP_HEADERS") {
        let headers: Vec<String> = hdr_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        tracing::debug!(source="env_general", headers=?headers, "Using headers from OTEL_EXPORTER_OTLP_HEADERS");
        headers
    } else {
        Vec::new() // No headers specified anywhere
    };

    // 5. Post-Resolution Validation
    if args.forward_only && endpoint_opt.is_none() {
        return Err(anyhow::anyhow!("
            --forward-only requires --otlp-endpoint argument or OTEL_EXPORTER_OTLP_TRACES_ENDPOINT/OTEL_EXPORTER_OTLP_ENDPOINT env var to be set"));
    }
    if !args.forward_only && endpoint_opt.is_none() {
        tracing::debug!("Running in console-only mode. No OTLP endpoint configured.");
    }

    // 6. AWS Setup (Config, Clients, Discovery, Validation, ARN Construction)
    let aws_result = setup_aws_resources(&args).await?;
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
    let console_enabled = !args.forward_only;
    let event_attr_globs = parse_event_attr_globs(&args);

    // --- Preamble Output (List Style) --- 
    let preamble_width: usize = 80; // Explicitly usize
    let config_heading = " Livetrace Configuration";
    let config_padding = preamble_width.saturating_sub(config_heading.len() + 3);

    println!("\n");
    println!(" {} {} {}", 
             "─".dimmed(), 
             config_heading.bold(), 
             "─".repeat(config_padding).dimmed()
    );
    println!("  {:<18}: {}", "Account ID".dimmed(), account_id);
    println!("  {:<18}: {}", "Region".dimmed(), region_str);
    // Need validated names for the count/list - let's re-get them from ARNs for simplicity here
    // In a real scenario, might pass validated_names through AwsSetupResult
    let validated_log_group_names_for_display: Vec<String> = resolved_log_group_arns.iter()
        .map(|arn| arn.split(':').last().unwrap_or("unknown-name").to_string())
        .collect();
    println!("  {:<18}: ({})", "Log Groups".dimmed(), validated_log_group_names_for_display.len());
    for name in &validated_log_group_names_for_display {
        println!("{:<20}  - {}", "", name.bright_black());
    }
    println!("\n");
    // --- End Preamble ---

    // 9. Start Live Tail
    tracing::debug!("Attempting to start Live Tail for resolved log groups...");
    let mut live_tail_output = cwl_client // Use the client returned from aws_setup
        .start_live_tail()
        .set_log_group_identifiers(Some(resolved_log_group_arns))
        .log_event_filter_pattern("{ $.__otel_otlp_stdout = * }")
        .send()
        .await
        .context("Failed to start Live Tail")?;
    tracing::debug!("Waiting for Live Tail stream events...");

    // 8. Main Event Loop Setup
    let mut telemetry_buffer: Vec<TelemetryData> = Vec::new();
    let mut ticker = interval(Duration::from_secs(1));
    let timeout_duration = Duration::from_secs(args.session_timeout * 60);
    let timeout_sleep = sleep(timeout_duration); // Create the sleep future
    pin!(timeout_sleep); // Pin the future so it can be used in select!

    tracing::debug!(timeout_minutes = args.session_timeout, "Session timeout set.");

    loop {
        tokio::select! {
            event = live_tail_output.response_stream.recv() => {
                match event? {
                    Some(StartLiveTailResponseStream::SessionStart(start_info)) => {
                        tracing::debug!(
                            "Live Tail session started. Request ID: {}, Session ID: {}",
                            start_info.request_id().unwrap_or("N/A"),
                            start_info.session_id().unwrap_or("N/A")
                        );
                    }
                    Some(StartLiveTailResponseStream::SessionUpdate(update)) => {
                        let log_events = update.session_results();
                        tracing::trace!("Received session update with {} log events.", log_events.len());
                        for log_event in log_events {
                            if let Some(msg) = log_event.message() {
                                match process_log_event_message(msg) {
                                    Ok(Some(telemetry)) => {
                                        tracing::debug!(source = %telemetry.original_source, "Successfully processed log event into TelemetryData.");
                                        if console_enabled || endpoint_opt.is_some() {
                                            telemetry_buffer.push(telemetry);
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        tracing::warn!(message = ?msg, error = %e, "Failed to process potential OTLP/stdout log event");
                                    }
                                }
                            }
                        }
                    }
                    Some(_) => {
                        tracing::warn!("Received unknown or unhandled event from Live Tail stream.");
                    }
                    None => {
                        tracing::info!("Live Tail stream ended (no more events).");
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                if !telemetry_buffer.is_empty() {
                    tracing::debug!("Timer tick: Processing buffer with {} items.", telemetry_buffer.len());
                    let batch_to_send = std::mem::take(&mut telemetry_buffer);

                    if console_enabled {
                        // Call display function from console_display module
                        display_console(&batch_to_send, args.timeline_width, args.compact_display, &event_attr_globs)?;
                    }

                    if let Some(endpoint) = endpoint_opt {
                        // Call send_batch from forwarder module
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
            _ = &mut timeout_sleep => {
                tracing::info!(timeout = args.session_timeout, "Session timeout reached. Exiting.");
                break; // Exit the loop
            }
        }
    }
    tracing::debug!("Live Tail stream finished.");

    // 11. Final Flush
    if !telemetry_buffer.is_empty() {
        tracing::debug!(
            "Flushing remaining {} items from buffer before exiting.",
            telemetry_buffer.len()
        );
        let final_batch = std::mem::take(&mut telemetry_buffer);

        if console_enabled {
            display_console(&final_batch, args.timeline_width, args.compact_display, &event_attr_globs)?;
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
