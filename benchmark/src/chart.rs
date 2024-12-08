use crate::benchmark::calculate_stats;
use crate::types::BenchmarkReport;
use anyhow::Result;
use headless_chrome::{protocol::cdp::Page::CaptureScreenshotFormatOption, Browser, LaunchOptions};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tera::{Context, Tera};

pub async fn generate_chart_visualization(
    input_directory: &str,
    output_directory: &str,
    custom_title: Option<&str>,
    take_screenshots: bool,
) -> Result<()> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_directory)?;

    // Read all JSON files in the directory
    let mut results = Vec::new();
    let mut function_names = Vec::new();

    for entry in fs::read_dir(input_directory)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)?;
            let report: BenchmarkReport = serde_json::from_str(&content)?;

            // Extract function name from ARN (last part) or use full name
            let name = report
                .config
                .function_name
                .split(':')
                .last()
                .unwrap_or(&report.config.function_name)
                .to_string();

            function_names.push(name);
            results.push(report);
        }
    }

    // Statistics labels in order
    let stat_labels = ["AVG", "MIN", "MAX", "P95", "P50"];

    // Calculate statistics for cold starts
    let cold_stats: Vec<_> = results
        .iter()
        .map(|report| {
            let durations: Vec<f64> = report
                .cold_starts
                .iter()
                .filter_map(|m| m.init_duration)
                .collect();
            let stats = calculate_stats(&durations);
            (stats.mean, stats.min, stats.max, stats.p95, stats.p50)
        })
        .collect();

    // Calculate statistics for warm starts
    let warm_stats: Vec<_> = results
        .iter()
        .map(|report| {
            let durations: Vec<f64> = report.warm_starts.iter().map(|m| m.duration).collect();
            let stats = calculate_stats(&durations);
            (stats.mean, stats.min, stats.max, stats.p95, stats.p50)
        })
        .collect();

    // Calculate statistics for memory usage (warm starts only)
    let memory_stats: Vec<_> = results
        .iter()
        .map(|report| {
            let memory: Vec<f64> = report
                .warm_starts
                .iter()
                .map(|m| m.max_memory_used as f64)
                .collect();
            let stats = calculate_stats(&memory);
            (stats.mean, stats.min, stats.max, stats.p95, stats.p50)
        })
        .collect();

    // Create series data for each chart
    let cold_series: Vec<_> = function_names
        .iter()
        .zip(cold_stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": "{c} ms"
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
        .collect();

    let warm_series: Vec<_> = function_names
        .iter()
        .zip(warm_stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": "{c} ms"
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
        .collect();

    let memory_series: Vec<_> = function_names
        .iter()
        .zip(memory_stats.iter())
        .map(|(name, &(avg, min, max, p95, p50))| {
            json!({
                "type": "bar",
                "name": name,
                "label": {
                    "show": true,
                    "position": "right",
                    "formatter": "{c} MB"
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
        .collect();

    // Create the chart options
    let create_options = |series: Vec<serde_json::Value>, unit: &str| -> serde_json::Value {
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
                "top": "top",
                "orient": "vertical",
                "right": 10
            },
            "grid": [{
                "left": "10%",
                "top": "15%",
                "right": "15%",
                "bottom": "10%"
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
                "data": stat_labels
            }],
            "series": series
        })
    };

    // Initialize Tera
    let mut tera = Tera::default();
    tera.add_raw_template("chart.html", include_str!("templates/chart.html"))?;

    // Generate cold starts chart
    let mut cold_ctx = Context::new();
    cold_ctx.insert("title", custom_title.unwrap_or("Cold Start"));
    cold_ctx.insert("config", &results[0].config);
    cold_ctx.insert("chart_id", "chart");
    cold_ctx.insert(
        "options",
        &serde_json::to_string(&create_options(cold_series, "ms"))?,
    );
    cold_ctx.insert("page_type", "cold");
    let cold_html = tera.render("chart.html", &cold_ctx)?;
    let cold_path = PathBuf::from(output_directory).join("cold_starts.html");
    fs::write(&cold_path, cold_html)?;

    // Take screenshot of cold starts chart if requested
    if take_screenshots {
        take_chart_screenshot(cold_path).await?;
    }

    // Generate warm starts chart
    let mut warm_ctx = Context::new();
    warm_ctx.insert("title", custom_title.unwrap_or("Warm Start"));
    warm_ctx.insert("config", &results[0].config);
    warm_ctx.insert("chart_id", "chart");
    warm_ctx.insert(
        "options",
        &serde_json::to_string(&create_options(warm_series, "ms"))?,
    );
    warm_ctx.insert("page_type", "warm");
    let warm_html = tera.render("chart.html", &warm_ctx)?;
    let warm_path = PathBuf::from(output_directory).join("warm_starts.html");
    fs::write(&warm_path, warm_html)?;

    // Take screenshot of warm starts chart if requested
    if take_screenshots {
        take_chart_screenshot(warm_path).await?;
    }

    // Generate memory usage chart
    let mut memory_ctx = Context::new();
    memory_ctx.insert("title", custom_title.unwrap_or("Memory Usage"));
    memory_ctx.insert("config", &results[0].config);
    memory_ctx.insert("chart_id", "chart");
    memory_ctx.insert(
        "options",
        &serde_json::to_string(&create_options(memory_series, "MB"))?,
    );
    memory_ctx.insert("page_type", "memory");
    let memory_html = tera.render("chart.html", &memory_ctx)?;
    let memory_path = PathBuf::from(output_directory).join("memory_usage.html");
    fs::write(&memory_path, memory_html)?;

    // Take screenshot of memory usage chart if requested
    if take_screenshots {
        take_chart_screenshot(memory_path).await?;
    }

    Ok(())
}

async fn take_chart_screenshot(html_path: PathBuf) -> Result<()> {
    let browser = Browser::new(LaunchOptions::default_builder().build()?)?;
    let tab = browser.new_tab()?;
    let url = format!("file://{}", html_path.canonicalize()?.display());
    let viewport = tab
        .navigate_to(&url)?
        .wait_for_element("body")?
        .get_box_model()?
        .margin_viewport();
    let png_data = tab.capture_screenshot(
        CaptureScreenshotFormatOption::Png,
        Some(75),
        Some(viewport),
        true,
    )?;

    let screenshot_path = html_path.with_extension("png");
    fs::write(screenshot_path, png_data)?;
    Ok(())
}
