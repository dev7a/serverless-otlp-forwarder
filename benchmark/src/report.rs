use crate::benchmark::calculate_stats;
use crate::types::{
    BenchmarkConfig, BenchmarkReport, ClientMetrics, ColdStartMetrics, TestMetadata,
    WarmStartMetrics,
};
use anyhow::Result;
#[cfg(feature = "screenshots")]
use headless_chrome::{protocol::cdp::Page::CaptureScreenshotFormatOption, Browser, LaunchOptions};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde_json::json;
use statrs::statistics::OrderStatistics;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tera::Tera;

/// Calculate statistics for cold start init duration
fn calculate_cold_start_init_stats(
    cold_starts: &[ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = cold_starts.iter().map(|m| m.init_duration).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.min, stats.max, stats.p95, stats.p50))
}

/// Calculate statistics for cold start server duration
fn calculate_cold_start_server_stats(
    cold_starts: &[ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = cold_starts.iter().map(|m| m.duration).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.min, stats.max, stats.p95, stats.p50))
}

/// Calculate statistics for warm start metrics
fn calculate_warm_start_stats(
    warm_starts: &[WarmStartMetrics],
    field: fn(&WarmStartMetrics) -> f64,
) -> Option<(f64, f64, f64, f64, f64)> {
    if warm_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = warm_starts.iter().map(field).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.min, stats.max, stats.p95, stats.p50))
}

/// Calculate statistics for client metrics
fn calculate_client_stats(
    client_measurements: &[ClientMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    if client_measurements.is_empty() {
        return None;
    }
    let durations: Vec<f64> = client_measurements
        .iter()
        .map(|m| m.client_duration)
        .collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.min, stats.max, stats.p95, stats.p50))
}

/// Calculate memory usage statistics
fn calculate_memory_stats(warm_starts: &[WarmStartMetrics]) -> Option<(f64, f64, f64, f64, f64)> {
    if warm_starts.is_empty() {
        return None;
    }
    let memory: Vec<f64> = warm_starts
        .iter()
        .map(|m| m.max_memory_used as f64)
        .collect();
    let stats = calculate_stats(&memory);
    Some((stats.mean, stats.min, stats.max, stats.p95, stats.p50))
}

/// Create series data for cold start init duration chart
fn create_cold_start_init_series(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)],
    unit: &str,
) -> Vec<serde_json::Value> {
    function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": format!("{{c}} {}", unit)
                },
                "data": [
                    { "value": avg.round(), "name": "AVG" },
                    { "value": min.round(), "name": "MIN" },
                    { "value": max.round(), "name": "MAX" },
                    { "value": p95.round(), "name": "P95" },
                    { "value": p50.round(), "name": "P50" }
                ]
            })
        })
        .collect()
}

/// Create series data for cold start server duration chart
fn create_cold_start_server_series(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)],
    unit: &str,
) -> Vec<serde_json::Value> {
    function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": format!("{{c}} {}", unit)
                },
                "data": [
                    { "value": avg.round(), "name": "AVG" },
                    { "value": min.round(), "name": "MIN" },
                    { "value": max.round(), "name": "MAX" },
                    { "value": p95.round(), "name": "P95" },
                    { "value": p50.round(), "name": "P50" }
                ]
            })
        })
        .collect()
}

/// Create series data for client duration chart
fn create_client_series(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)],
    unit: &str,
) -> Vec<serde_json::Value> {
    function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": format!("{{c}} {}", unit)
                },
                "data": [
                    { "value": avg.round(), "name": "AVG" },
                    { "value": min.round(), "name": "MIN" },
                    { "value": max.round(), "name": "MAX" },
                    { "value": p95.round(), "name": "P95" },
                    { "value": p50.round(), "name": "P50" }
                ]
            })
        })
        .collect()
}

