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
use std::collections::HashMap;
use prost::Message;
use opentelemetry_proto::tonic::trace::v1::Span;
use opentelemetry_proto::tonic::trace::v1::status;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use hex;
use prettytable::{Table, Row, Cell, row, cell}; // Added prettytable imports
use prettytable::format; // Added for format constants
use colored::*; // Added colored import

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
    #[arg(short = 'e', long)]
    otlp_endpoint: Option<String>,

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

    /// Only forward telemetry, do not display it in the console.
    #[arg(long)]
    forward_only: bool,

    /// Width of the timeline bar in characters.
    #[arg(long, default_value_t = 80)]
    timeline_width: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Validate args
    if args.forward_only && args.otlp_endpoint.is_none() {
        return Err(anyhow::anyhow!("--forward-only requires --otlp-endpoint to be set"));
    }
    if !args.forward_only && args.otlp_endpoint.is_none() {
        tracing::info!("Running in console-only mode. No OTLP endpoint provided.");
    }

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

    // Determine operation mode
    let console_enabled = !args.forward_only;
    let endpoint_opt = args.otlp_endpoint.as_deref(); // Option<&str>

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
                                        // Don't buffer if only forwarding and endpoint is missing (validated earlier)
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
                        display_console(&batch_to_send, args.timeline_width)?;
                    }

                    if let Some(endpoint) = endpoint_opt {
                        send_batch(
                            &http_client,
                            endpoint,
                            batch_to_send,
                            args.compact,
                            &compaction_config,
                            otlp_header_map.clone(),
                        ).await?;
                    }
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
        let final_batch = std::mem::take(&mut telemetry_buffer);

        if console_enabled {
            display_console(&final_batch, args.timeline_width)?;
        }

        if let Some(endpoint) = endpoint_opt {
            send_batch(
                &http_client,
                endpoint,
                final_batch,
                args.compact,
                &compaction_config,
                otlp_header_map.clone(),
            ).await?;
        }
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

// --- Console Display Implementation ---

const SPAN_NAME_WIDTH: usize = 60;
const DURATION_WIDTH: usize = 10; // Width for "xx.xx ms" right-aligned
const TIMELINE_WIDTH: usize = 40; // Must match CONSOLE_BAR_WIDTH
const CONSOLE_BAR_WIDTH: f64 = TIMELINE_WIDTH as f64;

#[derive(Debug, Clone)]
struct ConsoleSpan {
    parent_id: Option<String>,
    name: String,
    start_time: u64,
    duration_ns: u64,
    children: Vec<ConsoleSpan>,
    status_code: status::StatusCode,
}

