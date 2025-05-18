//! Handles the rendering of trace and event data to the console.
//!
//! This module is responsible for:
//! - Defining color themes and palettes.
//! - Structuring trace data into a hierarchical, readable format (waterfall view).
//! - Formatting individual spans and events with appropriate colors and indentation.
//! - Generating a timeline scale for trace visualization.
//! - Displaying span attributes and event attributes, with optional filtering.
//! - Managing terminal width for responsive output.

use crate::cli::ColoringMode;
use crate::processing::TelemetryData;
use anyhow::Result;
use chrono::{TimeZone, Utc};
use colored::*;
use comfy_table::{
    presets, Attribute, Cell, CellAlignment, Color as TableColor, ColumnConstraint,
    ContentArrangement, Table, TableComponent, Width::Fixed,
};
use globset::GlobSet;
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{any_value::Value as ProtoValue, AnyValue, KeyValue},
    trace::v1::{status, Span},
};
use prost::Message;
use regex::Regex;
use std::collections::HashMap;
use terminal_size::{self, Height, Width};

// Add clap::ValueEnum and serde imports
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

// Constants
const SERVICE_NAME_WIDTH: usize = 25;
const SPAN_NAME_WIDTH: usize = 40;
const SPAN_ID_WIDTH: usize = 10;
const SPAN_KIND_WIDTH: usize = 10; // Width for the Span Kind column
const STATUS_WIDTH: usize = 9; // Width for the Status column
const DURATION_WIDTH: usize = 13; // Width for the Duration column

// Define all color palettes
// Default color palette
const DEFAULT_COLORS: [(u8, u8, u8); 12] = [
    (46, 134, 193), // Blue
    (142, 68, 173), // Purple
    (39, 174, 96),  // Green
    (41, 128, 185), // Medium Blue
    (23, 165, 137), // Teal
    (40, 116, 166), // Dark Blue
    (156, 89, 182), // Medium Purple
    (52, 152, 219), // Light Blue
    (26, 188, 156), // Turquoise
    (22, 160, 133), // Sea Green
    (106, 90, 205), // Slate Blue
    (52, 73, 94),   // Dark Slate
];

// Alternative color palettes
const TABLEAU_12: [(u8, u8, u8); 12] = [
    (31, 119, 180),  // Blue
    (255, 127, 14),  // Orange
    (44, 160, 44),   // Green
    (100, 100, 100), // Medium Gray (Replacement for Red)
    (148, 103, 189), // Purple
    (140, 86, 75),   // Brown
    (227, 119, 194), // Pink
    (127, 127, 127), // Gray
    (188, 189, 34),  // Olive
    (23, 190, 207),  // Cyan
    (199, 199, 199), // Light Gray
    (255, 187, 120), // Light Orange
];

const COLORBREWER_SET3_12: [(u8, u8, u8); 12] = [
    (141, 211, 199), // Teal
    (255, 255, 179), // Light Yellow
    (190, 186, 218), // Lavender
    (251, 128, 114), // Salmon
    (128, 177, 211), // Blue
    (253, 180, 98),  // Orange
    (179, 222, 105), // Light Green
    (252, 205, 229), // Pink
    (217, 217, 217), // Gray
    (188, 128, 189), // Purple
    (204, 235, 197), // Mint
    (255, 237, 111), // Yellow
];

const MATERIAL_12: [(u8, u8, u8); 12] = [
    (100, 180, 100), // Muted Green (Replacement for Red)
    (180, 100, 180), // Muted Purple (Replacement for Pink)
    (156, 39, 176),  // Purple
    (103, 58, 183),  // Deep Purple
    (63, 81, 181),   // Indigo
    (33, 150, 243),  // Blue
    (0, 188, 212),   // Cyan
    (0, 150, 136),   // Teal
    (76, 175, 80),   // Green
    (205, 220, 57),  // Lime
    (255, 152, 0),   // Orange
    (121, 85, 72),   // Brown
];

const SOLARIZED_12: [(u8, u8, u8); 12] = [
    (38, 139, 210),  // Blue
    (211, 54, 130),  // Magenta
    (42, 161, 152),  // Cyan
    (133, 153, 0),   // Green
    (203, 75, 22),   // Orange
    (100, 100, 180), // Muted Blue (Replacement for Red)
    (181, 137, 0),   // Yellow
    (108, 113, 196), // Violet
    (147, 161, 161), // Base1 (Gray)
    (101, 123, 131), // Base01 (Dark Gray)
    (238, 232, 213), // Base3 (Light)
    (7, 54, 66),     // Base02 (Very Dark)
];

const MONOCHROME_12: [(u8, u8, u8); 12] = [
    (100, 100, 100),
    (115, 115, 115),
    (130, 130, 130),
    (145, 145, 145),
    (160, 160, 160),
    (175, 175, 175),
    (90, 90, 90),
    (105, 105, 105),
    (120, 120, 120),
    (135, 135, 135),
    (150, 150, 150),
    (165, 165, 165),
];

