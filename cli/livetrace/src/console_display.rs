use anyhow::Result;
use chrono::{TimeZone, Utc};
use colored::*;
use globset::GlobSet;
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue},
    trace::v1::{status, Span},
};
use prettytable::{format, row, Table};
use prost::Message;
use std::collections::HashMap;

use crate::processing::TelemetryData; // Need TelemetryData for display_console

// --- Constants ---
const SERVICE_NAME_WIDTH: usize = 25;
const SPAN_NAME_WIDTH: usize = 40;
const SPAN_ID_WIDTH: usize = 32;

// --- Data Structures ---
#[derive(Debug, Clone)]
struct ConsoleSpan {
    id: String,
    parent_id: Option<String>,
    name: String,
    start_time: u64,
    duration_ns: u64,
    children: Vec<ConsoleSpan>,
    status_code: status::StatusCode,
    service_name: String,
}

#[derive(Debug)]
struct EventInfo {
    timestamp_ns: u64,
    name: String,
    span_id: String,
    trace_id: String,
    attributes: Vec<KeyValue>,
    service_name: String,
}

// --- Public Display Function ---
pub fn display_console(
    batch: &[TelemetryData],
    timeline_width: usize,
    compact_display: bool,
    event_attr_globs: &Option<GlobSet>,
) -> Result<()> {
    let mut spans_with_service: Vec<(Span, String)> = Vec::new();

    for item in batch {
        match ExportTraceServiceRequest::decode(item.payload.as_slice()) {
            Ok(request) => {
                for resource_span in request.resource_spans {
                    let service_name = find_service_name(resource_span.resource.as_ref().map_or(&[], |r| &r.attributes));
                    for scope_span in resource_span.scope_spans {
                        for span in scope_span.spans {
                            spans_with_service.push((span.clone(), service_name.clone()));
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to decode payload for console display, skipping item.");
                // Continue processing other items in the batch if possible
            }
        }
    }

    if spans_with_service.is_empty() {
        return Ok(());
    }

    let mut traces: HashMap<String, Vec<(Span, String)>> = HashMap::new();
    for (span, service_name) in spans_with_service {
        let trace_id_hex = hex::encode(&span.trace_id);
        traces.entry(trace_id_hex).or_default().push((span, service_name));
    }

    // Calculate approximate total table width for header ruling
    const DURATION_ESTIMATE: usize = 6; // Approx width for duration
    const SPACING_NON_COMPACT: usize = 4; // Approx spaces between 5 columns
    const SPACING_COMPACT: usize = 2; // Approx spaces between 3 columns

    let total_table_width = if compact_display {
        SERVICE_NAME_WIDTH + SPAN_NAME_WIDTH + DURATION_ESTIMATE + SPACING_COMPACT + timeline_width
    } else {
        SERVICE_NAME_WIDTH + SPAN_NAME_WIDTH + DURATION_ESTIMATE + SPAN_ID_WIDTH + SPACING_NON_COMPACT + timeline_width
    };

    for (trace_id, spans_in_trace_with_service) in traces {
        // --- Print Trace ID Header ---
        let trace_heading = format!("Trace ID: {}", trace_id);
        // Calculate padding based on total table width
        let trace_padding = total_table_width.saturating_sub(trace_heading.len() + 3); // 3 for " ─ " and spaces
        println!("\n {} {} {}", 
                 "─".dimmed(), 
                 trace_heading.bold(), 
                 "─".repeat(trace_padding).dimmed()
        );

        if spans_in_trace_with_service.is_empty() {
            continue;
        }

        let mut trace_events: Vec<EventInfo> = Vec::new();
        for (span, service_name) in &spans_in_trace_with_service {
            let span_id_hex = hex::encode(&span.span_id);
            for event in &span.events {
                trace_events.push(EventInfo {
                    timestamp_ns: event.time_unix_nano,
                    name: event.name.clone(),
                    span_id: span_id_hex.clone(),
                    trace_id: trace_id.clone(),
                    attributes: event.attributes.clone(),
                    service_name: service_name.clone(),
                });
            }
        }
        trace_events.sort_by_key(|e| e.timestamp_ns);

        let mut span_map: HashMap<String, Span> = HashMap::new();
        let mut service_name_map: HashMap<String, String> = HashMap::new();
        let mut parent_to_children_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut root_ids: Vec<String> = Vec::new();

        for (span, service_name) in spans_in_trace_with_service {
            let span_id_hex = hex::encode(&span.span_id);
            span_map.insert(span_id_hex.clone(), span);
            service_name_map.insert(span_id_hex.clone(), service_name);
        }

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

        let mut roots: Vec<ConsoleSpan> = root_ids
            .iter()
            .map(|root_id| build_console_span(root_id, &span_map, &parent_to_children_map, &service_name_map))
            .collect();
        roots.sort_by_key(|s| s.start_time);

        let min_start_time = roots.iter().map(|r| r.start_time).min().unwrap_or(0);
        let max_end_time = span_map.values().map(|s| s.end_time_unix_nano).max().unwrap_or(0);
        let trace_duration_ns = max_end_time.saturating_sub(min_start_time);

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_CLEAN);

        for root in roots {
            add_span_to_table(
                &mut table,
                &root,
                0,
                min_start_time,
                trace_duration_ns,
                timeline_width,
                compact_display,
            )?;
        }
        table.printstd();

        // Display sorted events *for this trace*
        if !trace_events.is_empty() {
            // --- Print Events Header ---
            let events_heading = format!("Events for Trace: {}", trace_id);
            // Calculate padding based on total table width
            let events_padding = total_table_width.saturating_sub(events_heading.len() + 3);
            println!("\n {} {} {}",
                     "─".dimmed(),
                     events_heading.bold(),
                     "─".repeat(events_padding).dimmed()
            );
            for event in trace_events {
                let timestamp = Utc.timestamp_nanos(event.timestamp_ns as i64);
                let formatted_time = timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
                let mut attrs_to_display: Vec<String> = Vec::new();
                if let Some(globs) = event_attr_globs {
                    for attr in &event.attributes {
                        if globs.is_match(&attr.key) {
                            attrs_to_display.push(format_keyvalue(attr));
                        }
                    }
                }

                let log_line_start = format!(
                    "{} {} [{}] {}",
                    formatted_time.bright_black(),
                    event.span_id.cyan(),
                    event.service_name.yellow(),
                    event.name,
                );

                if !attrs_to_display.is_empty() {
                    println!("{} - Attrs: {}", log_line_start, attrs_to_display.join(", "));
                } else {
                    println!("{}", log_line_start);
                }
            }
        }
    }

    Ok(())
}

// --- Private Helper Functions ---

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

fn build_console_span(
    span_id: &str,
    span_map: &HashMap<String, Span>,
    parent_to_children_map: &HashMap<String, Vec<String>>,
    service_name_map: &HashMap<String, String>,
) -> ConsoleSpan {
    let span = span_map.get(span_id).expect("Span ID should exist in map");
    let service_name = service_name_map.get(span_id).cloned().unwrap_or_else(|| "<unknown>".to_string());

    let start_time = span.start_time_unix_nano;
    let end_time = span.end_time_unix_nano;
    let duration_ns = end_time.saturating_sub(start_time);

    let status_code = span.status.as_ref().map_or(status::StatusCode::Unset, |s| {
        status::StatusCode::try_from(s.code).unwrap_or(status::StatusCode::Unset)
    });

    let child_ids = parent_to_children_map.get(span_id).cloned().unwrap_or_default();

    let mut children: Vec<ConsoleSpan> = child_ids
        .iter()
        .map(|child_id| build_console_span(child_id, span_map, parent_to_children_map, service_name_map))
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
        service_name,
    }
}

fn add_span_to_table(
    table: &mut Table,
    node: &ConsoleSpan,
    depth: usize,
    trace_start_time_ns: u64,
    trace_duration_ns: u64,
    timeline_width: usize,
    compact_display: bool,
) -> Result<()> {
    let indent = "  ".repeat(depth);
    let service_name_content = node.service_name.chars().take(SERVICE_NAME_WIDTH).collect::<String>();
    let span_name_width = SPAN_NAME_WIDTH.saturating_sub(indent.len());
    let truncated_span_name = node.name.chars().take(span_name_width).collect::<String>();
    let span_name_cell_content = format!("{} {}", indent, truncated_span_name);

    let duration_ms = node.duration_ns as f64 / 1_000_000.0;
    let colored_duration = if node.status_code == status::StatusCode::Error {
        format!("{:.2}", duration_ms).red().to_string()
    } else {
        format!("{:.2}", duration_ms).bright_black().to_string()
    };

    let bar_cell_content = render_bar(
        node.start_time,
        node.duration_ns,
        trace_start_time_ns,
        trace_duration_ns,
        timeline_width,
        node.status_code,
    );

    if compact_display {
        table.add_row(row![
            service_name_content,
            span_name_cell_content,
            colored_duration,
            bar_cell_content
        ]);
    } else {
        let span_id_content = node.id.chars().take(SPAN_ID_WIDTH).collect::<String>();
        table.add_row(row![
            service_name_content,
            span_name_cell_content,
            colored_duration,
            span_id_content,
            bar_cell_content
        ]);
    }

    let mut children = node.children.clone();
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
        )?;
    }

    Ok(())
}

