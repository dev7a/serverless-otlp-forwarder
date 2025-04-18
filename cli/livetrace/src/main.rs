use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudwatchlogs::{types::StartLiveTailResponseStream, Client as CwlClient};
use aws_sdk_cloudformation::Client as CfnClient;
use aws_sdk_sts::Client as StsClient;
use chrono::{TimeZone, Utc};
use clap::{Parser, ArgGroup};
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
    tracing::debug!("Logged in AWS config with region: {:?}", aws_config.region());

    // 2. Create CloudWatchLogs client
    let cwl_client = CwlClient::new(&aws_config);
    tracing::debug!("CloudWatch Logs client created.");

    let cfn_client = CfnClient::new(&aws_config);
    tracing::debug!("CloudFormation client created.");

    let sts_client = StsClient::new(&aws_config);
    tracing::debug!("STS client created.");

    // Get Account ID and Region for ARN construction
    let region_str = aws_config.region().ok_or_else(|| anyhow::anyhow!("Could not determine AWS region from config"))?.to_string();
    let caller_identity = sts_client.get_caller_identity().send().await.context("Failed to get caller identity from STS")?;
    let account_id = caller_identity.account().ok_or_else(|| anyhow::anyhow!("Could not determine AWS Account ID from STS caller identity"))?.to_string();
    let partition = "aws"; // Assuming standard AWS partition
    tracing::debug!(region = %region_str, account_id = %account_id, partition = %partition, "Determined region, account ID, and partition");

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

    // 5. Discover Log Groups based on pattern or stack name
    let resolved_log_group_names: Vec<String> = if let Some(stack_name) = args.stack_name.as_deref() {
        tracing::info!("Discovering log groups from stack: '{}'", stack_name);
        let mut discovered_groups = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = cfn_client.list_stack_resources().stack_name(stack_name);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let output = request.send().await.with_context(|| format!("Failed to list resources for stack '{}'", stack_name))?;

            if let Some(summaries) = output.stack_resource_summaries {
                for summary in summaries {
                    if summary.resource_type.as_deref() == Some("AWS::Logs::LogGroup") {
                        if let Some(physical_id) = summary.physical_resource_id {
                            discovered_groups.push(physical_id); // Physical ID is the log group name
                        } else {
                            tracing::warn!(resource_summary = ?summary, "Found LogGroup resource without physical ID");
                        }
                    } else if summary.resource_type.as_deref() == Some("AWS::Lambda::Function") {
                        if let Some(physical_id) = summary.physical_resource_id {
                            let lambda_log_group_name = format!("/aws/lambda/{}", physical_id);
                            tracing::debug!(lambda_function = %physical_id, derived_log_group = %lambda_log_group_name, "Adding derived log group for Lambda function");
                            discovered_groups.push(lambda_log_group_name);
                        } else {
                            tracing::warn!(resource_summary = ?summary, "Found Lambda function resource without physical ID");
                        }
                    }
                }
            }

            if let Some(token) = output.next_token {
                next_token = Some(token);
            } else {
                break; // No more pages
            }
        }
        discovered_groups

    } else if let Some(pattern) = args.log_group_pattern.as_deref() {
        tracing::info!("Discovering log groups matching pattern: '{}'", pattern);
        let describe_output = cwl_client
            .describe_log_groups()
            .log_group_name_pattern(pattern) // Use the pattern string
            .send()
            .await
            .context("Failed to describe log groups")?;

        describe_output
            .log_groups
            .unwrap_or_default() // Handle case where no groups are returned
            .into_iter()
            .filter_map(|lg| lg.log_group_name) // Extract NAME now, not ARN
            // Removed ARN stripping logic
            .collect()
    } else {
        // This case should be prevented by the clap ArgGroup requirement
        return Err(anyhow::anyhow!("Internal error: No log group pattern or stack name provided despite requirement."));
    };

    // Add validation step
    tracing::info!("Validating discovered log group names...");
    let validated_log_group_names = validate_log_groups(&cwl_client, resolved_log_group_names).await?;
    tracing::debug!("Validation complete. Valid names: {:?}", validated_log_group_names);

    // Validate count of *validated* names
    let group_count = validated_log_group_names.len(); // Use validated count
    if group_count == 0 {
        // Adjust error message based on discovery method - now indicates validation failure
        if args.stack_name.is_some() {
             return Err(anyhow::anyhow!("Stack '{}' contained 0 discoverable and valid LogGroup resources (checked Lambda@Edge variants).", args.stack_name.unwrap()));
        } else {
             return Err(anyhow::anyhow!("Pattern '{}' matched 0 valid log groups (checked Lambda@Edge variants).", args.log_group_pattern.unwrap()));
        }
    } else if group_count > 10 {
        // Adjust error message based on discovery method
        let (method, value) = if let Some(stack) = args.stack_name {
            ("Stack", stack)
        } else {
            ("Pattern", args.log_group_pattern.unwrap())
        };
        // Log validated names here
        return Err(anyhow::anyhow!("{} '{}' resulted in {} valid log groups (max 10 allowed for live tail). Found: {:?}",
            method, value, group_count, validated_log_group_names));
    } else {
        // Use validated names in log message
        tracing::info!("Proceeding with {} validated log group name(s): {:?}", group_count, validated_log_group_names);
    }

    // Construct ARNs from *validated* names
    let resolved_log_group_arns: Vec<String> = validated_log_group_names // Use validated names
        .into_iter()
        .map(|name| format!("arn:{}:logs:{}:{}:log-group:{}", partition, region_str, account_id, name))
        .collect();
    tracing::info!("Constructed ARNs: {:?}", resolved_log_group_arns);

    // 6. Start Live Tail stream for resolved groups (using ARNs)
    tracing::info!("Attempting to start Live Tail for resolved log groups...");
    let mut live_tail_output = cwl_client
        .start_live_tail()
        .set_log_group_identifiers(Some(resolved_log_group_arns)) // Use resolved ARNs
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