// Theme enum to represent all available themes
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default] // Default for the enum
    Default,
    Tableau,
    ColorBrewer,
    Material,
    Solarized,
    Monochrome,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Default => write!(f, "default"),
            Theme::Tableau => write!(f, "tableau"),
            Theme::ColorBrewer => write!(f, "color-brewer"),
            Theme::Material => write!(f, "material"),
            Theme::Solarized => write!(f, "solarized"),
            Theme::Monochrome => write!(f, "monochrome"),
        }
    }
}

// Keep the original methods for Theme, but from_str is now part of FromStr
impl Theme {
    // FNV-1a constants (32-bit)
    const FNV_OFFSET_BASIS: usize = 2166136261;
    const FNV_PRIME: usize = 16777619;

    // Helper function for FNV-1a hashing
    fn fnv1a_hash_str(input: &str) -> usize {
        let mut hash = Self::FNV_OFFSET_BASIS;
        for byte in input.as_bytes() {
            // Process bytes for better distribution
            hash ^= *byte as usize;
            hash = hash.wrapping_mul(Self::FNV_PRIME);
        }
        hash
    }

    // Get the color palette for the theme
    pub fn get_palette(&self) -> &'static [(u8, u8, u8); 12] {
        match self {
            Theme::Default => &DEFAULT_COLORS,
            Theme::Tableau => &TABLEAU_12,
            Theme::ColorBrewer => &COLORBREWER_SET3_12,
            Theme::Material => &MATERIAL_12,
            Theme::Solarized => &SOLARIZED_12,
            Theme::Monochrome => &MONOCHROME_12,
        }
    }

    // Get a color for a service based on its name hash
    pub fn get_color_for_service(&self, service_name: &str) -> (u8, u8, u8) {
        let service_hash = Self::fnv1a_hash_str(service_name);
        let palette = self.get_palette();
        palette[service_hash % palette.len()]
    }

    // Get a color for a span based on its ID hash
    pub fn get_color_for_span(&self, span_id: &str) -> (u8, u8, u8) {
        let span_hash = Self::fnv1a_hash_str(span_id);
        let palette = self.get_palette();
        palette[span_hash % palette.len()]
    }
}

// Data Structures
#[derive(Debug, Clone)]
struct ConsoleSpan {
    id: String,
    #[allow(dead_code)]
    parent_id: Option<String>,
    name: String,
    start_time: u64,
    duration_ns: u64,
    children: Vec<ConsoleSpan>,
    status_code: status::StatusCode,
    service_name: String,
}

// Structs for the timeline view
#[derive(Debug, Clone, Copy, PartialEq)]
enum ItemType {
    SpanStart,
    Event,
}

#[derive(Debug)]
struct TimelineItem {
    timestamp_ns: u64,
    item_type: ItemType,
    service_name: String,
    span_id: String,
    name: String,              // Span Name or Event Name
    level_or_status: String,   // Formatted Level (e.g., INFO) or Status (e.g., OK)
    attributes: Vec<KeyValue>, // Filtered attributes for the specific item
    // Optional: Store parent span attributes separately *only* for events
    parent_span_attributes: Option<Vec<KeyValue>>,
}

// Function to get terminal width with a default fallback
pub fn get_terminal_width(default_width: usize) -> usize {
    if let Some((Width(w), Height(_h))) = terminal_size::terminal_size() {
        w as usize
    } else {
        default_width // Fallback if terminal size can't be determined
    }
}

fn format_duration_for_scale(duration_ns: u64) -> String {
    if duration_ns == 0 {
        return "▾0ms".to_string();
    }

    let ms = duration_ns as f64 / 1_000_000.0;
    if ms < 1.0 {
        // Show microseconds for very small durations
        let us = duration_ns as f64 / 1_000.0;
        format!("▾{:.0}μs", us)
    } else if ms < 1000.0 {
        // Show milliseconds for normal durations
        format!("▾{:.0}ms", ms)
    } else {
        // Show seconds for large durations
        format!("▾{:.1}s", ms / 1000.0)
    }
}

fn generate_timeline_scale(trace_duration_ns: u64, timeline_width: usize) -> String {
    // Return empty if no duration or width is too small
    if trace_duration_ns == 0 || timeline_width < 10 {
        return " ".repeat(timeline_width);
    }

    const NUM_MARKERS: usize = 5;

    // Create our buffer filled with spaces
    let mut buffer = vec![' '; timeline_width];

    // Create the markers: 0%, 25%, 50%, 75%, 100% of duration
    for i in 0..NUM_MARKERS {
        // Calculate the time at this marker position
        let percentage = i as f64 / (NUM_MARKERS - 1) as f64;
        let marker_time_ns = (trace_duration_ns as f64 * percentage).round() as u64;

        // Format the time label
        let label = format_duration_for_scale(marker_time_ns);

        // Calculate exact position in the timeline for this percentage
        let position = (percentage * (timeline_width - 1) as f64).round() as usize;

        // Place the label with specific alignment:
        // - First label (0ms): Left-aligned at position 0
        // - Last label: Right-aligned at the end
        // - Middle labels: Centered around their position
        let label_len = label.chars().count();
        let label_start = if i == 0 {
            // First label starts at position 0
            0
        } else if i == NUM_MARKERS - 1 {
            // Last label ends at the last position
            timeline_width.saturating_sub(label_len)
        } else {
            // Middle labels centered (but shifted left if needed)
            position
                .saturating_sub(label_len / 2)
                .min(timeline_width.saturating_sub(label_len))
        };

        // Write the label to the buffer
        for (j, ch) in label.chars().enumerate() {
            let idx = label_start + j;
            if idx < timeline_width {
                buffer[idx] = ch;
            }
        }
    }
    buffer.into_iter().collect()
}