fn render_bar(
    start_time_ns: u64,
    duration_ns: u64,
    trace_start_time_ns: u64,
    trace_duration_ns: u64,
    timeline_width: usize,
    status_code: status::StatusCode,
) -> String {
    if trace_duration_ns == 0 {
        return " ".repeat(timeline_width);
    }
    let timeline_width_f = timeline_width as f64;
    let offset_ns = start_time_ns.saturating_sub(trace_start_time_ns);
    let offset_fraction = offset_ns as f64 / trace_duration_ns as f64;
    let duration_fraction = duration_ns as f64 / trace_duration_ns as f64;
    let start_char_f = offset_fraction * timeline_width_f;
    let end_char_f = start_char_f + (duration_fraction * timeline_width_f);
    let mut bar = String::with_capacity(timeline_width);
    for i in 0..timeline_width {
        let cell_midpoint = i as f64 + 0.5;
        if cell_midpoint >= start_char_f && cell_midpoint < end_char_f {
            if status_code == status::StatusCode::Error {
                bar.push_str(&'▄'.to_string().red().to_string());
            } else {
                bar.push_str(&'▄'.to_string().truecolor(128, 128, 128).to_string());
            }
        } else {
            bar.push(' ');
        }
    }
    bar
}

fn format_keyvalue(kv: &KeyValue) -> String {
    let value_str = format_anyvalue(&kv.value);
    format!("{}: {}", kv.key.bright_black(), value_str)
}

fn format_anyvalue(av: &Option<AnyValue>) -> String {
    match av {
        Some(any_value) => match &any_value.value {
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) => s.clone(),
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::BoolValue(b)) => b.to_string(),
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i)) => i.to_string(),
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::DoubleValue(d)) => d.to_string(),
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::ArrayValue(_)) => "[array]".to_string(),
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::KvlistValue(_)) => "[kvlist]".to_string(),
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::BytesValue(_)) => "[bytes]".to_string(),
            None => "<empty_value>".to_string(),
        },
        None => "<no_value>".to_string(),
    }
} 