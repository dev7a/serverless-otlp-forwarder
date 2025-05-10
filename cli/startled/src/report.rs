use crate::screenshot::take_chart_screenshot;
use crate::stats::{
    calculate_client_stats, calculate_cold_start_extension_overhead_stats,
    calculate_cold_start_init_stats, calculate_cold_start_server_stats,
    calculate_cold_start_total_duration_stats, calculate_memory_stats, calculate_warm_start_stats,
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
    page_type: String, // e.g., "cold_init", for context in JS if needed
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
}

#[derive(Serialize)]
enum ChartRenderData {
    Bar(BarChartRenderData),
    Line(LineChartRenderData),
}

/// Generate a chart with the given options
#[allow(clippy::too_many_arguments)]
async fn generate_chart(
    html_dir: &Path,
    png_dir: Option<&Path>,
    name: &str,
    chart_render_data: &ChartRenderData,
    config: &BenchmarkConfig,
    screenshot_theme: Option<&str>,
    pb: &ProgressBar,
    report_structure: &ReportStructure,
    current_group: &str,
    current_subgroup: &str,
    template_dir: Option<&String>,
    base_url: Option<&str>,
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
    // Extract title and page_type from the enum variant
    let (title, page_type) = match chart_render_data {
        ChartRenderData::Bar(data) => (data.title.as_str(), data.page_type.as_str()),
        ChartRenderData::Line(data) => (data.title.as_str(), data.page_type.as_str()),
    };
    ctx.insert("title", title);
    ctx.insert("config", config);
    ctx.insert("chart_id", "chart");
    ctx.insert("page_type", page_type);
    ctx.insert("chart_data_js", data_js_filename);

    // Add sidebar context
    ctx.insert("report_structure", report_structure);
    ctx.insert("current_group", current_group);
    ctx.insert("current_subgroup", current_subgroup);
    ctx.insert("base_path", &calculate_base_path(html_dir, base_url)?);

    // Use the kebab-case name for URL references
    ctx.insert("kebab_name", &kebab_name);

    // Render the index.html file inside the chart directory
    let html_path = chart_dir.join("index.html");
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

/// Generates the main landing page (index.html) for the reports.
async fn generate_landing_page(
    output_directory: &str,
    report_structure: &ReportStructure,
    custom_title: Option<&str>,
    pb: &ProgressBar,
    template_dir: Option<&String>,
    readme_file: Option<&str>,
    base_url: Option<&str>,
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
    // Landing page specific context
    ctx.insert("is_landing_page", &true);
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
        // Link to the first subgroup's default chart - use kebab-case with trailing slash
        let link_path = format!("{}/{}/cold-start-init/", group_name, first_subgroup_name);
        items.push(
            IndexItem::new(group_name, link_path)
                .with_subtitle(format!("{} configurations", subgroups.len())),
        );
    }
    ctx.insert("items", &items);

    let index_path = Path::new(output_directory).join("index.html");
    pb.set_message(format!("Generating landing page: {}", index_path.display()));
    let html = tera.render("index.html", &ctx)?;
    fs::write(&index_path, html)?;
    Ok(())
}