// Helper function to convert AnyValue to a comprehensive string for grep matching
fn get_string_value_for_grep(value_opt: &Option<AnyValue>) -> String {
    if let Some(any_value) = value_opt {
        if let Some(ref val_type) = any_value.value {
            return match val_type {
                ProtoValue::StringValue(s) => s.clone(),
                ProtoValue::BoolValue(b) => b.to_string(),
                ProtoValue::IntValue(i) => i.to_string(),
                ProtoValue::DoubleValue(d) => d.to_string(),
                ProtoValue::ArrayValue(arr) => arr
                    .values
                    .iter()
                    .map(|v_val| get_string_value_for_grep(&Some(v_val.clone()))) // Recurse for elements
                    .collect::<Vec<String>>()
                    .join(", "),
                ProtoValue::KvlistValue(kv_list) => kv_list
                    .values
                    .iter()
                    .map(|kv| format!("{}:{}", kv.key, get_string_value_for_grep(&kv.value)))
                    .collect::<Vec<String>>()
                    .join(", "),
                ProtoValue::BytesValue(b) => format!("bytes_len:{}", b.len()),
            };
        }
    }
    String::new()
}

// Helper function to prepare trace data from a batch
fn prepare_trace_data_from_batch(
    batch: &[TelemetryData],
) -> Result<HashMap<String, Vec<(Span, String)>>> {
    // Initialize a vector to store tuples of (Span, ServiceName).
    // This will be populated by decoding each TelemetryData item in the batch.
    let mut spans_with_service: Vec<(Span, String)> = Vec::new();

    // Iterate over each TelemetryData item in the input batch.
    for item in batch {
        // Attempt to decode the payload of the TelemetryData item into an ExportTraceServiceRequest.
        // The payload is expected to be a protobuf message.
        match ExportTraceServiceRequest::decode(item.payload.as_slice()) {
            Ok(request) => {
                // If decoding is successful, iterate over the resource_spans in the request.
                for resource_span in request.resource_spans {
                    // Find the service name from the resource attributes.
                    // Defaults to "<unknown>" if "service.name" attribute is not found.
                    let service_name = find_service_name(
                        resource_span
                            .resource
                            .as_ref()
                            .map_or(&[], |r| &r.attributes),
                    );
                    // Iterate over scope_spans within the current resource_span.
                    for scope_span in resource_span.scope_spans {
                        // Iterate over individual spans within the current scope_span.
                        for span in scope_span.spans {
                            // Add the cloned span and its associated service name to the spans_with_service vector.
                            spans_with_service.push((span.clone(), service_name.clone()));
                        }
                    }
                }
            }
            Err(e) => {
                // If decoding fails, log a warning and skip this item.
                // Processing continues with the next item in the batch.
                tracing::warn!(error = %e, "Failed to decode payload for console display, skipping item.");
            }
        }
    }

    // If no spans were successfully decoded and collected, return an empty map or handle as an error if preferred.
    // For now, an empty map means no traces to display further down.
    if spans_with_service.is_empty() {
        return Ok(HashMap::new());
    }

    // Group spans by trace ID.
    // A HashMap is used where keys are trace IDs (hex encoded strings) and
    // values are vectors of (Span, ServiceName) tuples belonging to that trace.
    let mut traces: HashMap<String, Vec<(Span, String)>> = HashMap::new();
    for (span, service_name) in spans_with_service {
        let trace_id_hex = hex::encode(&span.trace_id);
        traces
            .entry(trace_id_hex)
            .or_default()
            .push((span, service_name));
    }
    Ok(traces)
}

// Helper function to calculate console layout widths
fn calculate_layout_widths(default_terminal_width: usize) -> (usize, usize, usize) {
    // Define fixed widths for various columns in the waterfall display.
    const SPACING: usize = 6; // Approximate number of spaces for padding between 7 columns.

    // Calculate the total width occupied by columns with fixed sizes.
    let fixed_width_excluding_timeline = SERVICE_NAME_WIDTH
        + SPAN_NAME_WIDTH
        + SPAN_KIND_WIDTH
        + DURATION_WIDTH
        + SPAN_ID_WIDTH
        + STATUS_WIDTH
        + SPACING;

    // Get the current terminal width, defaulting to `default_terminal_width` if it cannot be determined.
    let terminal_width = get_terminal_width(default_terminal_width);
    // Calculate the width available for the timeline visualization.
    // It's the terminal width minus the fixed column widths, with a minimum of 10 characters.
    let calculated_timeline_width = terminal_width
        .saturating_sub(fixed_width_excluding_timeline + 1) // +1 to leave one space to the right of the timeline
        .max(10);

    (terminal_width, calculated_timeline_width, terminal_width)
}

