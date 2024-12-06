mod benchmark;
mod chart;
mod types;
use anyhow::Result;
use aws_sdk_lambda::Client as LambdaClient;
use clap::{Parser, Subcommand};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use crate::benchmark::run_benchmark;
use crate::chart::generate_chart_visualization;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run benchmark tests on a Lambda function
    Test {
        /// Lambda function ARN or name
        function_name: String,

        /// Memory size in MB
        #[arg(short, long)]
        memory_size: Option<i32>,

        /// Number of concurrent invocations
        #[arg(short = 'c', long, default_value_t = 1)]
        concurrent_invocations: u32,

        /// Number of rounds for warm starts
        #[arg(short = 'r', long, default_value_t = 1)]
        rounds: u32,

        /// Directory to save the JSON report (will be created if it doesn't exist)
        #[arg(short = 'd', long = "output-dir", default_value = "benchmark_results")]
        output_directory: String,
    },

    /// Generate interactive HTML charts using ECharts
    Chart {
        /// Directory containing benchmark JSON files
        directory: String,

        /// Output directory for HTML files
        #[arg(long = "output-dir")]
        output_dir: String,

        /// Custom title for the charts
        #[arg(short, long)]
        title: Option<String>,

        /// Generate screenshots of the charts
        #[arg(long)]
        screenshot: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt::init();

    match args.command {
        Commands::Test {
            function_name,
            memory_size,
            concurrent_invocations,
            rounds,
            output_directory,
        } => {
            let config = aws_config::load_from_env().await;
            let client = LambdaClient::new(&config);

            let report = run_benchmark(
                &client,
                function_name.clone(),
                memory_size,
                concurrent_invocations as usize,
                rounds as usize,
            )
            .await?;

            // Create output directory if it doesn't exist
            fs::create_dir_all(&output_directory)?;

            // Extract function name from ARN or use as is
            let function_name = function_name
                .split(':')
                .last()
                .unwrap_or(&function_name)
                .to_string();

            // Create filename with function name and memory size
            let memory_suffix = memory_size.map(|m| format!("-{}mb", m)).unwrap_or_default();

            let filename = format!("{}{}.json", function_name, memory_suffix);
            let output_path = PathBuf::from(output_directory).join(filename);

            let json = serde_json::to_string_pretty(&report)?;
            let mut file = File::create(&output_path)?;
            file.write_all(json.as_bytes())?;
            println!("\nDetailed report saved to: {}", output_path.display());
        }

        Commands::Chart {
            directory,
            output_dir,
            title,
            screenshot,
        } => {
            generate_chart_visualization(&directory, &output_dir, title.as_deref(), screenshot)
                .await?;
        }
    }

    Ok(())
}
