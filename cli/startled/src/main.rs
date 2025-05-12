use startled::{
    benchmark::{run_function_benchmark, run_stack_benchmark},
    report::generate_reports,
    telemetry::init_telemetry,
    types::{EnvVar, StackBenchmarkConfig},
    utils::validate_fs_safe_name,
};

use anyhow::{anyhow, Context, Result};
use aws_sdk_cloudformation::Client as CloudFormationClient;
use aws_sdk_lambda::Client as LambdaClient;
use clap::{crate_authors, crate_description, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::generate;
use clap_complete::Shell as ClapShell; // Alias to avoid conflict with local Theme if any, or just for clarity
use std::fs;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Theme {
    Light,
    Dark,
}

const USAGE_EXAMPLES: &str = "\
EXAMPLES:
    # Benchmark a single Lambda function with 10 concurrent invocations
    startled function my-lambda-function -c 10

    # Benchmark all functions in a CloudFormation stack matching \"service-a\"
    startled stack my-app-stack -s \"service-a\" --output-dir ./benchmark_results

    # Benchmark a function with a specific memory size and payload from a file
    startled function my-lambda-function --memory 512 --payload-file ./payload.json

    # Generate HTML reports from benchmark results in a directory
    startled report -d ./benchmark_results -o ./reports --screenshot light

    # Generate shell completions for bash
    startled generate-completions bash";

#[derive(Parser)]
#[command(author = crate_authors!(", "), version, about = crate_description!(), long_about = None, after_help = USAGE_EXAMPLES)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Test a single Lambda function
    Function {
        /// Lambda function ARN or name
        function_name: String,

        /// Memory size in MB
        #[arg(short, long)]
        memory: i32,

        /// Number of concurrent invocations
        #[arg(short = 'c', long, default_value_t = 1)]
        concurrent: u32,

        /// Number of requests/repetitions for warm starts
        #[arg(short = 'n', long = "number", default_value_t = 1)]
        rounds: u32,

        /// Directory to save the benchmark results (optional)
        #[arg(short = 'd', long = "dir")]
        output_dir: Option<String>,

        /// JSON payload to send with each invocation
        #[arg(long, conflicts_with = "payload_file")]
        payload: Option<String>,

        /// JSON file containing the payload to send with each invocation
        #[arg(long = "payload-file", conflicts_with = "payload")]
        payload_file: Option<String>,

        /// Environment variables to set (can be specified multiple times)
        #[arg(short = 'e', long = "env", value_parser = clap::value_parser!(EnvVar))]
        environment: Vec<EnvVar>,

        /// Proxy Lambda function to use for client-side measurements
        #[arg(long = "proxy")]
        proxy: Option<String>,
    },

    /// Test all functions in a CloudFormation stack
    Stack {
        /// CloudFormation stack name
        stack_name: String,

        /// Pattern for substring matching against function names/ARNs. Used for directory naming if --select-name is not provided.
        #[arg(short = 's', long)]
        select: String,

        /// Optional: Regular expression for filtering functions. Overrides --select for filtering if provided.
        #[arg(long = "select-regex")]
        select_regex: Option<String>,

        /// Optional: Specific name to use for the output directory group. Overrides --select for directory naming.
        #[arg(long = "select-name")]
        select_name: Option<String>,

        /// Memory size in MB
        #[arg(short = 'm', long)]
        memory: i32,

        /// Number of concurrent invocations
        #[arg(short = 'c', long, default_value_t = 1)]
        concurrent: u32,

        /// Number of requests/repetitions for warm starts
        #[arg(short = 'n', long = "number", default_value_t = 1)]
        rounds: u32,

        /// Directory to save the benchmark results (optional)
        #[arg(short = 'd', long = "dir")]
        output_dir: Option<String>,

        /// JSON payload to send with each invocation
        #[arg(long, conflicts_with = "payload_file")]
        payload: Option<String>,

        /// JSON file containing the payload to send with each invocation
        #[arg(long = "payload-file", conflicts_with = "payload")]
        payload_file: Option<String>,

        /// Environment variables to set (can be specified multiple times)
        #[arg(short = 'e', long = "env", value_parser = clap::value_parser!(EnvVar))]
        environment: Vec<EnvVar>,

        /// Proxy Lambda function to use for client-side measurements
        #[arg(long = "proxy")]
        proxy: Option<String>,

        /// Run benchmarks for selected functions in parallel
        #[arg(long, default_value_t = false)]
        parallel: bool,
    },

    /// Generate visualization reports from benchmark results
    Report {
        /// Directory containing benchmark results
        #[arg(short = 'd', long = "dir", required = true)]
        input_dir: String,

        /// Output directory for report files
        #[arg(short = 'o', long = "output", required = true)]
        output_dir: String,

        /// Generate screenshots with specified theme
        #[arg(long, value_name = "THEME")]
        screenshot: Option<Theme>,

        /// Custom template directory for report generation
        #[arg(long = "template-dir")]
        template_dir: Option<String>,

        /// Markdown file to include as content on the landing page
        #[arg(long = "readme", value_name = "MARKDOWN_FILE")]
        readme_file: Option<String>,

        /// Base URL path for generated links (e.g., "/reports/")
        #[arg(long = "base-url", value_name = "URL_PATH")]
        base_url: Option<String>,
    },
    /// Generate shell completion script
    #[command(name = "generate-completions", hide = true)]
    GenerateCompletions {
        /// Shell for which to generate completions
        #[arg(value_enum)]
        shell: ClapShell,
    },
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("\n❌ Error: {}", err);

        // For other errors, show the error chain for debugging
        if let Some(cause) = err.source() {
            eprintln!("\nCaused by:");
            let mut current = Some(cause);
            let mut i = 0;
            while let Some(e) = current {
                eprintln!("  {}: {}", i, e);
                current = e.source();
                i += 1;
            }
        }
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();

    // Handle generate-completions before initializing telemetry or other logic
    if let Commands::GenerateCompletions { shell } = args.command {
        let mut cmd = Args::command();
        let bin_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }

    let tracer_provider = init_telemetry().await?;

    match args.command {
        Commands::Function {
            function_name,
            memory,
            concurrent,
            rounds,
            output_dir,
            payload,
            payload_file,
            environment,
            proxy,
        } => {
            let config = aws_config::load_from_env().await;
            let client = LambdaClient::new(&config);

            // Handle payload options
            let payload = if let Some(file) = payload_file {
                Some(
                    fs::read_to_string(&file)
                        .context(format!("Failed to read payload file: {}", file))?,
                )
            } else {
                payload
            };

            // Validate JSON if payload is provided
            if let Some(ref p) = payload {
                serde_json::from_str::<serde_json::Value>(p).context("Invalid JSON payload")?;
            }

            // Adjust output_dir to include "function" subdirectory
            let final_output_dir = output_dir.map(|base_path| {
                let mut path = std::path::PathBuf::from(base_path);
                path.push("function");
                path.to_string_lossy().into_owned()
            });

            run_function_benchmark(
                &client,
                &function_name,
                memory,
                concurrent,
                rounds,
                payload.as_deref(),
                final_output_dir.as_deref(),
                &environment
                    .iter()
                    .map(|e| (e.key.as_str(), e.value.as_str()))
                    .collect::<Vec<_>>(),
                true,
                proxy.as_deref(),
                false,
                None,
            )
            .await
        }

        Commands::Stack {
            stack_name,
            select,
            select_regex,
            select_name,
            memory,
            concurrent,
            rounds,
            output_dir,
            payload,
            payload_file,
            environment,
            proxy,
            parallel,
        } => {
            let directory_group_name = if let Some(name_override) = &select_name {
                validate_fs_safe_name(name_override)
                    .map_err(|e| anyhow!("Invalid --select-name: {}", e))?;
                name_override.clone()
            } else {
                validate_fs_safe_name(&select)
                        .map_err(|e| anyhow!("Invalid --select pattern for directory name: {}. Use --select-name to specify a different directory name.", e))?;
                select.clone()
            };

            // If output_dir is Some, construct the full path including the directory_group_name.
            // If output_dir is None, then final_output_dir_for_benchmark_group will also be None.
            let final_output_dir_for_benchmark_group: Option<String> =
                output_dir.map(|base_path| format!("{}/{}", base_path, directory_group_name));

            execute_stack_command(
                stack_name,
                select,       // This is select_arg (pattern)
                select_regex, // This is select_regex_arg
                memory,
                concurrent,
                rounds,
                final_output_dir_for_benchmark_group,
                payload,
                payload_file,
                environment,
                proxy,
                parallel,
            )
            .await
        }

        Commands::Report {
            input_dir,
            output_dir,
            screenshot,
            template_dir,
            readme_file,
            base_url,
        } => {
            let screenshot_theme = screenshot.map(|theme| match theme {
                Theme::Light => "light",
                Theme::Dark => "dark",
            });
            generate_reports(
                &input_dir,
                &output_dir,
                None,
                base_url.as_deref(),
                screenshot_theme,
                template_dir,
                readme_file,
            )
            .await
        }
        Commands::GenerateCompletions { .. } => {
            unreachable!(
                "clap should have handled GenerateCompletions and exited before this match arm"
            );
        }
    }?;
    // Ensure all spans are exported before exit
    if let Err(e) = tracer_provider.force_flush() {
        tracing::error!("Failed to flush spans: {}", e);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn execute_stack_command(
    stack_name: String,
    select_pattern_arg: String,       // from --select
    select_regex_arg: Option<String>, // from --select-regex
    memory: i32,
    concurrent: u32,
    rounds: u32,
    output_dir: Option<String>, // This is now base_dir/group_name or group_name
    payload: Option<String>,
    payload_file: Option<String>,
    environment: Vec<EnvVar>,
    proxy: Option<String>,
    parallel: bool,
) -> Result<()> {
    let config = aws_config::load_from_env().await;
    let lambda_client = LambdaClient::new(&config);
    let cf_client = CloudFormationClient::new(&config);

    // Handle payload options - payload takes precedence over payload_file
    let payload = if payload.is_some() {
        payload // Use the direct JSON string if provided
    } else if let Some(file) = payload_file {
        Some(fs::read_to_string(&file).context(format!("Failed to read payload file: {}", file))?)
    } else {
        None
    };

    // Validate JSON if payload is provided
    if let Some(ref p) = payload {
        serde_json::from_str::<serde_json::Value>(p).context("Invalid JSON payload")?;
    }

    let config = StackBenchmarkConfig {
        stack_name,
        select_pattern: select_pattern_arg,
        select_regex: select_regex_arg,
        memory_size: memory,
        concurrent_invocations: concurrent as usize,
        rounds: rounds as usize,
        output_dir, // Already correctly formed
        payload,
        environment,
        client_metrics_mode: true,
        proxy_function: proxy,
        parallel,
    };

    run_stack_benchmark(&lambda_client, &cf_client, config).await
}