// Helper function to print the trace header
fn print_trace_header(trace_id: &str, root_span_received: bool, total_table_width: usize) {
    // Construct the base heading for the trace.
    let base_heading = format!("Trace ID: {}", trace_id);
    // Add a suffix if the root span for this trace was not received.
    let suffix = if root_span_received {
        ""
    } else {
        " (Missing Root)"
    };

    // Calculate the visible length of the heading (base + suffix).
    let visible_heading_len = base_heading.len() + suffix.len();

    // Style the heading: bold for the base, dimmed for the suffix.
    let styled_heading = format!("{}{}", base_heading.bold(), suffix.dimmed());

    // Calculate the number of dashes needed for padding around the heading
    // to make the header line span the `total_table_width`.
    let total_dash_len = total_table_width.saturating_sub(visible_heading_len + 2); // +2 for spaces around heading
    let left_dashes = 1; // At least one dash on the left.
    let right_dashes = total_dash_len.saturating_sub(left_dashes);

    // Print the formatted trace header.
    println!(
        "\n{} {} {}\n\n",
        "─".repeat(left_dashes).dimmed(),
        styled_heading,
        "─".repeat(right_dashes).dimmed()
    );
}

// Helper function to collect and filter timeline items for a trace
fn collect_and_filter_timeline_items_for_trace(
    spans_in_trace_with_service: &[(Span, String)],
    attr_globs: &Option<GlobSet>,
    event_severity_attribute_name: &str,
    events_only: bool,
    grep_regex: Option<&Regex>,
) -> Vec<TimelineItem> {
    let mut timeline_items: Vec<TimelineItem> = Vec::new();

    for (span, service_name) in spans_in_trace_with_service {
        let span_id_hex = hex::encode(&span.span_id);
        let status_code = span.status.as_ref().map_or(status::StatusCode::Unset, |s| {
            status::StatusCode::try_from(s.code).unwrap_or(status::StatusCode::Unset)
        });
        let is_error = status_code == status::StatusCode::Error;

        if !events_only {
            let filtered_span_attrs: Vec<KeyValue> = match attr_globs {
                Some(globs) => span
                    .attributes
                    .iter()
                    .filter(|kv| globs.is_match(&kv.key))
                    .cloned()
                    .collect(),
                None => span.attributes.clone(),
            };

            let span_start_item = TimelineItem {
                timestamp_ns: span.start_time_unix_nano,
                item_type: ItemType::SpanStart,
                service_name: service_name.clone(),
                span_id: span_id_hex.clone(),
                name: span.name.clone(),
                level_or_status: format_span_status(status_code),
                attributes: filtered_span_attrs,
                parent_span_attributes: None,
            };

            let mut include_item = true;
            if let Some(re) = grep_regex {
                include_item = false;
                for attr in &span_start_item.attributes {
                    let value_str = get_string_value_for_grep(&attr.value);
                    if re.is_match(&value_str) {
                        include_item = true;
                        break;
                    }
                }
            }
            if include_item {
                timeline_items.push(span_start_item);
            }
        }

        for event in &span.events {
            let mut level = if is_error {
                "ERROR".to_string()
            } else {
                "INFO".to_string()
            };

            for attr in &event.attributes {
                if attr.key == event_severity_attribute_name {
                    if let Some(val) = &attr.value {
                        if let Some(ProtoValue::StringValue(s)) = &val.value {
                            level = s.clone().to_uppercase();
                            break;
                        }
                    }
                }
            }

            let filtered_event_attrs: Vec<KeyValue> = match attr_globs {
                Some(globs) => event
                    .attributes
                    .iter()
                    .filter(|kv| globs.is_match(&kv.key))
                    .cloned()
                    .collect(),
                None => event.attributes.clone(),
            };
            let filtered_parent_span_attrs: Vec<KeyValue> = match attr_globs {
                Some(globs) => span
                    .attributes
                    .iter()
                    .filter(|kv| globs.is_match(&kv.key))
                    .cloned()
                    .collect(),
                None => span.attributes.clone(),
            };

            let event_item = TimelineItem {
                timestamp_ns: event.time_unix_nano,
                item_type: ItemType::Event,
                service_name: service_name.clone(),
                span_id: span_id_hex.clone(),
                name: event.name.clone(),
                level_or_status: level,
                attributes: filtered_event_attrs,
                parent_span_attributes: Some(filtered_parent_span_attrs),
            };

            let mut include_event_item = true;
            if let Some(re) = grep_regex {
                include_event_item = false;
                for attr in &event_item.attributes {
                    let value_str = get_string_value_for_grep(&attr.value);
                    if re.is_match(&value_str) {
                        include_event_item = true;
                        break;
                    }
                }
                if !include_event_item {
                    if let Some(parent_attrs_for_event) = &event_item.parent_span_attributes {
                        for attr in parent_attrs_for_event {
                            let value_str = get_string_value_for_grep(&attr.value);
                            if re.is_match(&value_str) {
                                include_event_item = true;
                                break;
                            }
                        }
                    }
                }
            }
            if include_event_item {
                timeline_items.push(event_item);
            }
        }
    }

    timeline_items.sort_by_key(|item| item.timestamp_ns);
    timeline_items
}

