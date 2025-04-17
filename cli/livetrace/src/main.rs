use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudwatchlogs::types::StartLiveTailResponseStream;
use aws_sdk_cloudwatchlogs::Client as CwlClient;
use clap::Parser;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client as ReqwestClient;
use std::str::FromStr;
use std::time::Duration; // For interval timer
use tokio::time::interval; // For interval timer
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod processing; // Declare the module
use processing::{compact_telemetry_payloads, send_telemetry_payload, SpanCompactionConfig, TelemetryData, process_log_event_message};

/// livetrace: Tail CloudWatch Logs for OTLP/stdout traces and forward them.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// The CloudWatch Log Group name(s) to tail. Can be specified multiple times.
    #[arg(short = 'g', long, required = true)]
    log_group_name: Vec<String>,

    /// The OTLP HTTP endpoint URL to send traces to (e.g., http://localhost:4318/v1/traces).
    #[arg(short = 'e', long, required = true)]
    otlp_endpoint: String,

    /// Add custom HTTP headers to the outgoing OTLP request (e.g., "Authorization=Bearer token"). Can be specified multiple times.
    #[arg(short = 'H', long = "otlp-header")]
    otlp_headers: Vec<String>,

    /// Compact multiple OTLP messages into a single outgoing HTTP request.
    #[arg(long)]
    compact: bool,

    /// AWS Region to use. Defaults to environment/profile configuration.
    #[arg(short, long)]
    region: Option<String>,

    /// AWS Profile to use. Defaults to environment/profile configuration.
    #[arg(short, long)]
    profile: Option<String>,

    /// Increase logging verbosity (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Initialize logging
    let log_level = match args.verbose {
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(log_level.into())
                .from_env_lossy(),
        )
        .init();

    tracing::info!("Starting livetrace with args: {:?}", args);
    tracing::info!("Starting livetrace with args: {:?}", args);

    // 1. Load AWS Config
    let region_provider = RegionProviderChain::first_try(args.region.clone().map(aws_config::Region::new))
        .or_default_provider()
        .or_else(aws_config::Region::new("us-east-1")); // Default fallback region

    let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider);

    if let Some(profile) = args.profile.clone() {
        config_loader = config_loader.profile_name(profile);
    }

    let aws_config = config_loader.load().await;
    tracing::info!(
        "Loaded AWS config with region: {:?}",
        aws_config.region()
    );

    // 2. Create CloudWatchLogs client
    let cwl_client = CwlClient::new(&aws_config);
    tracing::debug!("CloudWatch Logs client created.");

    // 3. Create Reqwest client
    let http_client = ReqwestClient::builder()
        .build()
        .context("Failed to build Reqwest client")?;
    tracing::debug!("Reqwest HTTP client created.");

    // 4. Parse OTLP headers
    let mut otlp_header_map = HeaderMap::new();
    for header_str in &args.otlp_headers {
        let parts: Vec<&str> = header_str.splitn(2, '=').collect();
        if parts.len() == 2 {
            let header_name = HeaderName::from_str(parts[0])
                .with_context(|| format!("Invalid OTLP header name: {}", parts[0]))?;
            let header_value = HeaderValue::from_str(parts[1])
                .with_context(|| format!("Invalid OTLP header value for {}: {}", parts[0], parts[1]))?;
            otlp_header_map.insert(header_name, header_value);
        } else {
            tracing::warn!(
                "Ignoring malformed OTLP header (expected Key=Value): {}",
                header_str
            );
        }
    }
    if !otlp_header_map.is_empty() {
        tracing::debug!("Parsed OTLP headers: {:?}", otlp_header_map);
    }

    // 5. Start Live Tail stream
    tracing::info!(
        "Attempting to start Live Tail for log groups: {:?}",
        args.log_group_name
    );
    let mut live_tail_output = cwl_client
        .start_live_tail()
        .set_log_group_identifiers(Some(args.log_group_name.clone()))
        .log_event_filter_pattern("{ $.__otel_otlp_stdout = * }")
        .send()
        .await
        .context("Failed to start Live Tail")?;

    tracing::info!("Waiting for Live Tail stream events...");

    let mut telemetry_buffer: Vec<TelemetryData> = Vec::new();
    let mut ticker = interval(Duration::from_secs(1));
    let compaction_config = SpanCompactionConfig::default();

    loop {
        tokio::select! {
            event = live_tail_output.response_stream.recv() => {
                match event? {
                    Some(StartLiveTailResponseStream::SessionStart(start_info)) => {
                        tracing::info!(
                            "Live Tail session started. Request ID: {:?}, Session ID: {:?}",
                            start_info.request_id(),
                            start_info.session_id()
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
                                        telemetry_buffer.push(telemetry);
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
                    send_batch(
                        &http_client,
                        &args.otlp_endpoint,
                        batch_to_send,
                        args.compact,
                        &compaction_config,
                        otlp_header_map.clone(),
                    ).await?;
                }
            }
        }
    }

    tracing::info!("Live Tail stream finished.");

    // 7. Final Flush: Send any remaining data in the buffer
    if !telemetry_buffer.is_empty() {
        tracing::info!(
            "Flushing remaining {} items from buffer before exiting.",
            telemetry_buffer.len()
        );
        send_batch(
            &http_client,
            &args.otlp_endpoint,
            telemetry_buffer, // Send the final buffer contents
            args.compact,
            &compaction_config,
            otlp_header_map.clone(),
        )
        .await?;
    }

    tracing::info!("livetrace finished successfully.");
    Ok(())
}


/// Helper function to send a batch of telemetry data, handling compaction.
async fn send_batch(
    http_client: &ReqwestClient,
    endpoint: &str,
    batch: Vec<TelemetryData>,
    compact: bool,
    compaction_config: &SpanCompactionConfig,
    headers: HeaderMap,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    if compact {
        tracing::debug!("Compacting batch of {} items...", batch.len());
        match compact_telemetry_payloads(batch, compaction_config) {
            Ok(compacted_data) => {
                tracing::info!(
                    "Sending compacted batch ({} bytes) to {}",
                    compacted_data.payload.len(),
                    endpoint
                );
                if let Err(e) = send_telemetry_payload(
                    http_client,
                    endpoint,
                    compacted_data.payload, // Already compressed by compact_telemetry_payloads
                    headers,
                )
                .await
                {
                    tracing::error!("Failed to send compacted batch: {}", e);
                    // Decide if this should be a fatal error for the whole app
                    // For now, just log it. Consider adding retry logic later.
                    // return Err(e); // Uncomment to make send errors fatal
                }
            }
            Err(e) => {
                tracing::error!("Failed to compact telemetry batch: {}", e);
                // Don't send if compaction failed
            }
        }
    } else {
        tracing::debug!("Sending batch of {} items individually...", batch.len());
        for telemetry in batch {
             // Need to compress individual payloads if not compacting
             match processing::compress_payload(&telemetry.payload, compaction_config.compression_level) {
                Ok(compressed_payload) => {
                    tracing::info!(
                        "Sending individual payload ({} bytes) from source '{}' to {}",
                        compressed_payload.len(),
                        telemetry.original_source,
                        endpoint
                    );
                     if let Err(e) = send_telemetry_payload(
                        http_client,
                        endpoint,
                        compressed_payload, // Send compressed payload
                        headers.clone(), // Clone headers for each request
                    )
                    .await {
                         tracing::error!(source = %telemetry.original_source, "Failed to send individual telemetry payload: {}", e);
                         // Log and continue with the next item
                     }
                }
                Err(e) => {
                     tracing::error!(source = %telemetry.original_source, "Failed to compress individual telemetry payload: {}", e);
                     // Skip sending this item
                }
            }
        }
    }
    Ok(())
}
