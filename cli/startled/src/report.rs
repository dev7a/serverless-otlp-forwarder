use crate::screenshot::take_chart_screenshot;
use crate::stats::{
    calculate_client_stats, calculate_cold_start_extension_overhead_stats,
    calculate_cold_start_init_stats, calculate_cold_start_produced_bytes_stats,
    calculate_cold_start_response_duration_stats, calculate_cold_start_response_latency_stats,
    calculate_cold_start_runtime_done_metrics_duration_stats,
    calculate_cold_start_runtime_overhead_stats, calculate_cold_start_server_stats,
    calculate_cold_start_total_duration_stats, calculate_memory_stats,
    calculate_warm_start_produced_bytes_stats, calculate_warm_start_response_duration_stats,
    calculate_warm_start_response_latency_stats,
    calculate_warm_start_runtime_done_metrics_duration_stats,
    calculate_warm_start_runtime_overhead_stats, calculate_warm_start_stats,
};
use crate::types::{BenchmarkConfig, BenchmarkReport};
use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use pulldown_cmark::{html, Options, Parser};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tera::{Context as TeraContext, Tera};

/// Define a type alias for the report structure
type ReportStructure = BTreeMap<String, Vec<String>>;

/// Convert snake_case to kebab-case for SEO-friendly URLs
fn snake_to_kebab(input: &str) -> String {
    input.replace('_', "-")
}

#[derive(Serialize)]
struct SeriesRenderData {
    name: String,
    values: Vec<f64>, // e.g., [avg, p99, p95, p50]
}

#[derive(Serialize)]
struct BarChartRenderData {
    title: String,                  // e.g., "Cold Start - Init Duration"
    unit: String,                   // e.g., "ms"
    y_axis_categories: Vec<String>, // e.g., ["AVG", "P99", "P95", "P50"]
    series: Vec<SeriesRenderData>,
    page_type: String,           // e.g., "cold_init", for context in JS if needed
    description: Option<String>, // AWS-documentation-based description of the metric
}

#[derive(Serialize)]
struct ScatterPoint {
    x: usize, // Original index or offsetted index
    y: f64,   // Duration
}

#[derive(Serialize)]
struct LineSeriesRenderData {
    name: String,
    points: Vec<ScatterPoint>,
    mean: Option<f64>,
}

#[derive(Serialize)]
struct LineChartRenderData {
    title: String,
    x_axis_label: String,
    y_axis_label: String,
    unit: String,
    series: Vec<LineSeriesRenderData>,
    total_x_points: usize,
    page_type: String,
    description: Option<String>, // AWS-documentation-based description of the metric
}

/// Data structure for individual metrics in the summary
#[derive(Debug, Serialize)]
struct SummaryMetricData {
    id: String,                   // e.g., "cold-total-duration"
    title: String,                // e.g., "Cold Start Total Duration"
    unit: String,                 // e.g., "ms"
    link: String,                 // e.g., "../cold-start-total-duration/"
    data: Vec<SummarySeriesData>, // Function performance data
}

#[derive(Debug, Serialize)]
struct SummarySeriesData {
    name: String, // Function name
    value: f64,   // Average value for this metric
}

/// Data structure for the complete summary page
#[derive(Debug, Serialize)]
struct SummaryChartRenderData {
    title: String,
    description: String,
    metrics: Vec<SummaryMetricData>,
    page_type: String,
}

#[derive(Serialize)]
enum ChartRenderData {
    Combined {
        bar: Box<BarChartRenderData>,
        line: Box<LineChartRenderData>,
    },
    Summary(SummaryChartRenderData),
}

/// Generate a chart with the given options
#[allow(clippy::too_many_arguments)]
async fn generate_chart(
    html_dir: &Path,
    png_dir: Option<&Path>,
    name: &str,
    chart_render_data: &ChartRenderData,
    config: &BenchmarkConfig,
    suffix: &str,
    screenshot_theme: Option<&str>,
    pb: &ProgressBar,
    report_structure: &ReportStructure,
    current_group: &str,
    current_subgroup: &str,
    template_dir: Option<&String>,
    base_url: Option<&str>,
    local_browsing: bool,
) -> Result<()> {
    // Initialize Tera for HTML templates (chart.html, _sidebar.html)
    let mut tera_html = Tera::default();
    if let Some(custom_template_dir) = template_dir {
        let base_path = PathBuf::from(custom_template_dir);
        if !base_path.exists() {
            anyhow::bail!(
                "Custom template directory not found: {}",
                custom_template_dir
            );
        }
        let glob_pattern = base_path.join("*.html").to_string_lossy().into_owned();
        tera_html = Tera::new(&glob_pattern).with_context(|| {
            format!(
                "Failed to load HTML templates from custom directory: {}",
                glob_pattern
            )
        })?;
        if !tera_html.get_template_names().any(|n| n == "chart.html") {
            anyhow::bail!(
                "Essential HTML template 'chart.html' not found in custom directory: {}",
                custom_template_dir
            );
        }
        if !tera_html.get_template_names().any(|n| n == "_sidebar.html") {
            anyhow::bail!(
                "Essential HTML template '_sidebar.html' not found in custom directory: {}",
                custom_template_dir
            );
        }
    } else {
        tera_html.add_raw_template("chart.html", include_str!("templates/chart.html"))?;
        tera_html.add_raw_template("_sidebar.html", include_str!("templates/_sidebar.html"))?;
    }

    // Create kebab-case chart directory name
    let kebab_name = snake_to_kebab(name);

    // Create directory for this chart
    let chart_dir = html_dir.join(&kebab_name);
    fs::create_dir_all(&chart_dir)?;

    // Write ChartRenderData variant to *_data.js file in the chart directory
    let data_js_filename = "chart_data.js";
    let data_js_path = chart_dir.join(data_js_filename);
    let json_data_string = serde_json::to_string(chart_render_data)
        .context("Failed to serialize chart render data enum")?;
    fs::write(
        &data_js_path,
        // JS will need to check the structure or use page_type/chart_type
        format!("window.currentChartSpecificData = {};", json_data_string),
    )?;

    // Create context FOR HTML PAGE (chart.html)
    let mut ctx = TeraContext::new();
    // Extract title, page_type, and description from the enum variant
    let (title, page_type, description) = match chart_render_data {
        ChartRenderData::Combined { bar, line: _ } => {
            (bar.title.as_str(), bar.page_type.as_str(), &bar.description)
        }
        ChartRenderData::Summary(summary) => {
            (summary.title.as_str(), summary.page_type.as_str(), &Some(summary.description.clone()))
        }
    };

    ctx.insert("title", title);
    ctx.insert("config", config);
    ctx.insert("chart_id", "chart");
    ctx.insert("page_type", page_type);
    ctx.insert("chart_data_js", data_js_filename);
    ctx.insert("description", description);

    // Add sidebar context
    ctx.insert("report_structure", report_structure);
    ctx.insert("current_group", current_group);
    ctx.insert("current_subgroup", current_subgroup);
    ctx.insert("base_path", &calculate_base_path(html_dir, base_url)?);

    // Use the kebab-case name for URL references
    ctx.insert("kebab_name", &kebab_name);

    // Add link_suffix for local browsing
    ctx.insert(
        "link_suffix",
        if local_browsing { "index.html" } else { "" },
    );

    // Render the index file inside the chart directory
    let html_path = chart_dir.join(format!("index.{}", suffix));
    pb.set_message(format!("Rendering {}...", html_path.display()));
    let html = tera_html.render("chart.html", &ctx)?;
    fs::write(&html_path, html)?;

    // Take screenshot if requested
    if let Some(png_dir_path) = png_dir {
        if let Some(theme_str) = screenshot_theme {
            let screenshot_path = png_dir_path.join(format!("{}.png", name));
            pb.set_message(format!("Generating {}...", screenshot_path.display()));
            take_chart_screenshot(&html_path, &screenshot_path, theme_str).await?;
        }
    }

    Ok(())
}