// Helper function to build waterfall hierarchy and gather metadata
fn build_waterfall_hierarchy_and_meta(
    spans_in_trace_with_service: &[(Span, String)],
) -> (Vec<ConsoleSpan>, u64, u64, HashMap<String, Span>) {
    // `span_map`: Maps span ID (hex string) to the `Span` object.
    // `service_name_map`: Maps span ID (hex string) to its service name.
    // `parent_to_children_map`: Maps parent span ID (hex string) to a list of its child span IDs.
    // `root_ids`: A list of span IDs that are roots (have no parent or parent is not in this trace batch).
    let mut span_map: HashMap<String, Span> = HashMap::new();
    let mut service_name_map: HashMap<String, String> = HashMap::new();
    let mut parent_to_children_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut root_ids: Vec<String> = Vec::new();

    // Populate `span_map` and `service_name_map`.
    for (span, service_name) in spans_in_trace_with_service {
        let span_id_hex = hex::encode(&span.span_id);
        // Clone span and service_name before inserting, as `spans_in_trace_with_service` is a slice of tuples.
        span_map.insert(span_id_hex.clone(), span.clone());
        service_name_map.insert(span_id_hex.clone(), service_name.clone());
    }

    // Populate `parent_to_children_map` and identify `root_ids`.
    for (span_id_hex, span) in &span_map {
        // Iterate over the populated span_map.
        let parent_id_hex = if span.parent_span_id.is_empty() {
            None
        } else {
            Some(hex::encode(&span.parent_span_id))
        };
        match parent_id_hex {
            Some(ref p_id) if span_map.contains_key(p_id) => {
                // If span has a parent and the parent is in our map, add it to children list.
                parent_to_children_map
                    .entry(p_id.clone())
                    .or_default()
                    .push(span_id_hex.clone());
            }
            _ => {
                // Otherwise, it's a root span (or its parent is missing from this batch).
                root_ids.push(span_id_hex.clone());
            }
        }
    }

    // Build `ConsoleSpan` structures (hierarchical representation for waterfall) from root spans.
    let mut roots: Vec<ConsoleSpan> = root_ids
        .iter()
        .map(|root_id| {
            build_console_span(
                root_id,
                &span_map,
                &parent_to_children_map,
                &service_name_map,
            )
        })
        .collect();
    // Sort root spans by their start time.
    roots.sort_by_key(|s| s.start_time);

    // Determine the overall start time and end time of the trace from all spans.
    let min_start_time_ns = roots.iter().map(|r| r.start_time).min().unwrap_or(0);
    let max_end_time_ns = span_map // Uses the span_map which contains all spans in this trace
        .values()
        .map(|s| s.end_time_unix_nano)
        .max()
        .unwrap_or(0);
    // Calculate the total duration of the trace.
    let trace_duration_ns = max_end_time_ns.saturating_sub(min_start_time_ns);

    (roots, min_start_time_ns, trace_duration_ns, span_map)
}

// Helper function to render the waterfall table
#[allow(clippy::too_many_arguments)]
fn render_waterfall_table(
    roots: &[ConsoleSpan],
    min_start_time_ns: u64,
    trace_duration_ns: u64,
    calculated_timeline_width: usize,
    total_table_width: usize, // For table.set_width()
    theme: Theme,
    color_by: ColoringMode,
    span_map: &HashMap<String, Span>, // For add_span_to_table
) -> Result<()> {
    let mut table = Table::new();
    table
        .load_preset(presets::NOTHING)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth)
        .set_width(total_table_width as u16)
        .set_style(TableComponent::MiddleHeaderIntersections, '┴')
        .set_style(TableComponent::BottomBorder, '─')
        .set_style(TableComponent::BottomBorderIntersections, '─')
        .set_style(TableComponent::HeaderLines, '─');

    table.set_header(vec![
        Cell::new("Service").add_attribute(Attribute::Bold),
        Cell::new("Span Name").add_attribute(Attribute::Bold),
        Cell::new("Kind").add_attribute(Attribute::Bold),
        Cell::new("Duration (ms)").add_attribute(Attribute::Bold),
        Cell::new("Span ID").add_attribute(Attribute::Bold),
        Cell::new("Status").add_attribute(Attribute::Bold),
        Cell::new("Timeline").add_attribute(Attribute::Bold),
    ]);

    let column_widths: [(usize, u16); 6] = [
        (0, SERVICE_NAME_WIDTH as u16),
        (1, SPAN_NAME_WIDTH as u16),
        (2, SPAN_KIND_WIDTH as u16),
        (3, DURATION_WIDTH as u16),
        (4, SPAN_ID_WIDTH as u16),
        (5, STATUS_WIDTH as u16),
    ];

    for (index, width) in &column_widths {
        if let Some(column) = table.column_mut(*index) {
            column.set_constraint(ColumnConstraint::UpperBoundary(Fixed(*width)));
        }
    }

    if trace_duration_ns > 0 {
        let scale_content = generate_timeline_scale(trace_duration_ns, calculated_timeline_width);
        if !scale_content.trim().is_empty() {
            table.add_row(vec![
                Cell::new(""),
                Cell::new(""),
                Cell::new(""),
                Cell::new(""),
                Cell::new(""),
                Cell::new(""),
                Cell::new(scale_content),
            ]);
        }
    }

    for root_span in roots {
        // Changed variable name to avoid conflict with `roots` parameter in outer scope if this was inlined
        add_span_to_table(
            &mut table,
            root_span,
            0, // Initial depth
            min_start_time_ns,
            trace_duration_ns,
            calculated_timeline_width,
            theme,
            span_map, // Pass the original `Span` map
            color_by,
        )?;
    }

    println!("{}", table);
    Ok(())
}

