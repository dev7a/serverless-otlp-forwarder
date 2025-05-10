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
    presets, Attribute, Cell, CellAlignment, ColumnConstraint, ContentArrangement, Table,
    TableComponent, Width::Fixed,
};
use globset::GlobSet;
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{any_value::Value as ProtoValue, AnyValue, KeyValue},
    trace::v1::{status, Span},
};
use prost::Message;
use std::collections::HashMap;
use std::str::FromStr; // Added for FromStr trait
use terminal_size::{self, Height, Width}; // Need TelemetryData for display_console

// Constants
const SERVICE_NAME_WIDTH: usize = 25;
const SPAN_NAME_WIDTH: usize = 40;
const SPAN_ID_WIDTH: usize = 10;
const SPAN_KIND_WIDTH: usize = 10; // Width for the Span Kind column
const STATUS_WIDTH: usize = 8; // Width for the Status column
const DURATION_WIDTH: usize = 13; // Width for the Duration column

// Define a bright red color for errors
const ERROR_COLOR: (u8, u8, u8) = (255, 0, 0); // Bright red

// Define all color palettes
// Default color palette
const SERVICE_COLORS: [(u8, u8, u8); 12] = [
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
    (214, 39, 40),   // Red
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
    (244, 67, 54),  // Red
    (233, 30, 99),  // Pink
    (156, 39, 176), // Purple
    (103, 58, 183), // Deep Purple
    (63, 81, 181),  // Indigo
    (33, 150, 243), // Blue
    (0, 188, 212),  // Cyan
    (0, 150, 136),  // Teal
    (76, 175, 80),  // Green
    (205, 220, 57), // Lime
    (255, 152, 0),  // Orange
    (121, 85, 72),  // Brown
];

const SOLARIZED_12: [(u8, u8, u8); 12] = [
    (38, 139, 210),  // Blue
    (211, 54, 130),  // Magenta
    (42, 161, 152),  // Cyan
    (133, 153, 0),   // Green
    (203, 75, 22),   // Orange
    (220, 50, 47),   // Red
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Default,
    Tableau,
    ColorBrewer,
    Material,
    Solarized,
    Monochrome,
}

impl FromStr for Theme {
    type Err = String; // Define an error type for parsing

    // Parse a theme name string to an enum value
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Theme::Default),
            "tableau" => Ok(Theme::Tableau),
            "colorbrewer" => Ok(Theme::ColorBrewer),
            "material" => Ok(Theme::Material),
            "solarized" => Ok(Theme::Solarized),
            "monochrome" => Ok(Theme::Monochrome),
            _ => Err(format!("Invalid theme name: {}", s)),
        }
    }
}