fn display_console(batch: &[TelemetryData], timeline_width: usize) -> Result<()> {
    let mut all_spans = Vec::new();
    for item in batch {
        match ExportTraceServiceRequest::decode(item.payload.as_slice()) {
            Ok(request) => {
                for resource_span in request.resource_spans {
                    for scope_span in resource_span.scope_spans {
                        for span in scope_span.spans {
                            all_spans.push(span);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to decode payload for console display, skipping item.");
            }
        }
    }

    if all_spans.is_empty() {
        return Ok(());
    }

    // Group spans by trace ID
    let mut traces: HashMap<String, Vec<Span>> = HashMap::new();
    for span in all_spans {
        let trace_id_hex = hex::encode(&span.trace_id);
        traces.entry(trace_id_hex).or_default().push(span);
    }

    // Process each trace
    for (trace_id, spans) in traces {
        println!("--- Trace ID: {} ---", trace_id);

        if spans.is_empty() { continue; }

        // Step 1: Collect spans and build parent-child map
        let mut span_map: HashMap<String, Span> = HashMap::new();
        let mut parent_to_children_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut root_ids: Vec<String> = Vec::new();

        for span in spans {
            let span_id_hex = hex::encode(&span.span_id);
            span_map.insert(span_id_hex.clone(), span); // Keep original span temporarily
        }

        // Second pass to determine roots and children, now that span_map is complete
        for (span_id_hex, span) in &span_map {
            let parent_id_hex = if span.parent_span_id.is_empty() { None } else { Some(hex::encode(&span.parent_span_id)) };

            match parent_id_hex {
                Some(ref p_id) if span_map.contains_key(p_id) => {
                    parent_to_children_map.entry(p_id.clone()).or_default().push(span_id_hex.clone());
                }
                _ => {
                    root_ids.push(span_id_hex.clone());
                }
            }
        }

        // Step 2 & 3: Recursively build the tree and sort roots
        let mut roots: Vec<ConsoleSpan> = root_ids
            .iter()
            .map(|root_id| build_console_span(root_id, &span_map, &parent_to_children_map))
            .collect();

        roots.sort_by_key(|s| s.start_time);

        // Calculate overall trace duration AFTER building the tree (needed for render_bar)
        let min_start_time = roots.iter().map(|r| r.start_time).min().unwrap_or(0);
        // Need max end time - traverse tree or recalculate from original spans
        let max_end_time = span_map.values().map(|s| s.end_time_unix_nano).max().unwrap_or(0);
        let trace_duration_ns = if max_end_time >= min_start_time { max_end_time - min_start_time } else { 0 };

        // Print table
        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_CLEAN); // Remove borders/lines

        for root in roots {
            add_span_to_table(&mut table, &root, 0, min_start_time, trace_duration_ns, timeline_width);
        }
        table.printstd();
    }

    Ok(())
}

// Step 2 (Helper): Recursively build ConsoleSpan tree
fn build_console_span(span_id: &str, span_map: &HashMap<String, Span>, parent_to_children_map: &HashMap<String, Vec<String>>) -> ConsoleSpan {
    let span = span_map.get(span_id).expect("Span ID should exist in map"); // Assume ID is valid

    let start_time = span.start_time_unix_nano;
    let end_time = span.end_time_unix_nano;
    let duration_ns = if end_time >= start_time { end_time - start_time } else { 0 };

    // Extract status code, default to Unset if status is None
    let status_code = span.status.as_ref().map_or(status::StatusCode::Unset, |s| status::StatusCode::try_from(s.code).unwrap_or(status::StatusCode::Unset));

    let child_ids = parent_to_children_map.get(span_id).cloned().unwrap_or_default();

    let mut children: Vec<ConsoleSpan> = child_ids
        .iter()
        .map(|child_id| build_console_span(child_id, span_map, parent_to_children_map))
        .collect();

    children.sort_by_key(|c| c.start_time);

    ConsoleSpan {
        parent_id: if span.parent_span_id.is_empty() { None } else { Some(hex::encode(&span.parent_span_id)) },
        name: span.name.clone(),
        start_time,
        duration_ns,
        children,
        status_code,
    }
}

// Recursive helper to add spans to the prettytable
fn add_span_to_table(table: &mut Table, node: &ConsoleSpan, depth: usize, trace_start_time_ns: u64, trace_duration_ns: u64, timeline_width: usize) {
    let indent = "  ".repeat(depth); // Two spaces per depth level

    // Use only indent for hierarchy
    // Truncate node.name first to fit remaining width after indentation
    let name_width = SPAN_NAME_WIDTH.saturating_sub(indent.len());
    let truncated_name = node.name.chars().take(name_width).collect::<String>();
    let name_cell_content = format!("{}{}", indent, truncated_name);

    let duration_ms = node.duration_ns as f64 / 1_000_000.0;
    // Format duration simply, no fixed width padding, add color based on status
    let duration_cell_content = format!("{:.2}", duration_ms);
    let colored_duration = if node.status_code == status::StatusCode::Error {
        duration_cell_content.red().to_string()
    } else {
        duration_cell_content.bright_black().to_string() // Use bright black for visibility
    };

    // Render bar with offset, passing the timeline_width argument and status
    let bar_cell_content = render_bar(node.start_time, node.duration_ns, trace_start_time_ns, trace_duration_ns, timeline_width, node.status_code);

    // Add row
    table.add_row(row![
        name_cell_content,
        colored_duration,
        bar_cell_content
    ]);

    // Sort children by start time before recursing
    let mut children = node.children.clone(); // Clone to sort
    children.sort_by_key(|c| c.start_time);

    for child in &children {
        add_span_to_table(table, child, depth + 1, trace_start_time_ns, trace_duration_ns, timeline_width);
    }
}

// Render bar with offset and duration using simple full blocks and color
fn render_bar(start_time_ns: u64, duration_ns: u64, trace_start_time_ns: u64, trace_duration_ns: u64, timeline_width: usize, status_code: status::StatusCode) -> String {
    if trace_duration_ns == 0 {
        return " ".repeat(timeline_width); // Return empty bar
    }

    let timeline_width_f = timeline_width as f64;

    // Calculate fractions
    let offset_ns = if start_time_ns >= trace_start_time_ns { start_time_ns - trace_start_time_ns } else { 0 };
    let offset_fraction = offset_ns as f64 / trace_duration_ns as f64;
    let duration_fraction = duration_ns as f64 / trace_duration_ns as f64;

    // Calculate start and end points as character positions
    let start_char_f = offset_fraction * timeline_width_f;
    let end_char_f = start_char_f + (duration_fraction * timeline_width_f);

    let mut bar = String::with_capacity(timeline_width);

    for i in 0..timeline_width {
        let cell_midpoint = i as f64 + 0.5;
        // Check if the midpoint of the cell falls within the span's range
        if cell_midpoint >= start_char_f && cell_midpoint < end_char_f {
            // Color based on status
            if status_code == status::StatusCode::Error {
                bar.push_str(&'▄'.to_string().red().to_string());
            } else {
                bar.push_str(&'▄'.to_string().truecolor(128, 128, 128).to_string()); // Grayscale
            }
        } else {
            bar.push(' '); // Use space
        }
    }

    bar
}