// Helper function to print the timeline log
fn print_timeline_log(
    timeline_items: &[TimelineItem],
    color_by: ColoringMode,
    theme: Theme,
    grep_regex: Option<&Regex>,
) {
    if timeline_items.is_empty() {
        return;
    }

    // First pass: determine maximum lengths for service_name and item.name
    let mut max_service_name_len = 0;
    let mut max_item_name_len = 0;

    for item in timeline_items {
        max_service_name_len = max_service_name_len.max(item.service_name.len());
        max_item_name_len = max_item_name_len.max(item.name.len());
    }

    // Second pass: print timeline items with padding
    for item in timeline_items {
        // Iterate over the sorted `TimelineItem`s.
        // Format timestamp into a readable string.
        let timestamp = Utc.timestamp_nanos(item.timestamp_ns as i64);
        let formatted_time = timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();

        // Get color for the span ID prefix based on `color_by` setting (service or span ID).
        let (prefix_r, prefix_g, prefix_b) = match color_by {
            ColoringMode::Service => theme.get_color_for_service(&item.service_name),
            ColoringMode::Span => theme.get_color_for_span(&item.span_id),
        };

        // Take the first 8 characters of the span ID for display.
        let span_id_prefix = item.span_id.chars().take(8).collect::<String>();

        // Determine the type tag ("SPAN" or "EVENT").
        let type_tag = match item.item_type {
            ItemType::SpanStart => "SPAN".to_string(),
            ItemType::Event => "EVENT".to_string(),
        };

        // Logic to get raw status/level text for consistent processing
        let raw_text_for_status_or_level = if item.item_type == ItemType::SpanStart {
            if item.level_or_status.contains("UNSET") {
                "UNSET".to_string()
            } else if item.level_or_status.contains("ERROR") {
                "ERROR".to_string()
            } else if item.level_or_status.contains("OK") {
                "OK".to_string()
            } else {
                "STATUS?".to_string()
            }
        } else {
            item.level_or_status.to_uppercase()
        };

        let text_to_format_and_color = format!("{} {:5}", type_tag, raw_text_for_status_or_level);

        // --- Re-apply coloring for the actual output based on the raw status/level text ---
        let level_status_colored = match raw_text_for_status_or_level.as_str() {
            "ERROR" => text_to_format_and_color.red(),
            "WARN" | "WARNING" => text_to_format_and_color.yellow(),
            "INFO" | "OK" => text_to_format_and_color.green(),
            "UNSET" => text_to_format_and_color.dimmed(),
            _ => text_to_format_and_color.dimmed(),
        };

        let colored_span_id_for_print = span_id_prefix
            .truecolor(prefix_r, prefix_g, prefix_b)
            .to_string();

        // Pad service_name and item.name to their respective maximum lengths for alignment
        let service_name_padded = format!(
            "{:<width$}",
            item.service_name,
            width = max_service_name_len
        );
        let item_name_padded = format!("{:<width$}", item.name, width = max_item_name_len);

        let mut attrs_to_display: Vec<String> = Vec::new();
        for attr in &item.attributes {
            attrs_to_display.push(format_keyvalue(attr, grep_regex));
        }
        if let Some(parent_attrs) = &item.parent_span_attributes {
            for attr in parent_attrs {
                let value_str = format_anyvalue(&attr.value);
                attrs_to_display.push(format!("{}: {}", attr.key.dimmed(), value_str));
            }
        }

        let attrs_suffix = if !attrs_to_display.is_empty() {
            format!(" - {}", attrs_to_display.join(", "))
        } else {
            String::new()
        };

        println!(
            "{}  {:12} {} {} {} {}", // Adjusted for padded names and attrs_suffix structure
            formatted_time.dimmed(),
            level_status_colored,
            colored_span_id_for_print,
            service_name_padded, // Use padded service name
            item_name_padded,    // Use padded item name
            attrs_suffix         // attrs_suffix includes " - " or is empty
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn display_console(
    batch: &[TelemetryData], // A collection of telemetry data items, each potentially containing multiple spans.
    attr_globs: &Option<GlobSet>, // Optional set of glob patterns for filtering attributes to display.
    event_severity_attribute_name: &str, // The attribute key used to determine event severity (e.g., "event.severity").
    theme: Theme,                        // The color theme to use for console output.
    color_by: ColoringMode,              // How to color items: by service name or by span ID.
    events_only: bool, // If true, only events are shown in the timeline log (spans are hidden).
    root_span_received: bool, // Indicates if the root span for the trace was found.
    grep_regex: Option<&Regex>, // Optional regex for filtering timeline items by attribute values.
) -> Result<()> {
    // Debug logging with theme and coloring mode
    tracing::debug!("Display console called with theme={:?}, color_by={:?}, events_only={}, root_span_received={}, has_grep_regex={}",
                  theme, color_by, events_only, root_span_received, grep_regex.is_some());

    // Prepare trace data: decode batch, extract spans, and group by trace ID.
    let traces = prepare_trace_data_from_batch(batch)?;

    // If no traces were found after preparation, exit early.
    if traces.is_empty() {
        tracing::debug!("No traces found in batch after preparation.");
        return Ok(());
    }

    // Calculate console layout widths.
    let (_terminal_width, calculated_timeline_width, total_table_width) =
        calculate_layout_widths(120);

    // Iterate over each trace collected in the `traces` HashMap.
    // `trace_id` is the hex string of the trace ID.
    // `spans_in_trace_with_service` is a vector of (Span, ServiceName) for this trace.
    for (trace_id, spans_in_trace_with_service) in traces {
        // ---- Print Trace Header ----
        print_trace_header(&trace_id, root_span_received, total_table_width);

        // If there are no spans in this particular trace (e.g., after filtering or if data was empty),
        // skip to the next trace.
        if spans_in_trace_with_service.is_empty() {
            continue;
        }

        // Collect and filter timeline items (span starts and events) for the current trace.
        let timeline_items = collect_and_filter_timeline_items_for_trace(
            &spans_in_trace_with_service,
            attr_globs,
            event_severity_attribute_name,
            events_only,
            grep_regex,
        );

        // Build the waterfall hierarchy (ConsoleSpans) and get timing metadata.
        let (roots, min_start_time_ns, trace_duration_ns, span_map) =
            build_waterfall_hierarchy_and_meta(&spans_in_trace_with_service);

        // Render and print the waterfall table.
        render_waterfall_table(
            &roots, // Pass roots as a slice
            min_start_time_ns,
            trace_duration_ns,
            calculated_timeline_width,
            total_table_width,
            theme,
            color_by,
            &span_map,
        )?;

        // ---- Print Timeline Log ----
        // If there are sorted timeline items (span starts or events), print them.
        if !timeline_items.is_empty() {
            print_timeline_log(&timeline_items, color_by, theme, grep_regex);
        }
    }
    // End of loop for each trace.

    Ok(()) // Indicate successful display.
}

// Private Helper Functions

fn find_service_name(attrs: &[KeyValue]) -> String {
    attrs
        .iter()
        .find(|kv| kv.key == "service.name")
        .and_then(|kv| {
            kv.value.as_ref().and_then(|av| {
                if let Some(
                    opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s),
                ) = &av.value
                {
                    Some(s.clone())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn build_console_span(
    span_id: &str,
    span_map: &HashMap<String, Span>,
    parent_to_children_map: &HashMap<String, Vec<String>>,
    service_name_map: &HashMap<String, String>,
) -> ConsoleSpan {
    let span = span_map.get(span_id).expect("Span ID should exist in map");
    let service_name = service_name_map
        .get(span_id)
        .cloned()
        .unwrap_or_else(|| "<unknown>".to_string());

    let start_time = span.start_time_unix_nano;
    let end_time = span.end_time_unix_nano;
    let duration_ns = end_time.saturating_sub(start_time);

    let status_code = span.status.as_ref().map_or(status::StatusCode::Unset, |s| {
        status::StatusCode::try_from(s.code).unwrap_or(status::StatusCode::Unset)
    });

    let child_ids = parent_to_children_map
        .get(span_id)
        .cloned()
        .unwrap_or_default();

    let mut children: Vec<ConsoleSpan> = child_ids
        .iter()
        .map(|child_id| {
            build_console_span(child_id, span_map, parent_to_children_map, service_name_map)
        })
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

#[allow(clippy::too_many_arguments)]
fn add_span_to_table(
    table: &mut Table,
    node: &ConsoleSpan,
    depth: usize,
    trace_start_time_ns: u64,
    trace_duration_ns: u64,
    timeline_width: usize,
    theme: Theme,
    span_map: &HashMap<String, Span>,
    color_by: ColoringMode,
) -> Result<()> {
    let indent = "  ".repeat(depth);

    // Get Color (still needed for timeline bar)
    let (r, g, b) = match color_by {
        ColoringMode::Service => theme.get_color_for_service(&node.service_name),
        ColoringMode::Span => theme.get_color_for_span(&node.id),
    };

    // Create Cell Content
    let service_name_content = node
        .service_name
        .chars()
        .take(SERVICE_NAME_WIDTH)
        .collect::<String>();

    let span_name_cell_content = format!("{} {}", indent, node.name)
        .chars()
        .take(SPAN_NAME_WIDTH)
        .collect::<String>();

    // Timeline bar remains colored
    let bar_cell_content = render_bar(
        node.start_time,
        node.duration_ns,
        trace_start_time_ns,
        trace_duration_ns,
        timeline_width,
    );

    // Get the actual span object for additional data
    let span_obj = span_map.get(&node.id);

    // Format span kind
    let kind_cell_content = span_obj
        .map_or("UNKNOWN".to_string(), |span| format_span_kind(span.kind))
        .chars()
        .take(SPAN_KIND_WIDTH)
        .collect::<String>();

    let span_id_prefix = node.id.chars().take(8).collect::<String>();

    let status_content_str = format_span_status(node.status_code);
    let formatted_duration = format!("{:.2}", node.duration_ns as f64 / 1_000_000.0);

    table.add_row(vec![
        Cell::new(service_name_content),
        Cell::new(span_name_cell_content),
        Cell::new(kind_cell_content),
        Cell::new(formatted_duration).set_alignment(CellAlignment::Right), // Right-align duration
        Cell::new(span_id_prefix).fg(TableColor::Rgb { r, g, b }),
        format_cell_level_color(&status_content_str),
        Cell::new(bar_cell_content).fg(TableColor::Rgb { r, g, b }),
    ]);

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
            theme,
            span_map,
            color_by,
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
) -> String {
    // If there's no duration, return empty space
    if trace_duration_ns == 0 {
        return " ".repeat(timeline_width);
    }

    // Calculate start and end positions
    let timeline_width_f = timeline_width as f64;
    let offset_ns = start_time_ns.saturating_sub(trace_start_time_ns);
    let offset_fraction = offset_ns as f64 / trace_duration_ns as f64;
    let duration_fraction = duration_ns as f64 / trace_duration_ns as f64;
    let start_pos = (offset_fraction * timeline_width_f).floor() as usize;
    let end_pos = ((offset_fraction + duration_fraction) * timeline_width_f).ceil() as usize;

    // Build the bar content string with spaces and blocks
    let mut bar_content = String::with_capacity(timeline_width);
    for i in 0..timeline_width {
        if i >= start_pos && i < end_pos.min(timeline_width) {
            bar_content.push('▄');
        } else {
            bar_content.push(' ');
        }
    }

    bar_content
}

fn format_span_kind(kind: i32) -> String {
    match kind {
        1 => "INTERNAL".to_string(),
        2 => "SERVER".to_string(),
        3 => "CLIENT".to_string(),
        4 => "PRODUCER".to_string(),
        5 => "CONSUMER".to_string(),
        _ => "UNSPECIFIED".to_string(),
    }
}

fn format_keyvalue(kv: &KeyValue, grep_regex: Option<&Regex>) -> String {
    let value_str = format_anyvalue(&kv.value);
    if let Some(re) = grep_regex {
        if re.is_match(&value_str) {
            let mut highlighted_value = String::new();
            let mut last_end = 0;
            for mat in re.find_iter(&value_str) {
                highlighted_value.push_str(&value_str[last_end..mat.start()]);
                highlighted_value
                    .push_str(&mat.as_str().on_truecolor(255, 255, 153).black().to_string()); // Bright yellow background, black text
                last_end = mat.end();
            }
            highlighted_value.push_str(&value_str[last_end..]);
            return format!("{}: {}", kv.key.dimmed(), highlighted_value);
        }
    }
    format!("{}: {}", kv.key.dimmed(), value_str)
}

fn format_anyvalue(av: &Option<AnyValue>) -> String {
    match av {
        Some(any_value) => match &any_value.value {
            Some(ProtoValue::StringValue(s)) => s.clone(),
            Some(ProtoValue::BoolValue(b)) => b.to_string(),
            Some(ProtoValue::IntValue(i)) => i.to_string(),
            Some(ProtoValue::DoubleValue(d)) => d.to_string(),
            Some(ProtoValue::ArrayValue(_)) => "[array]".to_string(),
            Some(ProtoValue::KvlistValue(_)) => "[kvlist]".to_string(),
            Some(ProtoValue::BytesValue(_)) => "[bytes]".to_string(),
            None => "<empty_value>".to_string(),
        },
        None => "<no_value>".to_string(),
    }
}

fn format_span_status(status_code: status::StatusCode) -> String {
    match status_code {
        status::StatusCode::Ok => "OK",
        status::StatusCode::Error => "ERROR",
        status::StatusCode::Unset => "UNSET",
    }
    .to_string()
}

fn format_cell_level_color(value: &str) -> Cell {
    match value {
        "OK" => Cell::new(value).fg(TableColor::Green),
        "ERROR" => Cell::new(value).fg(TableColor::Red),
        _ => Cell::new(value).fg(TableColor::DarkGrey),
    }
}
