use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudwatchlogs::{types::StartLiveTailResponseStream, Client as CwlClient};
use chrono::{TimeZone, Utc};
use clap::Parser;
use colored::*;
use globset::{Glob, GlobSet, GlobSetBuilder};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue},
    trace::v1::{status, Span},
};
use prettytable::{format, row, Table};
use prost::Message;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client as ReqwestClient,
};
use std::{collections::HashMap, str::FromStr, time::Duration};
// For interval timer
use tokio::time::interval; // For interval timer
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter}; // Added common types

mod processing; // Declare the module
use processing::{
    compact_telemetry_payloads, process_log_event_message, send_telemetry_payload,
    SpanCompactionConfig, TelemetryData,
};

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Validate args
    if args.forward_only && args.otlp_endpoint.is_none() {
        return Err(anyhow::anyhow!(
            "--forward-only requires --otlp-endpoint to be set"
        ));
    }
    if !args.forward_only && args.otlp_endpoint.is_none() {
        tracing::info!("Running in console-only mode. No OTLP endpoint provided.");
    }

    // Parse event attribute globs if provided
    let event_attr_globs: Option<GlobSet> = match args.event_attrs.as_deref() {
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
    };

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
                .with_default_directive(LevelFilter::WARN.into())
                .parse_lossy(format!("{}={}", env!("CARGO_PKG_NAME"), log_level))
        )
        .init();

    tracing::info!("Starting livetrace with args: {:?}", args);

    // 1. Load AWS Config
    let region_provider =
        RegionProviderChain::first_try(args.region.clone().map(aws_config::Region::new))
            .or_default_provider()
            .or_else(aws_config::Region::new("us-east-1")); // Default fallback region

    let mut config_loader =
        aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_provider);

    if let Some(profile) = args.profile.clone() {
        config_loader = config_loader.profile_name(profile);
    }

    let aws_config = config_loader.load().await;
    tracing::debug!("Loaded AWS config with region: {:?}", aws_config.region());

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
            let header_value = HeaderValue::from_str(parts[1]).with_context(|| {
                format!("Invalid OTLP header value for {}: {}", parts[0], parts[1])
            })?;
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
                        display_console(&batch_to_send, args.timeline_width, args.compact_display, &event_attr_globs)?;
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
            display_console(
                &final_batch,
                args.timeline_width,
                args.compact_display,
                &event_attr_globs,
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

/// Helper function to send a batch of telemetry data, handling compaction.
async fn send_batch(
    http_client: &ReqwestClient,
    endpoint: &str,
    batch: Vec<TelemetryData>,
    compaction_config: &SpanCompactionConfig,
    headers: HeaderMap,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    if batch.len() == 1 {
        // Only one item, just compress and send
        let telemetry = batch.into_iter().next().unwrap(); // batch is not empty checked above
        tracing::debug!("Sending single item (compressing first)...");
        match processing::compress_payload(&telemetry.payload, compaction_config.compression_level)
        {
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
                    headers.clone(),    // Clone headers for each request
                )
                .await
                {
                    tracing::error!(source = %telemetry.original_source, "Failed to send individual telemetry payload: {}", e);
                    // Log and continue
                }
            }
            Err(e) => {
                tracing::error!(source = %telemetry.original_source, "Failed to compress single telemetry payload: {}", e);
                // Skip sending this item
            }
        }
    } else {
        // Multiple items, compact (merge + compress) and send
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
                    // Log and continue (don't make fatal for now)
                }
            }
            Err(e) => {
                tracing::error!("Failed to compact telemetry batch: {}", e);
                // Don't send if compaction failed
            }
        }
    }
    Ok(())
}

// --- Console Display Implementation ---

const SPAN_NAME_WIDTH: usize = 60;
const SPAN_ID_WIDTH: usize = 32;

#[derive(Debug, Clone)]
struct ConsoleSpan {
    id: String,
    #[allow(dead_code)] // Allow unused field for now
    parent_id: Option<String>,
    name: String,
    start_time: u64,
    duration_ns: u64,
    children: Vec<ConsoleSpan>,
    status_code: status::StatusCode,
}

// Structure to hold extracted event info for later display
#[derive(Debug)] // Added Debug for potential inspection
struct EventInfo {
    timestamp_ns: u64,
    name: String,
    span_id: String,
    #[allow(dead_code)] // Allow unused field for now
    trace_id: String,
    attributes: Vec<KeyValue>,
}