/// Create series data for server duration chart
fn create_server_series(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)],
    unit: &str,
) -> Vec<serde_json::Value> {
    function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": format!("{{c}} {}", unit)
                },
                "data": [
                    { "value": avg.round(), "name": "AVG" },
                    { "value": min.round(), "name": "MIN" },
                    { "value": max.round(), "name": "MAX" },
                    { "value": p95.round(), "name": "P95" },
                    { "value": p50.round(), "name": "P50" }
                ]
            })
        })
        .collect()
}

/// Create series data for net duration chart
fn create_net_series(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)],
    unit: &str,
) -> Vec<serde_json::Value> {
    function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": format!("{{c}} {}", unit)
                },
                "data": [
                    { "value": avg.round(), "name": "AVG" },
                    { "value": min.round(), "name": "MIN" },
                    { "value": max.round(), "name": "MAX" },
                    { "value": p95.round(), "name": "P95" },
                    { "value": p50.round(), "name": "P50" }
                ]
            })
        })
        .collect()
}

/// Create series data for memory usage chart
fn create_memory_series(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)],
    unit: &str,
) -> Vec<serde_json::Value> {
    function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": format!("{{c}} {}", unit)
                },
                "data": [
                    { "value": avg.round(), "name": "AVG" },
                    { "value": min.round(), "name": "MIN" },
                    { "value": max.round(), "name": "MAX" },
                    { "value": p95.round(), "name": "P95" },
                    { "value": p50.round(), "name": "P50" }
                ]
            })
        })
        .collect()
}

/// Create chart options for bar charts
fn create_bar_chart_options(
    series: Vec<serde_json::Value>,
    _title: &str,
    unit: &str,
) -> serde_json::Value {
    json!({
        "tooltip": {
            "trigger": "axis",
            "axisPointer": {
                "type": "shadow"
            }
        },
        "color": [
            "#3fb1e3",
            "#6be6c1",
            "#626c91",
            "#a0a7e6",
            "#c4ebad",
            "#96dee8"
        ],
        "legend": {
            "orient": "horizontal",
            "top": 10
        },
        "grid": [{
            "left": "10%",
            "top": "15%",
            "right": "15%",
            "bottom": "10%",
            "containLabel": true
        }],
        "xAxis": [{
            "type": "value",
            "name": format!("{} ({})", if unit == "MB" { "Memory" } else { "Duration" }, unit),
            "nameLocation": "middle",
            "nameGap": 30,
            "axisLabel": {
                "formatter": format!("{{value}} {}", unit)
            },
            "minInterval": 1
        }],
        "yAxis": [{
            "type": "category",
            "inverse": true,
            "data": ["AVG", "MIN", "MAX", "P95", "P50"]
        }],
        "series": series,
        "toolbox": {
            "feature": {
                "restore": {},
                "saveAsImage": {}
            },
            "right": "20px"
        }

    })
}

