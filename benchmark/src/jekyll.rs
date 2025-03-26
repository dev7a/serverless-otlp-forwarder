use crate::benchmark::calculate_stats;
use crate::types::{
    BatchConfig, BatchTestConfig, BenchmarkReport, ClientMetrics, ColdStartMetrics,
    WarmStartMetrics,
};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;
use std::{fs, path::Path};
use tera::{Context as TeraContext, Tera};

#[derive(Debug, Serialize)]
struct ChartSection {
    title: String,
    chart_id: String,
    anchor_id: String,
    memory_size: u32,
    memory_links: Vec<MemoryLink>,
    navigation: NavigationData,
    options_json: String,
    data_filename: String,
}

#[derive(Debug, Serialize)]
struct MemoryLink {
    size: String,
    is_current: bool,
    anchor_id: String,
}

#[derive(Debug, Serialize)]
struct NavigationData {
    cold_start: NavigationGroup,
    warm_start: NavigationGroup,
    resources: NavigationGroup,
}

#[derive(Debug, Serialize)]
struct NavigationGroup {
    label: String,
    links: Vec<NavigationLink>,
}

#[derive(Debug, Serialize)]
struct NavigationLink {
    title: String,
    anchor_id: String,
    is_current: bool,
}

#[derive(Debug, Clone, Copy)]
enum ChartType {
    ColdStartInit,
    ColdStartServer,
    WarmStartClient,
    WarmStartServer,
    WarmStartNet,
    WarmStartNetOverTime,
    MemoryUsage,
}

impl ChartType {
    fn base_id(&self) -> &'static str {
        match self {
            ChartType::ColdStartInit => "cold-start-init-duration",
            ChartType::ColdStartServer => "cold-start-duration",
            ChartType::WarmStartClient => "client-duration",
            ChartType::WarmStartServer => "warm-server-duration",
            ChartType::WarmStartNet => "net-duration",
            ChartType::WarmStartNetOverTime => "net-duration-over-time",
            ChartType::MemoryUsage => "memory-usage",
        }
    }

    fn title(&self) -> &'static str {
        match self {
            ChartType::ColdStartInit => "Init Duration",
            ChartType::ColdStartServer => "Server Duration",
            ChartType::WarmStartClient => "Client Duration",
            ChartType::WarmStartServer => "Server Duration",
            ChartType::WarmStartNet => "Net Duration",
            ChartType::WarmStartNetOverTime => "Net Duration Over Time",
            ChartType::MemoryUsage => "Memory Usage",
        }
    }
}

/// Initialize Tera template engine
fn init_templates() -> Result<Tera> {
    let mut tera = Tera::default();
    tera.add_template_file("src/templates/chart.jekyll.md", Some("chart.jekyll.md"))
        .context("Failed to load chart template")?;
    tera.add_template_file("src/templates/index.jekyll.md", Some("index.jekyll.md"))
        .context("Failed to load index template")?;
    tera.add_template_file("src/templates/index.chart.md", Some("index.chart.md"))
        .context("Failed to load index chart template")?;
    Ok(tera)
}