fn display_console(
    batch: &[TelemetryData],
    timeline_width: usize,
    compact_display: bool,
    event_attr_globs: &Option<GlobSet>,
) -> Result<()> {
    let mut all_spans = Vec::new();

    for item in batch {
        match ExportTraceServiceRequest::decode(item.payload.as_slice()) {
            Ok(request) => {
                for resource_span in request.resource_spans {
                    for scope_span in resource_span.scope_spans {
                        for span in scope_span.spans {
                            all_spans.push(span.clone()); // Clone span for trace grouping
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

    // Group spans by trace ID (using the cloned spans)
    let mut traces: HashMap<String, Vec<Span>> = HashMap::new();
    for span in all_spans {
        let trace_id_hex = hex::encode(&span.trace_id);
        traces.entry(trace_id_hex).or_default().push(span);
    }

    // Process each trace
    for (trace_id, spans) in traces {
        println!("\n--- Trace ID: {} ---", trace_id);

        if spans.is_empty() {
            continue;
        }

        // Collect events *for this trace only*
        let mut trace_events: Vec<EventInfo> = Vec::new();
        for span in &spans {
            let span_id_hex = hex::encode(&span.span_id);
            let trace_id_hex = hex::encode(&span.trace_id);
            for event in &span.events {
                trace_events.push(EventInfo {
                    timestamp_ns: event.time_unix_nano,
                    name: event.name.clone(),
                    span_id: span_id_hex.clone(),
                    trace_id: trace_id_hex.clone(),
                    attributes: event.attributes.clone(),
                });
            }
        }
        // Sort events for this trace
        trace_events.sort_by_key(|e| e.timestamp_ns);

        // Step 1: Collect spans and build parent-child map (using original spans for structure)
        let mut span_map: HashMap<String, Span> = HashMap::new();
        let mut parent_to_children_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut root_ids: Vec<String> = Vec::new();

        for span in spans {
            let span_id_hex = hex::encode(&span.span_id);
            span_map.insert(span_id_hex.clone(), span); // Keep original span temporarily
        }

        // Second pass to determine roots and children, now that span_map is complete
        for (span_id_hex, span) in &span_map {
            let parent_id_hex = if span.parent_span_id.is_empty() {
                None
            } else {
                Some(hex::encode(&span.parent_span_id))
            };

            match parent_id_hex {
                Some(ref p_id) if span_map.contains_key(p_id) => {
                    parent_to_children_map
                        .entry(p_id.clone())
                        .or_default()
                        .push(span_id_hex.clone());
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
        let max_end_time = span_map
            .values()
            .map(|s| s.end_time_unix_nano)
            .max()
            .unwrap_or(0);
        let trace_duration_ns = max_end_time.saturating_sub(min_start_time);

        // Print table
        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_CLEAN); // Remove borders/lines

        for root in roots {
            add_span_to_table(
                &mut table,
                &root,
                0,
                min_start_time,
                trace_duration_ns,
                timeline_width,
                compact_display,
                event_attr_globs,
            )?;
        }
        table.printstd();

        // Display sorted events *for this trace*
        if !trace_events.is_empty() {
            println!("\n--- Events for Trace: {} ---", trace_id);
            for event in trace_events {
                // Format timestamp
                let timestamp = Utc.timestamp_nanos(event.timestamp_ns as i64);
                let formatted_time = timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
                // Filter and format attributes if globs are provided
                let mut attrs_to_display: Vec<String> = Vec::new();
                if let Some(globs) = event_attr_globs {
                    // Use the passed globs
                    for attr in &event.attributes {
                        // Iterate through event attributes
                        if globs.is_match(&attr.key) {
                            // Check if key matches any glob
                            attrs_to_display.push(format_keyvalue(attr));
                        }
                    }
                }

                // Construct final log line (simplified format)
                let log_line_start = format!(
                    "{} {} {}",
                    formatted_time.bright_black(), // Timestamp (Gray)
                    event.span_id.cyan(),          // Simplified Span ID (Cyan)
                    event.name                     // Event Name (Default color)
                );

                if !attrs_to_display.is_empty() {
                    println!(
                        "{} - Attrs: {}",
                        log_line_start,
                        attrs_to_display.join(", ")
                    );
                } else {
                    println!("{}", log_line_start); // Print without attributes if none match/selected
                }
            }
        }
    }

    Ok(())
}

// Step 2 (Helper): Recursively build ConsoleSpan tree
fn build_console_span(
    span_id: &str,
    span_map: &HashMap<String, Span>,
    parent_to_children_map: &HashMap<String, Vec<String>>,
) -> ConsoleSpan {
    let span = span_map.get(span_id).expect("Span ID should exist in map"); // Assume ID is valid

    let start_time = span.start_time_unix_nano;
    let end_time = span.end_time_unix_nano;
    let duration_ns = end_time.saturating_sub(start_time);

    // Extract status code, default to Unset if status is None
    let status_code = span.status.as_ref().map_or(status::StatusCode::Unset, |s| {
        status::StatusCode::try_from(s.code).unwrap_or(status::StatusCode::Unset)
    });

    let child_ids = parent_to_children_map
        .get(span_id)
        .cloned()
        .unwrap_or_default();

    let mut children: Vec<ConsoleSpan> = child_ids
        .iter()
        .map(|child_id| build_console_span(child_id, span_map, parent_to_children_map))
        .collect();

    children.sort_by_key(|c| c.start_time);

    ConsoleSpan {
        id: hex::encode(&span.span_id),
        parent_id: if span.parent_span_id.is_empty() {
            None
        } else {
            Some(hex::encode(&span.parent_span_id))
        },
        name: span.name.clone(),
        start_time,
        duration_ns,
        children,
        status_code,
    }
}

// Recursive helper to add spans to the prettytable
#[allow(clippy::too_many_arguments)]
fn add_span_to_table(
    table: &mut Table,
    node: &ConsoleSpan,
    depth: usize,
    trace_start_time_ns: u64,
    trace_duration_ns: u64,
    timeline_width: usize,
    compact_display: bool,
    event_attr_globs: &Option<GlobSet>,
) -> Result<()> {
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
    let bar_cell_content = render_bar(
        node.start_time,
        node.duration_ns,
        trace_start_time_ns,
        trace_duration_ns,
        timeline_width,
        node.status_code,
    );

    // Conditionally build the row
    if compact_display {
        table.add_row(row![name_cell_content, colored_duration, bar_cell_content]);
    } else {
        // Truncate Span ID if necessary (shouldn't be needed if width is fixed)
        let span_id_content = node.id.chars().take(SPAN_ID_WIDTH).collect::<String>();
        table.add_row(row![
            name_cell_content,
            colored_duration,
            span_id_content,
            bar_cell_content
        ]);
    }

    // Sort children by start time before recursing
    let mut children = node.children.clone(); // Clone to sort
    children.sort_by_key(|c| c.start_time);

    for child in &children {
        add_span_to_table(
            table,
            child,
            depth + 1,
            trace_start_time_ns,
            trace_duration_ns,
            timeline_width,
            compact_display,
            event_attr_globs,
        )?;
    }

    Ok(())
}

// Render bar with offset and duration using simple full blocks and color
fn render_bar(
    start_time_ns: u64,
    duration_ns: u64,
    trace_start_time_ns: u64,
    trace_duration_ns: u64,
    timeline_width: usize,
    status_code: status::StatusCode,
) -> String {
    if trace_duration_ns == 0 {
        return " ".repeat(timeline_width); // Return empty bar
    }

    let timeline_width_f = timeline_width as f64;

    // Calculate fractions
    let offset_ns = start_time_ns.saturating_sub(trace_start_time_ns);
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
                bar.push_str(&'▄'.to_string().truecolor(128, 128, 128).to_string());
                // Grayscale
            }
        } else {
            bar.push(' '); // Use space
        }
    }

    bar
}

// Helper to format a KeyValue pair into \"key: value\" string with colored key
fn format_keyvalue(kv: &KeyValue) -> String {
    let value_str = format_anyvalue(&kv.value);
    format!("{}: {}", kv.key.bright_black(), value_str) // Added space after colon
}

// Helper to format the inner AnyValue enum into a string representation
fn format_anyvalue(av: &Option<AnyValue>) -> String {
    match av {
        Some(any_value) => match &any_value.value {
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) => {
                s.clone()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::BoolValue(b)) => {
                b.to_string()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i)) => {
                i.to_string()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::DoubleValue(d)) => {
                d.to_string()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::ArrayValue(_)) => {
                "[array]".to_string()
            } // Placeholder
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::KvlistValue(_)) => {
                "[kvlist]".to_string()
            } // Placeholder
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::BytesValue(_)) => {
                "[bytes]".to_string()
            } // Placeholder
            None => "<empty_value>".to_string(),
        },
        None => "<no_value>".to_string(),
    }
}
