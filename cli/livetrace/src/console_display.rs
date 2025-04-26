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
use terminal_size::{self, Width, Height};
use crate::cli::ColoringMode;
use crate::processing::TelemetryData; // Need TelemetryData for display_console

// Constants
const SERVICE_NAME_WIDTH: usize = 25;
const SPAN_NAME_WIDTH: usize = 40;
const SPAN_ID_WIDTH: usize = 32;
const SPAN_KIND_WIDTH: usize = 10;    // Width for the Span Kind column
const SPAN_ATTRS_WIDTH: usize = 30;   // Width for the Span Attributes column
const DURATION_WIDTH: usize = 10;     // Width for the Duration column

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

impl Theme {
    // Parse a theme name string to an enum value
    pub fn from_str(theme_name: &str) -> Self {
        match theme_name.to_lowercase().as_str() {
            "tableau" => Theme::Tableau,
            "colorbrewer" => Theme::ColorBrewer,
            "material" => Theme::Material,
            "solarized" => Theme::Solarized,
            "monochrome" => Theme::Monochrome,
            _ => Theme::Default,
        }
    }

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

#[derive(Debug)]
struct EventInfo {
    timestamp_ns: u64,
    name: String,
    span_id: String,
    #[allow(dead_code)]
    trace_id: String,
    attributes: Vec<KeyValue>,     // Event's own attributes
    span_attributes: Vec<KeyValue>, // Attributes from the parent span
    service_name: String,
    is_error: bool,
    level: String,
}

// Function to get terminal width with a default fallback
pub fn get_terminal_width(default_width: usize) -> usize {
    if let Some((Width(w), Height(_h))) = terminal_size::terminal_size() {
        w as usize
    } else {
        default_width // Fallback if terminal size can't be determined
    }
}

// Public Display Function
pub fn display_console(
    batch: &[TelemetryData],
    compact_display: bool,
    event_attr_globs: &Option<GlobSet>,
    event_severity_attribute_name: &str,
    theme: Theme,
    span_attr_globs: &Option<GlobSet>,
    color_by: ColoringMode,
) -> Result<()> {
    // Debug logging with theme and coloring mode
    tracing::debug!("Display console called with theme={:?}, color_by={:?}", theme, color_by);

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
    const SPACING_NON_COMPACT: usize = 6; // Approx spaces between 7 columns
    const SPACING_COMPACT: usize = 3; // Approx spaces between 4 columns

    // Calculate the fixed width excluding the timeline
    let fixed_width_excluding_timeline = if compact_display {
        SERVICE_NAME_WIDTH + SPAN_NAME_WIDTH + DURATION_WIDTH + SPACING_COMPACT
    } else {
        SERVICE_NAME_WIDTH + SPAN_NAME_WIDTH + SPAN_KIND_WIDTH + DURATION_WIDTH + SPAN_ID_WIDTH + SPAN_ATTRS_WIDTH + SPACING_NON_COMPACT
    };
    
    // Get terminal width and calculate dynamic timeline width
    let terminal_width = get_terminal_width(120); // Use a larger default fallback
    // Ensure timeline width is at least some minimum (e.g., 10) or doesn't cause overflow
    let calculated_timeline_width = terminal_width.saturating_sub(fixed_width_excluding_timeline).max(10);

    // Total width is now just the terminal width for header padding
    let total_table_width = terminal_width;

    for (trace_id, spans_in_trace_with_service) in traces {
        // Print Trace ID Header
        let trace_heading = format!("Trace ID: {}", trace_id);
        // Calculate padding based on total table width
        let trace_padding = total_table_width.saturating_sub(trace_heading.len() + 3); // 3 for " ─ " and spaces
        println!(
            "\n{} {} {}",
            "─".dimmed(),
            trace_heading.bold(),
            "─".repeat(trace_padding).dimmed()
        );

        if spans_in_trace_with_service.is_empty() {
            continue;
        }

        let mut trace_events: Vec<EventInfo> = Vec::new();
        let span_error_status: HashMap<String, bool> = spans_in_trace_with_service
            .iter()
            .map(|(span, _)| {
                let span_id_hex = hex::encode(&span.span_id);
                let is_error = span.status.as_ref().is_some_and(|s| {
                    status::StatusCode::try_from(s.code).unwrap_or(status::StatusCode::Unset)
                        == status::StatusCode::Error
                });
                (span_id_hex, is_error)
            })
            .collect();

        for (span, service_name) in &spans_in_trace_with_service {
            let span_id_hex = hex::encode(&span.span_id);
            let is_error = *span_error_status.get(&span_id_hex).unwrap_or(&false);
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

                trace_events.push(EventInfo {
                    timestamp_ns: event.time_unix_nano,
                    name: event.name.clone(),
                    span_id: span_id_hex.clone(),
                    trace_id: trace_id.clone(),
                    attributes: event.attributes.clone(),
                    span_attributes: span.attributes.clone(),
                    service_name: service_name.clone(),
                    is_error,
                    level,
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
        // Use a custom format based on CLEAN
        let table_format = format::FormatBuilder::new()
            .column_separator(' ')
            .borders(' ')
            .separators(&[], format::LineSeparator::new(' ', ' ', ' ', ' '))
            .padding(1, 1)
            .build();
        table.set_format(table_format);

        // Add table headers appropriate for compact vs non-compact mode
        if compact_display {
            table.set_titles(row![
                "Service",
                "Span Name",
                "Duration (ms)",
                "Timeline"
            ]);
        } else {
            table.set_titles(row![ bl =>
                "Service",
                "Span Name",
                "Kind",
                "Duration (ms)",
                "Span ID",
                "Attributes",
                "Timeline"
            ]);
        }

        for root in roots {
            add_span_to_table(
                &mut table,
                &root,
                0,
                min_start_time,
                trace_duration_ns,
                calculated_timeline_width,
                compact_display,
                theme,
                &span_map,
                span_attr_globs,
                color_by,
            )?;
        }
        table.printstd();

        // Display sorted events *for this trace*
        if !trace_events.is_empty() {
            // Print Events Header
            let events_heading = format!("Events for Trace: {}", trace_id);
            // Calculate padding based on total table width
            let events_padding = total_table_width.saturating_sub(events_heading.len() + 3);
            println!(
                "\n{} {} {}",
                "─".dimmed(),
                events_heading.bold(),
                "─".repeat(events_padding).dimmed()
            );
            for event in trace_events {
                let timestamp = Utc.timestamp_nanos(event.timestamp_ns as i64);
                let formatted_time = timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
                let mut attrs_to_display: Vec<String> = Vec::new();
                if let Some(globs) = event_attr_globs {
                    // Add event attributes that match the glob patterns
                    for attr in &event.attributes {
                        if globs.is_match(&attr.key) {
                            attrs_to_display.push(format_keyvalue(attr));
                        }
                    }
                    
                    // Add span attributes that match the glob patterns, with a "span." prefix
                    for attr in &event.span_attributes {
                        if globs.is_match(&attr.key) {
                            // Add a "span." prefix to distinguish from event attributes
                            let value_str = format_anyvalue(&attr.value);
                            attrs_to_display.push(format!("span.{}: {}", attr.key.bright_black(), value_str));
                        }
                    }
                }

                // Get service color always for consistent service name display
                let service_color = theme.get_color_for_service(&event.service_name);
                
                // Get color for span ID display based on coloring mode
                let (prefix_r, prefix_g, prefix_b) = match color_by {
                    ColoringMode::Service => service_color,
                    ColoringMode::Span => theme.get_color_for_span(&event.span_id),
                };

                // Shorten span ID for display
                let span_id_prefix = event.span_id.chars().take(8).collect::<String>();
                
                // Apply appropriate color to span ID prefix unless it's an error
                let colored_span_id_prefix = if event.is_error {
                    span_id_prefix
                        .truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2)
                        .to_string()
                } else {
                    span_id_prefix.truecolor(prefix_r, prefix_g, prefix_b).to_string()
                };

                // Color the level based on its value
                let colored_level = match event.level.to_uppercase().as_str() {
                    "ERROR" => event
                        .level
                        .truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2)
                        .bold(),
                    "WARN" | "WARNING" => event.level.yellow().bold(),
                    _ => event.level.bright_black().bold(), // Keep others bold but with subdued color
                };

                let log_line_start = format!(
                    "{} {} [{}] [{}] {}",
                    formatted_time.bright_black(),
                    colored_span_id_prefix,
                    event.service_name.truecolor(service_color.0, service_color.1, service_color.2), // Always use service color for service name
                    colored_level,
                    event.name,
                );

                if !attrs_to_display.is_empty() {
                    println!(
                        "{} - Attrs: {}",
                        log_line_start,
                        attrs_to_display.join(", ")
                    );
                } else {
                    println!("{}", log_line_start);
                }
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
    compact_display: bool,
    theme: Theme,
    span_map: &HashMap<String, Span>,
    span_attr_globs: &Option<GlobSet>,
    color_by: ColoringMode,
) -> Result<()> {
    let indent = "  ".repeat(depth);

    // Get color based on the coloring mode
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
        node.status_code,
        (r, g, b), // Pass service color to render_bar
    );

    // Get the actual span object for additional data
    let span_obj = span_map.get(&node.id);
    
    if compact_display {
        table.add_row(row![
            service_name_content,
            span_name_cell_content,
            duration_content, // Uncolored
            bar_cell_content
        ]);
    } else {
        // Format span kind (uncolored)
        let kind_cell_content = span_obj.map_or("UNKNOWN".to_string(), |span| format_span_kind(span.kind))
                                       .chars().take(SPAN_KIND_WIDTH).collect::<String>();
        
        // Format span ID prefix (uncolored initially)
        let span_id_prefix = node.id.chars().take(8).collect::<String>();
        // ADD COLORING BACK for Span ID
        let colored_span_id_prefix = if node.status_code == status::StatusCode::Error {
            span_id_prefix
                .truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2)
                .to_string()
        } else {
            span_id_prefix.truecolor(r, g, b).to_string() // Use service color
        };
        
        // Format span attributes (uncolored)
        let attrs_cell_content = if let Some(globs) = span_attr_globs {
            if let Some(span) = span_obj {
                let mut attrs_display = Vec::new();
                for attr in &span.attributes {
                    if globs.is_match(&attr.key) {
                        // Use a plain key: value format without color
                        let value_str = format_anyvalue(&attr.value);
                        attrs_display.push(format!("{}:{}", attr.key, value_str)); 
                    }
                }
                if attrs_display.is_empty() {
                    "".to_string()
                } else {
                    let joined = attrs_display.join(", ");
                    if joined.len() > SPAN_ATTRS_WIDTH {
                        format!("{}...", joined.chars().take(SPAN_ATTRS_WIDTH - 3).collect::<String>())
                    } else {
                        joined
                    }
                }
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };

        table.add_row(row![
            service_name_content,
            span_name_cell_content,
            kind_cell_content,
            duration_content, // Uncolored
            colored_span_id_prefix, // Use the colored version
            attrs_cell_content, // Uncolored
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
            theme,
            span_map,
            span_attr_globs,
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
    status_code: status::StatusCode,
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
    if status_code == status::StatusCode::Error {
        bar_content.truecolor(ERROR_COLOR.0, ERROR_COLOR.1, ERROR_COLOR.2).to_string()
    } else {
        let (r, g, b) = service_color;
        bar_content.truecolor(r, g, b).to_string()
    }
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
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::KvlistValue(_)) => {
                "[kvlist]".to_string()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::BytesValue(_)) => {
                "[bytes]".to_string()
            }
            None => "<empty_value>".to_string(),
        },
        None => "<no_value>".to_string(),
    }
}