/// Generate a chart using the template
#[allow(clippy::too_many_arguments)]
fn generate_chart(
    tera: &Tera,
    chart_type: ChartType,
    options: serde_json::Value,
    memory_size: u32,
    function_title: &str,
    memory_sizes: &[u32],
    output_dir: &Path,
    reports: &[(String, BenchmarkReport)],
) -> Result<String> {
    let base_id = chart_type.base_id();
    let anchor_id = format!("{}-{}", base_id, memory_size);
    let chart_id = format!("chart-{}-{}", base_id, memory_size);
    let data_filename = format!("{}-{}.json", base_id, memory_size);

    // Create raw measurements data based on chart type
    let raw_data = match chart_type {
        ChartType::ColdStartInit => {
            let measurements: Vec<_> = reports
                .iter()
                .map(|(name, report)| {
                    json!({
                        "name": name,
                        "measurements": report.cold_starts.iter().map(|m| {
                            json!({
                                "init_duration": m.init_duration,
                                "timestamp": m.timestamp
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect();
            json!({
                "chart_type": chart_type.title(),
                "memory_size": memory_size,
                "function_title": function_title,
                "measurements": measurements
            })
        }
        ChartType::ColdStartServer => {
            let measurements: Vec<_> = reports
                .iter()
                .map(|(name, report)| {
                    json!({
                        "name": name,
                        "measurements": report.cold_starts.iter().map(|m| {
                            json!({
                                "duration": m.duration,
                                "timestamp": m.timestamp
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect();
            json!({
                "chart_type": chart_type.title(),
                "memory_size": memory_size,
                "function_title": function_title,
                "measurements": measurements
            })
        }
        ChartType::WarmStartClient => {
            let measurements: Vec<_> = reports
                .iter()
                .map(|(name, report)| {
                    json!({
                        "name": name,
                        "measurements": report.client_measurements.iter().map(|m| {
                            json!({
                                "client_duration": m.client_duration,
                                "timestamp": m.timestamp
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect();
            json!({
                "chart_type": chart_type.title(),
                "memory_size": memory_size,
                "function_title": function_title,
                "measurements": measurements
            })
        }
        ChartType::WarmStartServer | ChartType::WarmStartNet | ChartType::WarmStartNetOverTime => {
            let measurements: Vec<_> = reports
                .iter()
                .map(|(name, report)| {
                    json!({
                        "name": name,
                        "measurements": report.warm_starts.iter().map(|m| {
                            json!({
                                "duration": m.duration,
                                "net_duration": m.net_duration,
                                "timestamp": m.timestamp
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect();
            json!({
                "chart_type": chart_type.title(),
                "memory_size": memory_size,
                "function_title": function_title,
                "measurements": measurements
            })
        }
        ChartType::MemoryUsage => {
            let measurements: Vec<_> = reports
                .iter()
                .map(|(name, report)| {
                    json!({
                        "name": name,
                        "measurements": report.warm_starts.iter().map(|m| {
                            json!({
                                "max_memory_used": m.max_memory_used,
                                "timestamp": m.timestamp
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect();
            json!({
                "chart_type": chart_type.title(),
                "memory_size": memory_size,
                "function_title": function_title,
                "measurements": measurements
            })
        }
    };

    // Build memory links
    let memory_links: Vec<MemoryLink> = memory_sizes
        .iter()
        .map(|&size| MemoryLink {
            size: format!(" {}MB ", size),
            is_current: size == memory_size,
            anchor_id: format!("{}-{}", base_id, size),
        })
        .collect();

    // Build navigation data
    let navigation = NavigationData {
        cold_start: NavigationGroup {
            label: "Cold Start".to_string(),
            links: vec![
                NavigationLink {
                    title: ChartType::ColdStartInit.title().to_string(),
                    anchor_id: format!("{}-{}", ChartType::ColdStartInit.base_id(), memory_size),
                    is_current: matches!(chart_type, ChartType::ColdStartInit),
                },
                NavigationLink {
                    title: ChartType::ColdStartServer.title().to_string(),
                    anchor_id: format!("{}-{}", ChartType::ColdStartServer.base_id(), memory_size),
                    is_current: matches!(chart_type, ChartType::ColdStartServer),
                },
            ],
        },
        warm_start: NavigationGroup {
            label: "Warm Start".to_string(),
            links: vec![
                NavigationLink {
                    title: ChartType::WarmStartClient.title().to_string(),
                    anchor_id: format!("{}-{}", ChartType::WarmStartClient.base_id(), memory_size),
                    is_current: matches!(chart_type, ChartType::WarmStartClient),
                },
                NavigationLink {
                    title: ChartType::WarmStartServer.title().to_string(),
                    anchor_id: format!("{}-{}", ChartType::WarmStartServer.base_id(), memory_size),
                    is_current: matches!(chart_type, ChartType::WarmStartServer),
                },
                NavigationLink {
                    title: ChartType::WarmStartNet.title().to_string(),
                    anchor_id: format!("{}-{}", ChartType::WarmStartNet.base_id(), memory_size),
                    is_current: matches!(chart_type, ChartType::WarmStartNet),
                },
                NavigationLink {
                    title: ChartType::WarmStartNetOverTime.title().to_string(),
                    anchor_id: format!(
                        "{}-{}",
                        ChartType::WarmStartNetOverTime.base_id(),
                        memory_size
                    ),
                    is_current: matches!(chart_type, ChartType::WarmStartNetOverTime),
                },
            ],
        },
        resources: NavigationGroup {
            label: "Resources".to_string(),
            links: vec![NavigationLink {
                title: ChartType::MemoryUsage.title().to_string(),
                anchor_id: format!("{}-{}", ChartType::MemoryUsage.base_id(), memory_size),
                is_current: matches!(chart_type, ChartType::MemoryUsage),
            }],
        },
    };

    let section = ChartSection {
        title: chart_type.title().to_string(),
        chart_id,
        anchor_id,
        memory_size,
        memory_links,
        navigation,
        options_json: serde_json::to_string(&options)?,
        data_filename,
    };

    // Save raw measurements data in the same directory as index.md
    fs::write(
        output_dir.join(&section.data_filename),
        serde_json::to_string_pretty(&raw_data)?,
    )?;

    let mut context = TeraContext::new();
    context.insert("section", &section);
    context.insert("function_title", function_title);

    tera.render("chart.jekyll.md", &context)
        .context("Failed to render chart template")
}

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

/// Create series data for bar charts
fn create_bar_series(
    name: &str,
    stats: (f64, f64, f64, f64, f64),
    unit: &str,
) -> serde_json::Value {
    let (avg, min, max, p95, p50) = stats;
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
}

/// Create line chart series for warm start net duration over time
fn create_net_duration_time_series(report: &BenchmarkReport, name: &str) -> serde_json::Value {
    let data: Vec<_> = report
        .warm_starts
        .iter()
        .enumerate()
        .map(|(index, m)| {
            json!({
                "value": [index + 1, m.net_duration.round()],
            })
        })
        .collect();

    json!({
        "name": name,
        "type": "scatter",
        "smooth": true,
        "showSymbol": true,
        "label": {
            "show": false
        },
        "data": data
    })
}

/// Create bar chart options
fn create_bar_chart_options(series: Vec<serde_json::Value>, unit: &str) -> serde_json::Value {
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
        "backgroundColor": "rgba(0,0,0,0)",
        "legend": {
            "orient": "horizontal",
            "bottom": "5%",
            "left": "center"
        },
        "grid": [{
            "left": "0%",
            "top": "0%",
            "right": "15%",
            "bottom": "20%",
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
            "data": ["AVG", "MIN", "MAX", "P95", "P50"],
            "axisLabel": {
                "margin": 20
            }
        }],
        "series": series
    })
}

/// Create line chart options
fn create_line_chart_options(series: Vec<serde_json::Value>, unit: &str) -> serde_json::Value {
    json!({
        "tooltip": {
            "trigger": "axis",
            "axisPointer": {
                "type": "cross"
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
        "grid": {
            "left": "0%",
            "top": "0%",
            "right": "15%",
            "bottom": "20%",
            "containLabel": true
        },
        "backgroundColor": "rgba(0,0,0,0)",
        "legend": {
            "orient": "horizontal",
            "bottom": "5%",
            "left": "center"
        },
        "xAxis": {
            "type": "value",
            "name": "Request Number",
            "nameLocation": "middle",
            "nameGap": 30
        },
        "yAxis": {
            "type": "value",
            "name": format!("Duration ({})", unit),
            "nameLocation": "middle",
            "nameGap": 45,
            "axisLabel": {
                "formatter": format!("{{value}} {}", unit)
            }
        },
        "series": series
    })
}

/// Load all benchmark reports from a directory
fn load_benchmark_reports(data_dir: &Path) -> Result<Vec<(String, BenchmarkReport)>> {
    let mut reports = Vec::new();

    for entry in fs::read_dir(data_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip non-JSON files
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        // Get the function name from the filename (remove the .json extension)
        let function_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
            .to_string();

        // Read and parse the report
        let report_json = fs::read_to_string(&path)
            .context(format!("Failed to read report file: {}", path.display()))?;
        let report: BenchmarkReport = serde_json::from_str(&report_json)
            .context(format!("Failed to parse report file: {}", path.display()))?;

        reports.push((function_name, report));
    }

    if reports.is_empty() {
        return Err(anyhow::anyhow!("No JSON files found in directory"));
    }

    // Sort reports by function name for consistent ordering
    reports.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(reports)
}

/// Generate Jekyll documentation from benchmark results
pub async fn generate_jekyll_docs(
    input_dir: &str,
    output_dir: &str,
    batch_config: &BatchConfig,
) -> Result<()> {
    let output_path = Path::new(output_dir);

    // Create output directory if it doesn't exist
    fs::create_dir_all(output_path)?;

    // Generate main index
    generate_main_index(output_dir, batch_config)?;

    // Create directories and index files for each test
    for (index, test) in batch_config.tests.iter().enumerate() {
        let stack_name = test
            .stack_name
            .as_ref()
            .unwrap_or(&batch_config.global.stack_name);

        // Create directory name in the format: <stack>-<selector>-<test-name>
        let test_dir_name = format!("{}-{}-{}", stack_name, test.selector, test.name);
        let test_dir = Path::new(output_dir).join(&test_dir_name);

        // Create the test directory
        fs::create_dir_all(&test_dir).context(format!(
            "Failed to create test directory: {}",
            test_dir.display()
        ))?;

        // Determine data directory path for reports
        let data_dir = Path::new(input_dir)
            .join(stack_name)
            .join(&test.selector)
            .join(&test.name);

        // Generate test index.md
        generate_test_index(
            &test_dir,
            test,
            stack_name,
            batch_config,
            &data_dir,
            index + 1,
        )
        .context(format!(
            "Failed to generate test index for {}",
            test_dir.display()
        ))?;
    }

    Ok(())
}

/// Generate the main index.md file
fn generate_main_index(output_dir: &str, batch_config: &BatchConfig) -> Result<()> {
    let tera = init_templates()?;
    let index_path = Path::new(output_dir).join("index.md");
    let mut context = TeraContext::new();

    // Add title and description
    context.insert(
        "title",
        batch_config
            .global
            .title
            .as_deref()
            .unwrap_or("Benchmark Results"),
    );
    context.insert("nav_order", &90);
    context.insert(
        "description",
        batch_config.global.description.as_deref().unwrap_or(
            "Performance benchmarks for different OpenTelemetry instrumentation approaches.",
        ),
    );
    context.insert("function_title", "");

    // Add global description if available
    if let Some(global_desc) = &batch_config.global.description {
        context.insert("global_description", global_desc);
    }

    // Create items for each test
    let items: Vec<_> = batch_config
        .tests
        .iter()
        .map(|test| {
            let stack_name = test
                .stack_name
                .as_ref()
                .unwrap_or(&batch_config.global.stack_name);
            let dir_name = format!("{}-{}-{}", stack_name, test.selector, test.name);

            json!({
                "title": test.title,
                "path": dir_name,
                "subtitle": test.description
            })
        })
        .collect();
    context.insert("items", &items);

    // Render and write the file
    let content = tera
        .render("index.jekyll.md", &context)
        .context("Failed to render main index template")?;

    fs::write(&index_path, content).context(format!(
        "Failed to write index file: {}",
        index_path.display()
    ))?;

    println!("- {}", index_path.display());
    Ok(())
}

/// Generate a test-specific index.md file
fn generate_test_index(
    test_dir: &Path,
    test: &BatchTestConfig,
    stack_name: &str,
    batch_config: &BatchConfig,
    data_dir: &Path,
    nav_order: usize,
) -> Result<()> {
    let tera = init_templates()?;
    let index_path = test_dir.join("index.md");
    let mut context = TeraContext::new();

    // Add basic information
    context.insert("title", &test.title);
    context.insert("description", &test.description);
    context.insert("function_title", "Benchmark Results");
    context.insert("nav_order", &nav_order);

    // Add global information
    if let Some(global_title) = &batch_config.global.title {
        context.insert("global_title", global_title);
    }
    if let Some(global_desc) = &batch_config.global.description {
        context.insert("global_description", global_desc);
    }

    // Create metadata items
    let mut metadata = vec![
        json!({
            "label": "Stack",
            "value": format!("{}", stack_name)
        }),
        json!({
            "label": "Function",
            "value": format!("{}", test.selector)
        }),
    ];

    // Memory configurations
    let memory_sizes = test
        .memory_sizes
        .as_ref()
        .unwrap_or(&batch_config.global.memory_sizes);
    let memory_sizes_str = format!(
        "{} MB",
        memory_sizes
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    );
    metadata.push(json!({
        "label": "Memory Configurations",
        "value": memory_sizes_str
    }));

    // Concurrent invocations
    let concurrent = test
        .concurrent
        .map(|c| c.to_string())
        .unwrap_or_else(|| batch_config.global.concurrent.to_string());
    metadata.push(json!({
        "label": "Concurrent Invocations",
        "value": concurrent
    }));

    // Warm start rounds
    let rounds = test
        .rounds
        .map(|r| r.to_string())
        .unwrap_or_else(|| batch_config.global.rounds.to_string());
    metadata.push(json!({
        "label": "Warm Start Rounds",
        "value": rounds
    }));

    // Parallel execution
    let parallel = test.parallel.unwrap_or(batch_config.global.parallel);
    metadata.push(json!({
        "label": "Parallel Execution",
        "value": if parallel { "yes" } else { "no" }
    }));

    // Environment variables
    if !test.environment.is_empty() || !batch_config.global.environment.is_empty() {
        let mut env_vars = Vec::new();

        // Add global environment variables
        for (key, value) in &batch_config.global.environment {
            if !test.environment.contains_key(key) {
                env_vars.push(format!("{}={}", key, value));
            }
        }

        // Add test-specific environment variables
        for (key, value) in &test.environment {
            env_vars.push(format!("{}={}", key, value));
        }

        metadata.push(json!({
            "label": "Environment Variables",
            "value": env_vars.join("\n")
        }));
    }

    context.insert("metadata", &metadata);

    // Create items for memory configurations
    let mut items = Vec::new();
    let memory_sizes_vec: Vec<u32> = memory_sizes.iter().map(|&m| m as u32).collect();
    let memory_sizes_ref: &[u32] = &memory_sizes_vec;
    for memory in memory_sizes.iter().map(|&m| m as u32) {
        let memory_dir = data_dir.join(format!("{}mb", memory));

        let mut item = json!({
            "title": format!("{} MB Configuration", memory),
            "anchor_id": format!("{}-mb-configuration", memory)
        });

        if !memory_dir.exists() {
            item["subtitle"] = json!("No data available for this configuration.");
            items.push(item);
            continue;
        }

        // Load benchmark reports and generate charts
        match load_benchmark_reports(&memory_dir) {
            Ok(reports) => {
                let mut charts = Vec::new();

                // Init Duration (Cold Start)
                let series: Vec<_> = reports
                    .iter()
                    .filter_map(|(name, report)| {
                        calculate_cold_start_init_stats(&report.cold_starts)
                            .map(|stats| create_bar_series(name, stats, "ms"))
                    })
                    .collect();
                if !series.is_empty() {
                    let options = create_bar_chart_options(series, "ms");
                    let chart = generate_chart(
                        &tera,
                        ChartType::ColdStartInit,
                        options,
                        memory,
                        &test.title,
                        memory_sizes_ref,
                        test_dir,
                        &reports,
                    )?;
                    charts.push(chart);
                }

                // Server Duration (Cold Start)
                let series: Vec<_> = reports
                    .iter()
                    .filter_map(|(name, report)| {
                        calculate_cold_start_server_stats(&report.cold_starts)
                            .map(|stats| create_bar_series(name, stats, "ms"))
                    })
                    .collect();
                if !series.is_empty() {
                    let options = create_bar_chart_options(series, "ms");
                    let chart = generate_chart(
                        &tera,
                        ChartType::ColdStartServer,
                        options,
                        memory,
                        &test.title,
                        memory_sizes_ref,
                        test_dir,
                        &reports,
                    )?;
                    charts.push(chart);
                }

                // Client Duration (Warm Start)
                let series: Vec<_> = reports
                    .iter()
                    .filter_map(|(name, report)| {
                        calculate_client_stats(&report.client_measurements)
                            .map(|stats| create_bar_series(name, stats, "ms"))
                    })
                    .collect();
                if !series.is_empty() {
                    let options = create_bar_chart_options(series, "ms");
                    let chart = generate_chart(
                        &tera,
                        ChartType::WarmStartClient,
                        options,
                        memory,
                        &test.title,
                        memory_sizes_ref,
                        test_dir,
                        &reports,
                    )?;
                    charts.push(chart);
                }

                // Server Duration (Warm Start)
                let series: Vec<_> = reports
                    .iter()
                    .filter_map(|(name, report)| {
                        calculate_warm_start_stats(&report.warm_starts, |m| m.duration)
                            .map(|stats| create_bar_series(name, stats, "ms"))
                    })
                    .collect();
                if !series.is_empty() {
                    let options = create_bar_chart_options(series, "ms");
                    let chart = generate_chart(
                        &tera,
                        ChartType::WarmStartServer,
                        options,
                        memory,
                        &test.title,
                        memory_sizes_ref,
                        test_dir,
                        &reports,
                    )?;
                    charts.push(chart);
                }

                // Net Duration (Warm Start)
                let series: Vec<_> = reports
                    .iter()
                    .filter_map(|(name, report)| {
                        calculate_warm_start_stats(&report.warm_starts, |m| m.net_duration)
                            .map(|stats| create_bar_series(name, stats, "ms"))
                    })
                    .collect();
                if !series.is_empty() {
                    let options = create_bar_chart_options(series, "ms");
                    let chart = generate_chart(
                        &tera,
                        ChartType::WarmStartNet,
                        options,
                        memory,
                        &test.title,
                        memory_sizes_ref,
                        test_dir,
                        &reports,
                    )?;
                    charts.push(chart);
                }

                // Net Duration Over Time (Warm Start)
                let series: Vec<_> = reports
                    .iter()
                    .map(|(name, report)| create_net_duration_time_series(report, name))
                    .collect();
                if !series.is_empty() {
                    let options = create_line_chart_options(series, "ms");
                    let chart = generate_chart(
                        &tera,
                        ChartType::WarmStartNetOverTime,
                        options,
                        memory,
                        &test.title,
                        memory_sizes_ref,
                        test_dir,
                        &reports,
                    )?;
                    charts.push(chart);
                }

                // Memory Usage (Resources)
                let series: Vec<_> = reports
                    .iter()
                    .filter_map(|(name, report)| {
                        calculate_memory_stats(&report.warm_starts)
                            .map(|stats| create_bar_series(name, stats, "MB"))
                    })
                    .collect();
                if !series.is_empty() {
                    let options = create_bar_chart_options(series, "MB");
                    let chart = generate_chart(
                        &tera,
                        ChartType::MemoryUsage,
                        options,
                        memory,
                        &test.title,
                        memory_sizes_ref,
                        test_dir,
                        &reports,
                    )?;
                    charts.push(chart);
                }

                if !charts.is_empty() {
                    item["charts"] = json!(charts);
                }
                items.push(item);
            }
            Err(e) => {
                item["subtitle"] = json!(format!("Failed to load benchmark data: {}", e));
                items.push(item);
            }
        }
    }

    context.insert("items", &items);

    // Render and write the file
    let content = tera
        .render("index.chart.md", &context)
        .context("Failed to render test index template")?;

    fs::write(&index_path, content).context(format!(
        "Failed to write test index file: {}",
        index_path.display()
    ))?;

    println!("- {}", index_path.display());
    Ok(())
}