pub async fn generate_reports(
    input_directory: &str,
    output_directory: &str,
    custom_title: Option<&str>,
    base_url: Option<&str>,
    screenshot_theme: Option<&str>,
    template_dir: Option<String>,
    readme_file: Option<String>,
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
    println!("Scanning report structure...");
    let report_structure = scan_report_structure(input_directory)?;
    if report_structure.is_empty() {
        anyhow::bail!("No valid benchmark data found in the input directory structure.");
    }
    println!("✓ Report structure scanned: {:#?}", report_structure);

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
                screenshot_theme,
                &main_pb,
                &report_structure, // Pass full structure for sidebar
                group_name,
                subgroup_name,
                template_dir.as_ref(),
                base_url,
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
        &landing_pb,
        template_dir.as_ref(),
        readme_file.as_deref(),
        base_url,
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
        let js_lib_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/templates/js/lib.js");
        fs::copy(&js_lib_src, &js_lib_dst).context("Failed to copy default lib.js")?;
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
    screenshot_theme: Option<&str>,
    pb: &ProgressBar,
    report_structure: &ReportStructure,
    current_group: &str,
    current_subgroup: &str,
    template_dir: Option<&String>,
    base_url: Option<&str>,
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
            calculate_cold_start_init_stats(&report.cold_starts).unwrap_or((0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let cold_server_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_server_stats(&report.cold_starts).unwrap_or((0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let client_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_client_stats(&report.client_measurements).unwrap_or((0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let server_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_warm_start_stats(&report.warm_starts, |m| m.duration)
                .unwrap_or((0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let cold_extension_overhead_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_extension_overhead_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let cold_total_duration_stats: Vec<_> = results
        .iter()
        .map(|report| {
            calculate_cold_start_total_duration_stats(&report.cold_starts)
                .unwrap_or((0.0, 0.0, 0.0, 0.0))
        })
        .collect();

    let memory_stats: Vec<_> = results
        .iter()
        .map(|report| calculate_memory_stats(&report.warm_starts).unwrap_or((0.0, 0.0, 0.0, 0.0)))
        .collect();

    // Generate cold start init duration chart if we have data
    if results.iter().any(|r| !r.cold_starts.is_empty()) {
        let cold_init_render_data = prepare_bar_chart_render_data(
            &function_names,
            &cold_init_stats,
            custom_title.unwrap_or("Cold Start - Init Duration"),
            "ms",
            "cold_init",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_init",
            &ChartRenderData::Bar(cold_init_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;

        let cold_server_render_data = prepare_bar_chart_render_data(
            &function_names,
            &cold_server_stats,
            custom_title.unwrap_or("Cold Start - Server Duration"),
            "ms",
            "cold_server",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_server",
            &ChartRenderData::Bar(cold_server_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;

        let cold_ext_overhead_render_data = prepare_bar_chart_render_data(
            &function_names,
            &cold_extension_overhead_stats,
            custom_title.unwrap_or("Cold Start - Extension Overhead"),
            "ms",
            "cold_extension_overhead",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_extension_overhead",
            &ChartRenderData::Bar(cold_ext_overhead_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;

        let cold_total_duration_render_data = prepare_bar_chart_render_data(
            &function_names,
            &cold_total_duration_stats,
            custom_title.unwrap_or("Cold Start - Total Cold Start Duration"),
            "ms",
            "cold_total_duration",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "cold_start_total_duration",
            &ChartRenderData::Bar(cold_total_duration_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;
    }

    // Generate client duration chart if we have data
    if results.iter().any(|r| !r.client_measurements.is_empty()) {
        let client_duration_render_data = prepare_bar_chart_render_data(
            &function_names,
            &client_stats,
            custom_title.unwrap_or("Warm Start - Client Duration"),
            "ms",
            "client",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "client_duration",
            &ChartRenderData::Bar(client_duration_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;

        // // Restore client_duration_time.html - UNCOMMENT AND UPDATE
        let client_time_render_data = prepare_line_chart_render_data(
            &results,
            &function_names,
            custom_title.unwrap_or("Warm Start: Client Duration Over Time"),
            "ms",
            "client_time",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "client_duration_time",
            &ChartRenderData::Line(client_time_render_data), // Wrap in enum
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;
    }

    // Generate server duration chart if we have data
    if results.iter().any(|r| !r.warm_starts.is_empty()) {
        let server_duration_render_data = prepare_bar_chart_render_data(
            &function_names,
            &server_stats,
            custom_title.unwrap_or("Warm Start - Server Duration"),
            "ms",
            "server",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "server_duration",
            &ChartRenderData::Bar(server_duration_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;

        let warm_extension_overhead_stats: Vec<_> = results
            .iter()
            .map(|report| {
                calculate_warm_start_stats(&report.warm_starts, |m| m.extension_overhead)
                    .unwrap_or((0.0, 0.0, 0.0, 0.0))
            })
            .collect();
        let ext_overhead_render_data = prepare_bar_chart_render_data(
            &function_names,
            &warm_extension_overhead_stats,
            custom_title.unwrap_or("Warm Start - Extension Overhead"),
            "ms",
            "extension_overhead",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "extension_overhead",
            &ChartRenderData::Bar(ext_overhead_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;

        let memory_render_data = prepare_bar_chart_render_data(
            &function_names,
            &memory_stats,
            custom_title.unwrap_or("Memory Usage"),
            "MB",
            "memory",
        );
        generate_chart(
            &PathBuf::from(output_directory),
            png_dir.as_deref(),
            "memory_usage",
            &ChartRenderData::Bar(memory_render_data),
            &results[0].config,
            screenshot_theme,
            pb,
            report_structure,
            current_group,
            current_subgroup,
            template_dir,
            base_url,
        )
        .await?;
    }

    Ok(())
}

fn prepare_bar_chart_render_data(
    function_names: &[String],
    stats: &[(f64, f64, f64, f64)], // Expects (avg, p99, p95, p50) for each function
    title: &str,
    unit: &str,
    page_type: &str,
) -> BarChartRenderData {
    let series_render_data = function_names
        .iter()
        .zip(stats.iter())
        .map(|(name, &(avg, p99, p95, p50))| SeriesRenderData {
            name: name.clone(),
            values: vec![avg.round(), p99.round(), p95.round(), p50.round()],
        })
        .collect();

    BarChartRenderData {
        title: title.to_string(),
        unit: unit.to_string(),
        y_axis_categories: vec![
            "AVG".to_string(),
            "P99".to_string(),
            "P95".to_string(),
            "P50".to_string(),
        ],
        series: series_render_data,
        page_type: page_type.to_string(),
    }
}

fn prepare_line_chart_render_data(
    results: &[BenchmarkReport],
    function_names: &[String],
    title: &str,
    unit: &str, // e.g., "ms"
    page_type: &str,
) -> LineChartRenderData {
    let gap = 5; // Keep the gap logic for separating series visually on x-axis
    let mut current_offset = 0;
    let mut max_x = 0;

    let series_render_data: Vec<LineSeriesRenderData> = function_names
        .iter()
        .zip(results.iter())
        .map(|(name, report)| {
            let x_offset = current_offset;
            let num_points = report.client_measurements.len();
            current_offset += num_points + gap; // Update offset for next series
            if current_offset > gap {
                // Update max_x only if points were added
                max_x = current_offset - gap;
            } else {
                // If a series has 0 points, don't let max_x be negative or zero based on gap
                max_x = max_x.max(0);
            }

            let mut points_sum = 0.0;
            let points_data: Vec<ScatterPoint> = report
                .client_measurements
                .iter()
                .enumerate()
                .map(|(index, m)| {
                    let duration = Decimal::from_f64(m.client_duration)
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
        x_axis_label: "Test Sequence".to_string(), // Or make configurable
        y_axis_label: format!("Duration ({})", unit),
        unit: unit.to_string(),
        series: series_render_data,
        total_x_points: max_x, // Use the calculated max offset
        page_type: page_type.to_string(),
    }
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
            (10.5, 15.1, 14.2, 12.3), // avg, p99, p95, p50 for func_a
            (20.0, 25.5, 24.0, 22.5), // avg, p99, p95, p50 for func_b
        ];
        let title = "Test Bar Chart";
        let unit = "ms";
        let page_type = "test_bar";

        let render_data =
            prepare_bar_chart_render_data(&function_names, &stats, title, unit, page_type);

        assert_eq!(render_data.title, title);
        assert_eq!(render_data.unit, unit);
        assert_eq!(render_data.page_type, page_type);
        assert_eq!(
            render_data.y_axis_categories,
            vec!["AVG", "P99", "P95", "P50"]
        );

        assert_eq!(render_data.series.len(), 2);
        // Series 1 (func_a)
        assert_eq!(render_data.series[0].name, "func_a");
        assert_eq!(render_data.series[0].values, vec![11.0, 15.0, 14.0, 12.0]); // Rounded
                                                                                // Series 2 (func_b)
        assert_eq!(render_data.series[1].name, "func_b");
        assert_eq!(render_data.series[1].values, vec![20.0, 26.0, 24.0, 23.0]); // Rounded
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
                    memory_size: Some(128),
                    concurrent_invocations: 1,
                    rounds: 1,
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
                    memory_size: Some(128),
                    concurrent_invocations: 1,
                    rounds: 1,
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

        let render_data =
            prepare_line_chart_render_data(&results, &function_names, title, unit, page_type);

        assert_eq!(render_data.title, title);
        assert_eq!(render_data.unit, unit);
        assert_eq!(render_data.page_type, page_type);
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
                memory_size: Some(128),
                concurrent_invocations: 1,
                rounds: 1,
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
        let render_data =
            prepare_line_chart_render_data(&results, &function_names, "Empty", "ms", "empty_line");

        assert_eq!(render_data.series.len(), 1);
        assert_eq!(render_data.series[0].name, "func_a");
        assert_eq!(render_data.series[0].points.len(), 0);
        assert_eq!(render_data.series[0].mean, None);
        assert_eq!(render_data.total_x_points, 0); // max_x remains 0 if no points
    }
}