/// Create line chart series for warm start client duration over time
fn create_client_duration_time_series(
    results: &[BenchmarkReport],
    function_names: &[String],
) -> (Vec<serde_json::Value>, f64) {
    let gap = 5; // Fixed small gap
    let mut current_offset = 0;

    // Process each series in a single pass
    let series_data = function_names
        .iter()
        .zip(results.iter())
        .map(|(name, report)| {
            // Record current offset for this series
            let x_offset = current_offset;
            let points = report.client_measurements.len();

            // Update offset for next series
            current_offset += points + gap;

            // Calculate mean value for this series
            let mean_value = if !report.client_measurements.is_empty() {
                let sum: f64 = report
                    .client_measurements
                    .iter()
                    .map(|m| m.client_duration)
                    .sum();
                sum / report.client_measurements.len() as f64
            } else {
                0.0
            };

            // Create data points for this series
            let data = report
                .client_measurements
                .iter()
                .enumerate()
                .map(|(index, m)| {
                    json!({
                        "value": [x_offset + index, m.client_duration.round()],
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "name": name,
                "type": "scatter",
                "smooth": true,
                "showSymbol": true,
                "label": { "show": false },
                "data": data,
                "markLine": {
                    "silent": true,
                    "symbol": ["none", "none"],  // Remove arrows at both ends
                    "lineStyle": {
                        "color": "#333",
                        "width": 1,
                        "type": "dashed"
                    },
                    "data": [
                        {
                            "yAxis": mean_value.round(),
                            "xAxis": x_offset,
                        },
                        {
                            "yAxis": mean_value.round(),
                            "xAxis": x_offset + points - 1,
                            "label": {
                                "formatter": "{c} ms",
                                "position": "end"
                            }
                        }
                    ]
                }
            })
        })
        .collect();

    // Calculate total width (subtract final gap)
    let total_width = if current_offset > gap {
        current_offset.saturating_sub(gap)
    } else {
        0
    };

    (series_data, total_width as f64)
}

/// Create line chart options
fn create_line_chart_options(
    series: Vec<serde_json::Value>,
    _title: &str,
    unit: &str,
    max_x: Option<f64>,
) -> serde_json::Value {
    // Determine max_x value (either provided or calculated)
    let max_x = max_x.unwrap_or_else(|| {
        // Extract the maximum x value across all data points
        series
            .iter()
            .flat_map(|s| s["data"].as_array().unwrap_or(&Vec::new()).to_vec())
            .filter_map(|point| {
                point["value"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|x| x.as_f64())
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(100.0)
    });

    // Calculate reasonable y-axis range by finding all y values
    let all_y_values: Vec<f64> = series
        .iter()
        .flat_map(|s| s["data"].as_array().unwrap_or(&Vec::new()).to_vec())
        .filter_map(|point| {
            point["value"]
                .as_array()
                .and_then(|arr| arr.get(1))
                .and_then(|y| y.as_f64())
        })
        .collect();

    // Calculate just the 95th percentile directly
    let p95 = if all_y_values.is_empty() {
        1000.0 // Default value if no data
    } else {
        use statrs::statistics::Data;
        let mut data = Data::new(all_y_values.clone());
        data.percentile(95)
    };

    // Use p95 * 1.2 as the y-axis max to show most points while excluding extreme outliers
    let y_max = (p95 * 1.2).max(1000.0); // Ensure a reasonable minimum scale

    // Create a copy of the series array
    let mut enhanced_series = series.clone();

    // Calculate the mean value for each data series
    let series_means: Vec<(String, f64)> = series
        .iter()
        .map(|s| {
            let name = s["name"].as_str().unwrap_or("Unknown").to_string();
            let values: Vec<f64> = s["data"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|point| {
                    point["value"]
                        .as_array()
                        .and_then(|arr| arr.get(1))
                        .and_then(|y| y.as_f64())
                })
                .collect();

            let mean = if !values.is_empty() {
                values.iter().sum::<f64>() / values.len() as f64
            } else {
                0.0
            };

            (name, mean)
        })
        .collect();

    // Create mean trend lines for each series
    let mean_trend_data: Vec<serde_json::Value> = series
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            let name = s["name"].as_str().unwrap_or("Unknown");
            let mean = series_means[idx].1;

            // Find x-value range for this series
            let x_values: Vec<f64> = s["data"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|point| {
                    point["value"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|x| x.as_f64())
                })
                .collect();

            if x_values.is_empty() {
                return json!(null);
            }

            let min_x = x_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_x = x_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            json!({
                "name": format!("{} Mean", name),
                "type": "line",
                "smooth": true,
                "showSymbol": false,
                "symbolSize": 0,
                "lineStyle": {
                    "width": 2,
                    "type": "dashed"
                },
                "data": [
                    [min_x, mean.round()],
                    [max_x, mean.round()]
                ],
                "tooltip": {
                    "formatter": format!("{} Mean: {:.1} {}", name, mean, unit)
                },
                "zlevel": 1,  // Ensure it renders above the scatter points
                "legendHoverLink": false,  // Don't highlight when hovering legend
                "silent": true  // Don't respond to mouse events
            })
        })
        .filter(|item| !item.is_null())
        .collect();

    // Add all the mean trend lines to the series
    for trend_line in mean_trend_data {
        enhanced_series.push(trend_line);
    }

    // Filter the legend to only show data series (not mean lines)
    let legend_data: Vec<String> = enhanced_series
        .iter()
        .filter_map(|s| {
            let name = s["name"].as_str().unwrap_or("").to_string();
            if name.contains("Mean") {
                None // Skip mean lines in legend
            } else {
                Some(name)
            }
        })
        .collect();

    json!({
        "tooltip": { "trigger": "axis", "axisPointer": { "type": "cross" } },
        "color": ["#3fb1e3", "#6be6c1", "#626c91", "#a0a7e6", "#c4ebad", "#96dee8", "#ff9933", "#ff6666", "#99cc66", "#cc99ff"],
        "grid": {
            "top": "10%", "bottom": "5%", "left": "3%", "right": "9%", "containLabel": true
        },
        "legend": {
            "data": legend_data,
            "top": "10"
        },
        "xAxis": {
            "type": "value",
            "name": "Test Sequence",
            "nameLocation": "middle",
            "nameGap": 30,
            "min": 0,
            "max": max_x + 1.0,
            "minInterval": 1,
            "boundaryGap": false,
            "splitLine": { "show": false }
        },
        "yAxis": {
            "type": "value",
            "name": format!("Duration ({})", unit),
            "nameLocation": "middle",
            "nameGap": 50,
            "splitLine": { "show": true },
            "max": y_max,
            "axisLabel": {
                "formatter": format!("{{value}} {}", unit)
            }
        },
        "series": enhanced_series,
        "toolbox": {
            "feature": {
                "restore": {},
                "saveAsImage": {}
            },
            "right": "20px"
        }
    })
}

/// Generate a chart with the given options
#[allow(clippy::too_many_arguments)]
async fn generate_chart(
    html_dir: &Path,
    png_dir: Option<&Path>,
    name: &str,
    title: &str,
    options: serde_json::Value,
    config: &BenchmarkConfig,
    page_type: &str,
    screenshot_theme: Option<&str>,
    pb: &ProgressBar,
) -> Result<()> {
    // Initialize Tera
    let mut tera = Tera::default();
    tera.add_raw_template("chart.html", include_str!("templates/chart.html"))?;

    // Create context
    let mut ctx = tera::Context::new();
    ctx.insert("title", title);
    ctx.insert("config", config);
    ctx.insert("chart_id", "chart");
    ctx.insert("options", &serde_json::to_string(&options)?);
    ctx.insert("page_type", page_type);

    // Add breadcrumb navigation context
    let mut breadcrumbs = Vec::new();
    let mut current_path = PathBuf::from(html_dir);
    let relative_path = PathBuf::new();

    // Collect all path components
    let mut components = Vec::new();
    while let Some(component) = current_path.file_name() {
        components.push(component.to_string_lossy().to_string());
        current_path.pop();
    }
    components.reverse();

    // Build breadcrumbs with relative paths
    for (i, name) in components.iter().enumerate() {
        let path = if i == components.len() - 1 {
            // Directory component - add link to index
            "index.html".to_string()
        } else {
            // Parent directories - add relative path
            let mut path = relative_path.clone();
            for _ in i..components.len() - 1 {
                path.push("..");
            }
            format!("{}/index.html", path.display())
        };
        breadcrumbs.push(json!({
            "name": name,
            "path": path,
        }));
    }

    // Add the chart name as the last breadcrumb (no link)
    breadcrumbs.push(json!({
        "name": name,
        "path": "",
    }));

    ctx.insert("breadcrumbs", &breadcrumbs);

    // Add runtime info
    ctx.insert("runtime", config.runtime.as_deref().unwrap_or("unknown"));
    ctx.insert(
        "architecture",
        config.architecture.as_deref().unwrap_or("unknown"),
    );
    ctx.insert("memory", &config.memory_size.unwrap_or(128));
    ctx.insert("concurrency", &config.concurrent_invocations);
    ctx.insert("rounds", &config.rounds);
    ctx.insert("timestamp", &config.timestamp);

    // Generate HTML
    pb.set_message(format!("Generating {} chart...", name));
    let html_path = html_dir.join(format!("{}.html", name));
    pb.set_message(format!("Rendering {}...", html_path.display()));
    let html = tera.render("chart.html", &ctx)?;
    fs::write(&html_path, html)?;

    // Take screenshot if requested
    if let Some(png_dir) = png_dir {
        if let Some(theme) = screenshot_theme {
            let screenshot_path = png_dir.join(format!("{}.png", name));
            pb.set_message(format!("Generating {}...", screenshot_path.display()));
            take_chart_screenshot(&html_path, &screenshot_path, theme).await?;
        }
    }

    Ok(())
}

/// Generate an index page
async fn generate_index(
    output_dir: &Path,
    title: &str,
    description: Option<&str>,
    breadcrumbs: Vec<(String, String)>,
    items: Vec<IndexItem>,
    pb: &ProgressBar,
) -> Result<PathBuf> {
    // Initialize Tera
    let mut tera = Tera::default();
    tera.add_raw_template("index.html", include_str!("templates/index.html"))?;

    // Create context
    let mut ctx = tera::Context::new();
    ctx.insert("title", title);
    if let Some(desc) = description {
        ctx.insert("description", desc);
    }

    // Format breadcrumbs for template
    let breadcrumbs = breadcrumbs
        .into_iter()
        .map(|(name, path)| {
            json!({
                "name": name,
                "path": path,
            })
        })
        .collect::<Vec<_>>();
    ctx.insert("breadcrumbs", &breadcrumbs);

    // Format items for template
    let items = items
        .into_iter()
        .map(|item| {
            json!({
                "title": item.title,
                "path": item.path,
                "subtitle": item.subtitle,
                "metadata": item.metadata.into_iter()
                    .map(|(label, value)| json!({
                        "label": label,
                        "value": value,
                    }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    ctx.insert("items", &items);

    // Generate HTML
    pb.set_message(format!("Generating index for {}...", title));
    let index_path = output_dir.join("index.html");
    let html = tera.render("index.html", &ctx)?;
    fs::write(&index_path, html)?;

    Ok(index_path)
}

/// Represents an item in the index page
#[derive(Debug)]
struct IndexItem {
    title: String,
    subtitle: Option<String>,
    path: String,
    metadata: Vec<(String, String)>,
}

impl IndexItem {
    fn new(title: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            path: path.into(),
            metadata: Vec::new(),
        }
    }

    fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    fn with_metadata(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push((label.into(), value.into()));
        self
    }
}

pub async fn generate_reports(
    input_directory: &str,
    output_directory: &str,
    custom_title: Option<&str>,
    screenshot_theme: Option<&str>,
) -> Result<()> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_directory)?;

    // Setup progress indicators
    let m = MultiProgress::new();
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .unwrap();

    // Main progress bar
    let main_pb = m.add(ProgressBar::new_spinner());
    main_pb.set_style(spinner_style.clone());
    main_pb.set_prefix("[1/2] Processing");
    main_pb.enable_steady_tick(Duration::from_millis(100));
    main_pb.set_message(format!("Scanning directory {}...", input_directory));

    // Process the directory recursively
    process_directory(
        input_directory,
        output_directory,
        custom_title,
        screenshot_theme,
        &m,
        &spinner_style,
        &main_pb,
    )
    .await?;

    main_pb.finish_with_message(format!("✓ Completed scanning {}", input_directory));
    m.clear()?;

    // Print path to index.html
    let index_path = PathBuf::from(output_directory).join("index.html");
    if index_path.exists() {
        println!("\n✨ Report generated successfully!");
        println!("📊 View the report at: {}", index_path.display());
    }

    Ok(())
}

async fn process_directory(
    input_directory: &str,
    output_directory: &str,
    custom_title: Option<&str>,
    screenshot_theme: Option<&str>,
    m: &MultiProgress,
    spinner_style: &ProgressStyle,
    main_pb: &ProgressBar,
) -> Result<()> {
    main_pb.set_message(format!("Scanning directory {}...", input_directory));
    let mut has_json = false;
    let mut items = Vec::new();

    // Create output directory if it doesn't exist
    fs::create_dir_all(output_directory)?;

    // Try to read metadata.yaml if it exists
    let metadata_path = PathBuf::from(input_directory).join("metadata.yaml");
    let (title, description) = if metadata_path.exists() {
        let metadata: TestMetadata = serde_yaml::from_reader(
            std::fs::File::open(&metadata_path)
                .map_err(|e| anyhow::anyhow!("Failed to open metadata file: {}", e))?,
        )
        .map_err(|e| anyhow::anyhow!("Failed to parse metadata file: {}", e))?;
        (metadata.title, Some(metadata.description))
    } else {
        (
            PathBuf::from(input_directory)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Benchmark Results".to_string()),
            None,
        )
    };

    // Process subdirectories first
    for entry in fs::read_dir(input_directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let dir_name = entry.file_name();
            let input_subdir = PathBuf::from(input_directory).join(&dir_name);
            let output_subdir = PathBuf::from(output_directory).join(&dir_name);

            // Create output subdirectory
            fs::create_dir_all(&output_subdir)?;

            // Process subdirectory
            Box::pin(process_directory(
                input_subdir.to_str().unwrap(),
                output_subdir.to_str().unwrap(),
                custom_title,
                screenshot_theme,
                m,
                spinner_style,
                main_pb,
            ))
            .await?;

            // Add directory as an item
            items.push(
                IndexItem::new(
                    dir_name.to_string_lossy().as_ref(),
                    format!("{}/index.html", dir_name.to_string_lossy()),
                )
                .with_subtitle("Directory"),
            );
        }
    }

    // Check for JSON files and generate reports
    for entry in fs::read_dir(input_directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            has_json = true;
            break;
        }
    }

    if has_json {
        // Generate reports for this directory
        generate_reports_for_directory(
            input_directory,
            output_directory,
            custom_title,
            screenshot_theme,
            main_pb,
        )
        .await?;

        // Add chart files to items
        let chart_files = [
            (
                "Cold Start - Init Duration",
                "cold_start_init.html",
                "Initialization duration for cold starts",
            ),
            (
                "Cold Start - Duration",
                "cold_start_server.html",
                "Server duration for cold starts",
            ),
            (
                "Warm Start - Client Duration",
                "client_duration.html",
                "Client-side duration for warm starts",
            ),
            (
                "Warm Start - Server Duration",
                "server_duration.html",
                "Server-side duration for warm starts",
            ),
            (
                "Warm Start - Net Duration",
                "net_duration.html",
                "Net duration for warm starts",
            ),
            (
                "Warm Start - Client Duration Over Time",
                "client_duration_time.html",
                "Client duration trend over time",
            ),
            (
                "Memory Usage",
                "memory_usage.html",
                "Memory usage statistics",
            ),
        ];

        for (title, filename, description) in chart_files {
            if PathBuf::from(output_directory).join(filename).exists() {
                items.push(
                    IndexItem::new(title, filename.to_string())
                        .with_subtitle(description)
                        .with_metadata("Type", "Chart"),
                );
            }
        }
    }

    // Generate index.html for current directory if it has content
    if !items.is_empty() {
        let dir_pb = m.add(ProgressBar::new_spinner());
        dir_pb.set_style(spinner_style.clone());
        dir_pb.set_prefix("[2/2] Generating");
        dir_pb.enable_steady_tick(Duration::from_millis(100));
        dir_pb.set_message(format!("Creating index for {}...", input_directory));

        // Create breadcrumbs
        let mut breadcrumbs = Vec::new();
        let mut current_path = PathBuf::from(input_directory);
        let mut components = Vec::new();

        // Collect all path components
        while let Some(component) = current_path.file_name() {
            components.push(component.to_string_lossy().to_string());
            current_path.pop();
        }
        components.reverse();

        // Build breadcrumbs with relative paths
        let relative_path = PathBuf::new();
        for (i, name) in components.iter().enumerate() {
            let path = if i == components.len() - 1 {
                // Last component (current directory) - no link
                String::new()
            } else {
                // Parent directories - add relative path
                let mut path = relative_path.clone();
                for _ in i..components.len() - 1 {
                    path.push("..");
                }
                format!("{}/index.html", path.display())
            };
            breadcrumbs.push((name.clone(), path));
        }

        let index_path = PathBuf::from(output_directory).join("index.html");
        if !index_path.exists() {
            generate_index(
                index_path.parent().unwrap(),
                &title,
                description.as_deref(),
                breadcrumbs,
                items,
                &dir_pb,
            )
            .await?;
        }

        dir_pb.finish_with_message(format!("✓ Created index for {}", input_directory));
    }

    Ok(())
}

pub async fn generate_reports_for_directory(
    input_directory: &str,
    output_directory: &str,
    custom_title: Option<&str>,
    screenshot_theme: Option<&str>,
    pb: &ProgressBar,
) -> Result<()> {
    // Create output directory for PNG files if screenshots are enabled
    let png_dir = if screenshot_theme.is_some() {
        let dir = PathBuf::from(output_directory).join("png");
        fs::create_dir_all(&dir)?;
        Some(dir)
    } else {
        None
    };

    // Read all JSON files in the directory
    let mut results = Vec::new();
    let mut function_names = Vec::new();

    // Collect all files first
    let mut entries = Vec::new();
    for entry in fs::read_dir(input_directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            entries.push((
                path.clone(),
                path.file_stem().unwrap().to_string_lossy().to_string(),
            ));
        }
    }

    // Sort entries by function name for consistent ordering
    entries.sort_by(|a, b| a.1.cmp(&b.1));

    // Process sorted entries
    for (path, name) in entries {
        let content = fs::read_to_string(&path)?;
        let report: BenchmarkReport = serde_json::from_str(&content)?;
        results.push(report);
        function_names.push(name);
    }

    if results.is_empty() {
        return Err(anyhow::anyhow!("No benchmark results found in '{}' or its subdirectories. Please check the directory path.", input_directory));
    }

    // Calculate statistics and generate charts
    let cold_init_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_init_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let cold_server_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_server_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let client_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_client_stats(&report.client_measurements).unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let server_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_warm_start_stats(&report.warm_starts, |m| m.duration)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let net_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_warm_start_stats(&report.warm_starts, |m| m.net_duration)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let memory_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_memory_stats(&report.warm_starts).unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    // Generate cold start init duration chart if we have data
    if results.iter().any(|r| !r.cold_starts.is_empty()) {
        let cold_init_series =
            create_cold_start_init_series(&function_names, &cold_init_stats, "ms");
        let cold_init_options =
            create_bar_chart_options(cold_init_series, "Cold Start - Init Duration", "ms");
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_init",
            custom_title.unwrap_or("Cold Start - Init Duration"),
            cold_init_options,
            &results[0].config,
            "cold_init",
            screenshot_theme,
            pb,
        )
        .await?;

        // Generate cold start server duration chart
        let cold_server_series =
            create_cold_start_server_series(&function_names, &cold_server_stats, "ms");
        let cold_server_options =
            create_bar_chart_options(cold_server_series, "Cold Start - Duration", "ms");
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_server",
            custom_title.unwrap_or("Cold Start - Duration"),
            cold_server_options,
            &results[0].config,
            "cold_server",
            screenshot_theme,
            pb,
        )
        .await?;
    }

    // Generate client duration chart if we have data
    if results.iter().any(|r| !r.client_measurements.is_empty()) {
        let client_series = create_client_series(&function_names, &client_stats, "ms");
        let client_options =
            create_bar_chart_options(client_series, "Warm Start - Client Duration", "ms");
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "client_duration",
            custom_title.unwrap_or("Warm Start - Client Duration"),
            client_options,
            &results[0].config,
            "client",
            screenshot_theme,
            pb,
        )
        .await?;
    }

    // Generate server duration chart if we have data
    if results.iter().any(|r| !r.warm_starts.is_empty()) {
        let server_series = create_server_series(&function_names, &server_stats, "ms");
        let server_options =
            create_bar_chart_options(server_series, "Warm Start - Server Duration", "ms");
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "server_duration",
            custom_title.unwrap_or("Warm Start - Server Duration"),
            server_options,
            &results[0].config,
            "server",
            screenshot_theme,
            pb,
        )
        .await?;

        // Generate net duration chart
        let net_series = create_net_series(&function_names, &net_stats, "ms");
        let net_options = create_bar_chart_options(net_series, "Warm Start - Net Duration", "ms");
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "net_duration",
            custom_title.unwrap_or("Warm Start - Net Duration"),
            net_options,
            &results[0].config,
            "net",
            screenshot_theme,
            pb,
        )
        .await?;

        // Generate client duration over time chart
        let (client_time_series, total_points) =
            create_client_duration_time_series(&results, &function_names);
        let client_time_options = create_line_chart_options(
            client_time_series,
            "Warm Start: Client Duration Over Time",
            "ms",
            Some(total_points),
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "client_duration_time",
            custom_title.unwrap_or("Warm Start: Client Duration Over Time"),
            client_time_options,
            &results[0].config,
            "client_time",
            screenshot_theme,
            pb,
        )
        .await?;

        // Generate memory usage chart
        let memory_series = create_memory_series(&function_names, &memory_stats, "MB");
        let memory_options = create_bar_chart_options(memory_series, "Memory Usage", "MB");
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "memory_usage",
            custom_title.unwrap_or("Memory Usage"),
            memory_options,
            &results[0].config,
            "memory",
            screenshot_theme,
            pb,
        )
        .await?;
    }

    Ok(())
}

#[cfg(feature = "screenshots")]
pub async fn take_chart_screenshot(
    html_path: &Path,
    screenshot_path: &Path,
    theme: &str,
) -> Result<()> {
    let browser = Browser::new(LaunchOptions::default_builder().build().unwrap())?;
    let tab = browser.new_tab()?;

    // Convert to absolute path and create proper file URL
    let absolute_path = html_path.canonicalize()?;
    let url = format!("file://{}", absolute_path.display());

    // Navigate to the page
    tab.navigate_to(&url)?;
    tab.wait_until_navigated()?;

    // Inject theme into localStorage and force a theme update
    tab.evaluate(
        &format!(
            r#"
        localStorage.setItem('theme', '{}');
        setTheme('{}');
    "#,
            theme, theme
        ),
        true,
    )?;

    // Wait for the chart to render
    tab.wait_for_element("#chart")?;
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Capture the entire page
    let png_data = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)?;

    std::fs::write(screenshot_path, png_data)?;
    Ok(())
}

#[cfg(not(feature = "screenshots"))]
pub async fn take_chart_screenshot(
    _html_path: &Path,
    _screenshot_path: &Path,
    _theme: &str,
) -> Result<()> {
    Ok(())
}
