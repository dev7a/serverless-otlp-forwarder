use otel_benchmark_cli::{
    benchmark::{self, run_function_benchmark, run_stack_benchmark},
    jekyll::generate_jekyll_docs,
    report::generate_reports,
    telemetry::init_telemetry,
    types::{self, EnvVar, StackBenchmarkConfig},
};

use anyhow::{Context, Result};
use aws_sdk_cloudformation::Client as CloudFormationClient;
use aws_sdk_lambda::Client as LambdaClient;
use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Parser)]
#[command(author, version, about = "Benchmark Lambda implementations")]
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
        memory: Option<i32>,

        /// Number of concurrent invocations
        #[arg(short = 'c', long, default_value_t = 1)]
        concurrent: u32,

        /// Number of rounds for warm starts
        #[arg(short = 'r', long, default_value_t = 1)]
        rounds: u32,

        /// Directory to save the benchmark results
        #[arg(short = 'd', long = "dir", required = true)]
        output_dir: String,

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

        /// Select functions by name pattern
        #[arg(short = 's', long)]
        select: Option<String>,

        /// Memory size in MB
        #[arg(short, long)]
        memory: Option<i32>,

        /// Number of concurrent invocations
        #[arg(short = 'c', long, default_value_t = 1)]
        concurrent: u32,

        /// Number of rounds for warm starts
        #[arg(short = 'r', long, default_value_t = 1)]
        rounds: u32,

        /// Directory to save the benchmark results
        #[arg(short = 'd', long = "dir", required = true)]
        output_dir: String,

        /// JSON payload to send with each invocation
        #[arg(long, conflicts_with = "payload_file")]
        payload: Option<String>,

        /// JSON file containing the payload to send with each invocation
        #[arg(long = "payload-file", conflicts_with = "payload")]
        payload_file: Option<String>,

        /// Run function tests in parallel
        #[arg(short = 'p', long)]
        parallel: bool,

        /// Environment variables to set (can be specified multiple times)
        #[arg(short = 'e', long = "env", value_parser = clap::value_parser!(EnvVar))]
        environment: Vec<EnvVar>,

        /// Proxy Lambda function to use for client-side measurements
        #[arg(long = "proxy")]
        proxy: Option<String>,
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
    },

    /// Run batch benchmarks from a configuration file
    Batch {
        /// Path to the configuration file
        #[arg(short = 'c', long = "config", default_value = "benchmark-config.yaml")]
        config: String,

        /// Directory to save the benchmark results
        #[arg(short = 'd', long = "dir", required = true)]
        input_dir: String,

        /// Output directory for generated files
        #[arg(short = 'o', long = "output", required_if_eq_any([("report", "true"), ("jekyll", "true")]))]
        output_dir: Option<String>,

        /// Generate report after benchmarking
        #[arg(long = "report", conflicts_with = "jekyll")]
        report: bool,

        /// Generate Jekyll documentation after benchmarking
        #[arg(long = "jekyll", conflicts_with = "report")]
        jekyll: bool,
    },

    /// Generate Jekyll documentation from benchmark results
    Jekyll {
        /// Directory containing benchmark results
        #[arg(short = 'd', long = "dir", required = true)]
        input_dir: String,

        /// Output directory for Jekyll files
        #[arg(short = 'o', long = "output", required = true)]
        output_dir: String,

        /// Path to the configuration file
        #[arg(short = 'c', long = "config", default_value = "benchmark-config.yaml")]
        config: String,
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

    let tracer_provider = init_telemetry().await?;

    // Initialize AWS clients
    let config = aws_config::load_from_env().await;
    let lambda_client = aws_sdk_lambda::Client::new(&config);
    let cf_client = aws_sdk_cloudformation::Client::new(&config);

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

            run_function_benchmark(
                &client,
                &function_name,
                memory,
                concurrent,
                rounds,
                payload.as_deref(),
                &output_dir,
                &environment
                    .iter()
                    .map(|e| (e.key.as_str(), e.value.as_str()))
                    .collect::<Vec<_>>(),
                true,
                proxy.as_deref(),
            )
            .await
        }

        Commands::Stack {
            stack_name,
            select,
            memory,
            concurrent,
            rounds,
            output_dir,
            payload,
            payload_file,
            parallel,
            environment,
            proxy,
        } => {
            // If we have a selector, append it to the output directory
            let output_dir = if let Some(ref selector) = select {
                format!("{}/{}", output_dir, selector)
            } else {
                output_dir
            };

            execute_stack_command(
                stack_name,
                select,
                memory,
                concurrent,
                rounds,
                output_dir,
                payload,
                payload_file,
                parallel,
                environment,
                proxy,
            )
            .await
        }

        Commands::Report {
            input_dir,
            output_dir,
            screenshot,
        } => {
            let screenshot_theme = screenshot.map(|theme| match theme {
                Theme::Light => "light",
                Theme::Dark => "dark",
            });
            generate_reports(&input_dir, &output_dir, None, screenshot_theme).await
        }

        Commands::Jekyll {
            input_dir,
            output_dir,
            config,
        } => {
            // Read and parse batch configuration
            let batch_config: types::BatchConfig = serde_yaml::from_reader(
                fs::File::open(&config)
                    .context(format!("Failed to open config file: {}", config))?,
            )
            .context("Failed to parse config file")?;

            // Generate Jekyll documentation
            generate_jekyll_docs(&input_dir, &output_dir, &batch_config).await
        }

        Commands::Batch {
            config,
            input_dir,
            output_dir,
            report,
            jekyll,
        } => {
            // Read and parse batch configuration
            let batch_config: types::BatchConfig = serde_yaml::from_reader(
                fs::File::open(&config)
                    .context(format!("Failed to open config file: {}", config))?,
            )
            .context("Failed to parse config file")?;

            // Create test index
            let mut test_index = types::TestIndex {
                timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                tests: Vec::new(),
            };

            // Process each test
            for test in &batch_config.tests {
                if benchmark::is_interrupted() {
                    println!("\nInterrupted, skipping remaining tests");
                    break;
                }

                println!("\nProcessing test: {} ({})", test.title, test.description);

                // Resolve settings (test overrides take precedence over global settings)
                let memory_sizes = test
                    .memory_sizes
                    .as_ref()
                    .unwrap_or(&batch_config.global.memory_sizes);
                let concurrent = test.concurrent.unwrap_or(batch_config.global.concurrent);
                let rounds = test.rounds.unwrap_or(batch_config.global.rounds);
                let parallel = test.parallel.unwrap_or(batch_config.global.parallel);
                let stack_name: &String = test
                    .stack_name
                    .as_ref()
                    .unwrap_or(&batch_config.global.stack_name);

                // Create test metadata
                let test_metadata = types::TestMetadata {
                    title: test.title.clone(),
                    description: test.description.clone(),
                    stack_name: stack_name.clone(),
                    selector: test.selector.clone(),
                    timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                    memory_sizes: memory_sizes.clone(),
                    concurrent,
                    rounds,
                    parallel,
                    environment: test.merge_environment(&batch_config.global.environment),
                };

                // Add to test index
                test_index.tests.push(test_metadata.clone());

                // Create test directory
                let test_dir = PathBuf::from(&input_dir)
                    .join(stack_name)
                    .join(&test.selector)
                    .join(&test.name);
                fs::create_dir_all(&test_dir).context(format!(
                    "Failed to create test directory: {}",
                    test_dir.display()
                ))?;

                // Save test metadata
                serde_yaml::to_writer(
                    fs::File::create(test_dir.join("metadata.yaml"))
                        .context("Failed to create metadata file")?,
                    &test_metadata,
                )
                .context("Failed to write test metadata")?;

                // Run test for each memory configuration
                for memory in memory_sizes {
                    if benchmark::is_interrupted() {
                        println!("\nInterrupted, skipping remaining memory configurations");
                        break;
                    }

                    println!("\nTesting with {} MB memory", memory);

                    // Get payload
                    let payload = match &test.payload {
                        types::PayloadConfig::Path(path) => Some(
                            fs::read_to_string(path)
                                .context(format!("Failed to read payload file: {}", path))?,
                        ),
                        types::PayloadConfig::Inline(value) => Some(value.to_string()),
                    };

                    // Run benchmark with test directory as output
                    let config = types::StackBenchmarkConfig {
                        stack_name: stack_name.clone(),
                        pattern: Some(test.selector.clone()),
                        memory_size: Some(*memory),
                        concurrent_invocations: concurrent as usize,
                        rounds: rounds as usize,
                        output_dir: test_dir.to_string_lossy().to_string(),
                        payload,
                        parallel,
                        environment: test
                            .merge_environment(&batch_config.global.environment)
                            .iter()
                            .map(|(k, v)| types::EnvVar {
                                key: k.clone(),
                                value: v.clone(),
                            })
                            .collect(),
                        client_metrics_mode: true,
                        proxy_function: test.get_proxy_function(&batch_config.global),
                    };

                    benchmark::run_stack_benchmark(&lambda_client, &cf_client, config).await?;
                }
            }

            // Save test index
            serde_yaml::to_writer(
                fs::File::create(format!("{}/index.yaml", input_dir))
                    .context("Failed to create index file")?,
                &test_index,
            )
            .context("Failed to write test index")?;

            // Generate report if requested
            if report {
                println!("\nGenerating report...");
                let output_dir = output_dir
                    .as_ref()
                    .expect("Output directory is required when generating reports");
                generate_reports(&input_dir, output_dir, None, None).await?;
            }

            // Generate Jekyll documentation if requested
            if jekyll {
                println!("\nGenerating Jekyll documentation...");
                let output_dir = output_dir
                    .as_ref()
                    .expect("Output directory is required when generating Jekyll documentation");
                generate_jekyll_docs(&input_dir, output_dir, &batch_config).await?;
            }

            Ok(())
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
    select: Option<String>,
    memory: Option<i32>,
    concurrent: u32,
    rounds: u32,
    output_dir: String,
    payload: Option<String>,
    payload_file: Option<String>,
    parallel: bool,
    environment: Vec<EnvVar>,
    proxy: Option<String>,
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
        pattern: select,
        memory_size: memory,
        concurrent_invocations: concurrent as usize,
        rounds: rounds as usize,
        output_dir,
        payload,
        parallel,
        environment,
        client_metrics_mode: true,
        proxy_function: proxy,
    };

    run_stack_benchmark(&lambda_client, &cf_client, config).await
}