const SERVICE_NAME_WIDTH: usize = 25; // Added width for service name
const SPAN_NAME_WIDTH: usize = 40; // Reduced span name width
const SPAN_ID_WIDTH: usize = 32; // Width for 16-byte hex span ID

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
    service_name: String,
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
    service_name: String,
}

// Helper to find service.name from resource attributes
fn find_service_name(attrs: &[KeyValue]) -> String {
    attrs.iter().find(|kv| kv.key == "service.name").and_then(|kv| {
        kv.value.as_ref().and_then(|av| {
            if let Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) = &av.value {
                Some(s.clone())
            } else {
                None
            }
        })
    }).unwrap_or_else(|| "<unknown>".to_string())
}

fn display_console(
    batch: &[TelemetryData],
    timeline_width: usize,
    compact_display: bool,
    event_attr_globs: &Option<GlobSet>,
) -> Result<()> {
    // Store tuples of (Span, service_name) initially
    let mut spans_with_service: Vec<(Span, String)> = Vec::new();

    for item in batch {
        match ExportTraceServiceRequest::decode(item.payload.as_slice()) {
            Ok(request) => {
                for resource_span in request.resource_spans {
                    // Find service name for this resource
                    let service_name = find_service_name(resource_span.resource.as_ref().map_or(&[], |r| &r.attributes));
                    for scope_span in resource_span.scope_spans {
                        for span in scope_span.spans {
                            // Store span along with its service name
                            spans_with_service.push((span.clone(), service_name.clone()));
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to decode payload for console display, skipping item.");
                return Ok(());
            }
        }
    }

    // Group spans by trace ID, keeping the service name tuple
    let mut traces: HashMap<String, Vec<(Span, String)>> = HashMap::new();
    for (span, service_name) in spans_with_service { // Iterate through collected tuples
        let trace_id_hex = hex::encode(&span.trace_id);
        traces.entry(trace_id_hex).or_default().push((span, service_name));
    }

    // Process each trace
    for (trace_id, spans_in_trace_with_service) in traces {
        println!("\n--- Trace ID: {} ---", trace_id);

        if spans_in_trace_with_service.is_empty() {
            continue;
        }

        // Collect events *for this trace only*, associating service name
        let mut trace_events: Vec<EventInfo> = Vec::new();
        for (span, service_name) in &spans_in_trace_with_service { // Use the tuples
            let span_id_hex = hex::encode(&span.span_id);
            for event in &span.events {
                trace_events.push(EventInfo {
                    timestamp_ns: event.time_unix_nano,
                    name: event.name.clone(),
                    span_id: span_id_hex.clone(),
                    trace_id: trace_id.clone(), // Use trace_id from the outer loop key
                    attributes: event.attributes.clone(),
                    service_name: service_name.clone(), // Use service name from the tuple
                });
            }
        }
        // Sort events for this trace
        trace_events.sort_by_key(|e| e.timestamp_ns);

        // Step 1: Collect spans and build parent-child map
        // Need both span data and service name available for build_console_span
        let mut span_map: HashMap<String, Span> = HashMap::new();
        let mut service_name_map: HashMap<String, String> = HashMap::new(); // Map span ID to service name
        let mut parent_to_children_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut root_ids: Vec<String> = Vec::new();

        for (span, service_name) in spans_in_trace_with_service { // Use tuples again
            let span_id_hex = hex::encode(&span.span_id);
            span_map.insert(span_id_hex.clone(), span); // Store original span
            service_name_map.insert(span_id_hex.clone(), service_name); // Store service name by span ID
        }

        // Second pass to determine roots and children (unchanged, uses span_map keys)
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
            .map(|root_id| build_console_span(root_id, &span_map, &parent_to_children_map, &service_name_map))
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

                // Construct final log line (simplified format with service name)
                let log_line_start = format!(
                    "{} {} [{}] {}",
                    formatted_time.bright_black(),       // Timestamp (Gray)
                    event.span_id.cyan(),          // Simplified Span ID (Cyan)
                    event.service_name.yellow(),    // Service Name (Yellow)
                    event.name,                      // Event Name (Default color)
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
    service_name_map: &HashMap<String, String>, // Add service name map param
) -> ConsoleSpan {
    let span = span_map.get(span_id).expect("Span ID should exist in map"); // Assume ID is valid
    let service_name = service_name_map.get(span_id).cloned().unwrap_or_else(|| "<unknown>".to_string()); // Get service name

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
        .map(|child_id| build_console_span(child_id, span_map, parent_to_children_map, service_name_map)) // Pass map down
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
        service_name, // Set service name
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

    // Use only indent for hierarchy, prepend service name
    let service_name_content = node.service_name.chars().take(SERVICE_NAME_WIDTH).collect::<String>();

    // Truncate span name, accounting for indent
    let span_name_width = SPAN_NAME_WIDTH.saturating_sub(indent.len());
    let truncated_span_name = node.name.chars().take(span_name_width).collect::<String>();
    let span_name_cell_content = format!("{} {}", indent, truncated_span_name);

    let duration_ms = node.duration_ns as f64 / 1_000_000.0;
    // Format duration simply, no fixed width padding, add color based on status
    let colored_duration = if node.status_code == status::StatusCode::Error {
        format!("{:.2}", duration_ms).red().to_string()
    } else {
        format!("{:.2}", duration_ms).bright_black().to_string() // Use bright black for visibility
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
        table.add_row(row![
            service_name_content, // Service Name
            span_name_cell_content, // Indented Span Name
            colored_duration, // Duration
            bar_cell_content // Timeline
        ]);
    } else {
        // Truncate Span ID if necessary (shouldn't be needed if width is fixed)
        let span_id_content = node.id.chars().take(SPAN_ID_WIDTH).collect::<String>();
        table.add_row(row![
            service_name_content, // Service Name
            span_name_cell_content, // Indented Span Name
            colored_duration, // Duration
            span_id_content, // Span ID
            bar_cell_content // Timeline
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

/// Validates a list of potential log group names, checking for Lambda@Edge variants if necessary.
async fn validate_log_groups(
    cwl_client: &CwlClient,
    initial_names: Vec<String>,
) -> Result<Vec<String>> {
    let checks = initial_names.into_iter().map(|name| {
        let client = cwl_client.clone(); // Clone client for concurrent use
        async move {
            let describe_result = client
                .describe_log_groups()
                .log_group_name_prefix(&name) // Check if the original name exists
                .limit(1)
                .send()
                .await;

            match describe_result {
                Ok(output) => {
                    if output.log_groups.map_or(false, |lgs| lgs.iter().any(|lg| lg.log_group_name.as_deref() == Some(&name))) {
                        tracing::debug!(log_group = %name, "Validated existing log group");
                        Ok(Some(name)) // Original name found and matches
                    } else {
                        // Found something with the prefix, but not the exact name OR empty list
                        check_lambda_edge_variant(&client, name).await // Try edge variant
                    }
                }
                Err(e) => {
                    // Check the underlying error code for ResourceNotFoundException
                    if let Some(err) = e.as_service_error() {
                        match err.meta().code() {
                            Some("ResourceNotFoundException") => {
                                check_lambda_edge_variant(&client, name).await // Try edge variant
                            }
                            _ => {
                                // Other SDK service error - use original error 'e' for context
                                let context_msg = format!("Failed to describe log group '{}' due to SDK service error code: {:?}", name, err.meta().code());
                                Err(anyhow::Error::new(e).context(context_msg))
                            }
                        }
                    } else {
                        // Error is not an SDK service error
                        Err(anyhow::Error::new(e).context(format!("Failed to describe log group '{}'", name)))
                    }
                }
            }
        }
    });

    let results = futures::future::join_all(checks).await;

    let mut validated_names = Vec::new();
    for result in results {
        match result {
            Ok(Some(name)) => validated_names.push(name),
            Ok(None) => {} // Logged within the check, skip
            Err(e) => return Err(e), // Propagate the first error
        }
    }

    Ok(validated_names)
}

/// Helper to check for the Lambda@Edge log group variant.
async fn check_lambda_edge_variant(client: &CwlClient, original_name: String) -> Result<Option<String>> {
    const LAMBDA_PREFIX: &str = "/aws/lambda/";
    if original_name.starts_with(LAMBDA_PREFIX) {
        let base_name = &original_name[LAMBDA_PREFIX.len()..];
        let edge_name = format!("/aws/lambda/us-east-1.{}", base_name);
        tracing::debug!(original_log_group = %original_name, edge_variant = %edge_name, "Original log group not found/matched, checking Lambda@Edge variant");

        match client.describe_log_groups().log_group_name_prefix(&edge_name).limit(1).send().await {
            Ok(output) => {
                 if output.log_groups.map_or(false, |lgs| lgs.iter().any(|lg| lg.log_group_name.as_deref() == Some(&edge_name))) {
                    tracing::info!(log_group = %edge_name, "Found and validated Lambda@Edge log group variant");
                    Ok(Some(edge_name)) // Edge variant found and matches
                 } else {
                     tracing::warn!(log_group = %original_name, edge_variant = %edge_name, "Original log group and Lambda@Edge variant not found/matched. Skipping.");
                     Ok(None) // Neither found
                 }
            }
            Err(e) => {
                 // Check the underlying error code for ResourceNotFoundException
                 if let Some(err) = e.as_service_error() {
                     match err.meta().code() {
                         Some("ResourceNotFoundException") => {
                             tracing::warn!(log_group = %original_name, edge_variant = %edge_name, "Original log group and Lambda@Edge variant not found. Skipping.");
                             Ok(None) // Edge variant also not found
                         }
                         _ => {
                              // Use original error 'e' for context
                              let context_msg = format!("Failed to describe Lambda@Edge variant '{}' due to SDK service error code: {:?}", edge_name, err.meta().code());
                              Err(anyhow::Error::new(e).context(context_msg))
                         }
                     }
                 } else {
                      Err(anyhow::Error::new(e).context(format!("Failed to describe Lambda@Edge variant '{}'", edge_name)))
                 }
            }
        }
    } else {
        tracing::warn!(log_group = %original_name, "Log group not found and does not match Lambda pattern. Skipping.");
        Ok(None) // Original name not found, wasn't a lambda pattern
    }
}