// Keep the original methods for Theme, but from_str is now part of FromStr
impl Theme {
    // Get the color palette for the theme
    pub fn get_palette(&self) -> &'static [(u8, u8, u8); 12] {
        match self {
            Theme::Default => &SERVICE_COLORS,
            Theme::Tableau => &TABLEAU_12,
            Theme::ColorBrewer => &COLORBREWER_SET3_12,
            Theme::Material => &MATERIAL_12,
            Theme::Solarized => &SOLARIZED_12,
            Theme::Monochrome => &MONOCHROME_12,
        }
    }

    // Get a color for a service based on its name hash
    pub fn get_color_for_service(&self, service_name: &str) -> (u8, u8, u8) {
        let service_hash = service_name.chars().fold(0, |acc, c| acc + (c as usize));
        let palette = self.get_palette();
        palette[service_hash % palette.len()]
    }

    // Get a color for a span based on its ID hash
    pub fn get_color_for_span(&self, span_id: &str) -> (u8, u8, u8) {
        // Use a different hashing approach for span IDs to avoid collisions with service names
        let mut span_hash: usize = 5381; // Initial prime
        for c in span_id.chars() {
            span_hash = span_hash.wrapping_add(c as usize).wrapping_mul(33); // Basic multiplicative hash
        }
        let palette = self.get_palette();
        palette[span_hash % palette.len()]
    }

    // Helper method to check if a theme name is valid
    pub fn is_valid_theme(theme_name: &str) -> bool {
        matches!(
            theme_name.to_lowercase().as_str(),
            "default" | "tableau" | "colorbrewer" | "material" | "solarized" | "monochrome"
        )
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
    is_error: bool,            // To help color span ID for SpanStart items
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

// Public Display Function
pub fn display_console(
    batch: &[TelemetryData],
    attr_globs: &Option<GlobSet>,
    event_severity_attribute_name: &str,
    theme: Theme,
    color_by: ColoringMode,
    events_only: bool,
    root_span_received: bool, // New parameter to indicate if the root span was found
) -> Result<()> {
    // Debug logging with theme and coloring mode
    tracing::debug!("Display console called with theme={:?}, color_by={:?}, events_only={}, root_span_received={}", 
                  theme, color_by, events_only, root_span_received);

    let mut spans_with_service: Vec<(Span, String)> = Vec::new();

    for item in batch {
        match ExportTraceServiceRequest::decode(item.payload.as_slice()) {
            Ok(request) => {
                for resource_span in request.resource_spans {
                    let service_name = find_service_name(
                        resource_span
                            .resource
                            .as_ref()
                            .map_or(&[], |r| &r.attributes),
                    );
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
        traces
            .entry(trace_id_hex)
            .or_default()
            .push((span, service_name));
    }

    // Calculate approximate total table width for header ruling
    const SPACING: usize = 6; // Approx spaces between 7 columns

    // Calculate the fixed width excluding the timeline
    let fixed_width_excluding_timeline = SERVICE_NAME_WIDTH
        + SPAN_NAME_WIDTH
        + SPAN_KIND_WIDTH
        + DURATION_WIDTH
        + SPAN_ID_WIDTH
        + STATUS_WIDTH
        + SPACING;

    // Get terminal width and calculate dynamic timeline width
    let terminal_width = get_terminal_width(120); // Use a larger default fallback
                                                  // Ensure timeline width is at least some minimum (e.g., 10) or doesn't cause overflow
    let calculated_timeline_width = terminal_width
        .saturating_sub(fixed_width_excluding_timeline)
        .max(10);

    // Total width is now just the terminal width for header padding
    let total_table_width = terminal_width;

    for (trace_id, spans_in_trace_with_service) in traces {
        // 1. Define base and suffix
        let base_heading = format!("Trace ID: {}", trace_id);
        let suffix = if root_span_received {
            ""
        } else {
            " (Missing Root)"
        };

        // 2. Calculate total visible length
        let visible_heading_len = base_heading.len() + suffix.len();

        // 3. Apply styling
        let styled_heading = format!("{}{}", base_heading.bold(), suffix.dimmed());

        // 4. Calculate padding dashes needed
        // Subtract heading length and 2 spaces (one before, one after heading)
        let total_dash_len = total_table_width.saturating_sub(visible_heading_len + 2);
        // Ensure at least one dash on each side if possible
        let left_dashes = 1;
        let right_dashes = total_dash_len.saturating_sub(left_dashes);

        // 5. Print the header
        println!(
            "\n{} {} {}\n\n",
            "─".repeat(left_dashes).dimmed(),
            styled_heading, // Already styled
            "─".repeat(right_dashes).dimmed()
        );

        if spans_in_trace_with_service.is_empty() {
            continue;
        }

        // Replace the trace_events and span_error_status with timeline_items
        let mut timeline_items: Vec<TimelineItem> = Vec::new();

        for (span, service_name) in &spans_in_trace_with_service {
            let span_id_hex = hex::encode(&span.span_id);
            let status_code = span.status.as_ref().map_or(status::StatusCode::Unset, |s| {
                status::StatusCode::try_from(s.code).unwrap_or(status::StatusCode::Unset)
            });
            let is_error = status_code == status::StatusCode::Error;

            if !events_only {
                // Only add span starts if not events_only mode
                let filtered_span_attrs: Vec<KeyValue> = match attr_globs {
                    Some(globs) => span
                        .attributes
                        .iter()
                        .filter(|kv| globs.is_match(&kv.key))
                        .cloned()
                        .collect(),
                    None => span.attributes.clone(), // Collect all if no globs
                };

                timeline_items.push(TimelineItem {
                    timestamp_ns: span.start_time_unix_nano,
                    item_type: ItemType::SpanStart,
                    service_name: service_name.clone(),
                    span_id: span_id_hex.clone(),
                    name: span.name.clone(),
                    level_or_status: format_span_status(status_code),
                    is_error,
                    attributes: filtered_span_attrs,
                    parent_span_attributes: None, // Not applicable for SpanStart
                });
            }

            // Add events - similar to the existing events logic but store in timeline_items
            for event in &span.events {
                let mut level = if is_error {
                    "ERROR".to_string()
                } else {
                    "INFO".to_string()
                };

                for attr in &event.attributes {
                    if attr.key == event_severity_attribute_name {
                        if let Some(val) = &attr.value {
                            if let Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) = &val.value {
                                level = s.clone().to_uppercase();
                                break;
                            }
                        }
                    }
                }

                // Filter attributes based on globs
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

                timeline_items.push(TimelineItem {
                    timestamp_ns: event.time_unix_nano,
                    item_type: ItemType::Event,
                    service_name: service_name.clone(),
                    span_id: span_id_hex.clone(),
                    name: event.name.clone(),
                    level_or_status: level,
                    is_error,
                    attributes: filtered_event_attrs,
                    parent_span_attributes: Some(filtered_parent_span_attrs),
                });
            }
        }

        // Sort by timestamp
        timeline_items.sort_by_key(|item| item.timestamp_ns);

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
            .map(|root_id| {
                build_console_span(
                    root_id,
                    &span_map,
                    &parent_to_children_map,
                    &service_name_map,
                )
            })
            .collect();
        roots.sort_by_key(|s| s.start_time);

        let min_start_time = roots.iter().map(|r| r.start_time).min().unwrap_or(0);
        let max_end_time = span_map
            .values()
            .map(|s| s.end_time_unix_nano)
            .max()
            .unwrap_or(0);
        let trace_duration_ns = max_end_time.saturating_sub(min_start_time);

        let mut table = Table::new();
        // Apply comfy-table preset and dynamic arrangement
        table
            .load_preset(presets::NOTHING)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_width(total_table_width as u16)
            .set_style(TableComponent::MiddleHeaderIntersections, '┴')
            .set_style(TableComponent::BottomBorder, '─')
            .set_style(TableComponent::BottomBorderIntersections, '─')
            .set_style(TableComponent::HeaderLines, '─');

        // Add table headers using comfy-table API with bold attribute
        table.set_header(vec![
            Cell::new("Service").add_attribute(Attribute::Bold),
            Cell::new("Span Name").add_attribute(Attribute::Bold),
            Cell::new("Kind").add_attribute(Attribute::Bold),
            Cell::new("Duration (ms)").add_attribute(Attribute::Bold),
            Cell::new("Span ID").add_attribute(Attribute::Bold),
            Cell::new("Status").add_attribute(Attribute::Bold),
            Cell::new("Timeline").add_attribute(Attribute::Bold),
        ]);

        // Define column widths for constraints
        let column_widths: [(usize, u16); 6] = [
            (0, SERVICE_NAME_WIDTH as u16),
            (1, SPAN_NAME_WIDTH as u16),
            (2, SPAN_KIND_WIDTH as u16),
            (3, DURATION_WIDTH as u16),
            (4, SPAN_ID_WIDTH as u16),
            (5, STATUS_WIDTH as u16),
        ];

        // Apply constraints to fixed-width columns (0-5)
        for (index, width) in &column_widths {
            if let Some(column) = table.column_mut(*index) {
                column.set_constraint(ColumnConstraint::UpperBoundary(Fixed(*width)));
            }
        }

        // --- Add the timeline scale row ---
        if trace_duration_ns > 0 {
            let scale_content =
                generate_timeline_scale(trace_duration_ns, calculated_timeline_width);
            if !scale_content.trim().is_empty() {
                // Add the scale row using comfy_table Cells
                table.add_row(vec![
                    Cell::new(""),            // Service
                    Cell::new(""),            // Span Name
                    Cell::new(""),            // Kind
                    Cell::new(""),            // Duration (ms)
                    Cell::new(""),            // Span ID
                    Cell::new(""),            // Status
                    Cell::new(scale_content), // Timeline scale content
                ]);
            }
        }
        // --- End timeline scale row ---

        for root in roots {
            add_span_to_table(
                &mut table,
                &root,
                0,
                min_start_time,
                trace_duration_ns,
                calculated_timeline_width,
                theme,
                &span_map,
                color_by,
            )?;
        }

        println!("{}", table);

        if !timeline_items.is_empty() {
            // Print Header
            // let header_text = if events_only {
            //     format!("Events for Trace: {}", trace_id)
            // } else {
            //     format!("Timeline Log for Trace: {}", trace_id)
            // };
            // let events_padding = total_table_width.saturating_sub(header_text.len() + 3);
            // println!(
            //     "\n{} {} {}",
            //     "─".dimmed(),
            //     header_text.bold(),
            //     "─".repeat(events_padding)
            // );

            // Print each item
            for item in timeline_items {
                let timestamp = Utc.timestamp_nanos(item.timestamp_ns as i64);
                let formatted_time = timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();

                // Get color for span ID prefix
                let (prefix_r, prefix_g, prefix_b) = match color_by {
                    ColoringMode::Service => theme.get_color_for_service(&item.service_name),
                    ColoringMode::Span => theme.get_color_for_span(&item.span_id),
                };

                let span_id_prefix = item.span_id.chars().take(8).collect::<String>();

                // Color span ID
                let colored_span_id_prefix = if item.is_error {
                    span_id_prefix
                        .truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2)
                        .to_string()
                } else {
                    span_id_prefix
                        .truecolor(prefix_r, prefix_g, prefix_b)
                        .to_string()
                };

                // Type tag
                let type_tag = match item.item_type {
                    ItemType::SpanStart => "[SPAN]".to_string(),
                    ItemType::Event => "[EVENT]".to_string(),
                };

                // Format and color level/status
                let colored_level_status = match item.level_or_status.to_uppercase().as_str() {
                    "ERROR" => item
                        .level_or_status
                        .truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2)
                        .bold(),
                    "WARN" | "WARNING" => item.level_or_status.yellow().bold(),
                    "OK" => item.level_or_status.green(), // For span status OK
                    "UNSET" => item.level_or_status.dimmed(), // For span status UNSET
                    _ => item.level_or_status.bright_black().bold(), // Default for INFO etc.
                };
                let level_status_tag = format!("[{}]", colored_level_status);

                // Format attributes
                let mut attrs_to_display: Vec<String> = Vec::new();

                // Add the item's own attributes
                for attr in &item.attributes {
                    attrs_to_display.push(format_keyvalue(attr));
                }

                // If it's an Event, also add parent span attributes
                if let Some(parent_attrs) = &item.parent_span_attributes {
                    for attr in parent_attrs {
                        let value_str = format_anyvalue(&attr.value);
                        attrs_to_display.push(format!(
                            "{}: {}",
                            attr.key.bright_black(),
                            value_str
                        ));
                    }
                }

                // Format the attrs suffix
                let attrs_suffix = if !attrs_to_display.is_empty() {
                    format!(" - {}", attrs_to_display.join(", "))
                } else {
                    String::new()
                };

                // Assemble and print the final line
                println!(
                    "{} {} [{}] {} {} {}{}",
                    formatted_time.bright_black(),
                    colored_span_id_prefix,
                    item.service_name,
                    type_tag,
                    level_status_tag,
                    item.name,
                    attrs_suffix
                );
            }
        }
    }

    Ok(())
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

    // --- Get Color (still needed for timeline bar) ---
    let (r, g, b) = match color_by {
        ColoringMode::Service => theme.get_color_for_service(&node.service_name),
        ColoringMode::Span => theme.get_color_for_span(&node.id),
    };

    // --- Create Uncolored Cell Content ---
    let service_name_content = node
        .service_name
        .chars()
        .take(SERVICE_NAME_WIDTH)
        .collect::<String>();

    let span_name_width = SPAN_NAME_WIDTH.saturating_sub(indent.len());
    let truncated_span_name = node.name.chars().take(span_name_width).collect::<String>();
    let span_name_cell_content = format!("{} {}", indent, truncated_span_name);

    let duration_ms = node.duration_ns as f64 / 1_000_000.0;
    let duration_content = format!("{:.2}", duration_ms);

    // Timeline bar remains colored
    let bar_cell_content = render_bar(
        node.start_time,
        node.duration_ns,
        trace_start_time_ns,
        trace_duration_ns,
        timeline_width,
        (r, g, b), // Pass color to render_bar
    );

    // Get the actual span object for additional data
    let span_obj = span_map.get(&node.id);

    // Format span kind (uncolored)
    let kind_cell_content = span_obj
        .map_or("UNKNOWN".to_string(), |span| format_span_kind(span.kind))
        .chars()
        .take(SPAN_KIND_WIDTH)
        .collect::<String>();

    let span_id_prefix = node.id.chars().take(8).collect::<String>();
    let colored_span_id_str = if node.status_code == status::StatusCode::Error {
        span_id_prefix
            .truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2)
            .to_string()
    } else {
        span_id_prefix.truecolor(r, g, b).to_string()
    };

    let status_content_str = format_span_status(node.status_code);

    table.add_row(vec![
        Cell::new(service_name_content),
        Cell::new(span_name_cell_content),
        Cell::new(kind_cell_content),
        Cell::new(duration_content).set_alignment(CellAlignment::Right), // Right-align duration
        Cell::new(colored_span_id_str),
        Cell::new(status_content_str),
        Cell::new(bar_cell_content),
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
    service_color: (u8, u8, u8),
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

    // Apply color to the entire bar string
    let (r, g, b) = service_color;
    bar_content.truecolor(r, g, b).to_string()
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

fn format_keyvalue(kv: &KeyValue) -> String {
    let value_str = format_anyvalue(&kv.value);
    format!("{}: {}", kv.key.bright_black(), value_str)
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
        status::StatusCode::Ok => "OK".green().to_string(),
        status::StatusCode::Error => "ERROR"
            .truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2)
            .bold()
            .to_string(),
        status::StatusCode::Unset => "UNSET".dimmed().to_string(),
    }
}