/// Calculate the relative base path for sidebar links (needed for templates)
/// If base_url is provided, it will be used instead of calculating relative paths
fn calculate_base_path(current_dir: &Path, base_url: Option<&str>) -> Result<String> {
    if let Some(base) = base_url {
        // If a base URL is provided, use it for all paths
        // Ensure it ends with a trailing slash for path concatenation
        let mut base = base.to_string();
        if !base.ends_with('/') {
            base.push('/');
        }
        return Ok(base);
    }

    // Otherwise calculate relative paths as before
    // Calculate depth by counting directory components
    // For node/128mb/chart-name/ that would be 3 levels deep, resulting in "../../../"
    let path_components = current_dir.components().count();

    // When calculating the base path, we'll be one level deeper in the chart-type directory
    // So we need to add 1 to the standard depth calculation
    let depth = match path_components {
        0 => 0,
        // Count actual directory levels + 1 for chart subdirectory (but at most 3 levels deep)
        _ => std::cmp::min(path_components + 1, 3),
    };

    Ok("../".repeat(depth))
}

/// Represents an item in the index page
#[derive(Debug, Serialize)]
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
}

/// Scans the input directory to build the report structure for the sidebar.
fn scan_report_structure(base_input_dir: &str) -> Result<ReportStructure> {
    let mut structure = BTreeMap::new();
    let base_path = Path::new(base_input_dir);

    for group_entry in fs::read_dir(base_path)? {
        let group_entry = group_entry?;
        let group_path = group_entry.path();
        if group_path.is_dir() {
            let group_name = group_entry.file_name().to_string_lossy().to_string();
            let mut subgroups = Vec::new();
            for subgroup_entry in fs::read_dir(&group_path)? {
                let subgroup_entry = subgroup_entry?;
                let subgroup_path = subgroup_entry.path();
                if subgroup_path.is_dir() {
                    // Use match and is_some_and for clarity
                    let has_json = fs::read_dir(&subgroup_path)?.any(|entry_result| {
                        match entry_result {
                            Ok(e) => e.path().extension().is_some_and(|ext| ext == "json"),
                            Err(_) => false, // Ignore errors reading specific entries
                        }
                    });
                    if has_json {
                        subgroups.push(subgroup_entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
            // Sort subgroups numerically by name
            subgroups.sort_by_key(|name| {
                name.trim_end_matches("mb")
                    .parse::<u32>()
                    .unwrap_or(u32::MAX)
            });

            if !subgroups.is_empty() {
                structure.insert(group_name, subgroups);
            }
        }
    }

    Ok(structure)
}

/// Generates the main landing page for the reports.
#[allow(clippy::too_many_arguments)]
async fn generate_landing_page(
    output_directory: &str,
    report_structure: &ReportStructure,
    custom_title: Option<&str>,
    description: Option<&str>,
    suffix: &str,
    pb: &ProgressBar,
    template_dir: Option<&String>,
    readme_file: Option<&str>,
    base_url: Option<&str>,
    local_browsing: bool,
) -> Result<()> {
    let mut tera = Tera::default();
    if let Some(custom_template_dir) = template_dir {
        let base_path = PathBuf::from(custom_template_dir);
        if !base_path.exists() {
            anyhow::bail!(
                "Custom template directory not found: {}",
                custom_template_dir
            );
        }
        let glob_pattern = base_path.join("*.html").to_string_lossy().into_owned();
        tera = Tera::new(&glob_pattern).with_context(|| {
            format!(
                "Failed to load templates from custom directory: {}",
                glob_pattern
            )
        })?;

        if !tera.get_template_names().any(|n| n == "index.html") {
            anyhow::bail!(
                "Essential template 'index.html' not found in custom directory: {}",
                custom_template_dir
            );
        }
        if !tera.get_template_names().any(|n| n == "_sidebar.html") {
            anyhow::bail!(
                "Essential template '_sidebar.html' not found in custom directory: {}",
                custom_template_dir
            );
        }
    } else {
        // Fallback to embedded templates
        tera.add_raw_template("index.html", include_str!("templates/index.html"))?;
        tera.add_raw_template("_sidebar.html", include_str!("templates/_sidebar.html"))?;
    }

    let mut ctx = TeraContext::new();
    ctx.insert("title", custom_title.unwrap_or("Benchmark Reports"));
    if let Some(desc) = description {
        ctx.insert("description", desc);
    }
    // Landing page specific context
    ctx.insert("is_landing_page", &true);
    // Add link_suffix for local browsing
    ctx.insert(
        "link_suffix",
        if local_browsing { "index.html" } else { "" },
    );
    // Sidebar context
    ctx.insert("report_structure", report_structure);
    ctx.insert("current_group", "");
    ctx.insert("current_subgroup", "");

    // Handle base_url parameter
    let base_path = if let Some(base) = base_url {
        // Ensure it ends with a trailing slash for path concatenation
        let mut base = base.to_string();
        if !base.ends_with('/') {
            base.push('/');
        }
        base
    } else {
        // Default empty string for root path
        "".to_string()
    };
    ctx.insert("base_path", &base_path);

    // Parse markdown content if readme file provided
    if let Some(readme_path) = readme_file {
        pb.set_message(format!("Parsing markdown from {}...", readme_path));
        match fs::read_to_string(readme_path) {
            Ok(markdown_content) => {
                // Set up the parser with GitHub-flavored markdown options
                let mut options = Options::empty();
                options.insert(Options::ENABLE_TABLES);
                options.insert(Options::ENABLE_FOOTNOTES);
                options.insert(Options::ENABLE_STRIKETHROUGH);
                options.insert(Options::ENABLE_TASKLISTS);

                let parser = Parser::new_ext(&markdown_content, options);

                // Convert markdown to HTML
                let mut html_output = String::new();
                html::push_html(&mut html_output, parser);

                // Add the HTML content to the template context
                ctx.insert("readme_html", &html_output);
                ctx.insert("has_readme", &true);
            }
            Err(e) => {
                // Set progress bar message
                pb.set_message(format!("Warning: Failed to read markdown file: {}", e));

                // Print warning to stderr for better visibility
                eprintln!(
                    "\n⚠️  Warning: Failed to read readme file '{}': {}",
                    readme_path, e
                );
                eprintln!("    Report will be generated without readme content.\n");

                ctx.insert("has_readme", &false);
            }
        }
    } else {
        ctx.insert("has_readme", &false);
    }

    // Create items for the landing page grid
    let mut items = Vec::new();
    for (group_name, subgroups) in report_structure {
        let first_subgroup_name = subgroups.first().map(|s| s.as_str()).unwrap_or("");
        // Link to the first subgroup's summary page - use kebab-case with trailing slash
        let link_path = format!("{}/{}/summary/", group_name, first_subgroup_name);
        items.push(
            IndexItem::new(group_name, link_path)
                .with_subtitle(format!("{} configurations", subgroups.len())),
        );
    }
    ctx.insert("items", &items);

    let index_path = Path::new(output_directory).join(format!("index.{}", suffix));
    pb.set_message(format!("Generating landing page: {}", index_path.display()));
    let html = tera.render("index.html", &ctx)?;
    fs::write(&index_path, html)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_reports(
    input_directory: &str,
    output_directory: &str,
    custom_title: Option<&str>,
    description: Option<&str>,
    suffix: &str,
    base_url: Option<&str>,
    screenshot_theme: Option<&str>,
    template_dir: Option<String>,
    readme_file: Option<String>,
    local_browsing: bool,
) -> Result<()> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_directory)?;

    // Early check if readme file exists
    if let Some(readme_path) = &readme_file {
        if !Path::new(readme_path).exists() {
            eprintln!("\n⚠️  Warning: Readme file '{}' not found.", readme_path);
            eprintln!("    Report will be generated without readme content.\n");
        }
    }

    // Scan the structure first
    println!("Scanning report structure at {}", input_directory);
    let report_structure = scan_report_structure(input_directory)?;
    if report_structure.is_empty() {
        anyhow::bail!("No valid benchmark data found in the input directory structure.");
    }

    // Print the structure as an indented list instead of using Debug formatting
    println!("✓ Report structure scanned:");
    for (group_name, subgroups) in &report_structure {
        println!("  • {} ({} configurations)", group_name, subgroups.len());
        for subgroup in subgroups {
            println!("    - {}", subgroup);
        }
    }

    // Setup progress indicators
    let m = MultiProgress::new();
    let pb_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}")?;
    let main_pb = m.add(ProgressBar::new_spinner());
    main_pb.set_style(pb_style.clone());
    main_pb.set_prefix("[1/2] Generating Charts");
    main_pb.enable_steady_tick(Duration::from_millis(100));

    // Iterate through the structure and generate chart pages
    let total_subgroups: usize = report_structure.values().map(|v| v.len()).sum();
    main_pb.set_length(total_subgroups as u64);
    main_pb.set_message("Processing subgroups...");

    for (group_name, subgroups) in &report_structure {
        for subgroup_name in subgroups {
            main_pb.set_message(format!("Processing {}/{}...", group_name, subgroup_name));
            let current_input_dir = Path::new(input_directory)
                .join(group_name)
                .join(subgroup_name);
            let current_output_dir = Path::new(output_directory)
                .join(group_name)
                .join(subgroup_name);

            fs::create_dir_all(&current_output_dir)?;

            // Generate the actual chart reports for this specific group/subgroup
            generate_reports_for_directory(
                current_input_dir.to_str().unwrap(),
                current_output_dir.to_str().unwrap(),
                custom_title,
                suffix,
                screenshot_theme,
                &main_pb,
                &report_structure, // Pass full structure for sidebar
                group_name,
                subgroup_name,
                template_dir.as_ref(),
                base_url,
                local_browsing,
            )
            .await
            .context(format!(
                "Failed generating reports for {}/{}",
                group_name, subgroup_name
            ))?;
            main_pb.inc(1);
        }
    }
    main_pb.finish_with_message("✓ Charts generated.");

    // Generate the single landing page
    let landing_pb = m.add(ProgressBar::new_spinner());
    landing_pb.set_style(pb_style);
    landing_pb.set_prefix("[2/2] Finalizing");
    landing_pb.enable_steady_tick(Duration::from_millis(100));

    generate_landing_page(
        output_directory,
        &report_structure,
        custom_title,
        description,
        suffix,
        &landing_pb,
        template_dir.as_ref(),
        readme_file.as_deref(),
        base_url,
        local_browsing,
    )
    .await?;
    landing_pb.finish_with_message("✓ Landing page generated.");

    m.clear()?;

    // --- Add CSS Copy Step ---
    let css_dir = Path::new(output_directory).join("css");
    fs::create_dir_all(&css_dir).context("Failed to create css output directory")?;
    let css_path = css_dir.join("style.css");

    if let Some(custom_template_dir_str) = &template_dir {
        let css_src_path = PathBuf::from(custom_template_dir_str)
            .join("css")
            .join("style.css");
        if !css_src_path.exists() {
            anyhow::bail!(
                "style.css not found in custom template directory: {}",
                css_src_path.display()
            );
        }
        fs::copy(&css_src_path, &css_path).context(format!(
            "Failed to copy style.css from custom template directory: {}",
            css_src_path.display()
        ))?;
    } else {
        let css_content = include_str!("templates/css/style.css");
        fs::write(&css_path, css_content).context("Failed to write style.css")?;
    }
    println!("✓ CSS file copied.");
    // -------------------------

    // --- Add JS Copy Step ---
    let js_dir = Path::new(output_directory).join("js");
    fs::create_dir_all(&js_dir).context("Failed to create js output directory")?;
    let js_lib_dst = js_dir.join("lib.js");

    if let Some(custom_template_dir_str) = &template_dir {
        let js_lib_src_path = PathBuf::from(custom_template_dir_str)
            .join("js")
            .join("lib.js");

        if !js_lib_src_path.exists() {
            anyhow::bail!(
                "lib.js not found in custom template directory: {}",
                js_lib_src_path.display()
            );
        }

        fs::copy(&js_lib_src_path, &js_lib_dst).context(format!(
            "Failed to copy lib.js from custom template directory: {}",
            js_lib_src_path.display()
        ))?;

        println!("✓ lib.js copied (contains all chart generation code).");
    } else {
        let js_lib_content = include_str!("templates/js/lib.js");
        fs::write(&js_lib_dst, js_lib_content).context("Failed to write default lib.js")?;
        println!("✓ Default lib.js copied (contains all chart generation code).");
    }
    // -------------------------

    // Print path to the main index.html
    let index_path = PathBuf::from(output_directory).join("index.html");
    if index_path.exists() {
        println!("✨ Report generated successfully!");
        println!("📊 View the report at: {}", index_path.display());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_reports_for_directory(
    input_directory: &str,
    output_directory: &str,
    custom_title: Option<&str>,
    suffix: &str,
    screenshot_theme: Option<&str>,
    pb: &ProgressBar,
    report_structure: &ReportStructure,
    current_group: &str,
    current_subgroup: &str,
    template_dir: Option<&String>,
    base_url: Option<&str>,
    local_browsing: bool,
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
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0)) // 5-tuple default
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

    let cold_extension_overhead_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_extension_overhead_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let cold_total_duration_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_total_duration_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let memory_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_memory_stats(&report.warm_starts).unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    // --- Calculate New Platform Metrics Stats ---
    // Cold Start
    let cold_response_latency_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_response_latency_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    let cold_response_duration_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_response_duration_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    let cold_runtime_overhead_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_runtime_overhead_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    let cold_runtime_done_duration_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_runtime_done_metrics_duration_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    // Warm Start (also for produced_bytes, though it could be cold or warm)
    let warm_response_latency_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_warm_start_response_latency_stats(&report.warm_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    let warm_response_duration_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_warm_start_response_duration_stats(&report.warm_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    let warm_runtime_overhead_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_warm_start_runtime_overhead_stats(&report.warm_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    let warm_runtime_done_duration_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_warm_start_runtime_done_metrics_duration_stats(&report.warm_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    let produced_bytes_stats: Vec<_> = results // Assuming we take produced_bytes from warm starts, could be cold too.
        .iter()
        .map(|report| {
            calculate_warm_start_produced_bytes_stats(&report.warm_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
        })
        .collect();
    // --- End New Platform Metrics Stats ---

    // Generate cold start init duration chart if we have data
    if results.iter().any(|r| !r.cold_starts.is_empty()) {
        // Cold Start Init Duration - Combined Chart
        let cold_init_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_init_stats,
            &results,
            "Cold Start - Init Duration",
            "ms",
            "cold_init",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .map(|cs| cs.init_duration)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_init",
            &cold_init_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Cold Start Server Duration - Combined Chart
        let cold_server_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_server_stats,
            &results,
            "Cold Start - Server Duration",
            "ms",
            "cold_server",
            |report| report.cold_starts.iter().map(|cs| cs.duration).collect(),
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_server",
            &cold_server_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Cold Start Extension Overhead - Combined Chart
        let cold_ext_overhead_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_extension_overhead_stats,
            &results,
            "Cold Start - Extension Overhead",
            "ms",
            "cold_extension_overhead",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .map(|cs| cs.extension_overhead)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_extension_overhead",
            &cold_ext_overhead_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Cold Start Total Duration - Combined Chart
        let cold_total_duration_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_total_duration_stats,
            &results,
            "Cold Start - Total Cold Start Duration",
            "ms",
            "cold_total_duration",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .filter_map(|cs| cs.total_cold_start_duration)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_total_duration",
            &cold_total_duration_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // --- Generate New Cold Start Platform Metric Charts (Now Combined) ---
        // Cold Start Response Latency - Combined Chart
        let cold_resp_latency_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_response_latency_stats,
            &results,
            "Cold Start - Response Latency",
            "ms",
            "cold_start_response_latency",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .filter_map(|cs| cs.response_latency_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_response_latency",
            &cold_resp_latency_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Cold Start Response Duration - Combined Chart
        let cold_resp_duration_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_response_duration_stats,
            &results,
            "Cold Start - Response Duration",
            "ms",
            "cold_start_response_duration",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .filter_map(|cs| cs.response_duration_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_response_duration",
            &cold_resp_duration_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Cold Start Runtime Overhead - Combined Chart
        let cold_runtime_overhead_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_runtime_overhead_stats,
            &results,
            "Cold Start - Runtime Overhead",
            "ms",
            "cold_start_runtime_overhead",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .filter_map(|cs| cs.runtime_overhead_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_runtime_overhead",
            &cold_runtime_overhead_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Cold Start Runtime Done Duration - Combined Chart
        let cold_runtime_done_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_runtime_done_duration_stats,
            &results,
            "Cold Start - Runtime Done Duration",
            "ms",
            "cold_start_runtime_done_duration",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .filter_map(|cs| cs.runtime_done_metrics_duration_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_runtime_done_duration",
            &cold_runtime_done_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;
        // --- End New Cold Start Platform Metric Charts ---

        // --- Add Missing Cold Start Resource Metric Charts ---
        // Cold Start Memory Usage - Combined Chart
        let cold_memory_stats: Vec<_> = results
            .iter()
            .map(|report| {
                // Calculate memory stats for cold starts inline
                if report.cold_starts.is_empty() {
                    (0.0, 0.0, 0.0, 0.0, 0.0)
                } else {
                    let memory: Vec<f64> = report
                        .cold_starts
                        .iter()
                        .map(|cs| cs.max_memory_used as f64)
                        .collect();
                    let stats = crate::stats::calculate_stats(&memory);
                    (stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev)
                }
            })
            .collect();
        let cold_memory_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_memory_stats,
            &results,
            "Cold Start - Memory Usage",
            "MB",
            "cold_start_memory",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .map(|cs| cs.max_memory_used as f64)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_memory_usage",
            &cold_memory_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Cold Start Produced Bytes - Combined Chart
        let cold_produced_bytes_stats: Vec<_> = results
            .iter()
            .map(|report| {
                calculate_cold_start_produced_bytes_stats(&report.cold_starts)
                    .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
            })
            .collect();
        let cold_produced_bytes_combined = prepare_combined_chart_render_data(
            &function_names,
            &cold_produced_bytes_stats,
            &results,
            "Cold Start - Produced Bytes",
            "bytes",
            "cold_start_produced_bytes",
            |report| {
                report
                    .cold_starts
                    .iter()
                    .filter_map(|cs| cs.produced_bytes.map(|b| b as f64))
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_produced_bytes",
            &cold_produced_bytes_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;
        // --- End Missing Cold Start Resource Metric Charts ---
    }

    // Generate client duration chart if we have data
    if results.iter().any(|r| !r.client_measurements.is_empty()) {
        // Warm Start Client Duration - Combined Chart (RENAMED for consistency)
        let client_duration_combined = prepare_combined_chart_render_data(
            &function_names,
            &client_stats,
            &results,
            "Warm Start - Client Duration",
            "ms",
            "warm_start_client_duration", // CHANGED: was "client"
            |report| {
                report
                    .client_measurements
                    .iter()
                    .map(|m| m.client_duration)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_client_duration", // CHANGED: was "client_duration"
            &client_duration_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;
    }

    // Generate server duration chart if we have data
    if results.iter().any(|r| !r.warm_starts.is_empty()) {
        // Warm Start Server Duration - Combined Chart (RENAMED for consistency)
        let server_duration_combined = prepare_combined_chart_render_data(
            &function_names,
            &server_stats,
            &results,
            "Warm Start - Server Duration",
            "ms",
            "warm_start_server_duration", // CHANGED: was "server"
            |report| report.warm_starts.iter().map(|ws| ws.duration).collect(),
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_server_duration", // CHANGED: was "server_duration"
            &server_duration_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Warm Start Extension Overhead - Combined Chart (RENAMED for consistency)
        let warm_extension_overhead_stats: Vec<_> = results
            .iter()
            .map(|report| {
                calculate_warm_start_stats(&report.warm_starts, |m| m.extension_overhead)
                    .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0))
            })
            .collect();
        let ext_overhead_combined = prepare_combined_chart_render_data(
            &function_names,
            &warm_extension_overhead_stats,
            &results,
            "Warm Start - Extension Overhead",
            "ms",
            "warm_start_extension_overhead", // CHANGED: was "extension_overhead"
            |report| {
                report
                    .warm_starts
                    .iter()
                    .map(|ws| ws.extension_overhead)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_extension_overhead", // CHANGED: was "extension_overhead"
            &ext_overhead_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Warm Start Memory Usage - Combined Chart (RENAMED for consistency)
        let memory_combined = prepare_combined_chart_render_data(
            &function_names,
            &memory_stats,
            &results,
            "Warm Start - Memory Usage",
            "MB",
            "warm_start_memory", // CHANGED: was "memory"
            |report| {
                report
                    .warm_starts
                    .iter()
                    .map(|ws| ws.max_memory_used as f64)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_memory_usage", // CHANGED: was "memory_usage"
            &memory_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // --- Generate Complete Set of Warm Start Platform Metric Charts ---
        // Warm Start Response Latency - Combined Chart
        let warm_resp_latency_combined = prepare_combined_chart_render_data(
            &function_names,
            &warm_response_latency_stats,
            &results,
            "Warm Start - Response Latency",
            "ms",
            "warm_start_response_latency",
            |report| {
                report
                    .warm_starts
                    .iter()
                    .filter_map(|ws| ws.response_latency_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_response_latency",
            &warm_resp_latency_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Warm Start Response Duration - Combined Chart
        let warm_resp_duration_combined = prepare_combined_chart_render_data(
            &function_names,
            &warm_response_duration_stats,
            &results,
            "Warm Start - Response Duration",
            "ms",
            "warm_start_response_duration",
            |report| {
                report
                    .warm_starts
                    .iter()
                    .filter_map(|ws| ws.response_duration_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_response_duration",
            &warm_resp_duration_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Warm Start Runtime Overhead - Combined Chart
        let warm_runtime_overhead_combined = prepare_combined_chart_render_data(
            &function_names,
            &warm_runtime_overhead_stats,
            &results,
            "Warm Start - Runtime Overhead",
            "ms",
            "warm_start_runtime_overhead",
            |report| {
                report
                    .warm_starts
                    .iter()
                    .filter_map(|ws| ws.runtime_overhead_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_runtime_overhead",
            &warm_runtime_overhead_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Warm Start Runtime Done Duration - Combined Chart
        let warm_runtime_done_combined = prepare_combined_chart_render_data(
            &function_names,
            &warm_runtime_done_duration_stats,
            &results,
            "Warm Start - Runtime Done Duration",
            "ms",
            "warm_start_runtime_done_duration",
            |report| {
                report
                    .warm_starts
                    .iter()
                    .filter_map(|ws| ws.runtime_done_metrics_duration_ms)
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_runtime_done_duration",
            &warm_runtime_done_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;

        // Warm Start Produced Bytes - Combined Chart (RENAMED for consistency)
        let produced_bytes_combined = prepare_combined_chart_render_data(
            &function_names,
            &produced_bytes_stats,
            &results,
            "Warm Start - Produced Bytes",
            "bytes",
            "warm_start_produced_bytes", // CHANGED: was "produced_bytes"
            |report| {
                report
                    .warm_starts
                    .iter()
                    .filter_map(|ws| ws.produced_bytes.map(|b| b as f64))
                    .collect()
            },
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "warm_start_produced_bytes", // CHANGED: was "produced_bytes"
            &produced_bytes_combined,
            &results[0].config,
            suffix,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
            local_browsing,
        )
        .await?;
        // --- End Complete Set of Warm Start Platform Metric Charts ---
    }

    // Generate Summary Page
    let summary_combined = prepare_summary_chart_render_data(
        &function_names,
        &results,
        custom_title.unwrap_or("Performance Summary"),
    );
    generate_chart(
        &PathBuf::from(output_directory),
        png_dir.as_deref(),
        "summary",
        &summary_combined,
        &results[0].config,
        suffix,
        screenshot_theme,
        pb,
        report_structure,
        current_group,
        current_subgroup,
        template_dir,
        base_url,
        local_browsing,
    )
    .await?;

    Ok(())
}

fn prepare_bar_chart_render_data(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)], // Expects (avg, p99, p95, p50, std_dev)
    title: &str,
    unit: &str,
    page_type: &str,
) -> BarChartRenderData {
    let series_render_data = function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, p99, p95, p50, _std_dev))| {
            // Use Decimal for precise rounding to 3 decimal places
            let rounded_avg = Decimal::from_f64(avg)
                .unwrap_or_default()
                .round_dp(3)
                .to_f64()
                .unwrap_or(0.0);
            let rounded_p50 = Decimal::from_f64(p50)
                .unwrap_or_default()
                .round_dp(3)
                .to_f64()
                .unwrap_or(0.0);
            let rounded_p95 = Decimal::from_f64(p95)
                .unwrap_or_default()
                .round_dp(3)
                .to_f64()
                .unwrap_or(0.0);
            let rounded_p99 = Decimal::from_f64(p99)
                .unwrap_or_default()
                .round_dp(3)
                .to_f64()
                .unwrap_or(0.0);

            SeriesRenderData {
                name: name.clone(),
                values: vec![
                    rounded_avg, // AVG
                    rounded_p50, // P50
                    rounded_p95, // P95
                    rounded_p99, // P99
                ],
            }
        })
        .collect();

    BarChartRenderData {
        title: title.to_string(),
        unit: unit.to_string(),
        y_axis_categories: vec![
            "AVG".to_string(),
            "P50".to_string(),
            "P95".to_string(),
            "P99".to_string(),
        ],
        series: series_render_data,
        page_type: page_type.to_string(),
        description: get_metric_description(page_type).map(|s| s.to_string()),
    }
}

/// Prepares a combined chart with both bar chart (aggregates) and line chart (time series) data
fn prepare_combined_chart_render_data(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64, f64)], // Expects (avg, p99, p95, p50, std_dev)
    results: &[BenchmarkReport],
    title: &str,
    unit: &str,
    page_type: &str,
    value_extractor: impl Fn(&BenchmarkReport) -> Vec<f64>,
) -> ChartRenderData {
    // Prepare bar chart data
    let bar_data = prepare_bar_chart_render_data(function_names, stats, title, unit, page_type);

    // Prepare line chart data for the same metric over time
    let line_title = format!("{} - Over Time", title);
    let line_data = prepare_metric_line_chart_render_data(
        results,
        function_names,
        &line_title,
        unit,
        page_type,
        value_extractor,
    );

    ChartRenderData::Combined {
        bar: Box::new(bar_data),
        line: Box::new(line_data),
    }
}

/// Prepares line chart data for a specific metric using a value extraction function
fn prepare_metric_line_chart_render_data(
    results: &[BenchmarkReport],
    function_names: &[String],
    title: &str,
    unit: &str,
    page_type: &str,
    value_extractor: impl Fn(&BenchmarkReport) -> Vec<f64>,
) -> LineChartRenderData {
    let gap = 5; // Gap between series
    let mut current_offset = 0;
    let mut max_x = 0;

    let series_render_data: Vec<LineSeriesRenderData> = function_names
        .iter()
        .zip(results.iter())
        .map(|(name, report)| {
            let x_offset = current_offset;

            // Extract values for this specific metric
            let values = value_extractor(report);
            let num_points = values.len();

            current_offset += num_points + gap; // Update offset for next series
            if current_offset > gap {
                // Update max_x only if points were added
                max_x = current_offset - gap;
            } else {
                // If a series has 0 points, don't let max_x be negative or zero based on gap
                max_x = max_x.max(0);
            }

            let mut points_sum = 0.0;
            let points_data: Vec<ScatterPoint> = values
                .iter()
                .enumerate()
                .map(|(index, &value)| {
                    let duration = Decimal::from_f64(value)
                        .unwrap_or_default()
                        .round_dp(2)
                        .to_f64()
                        .unwrap_or(0.0);
                    points_sum += duration;
                    ScatterPoint {
                        x: x_offset + index,
                        y: duration,
                    }
                })
                .collect();

            let mean = if num_points > 0 {
                let mean_decimal = Decimal::from_f64(points_sum / num_points as f64)
                    .unwrap_or_default()
                    .round_dp(2);
                Some(mean_decimal.to_f64().unwrap_or(0.0))
            } else {
                None
            };

            LineSeriesRenderData {
                name: name.clone(),
                points: points_data,
                mean,
            }
        })
        .collect();

    LineChartRenderData {
        title: title.to_string(),
        x_axis_label: "Test Sequence".to_string(),
        y_axis_label: format!("Duration ({})", unit),
        unit: unit.to_string(),
        series: series_render_data,
        total_x_points: max_x,
        page_type: format!("{}_time", page_type),
        description: get_metric_description(page_type).map(|s| s.to_string()),
    }
}

/// Gets the AWS-documentation-based description for a metric type
/// These descriptions are based on official AWS Lambda documentation and help users understand
/// what each metric represents in terms of Lambda performance characteristics.
fn get_metric_description(page_type: &str) -> Option<&'static str> {
    match page_type {
        // Cold Start Metrics
        "cold_init" => Some(
            "The time AWS Lambda spends initializing your function during a cold start. This includes downloading code/layers, \
            initializing the runtime, and running initialization code outside the main handler. Cold starts occur when Lambda \
            creates a new execution environment (first invocation or after inactivity). The Init phase is limited to 10 seconds \
            for standard functions. Measured in milliseconds."
        ),
        "cold_server" => Some(
            "The time your function code spends processing an event during a cold start invocation. This measures only the \
            execution time of your function handler logic, excluding the initialization overhead. This is equivalent to the \
            AWS CloudWatch 'Duration' metric for cold start invocations. Measured in milliseconds."
        ),
        "cold_extension_overhead" => Some(
            "The additional time consumed by Lambda extensions after your function code completes during cold start. Extensions \
            are external processes that run alongside your function (e.g., monitoring, security tools). This is part of the \
            AWS CloudWatch 'PostRuntimeExtensionsDuration' metric. Higher values indicate extensions are impacting performance. \
            Measured in milliseconds."
        ),
        "cold_total_duration" => Some(
            "The complete end-to-end time for a cold start invocation, including initialization, function execution, and \
            extension processing. This represents the total latency experienced when Lambda creates a new execution environment. \
            This is the sum of Init Duration + Function Duration + Extension Overhead. Measured in milliseconds."
        ),
        "cold_start_response_latency" => Some(
            "The time between when the Lambda service receives an invocation request and when the response becomes available \
            during cold starts. This is measured at the platform level and includes network and service processing overhead \
            beyond your function's execution time. Part of the platform.runtimeDone metrics. Measured in milliseconds."
        ),
        "cold_start_response_duration" => Some(
            "The time taken by the Lambda runtime to prepare and send the response back to the caller during cold start invocations. \
            This measures the overhead of response serialization and transmission at the platform level. Part of the \
            platform.runtimeDone metrics from AWS Lambda's internal instrumentation. Measured in milliseconds."
        ),
        "cold_start_runtime_overhead" => Some(
            "The additional time consumed by the Lambda runtime infrastructure beyond your function's execution time during \
            cold starts. This includes runtime initialization, request/response handling, and internal Lambda service overhead. \
            Derived from platform.runtimeDone metrics that provide insight into Lambda's internal performance. Measured in milliseconds."
        ),
        "cold_start_runtime_done_duration" => Some(
            "The total time measured by Lambda's runtime from invocation start to completion during cold starts. This is an \
            internal AWS metric that captures the complete runtime processing time including function execution and runtime \
            overhead. Part of the platform.runtimeDone telemetry that provides deep runtime insights. Measured in milliseconds."
        ),

        // Warm Start Metrics  
        "warm_start_client_duration" => Some(
            "The end-to-end response time measured from the client perspective during warm start invocations. This includes \
            network latency, Lambda service processing time, and function execution time. Warm starts reuse existing execution \
            environments, skipping the Init phase, resulting in significantly lower latency than cold starts. Measured in milliseconds."
        ),
        "warm_start_server_duration" => Some(
            "The time your function code spends processing an event during warm start invocations. Since warm starts reuse \
            existing execution environments, this excludes initialization overhead and focuses purely on your application logic \
            performance. This corresponds to the AWS CloudWatch 'Duration' metric for warm invocations. Measured in milliseconds."
        ),
        "warm_start_extension_overhead" => Some(
            "The additional time consumed by Lambda extensions after your function code completes during warm starts. Even though \
            extensions are already initialized in warm starts, they may still perform post-invocation processing (e.g., sending \
            telemetry, cleanup). This is the AWS CloudWatch 'PostRuntimeExtensionsDuration' metric. Measured in milliseconds."
        ),
        "warm_start_response_latency" => Some(
            "The time between when the Lambda service receives an invocation request and when the response becomes available \
            during warm start invocations. Since warm starts skip initialization, this latency is typically much lower than \
            cold starts. Part of the platform.runtimeDone metrics providing platform-level insights. Measured in milliseconds."
        ),
        "warm_start_response_duration" => Some(
            "The time taken by the Lambda runtime to prepare and send the response back to the caller during warm start invocations. \
            This measures response processing overhead at the platform level for reused execution environments. Part of the \
            platform.runtimeDone metrics from AWS Lambda's internal instrumentation. Measured in milliseconds."
        ),
        "warm_start_runtime_overhead" => Some(
            "The additional time consumed by the Lambda runtime infrastructure beyond your function's execution time during \
            warm starts. While typically lower than cold starts, this still includes request/response handling and internal \
            service overhead. Derived from platform.runtimeDone metrics for runtime performance analysis. Measured in milliseconds."
        ),
        "warm_start_runtime_done_duration" => Some(
            "The total time measured by Lambda's runtime from invocation start to completion during warm starts. This internal \
            AWS metric captures the complete runtime processing time for reused execution environments. Part of the \
            platform.runtimeDone telemetry providing detailed runtime performance insights. Measured in milliseconds."
        ),

        // Resource Metrics
        "cold_start_memory" => Some(
            "The maximum amount of memory used by your Lambda function during cold start execution. This is reported by AWS CloudWatch \
            as 'MaxMemoryUsed' and helps you understand actual memory consumption versus allocated memory during initialization. \
            Cold starts may use slightly more memory due to runtime loading. Measured in megabytes (MB)."
        ),
        "warm_start_memory" => Some(
            "The maximum amount of memory used by your Lambda function during warm start execution. This is reported by AWS CloudWatch \
            as 'MaxMemoryUsed' and helps you understand actual memory consumption versus allocated memory in steady-state operations. \
            Optimizing memory allocation can improve both performance and cost-effectiveness. Memory impacts CPU allocation proportionally. Measured in megabytes (MB)."
        ),
        "cold_start_produced_bytes" => Some(
            "The number of bytes produced by your Lambda function during cold start execution, typically representing the size of the \
            response payload. This metric helps track data transfer during initialization scenarios and can indicate response \
            serialization efficiency during cold starts. Part of the platform.runtimeDone metrics. Measured in bytes."
        ),
        "warm_start_produced_bytes" => Some(
            "The number of bytes produced by your Lambda function during warm start execution, typically representing the size of the \
            response payload. This metric helps track data transfer and can indicate the efficiency of your response \
            serialization in steady-state operations. Large responses may impact performance and incur additional data transfer costs. Part of the \
            platform.runtimeDone metrics. Measured in bytes."
        ),

        _ => None,
    }
}

/// Prepares summary chart data containing avg values for selected key metrics
fn prepare_summary_chart_render_data(
    function_names: &[String],
    results: &[BenchmarkReport],
    title: &str,
) -> ChartRenderData {
    let metrics = vec![
        // Key Cold Start Metrics
        (
            "cold-start-total-duration",
            "Cold Start Total Duration",
            "ms",
            collect_avg_values(results, |r| {
                r.cold_starts
                    .iter()
                    .filter_map(|cs| cs.total_cold_start_duration)
                    .collect()
            }),
        ),
        (
            "cold-start-init",
            "Cold Start Init Duration",
            "ms",
            collect_avg_values(results, |r| {
                r.cold_starts.iter().map(|cs| cs.init_duration).collect()
            }),
        ),
        (
            "cold-start-server",
            "Cold Start Server Duration",
            "ms",
            collect_avg_values(results, |r| {
                r.cold_starts.iter().map(|cs| cs.duration).collect()
            }),
        ),
        (
            "cold-start-response-latency",
            "Cold Start Response Latency",
            "ms",
            collect_avg_values(results, |r| {
                r.cold_starts
                    .iter()
                    .filter_map(|cs| cs.response_latency_ms)
                    .collect()
            }),
        ),
        // Key Warm Start Metrics
        (
            "warm-start-client-duration",
            "Warm Start Client Duration",
            "ms",
            collect_avg_values(results, |r| {
                r.client_measurements
                    .iter()
                    .map(|cm| cm.client_duration)
                    .collect()
            }),
        ),
        (
            "warm-start-server-duration",
            "Warm Start Server Duration",
            "ms",
            collect_avg_values(results, |r| {
                r.warm_starts.iter().map(|ws| ws.duration).collect()
            }),
        ),
        (
            "warm-start-response-latency",
            "Warm Start Response Latency",
            "ms",
            collect_avg_values(results, |r| {
                r.warm_starts
                    .iter()
                    .filter_map(|ws| ws.response_latency_ms)
                    .collect()
            }),
        ),
        // Resource Metrics
        (
            "warm-start-memory-usage",
            "Warm Start Memory Usage",
            "MB",
            collect_avg_values(results, |r| {
                r.warm_starts
                    .iter()
                    .map(|ws| ws.max_memory_used as f64)
                    .collect()
            }),
        ),
    ];

    let summary_metrics: Vec<SummaryMetricData> = metrics
        .into_iter()
        .map(|(id, title, unit, avg_values)| {
            let data: Vec<SummarySeriesData> = function_names
                .iter()
                .zip(avg_values.iter())
                .map(|(name, &value)| SummarySeriesData {
                    name: name.clone(),
                    value,
                })
                .collect();

            SummaryMetricData {
                id: id.to_string(),
                title: title.to_string(),
                unit: unit.to_string(),
                link: format!("../{}/", id),
                data,
            }
        })
        .collect();

    let summary_data = SummaryChartRenderData {
        title: title.to_string(),
        description: "Overview of key performance metrics across all functions".to_string(),
        metrics: summary_metrics,
        page_type: "summary".to_string(),
    };

    ChartRenderData::Summary(summary_data)
}

/// Helper function to collect average values for a metric across all results
fn collect_avg_values(
    results: &[BenchmarkReport],
    value_extractor: impl Fn(&BenchmarkReport) -> Vec<f64>,
) -> Vec<f64> {
    results
        .iter()
        .map(|report| {
            let values = value_extractor(report);
            if values.is_empty() {
                0.0
            } else {
                let sum: f64 = values.iter().sum();
                let avg = sum / values.len() as f64;
                // Round to 3 decimal places using Decimal
                Decimal::from_f64(avg)
                    .unwrap_or_default()
                    .round_dp(3)
                    .to_f64()
                    .unwrap_or(0.0)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BenchmarkConfig, BenchmarkReport, ClientMetrics}; // Removed unused ColdStartMetrics, EnvVar, WarmStartMetrics
    use std::path::PathBuf;

    #[test]
    fn test_snake_to_kebab() {
        assert_eq!(snake_to_kebab("hello_world"), "hello-world");
        assert_eq!(snake_to_kebab("another_test_case"), "another-test-case");
        assert_eq!(snake_to_kebab("single"), "single");
        assert_eq!(snake_to_kebab(""), "");
        assert_eq!(snake_to_kebab("_leading_underscore"), "-leading-underscore");
        assert_eq!(
            snake_to_kebab("trailing_underscore_"),
            "trailing-underscore-"
        );
    }

    #[test]
    fn test_calculate_base_path_no_base_url() {
        let _path0 = PathBuf::from(""); // Represents being at the root before any group/subgroup
        let _path1 = PathBuf::from("group1");
        let _path2 = PathBuf::from("group1/subgroupA");
        let _path3 = PathBuf::from("group1/subgroupA/chart_type"); // Max depth for calculation logic
        let _path4 = PathBuf::from("group1/subgroupA/chart_type/another_level");

        // The logic in calculate_base_path adds 1 to component count, then caps at 3.
        // current_dir is the output_directory/group_name/subgroup_name
        // The actual HTML file will be one level deeper (e.g., .../chart_name/index.html)
        // So, if current_dir is "group1/subgroupA", components = 2. depth = min(2+1, 3) = 3. Result: "../../../"

        // If html_dir is root of output (e.g. "output_dir")
        // This case is not directly hit by generate_chart's usage, as html_dir is usually deeper.
        // However, testing the function directly:
        // If current_dir is "output_dir", components = 1. depth = min(1+1, 3) = 2. Result: "../../"
        assert_eq!(
            calculate_base_path(&PathBuf::from("output_dir"), None).unwrap(),
            "../../"
        );

        // If html_dir is "output_dir/group1"
        // This means the chart's index.html will be at "output_dir/group1/chart_name/index.html"
        // current_dir for calculate_base_path is "output_dir/group1"
        // components = 2. depth = min(2+1, 3) = 3. Result: "../../../"
        assert_eq!(
            calculate_base_path(&PathBuf::from("output_dir/group1"), None).unwrap(),
            "../../../"
        );

        // If html_dir is "output_dir/group1/subgroupA" (typical case for generate_chart)
        // Chart's index.html will be at "output_dir/group1/subgroupA/chart_name/index.html"
        // current_dir for calculate_base_path is "output_dir/group1/subgroupA"
        // components = 3. depth = min(3+1, 3) = 3. Result: "../../../"
        assert_eq!(
            calculate_base_path(&PathBuf::from("output_dir/group1/subgroupA"), None).unwrap(),
            "../../../"
        );

        // Test with a path that would exceed max depth if not capped
        assert_eq!(
            calculate_base_path(&PathBuf::from("output_dir/group1/subgroupA/extra"), None).unwrap(),
            "../../../"
        );
    }

    #[test]
    fn test_calculate_base_path_with_base_url() {
        let current_dir = PathBuf::from("any/path");
        assert_eq!(
            calculate_base_path(&current_dir, Some("http://example.com")).unwrap(),
            "http://example.com/"
        );
        assert_eq!(
            calculate_base_path(&current_dir, Some("http://example.com/")).unwrap(),
            "http://example.com/"
        );
        assert_eq!(
            calculate_base_path(&current_dir, Some("https://cdn.test/reports/")).unwrap(),
            "https://cdn.test/reports/"
        );
        assert_eq!(calculate_base_path(&current_dir, Some("")).unwrap(), "/"); // Empty base_url becomes "/"
    }

    #[test]
    fn test_prepare_bar_chart_render_data() {
        let function_names = vec!["func_a".to_string(), "func_b".to_string()];
        let stats = vec![
            (10.5126, 15.1001, 14.2999, 12.3456, 1.0), // avg, p99, p95, p50, std_dev for func_a
            (20.0004, 25.5555, 24.0011, 22.5678, 1.5), // avg, p99, p95, p50, std_dev for func_b
        ];
        let title = "Test Bar Chart";
        let unit = "ms";
        let page_type = "test_bar";

        let render_data =
            prepare_bar_chart_render_data(&function_names, &stats, title, unit, page_type);

        assert_eq!(render_data.title, title);
        assert_eq!(render_data.unit, unit);
        assert_eq!(render_data.page_type, page_type);
        assert_eq!(render_data.description, None); // test_bar doesn't have a description
        assert_eq!(
            render_data.y_axis_categories,
            vec!["AVG", "P50", "P95", "P99"]
        );

        assert_eq!(render_data.series.len(), 2);
        // Series 1 (func_a) - avg=10.513, p50=12.346, p95=14.300, p99=15.100
        assert_eq!(render_data.series[0].name, "func_a");
        assert_eq!(
            render_data.series[0].values,
            vec![10.513, 12.346, 14.300, 15.100] // Expected rounded values
        );
        // Series 2 (func_b) - avg=20.000, p50=22.568, p95=24.001, p99=25.556
        assert_eq!(render_data.series[1].name, "func_b");
        assert_eq!(
            render_data.series[1].values,
            vec![20.000, 22.568, 24.001, 25.556] // Expected rounded values
        );
    }

    #[test]
    fn test_prepare_line_chart_render_data() {
        let func_a_metrics = vec![
            ClientMetrics {
                timestamp: "t1".to_string(),
                client_duration: 10.12,
                memory_size: 128,
            },
            ClientMetrics {
                timestamp: "t2".to_string(),
                client_duration: 12.34,
                memory_size: 128,
            },
        ];
        let func_b_metrics = vec![ClientMetrics {
            timestamp: "t3".to_string(),
            client_duration: 20.56,
            memory_size: 128,
        }];

        let results = vec![
            BenchmarkReport {
                config: BenchmarkConfig {
                    function_name: "func_a".to_string(),
                    memory_size: 128,
                    concurrent_invocations: 1,
                    number: 1,
                    timestamp: "".to_string(),
                    runtime: None,
                    architecture: None,
                    environment: vec![],
                },
                cold_starts: vec![],
                warm_starts: vec![],
                client_measurements: func_a_metrics,
            },
            BenchmarkReport {
                config: BenchmarkConfig {
                    function_name: "func_b".to_string(),
                    memory_size: 128,
                    concurrent_invocations: 1,
                    number: 1,
                    timestamp: "".to_string(),
                    runtime: None,
                    architecture: None,
                    environment: vec![],
                },
                cold_starts: vec![],
                warm_starts: vec![],
                client_measurements: func_b_metrics,
            },
        ];
        let function_names = vec!["func_a".to_string(), "func_b".to_string()];
        let title = "Test Line Chart";
        let unit = "ms";
        let page_type = "test_line";

        let render_data = prepare_metric_line_chart_render_data(
            &results,
            &function_names,
            title,
            unit,
            page_type,
            |report| {
                report
                    .client_measurements
                    .iter()
                    .map(|m| m.client_duration)
                    .collect()
            },
        );

        assert_eq!(render_data.title, title);
        assert_eq!(render_data.unit, unit);
        assert_eq!(render_data.page_type, format!("{}_time", page_type));
        assert_eq!(render_data.description, None); // test_line doesn't have a description
        assert_eq!(render_data.x_axis_label, "Test Sequence");
        assert_eq!(render_data.y_axis_label, "Duration (ms)");

        assert_eq!(render_data.series.len(), 2);

        // Series 1 (func_a)
        assert_eq!(render_data.series[0].name, "func_a");
        assert_eq!(render_data.series[0].points.len(), 2);
        assert_eq!(render_data.series[0].points[0].x, 0); // offset 0, index 0
        assert_eq!(render_data.series[0].points[0].y, 10.12);
        assert_eq!(render_data.series[0].points[1].x, 1); // offset 0, index 1
        assert_eq!(render_data.series[0].points[1].y, 12.34);
        assert_eq!(render_data.series[0].mean, Some(11.23)); // (10.12 + 12.34) / 2 = 11.23

        // Series 2 (func_b)
        // current_offset for func_b starts at num_points_func_a (2) + gap (5) = 7
        assert_eq!(render_data.series[1].name, "func_b");
        assert_eq!(render_data.series[1].points.len(), 1);
        assert_eq!(render_data.series[1].points[0].x, 7); // offset 7, index 0
        assert_eq!(render_data.series[1].points[0].y, 20.56);
        assert_eq!(render_data.series[1].mean, Some(20.56));

        // total_x_points = last_offset (7) + num_points_func_b (1) - gap (if series added)
        // current_offset after func_a = 2 (len) + 5 (gap) = 7
        // max_x after func_a = 7 - 5 = 2
        // current_offset after func_b = 7 (prev_offset) + 1 (len) + 5 (gap) = 13
        // max_x after func_b = 13 - 5 = 8
        assert_eq!(render_data.total_x_points, 8);
    }

    #[test]
    fn test_prepare_line_chart_render_data_empty_measurements() {
        let results = vec![BenchmarkReport {
            config: BenchmarkConfig {
                function_name: "func_a".to_string(),
                memory_size: 128,
                concurrent_invocations: 1,
                number: 1,
                timestamp: "".to_string(),
                runtime: None,
                architecture: None,
                environment: vec![],
            },
            cold_starts: vec![],
            warm_starts: vec![],
            client_measurements: vec![], // Empty
        }];
        let function_names = vec!["func_a".to_string()];
        let render_data = prepare_metric_line_chart_render_data(
            &results,
            &function_names,
            "Empty",
            "ms",
            "empty_line",
            |report| {
                report
                    .client_measurements
                    .iter()
                    .map(|m| m.client_duration)
                    .collect()
            },
        );

        assert_eq!(render_data.series.len(), 1);
        assert_eq!(render_data.series[0].name, "func_a");
        assert_eq!(render_data.series[0].points.len(), 0);
        assert_eq!(render_data.series[0].mean, None);
        assert_eq!(render_data.total_x_points, 0); // max_x remains 0 if no points
    }

    #[test]
    fn test_metric_descriptions() {
        // Test known cold start metric types have descriptions
        assert!(get_metric_description("cold_init").is_some());
        assert!(get_metric_description("cold_server").is_some());
        assert!(get_metric_description("cold_start_memory").is_some());
        assert!(get_metric_description("cold_start_produced_bytes").is_some());

        // Test known warm start metric types have descriptions
        assert!(get_metric_description("warm_start_client_duration").is_some());
        assert!(get_metric_description("warm_start_server_duration").is_some());
        assert!(get_metric_description("warm_start_extension_overhead").is_some());
        assert!(get_metric_description("warm_start_memory").is_some());
        assert!(get_metric_description("warm_start_produced_bytes").is_some());

        // Test unknown metric type returns None
        assert!(get_metric_description("unknown_metric").is_none());

        // Test that bar chart includes description for known metric types
        let function_names = vec!["test_func".to_string()];
        let stats = vec![(10.0, 15.0, 14.0, 12.0, 1.0)];

        let bar_data = prepare_bar_chart_render_data(
            &function_names,
            &stats,
            "Cold Start - Init Duration",
            "ms",
            "cold_init",
        );

        assert!(bar_data.description.is_some());
        assert!(bar_data
            .description
            .unwrap()
            .contains("AWS Lambda spends initializing"));
    }
}
