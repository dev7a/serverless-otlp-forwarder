use anyhow::{anyhow, Result};
use aws_sdk_cloudformation::Client as CloudFormationClient;
use aws_sdk_lambda::Client as LambdaClient;
use chrono::Local;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::console;
use crate::lambda;
use crate::types::*;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn clear_interrupt() {
    INTERRUPTED.store(false, Ordering::SeqCst);
}

pub fn is_interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

#[derive(Clone)]
struct FunctionBenchmarkConfig {
    function_name: String,
    memory_size: Option<i32>,
    concurrent: u32,
    rounds: u32,
    payload: Option<String>,
    #[allow(dead_code)]
    output_dir: String,
    environment: Vec<(String, String)>,
    proxy_function: Option<String>,
}

impl FunctionBenchmarkConfig {
    #[allow(clippy::too_many_arguments)]
    fn new(
        function_name: impl Into<String>,
        memory_size: Option<i32>,
        concurrent: u32,
        rounds: u32,
        payload: Option<String>,
        output_dir: impl Into<String>,
        environment: Vec<(String, String)>,
        proxy_function: Option<String>,
    ) -> Self {
        Self {
            function_name: function_name.into(),
            memory_size,
            concurrent,
            rounds,
            payload,
            output_dir: output_dir.into(),
            environment,
            proxy_function,
        }
    }
}

pub async fn save_report(report: BenchmarkReport, output_dir: &str) -> Result<()> {
    let memory_dir = report
        .config
        .memory_size
        .map_or("default".to_string(), |m| format!("{}mb", m));
    let output_path = PathBuf::from(output_dir).join(&memory_dir);
    fs::create_dir_all(&output_path)?;

    let function_name = report.config.function_name.clone();
    let filename = format!("{}.json", function_name);
    let output_path = output_path.join(filename);

    let json = serde_json::to_string_pretty(&report)?;
    let mut file = File::create(&output_path)?;
    file.write_all(json.as_bytes())?;
    println!("\nReport saved to: {}", output_path.display());
    Ok(())
}

#[derive(Default)]
pub struct BenchmarkResults {
    pub cold_starts: Vec<InvocationMetrics>,
    pub warm_starts: Vec<InvocationMetrics>,
    pub client_measurements: Vec<InvocationMetrics>,
}

async fn run_benchmark_pass(
    client: &LambdaClient,
    config: &FunctionBenchmarkConfig,
    client_metrics_mode: bool,
) -> Result<(BenchmarkResults, usize, usize, Vec<String>)> {
    use tokio::signal;

    let mut results = BenchmarkResults {
        cold_starts: Vec::new(),
        warm_starts: Vec::new(),
        client_measurements: Vec::new(),
    };
    let mut successes = 0;
    let mut failures = 0;
    let mut errors = Vec::new();

    // Cold starts - run concurrently
    let mut handles = Vec::new();
    for _ in 0..config.concurrent {
        let client = client.clone();
        let function_name = config.function_name.clone();
        let payload = config.payload.clone();
        let environment = config.environment.clone();
        let memory_size = config.memory_size;
        let proxy_function = config.proxy_function.clone();

        handles.push(tokio::spawn(async move {
            lambda::invoke_function(
                &client,
                &function_name,
                memory_size,
                payload.as_deref(),
                &environment,
                client_metrics_mode,
                proxy_function.as_deref(),
            )
            .await
        }));
    }

    // Wait for cold starts with Ctrl-C handling
    let cold_start_future = async {
        for handle in handles {
            match handle.await? {
                Ok(metrics) => {
                    results.cold_starts.push(metrics);
                    successes += 1;
                }
                Err(e) => {
                    failures += 1;
                    errors.push(format!("Cold start error: {e}"));
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    };

    // Handle Ctrl-C during cold starts
    if tokio::select! {
        result = cold_start_future => result.is_err(),
        _ = signal::ctrl_c() => {
            println!("\n\nReceived Ctrl-C, interrupting...");
            INTERRUPTED.store(true, Ordering::SeqCst);
            true
        }
    } {
        return Ok((results, successes, failures, errors));
    }

    // Setup progress bar for warm starts
    let progress = if config.rounds > 1 {
        let pb = ProgressBar::new(config.rounds as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} rounds",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    // Warm starts with Ctrl-C handling
    for _round in 1..=config.rounds {
        let mut handles = Vec::new();
        for _ in 0..config.concurrent {
            let client = client.clone();
            let function_name = config.function_name.clone();
            let payload = config.payload.clone();
            let environment = config.environment.clone();
            let memory_size = config.memory_size;
            let proxy_function = config.proxy_function.clone();

            handles.push(tokio::spawn(async move {
                lambda::invoke_function(
                    &client,
                    &function_name,
                    memory_size,
                    payload.as_deref(),
                    &environment,
                    client_metrics_mode,
                    proxy_function.as_deref(),
                )
                .await
            }));
        }

        // Handle Ctrl-C for each round of warm starts
        let warm_start_future = async {
            for handle in handles {
                match handle.await? {
                    Ok(metrics) => {
                        results.warm_starts.push(metrics);
                        successes += 1;
                    }
                    Err(e) => {
                        failures += 1;
                        errors.push(format!("Warm start error: {e}"));
                    }
                }
            }
            Ok::<(), anyhow::Error>(())
        };

        if tokio::select! {
            result = warm_start_future => result.is_err(),
            _ = signal::ctrl_c() => {
                println!("\n\nReceived Ctrl-C, interrupting...");
                if let Some(pb) = &progress {
                    pb.finish_and_clear();
                }
                INTERRUPTED.store(true, Ordering::SeqCst);
                true
            }
        } {
            return Ok((results, successes, failures, errors));
        }

        if let Some(pb) = &progress {
            pb.inc(1);
        }
    }

    // Finish progress bar
    if let Some(pb) = progress {
        pb.finish_and_clear();
    }

    Ok((results, successes, failures, errors))
}

#[allow(clippy::too_many_arguments)]
pub async fn run_function_benchmark(
    client: &LambdaClient,
    function_name: &str,
    memory_size: Option<i32>,
    concurrent: u32,
    rounds: u32,
    payload: Option<&str>,
    output_dir: Option<&str>,
    environment: &[(&str, &str)],
    client_metrics_mode: bool,
    proxy_function: Option<&str>,
) -> Result<()> {
    println!("\nStarting benchmark for: {}", function_name);

    // Check proxy function existence if provided
    if let Some(proxy_name) = proxy_function {
        println!("Checking proxy function '{}'...", proxy_name);
        lambda::check_function_exists(client, proxy_name).await?;
        println!("✓ Proxy function found.");
    }

    // Get function configuration to extract runtime and architecture
    let function = client
        .get_function()
        .function_name(function_name)
        .send()
        .await
        .map_err(|err| {
            if err.to_string().contains("ResourceNotFoundException") {
                anyhow!("Function '{}' not found", function_name)
            } else {
                anyhow!(
                    "Something went wrong: {}. Error getting function configuration. Please check your AWS configuration",
                    err
                )
            }
        })?;

    let config = function.configuration().ok_or_else(|| {
        anyhow!(
            "Failed to get function configuration for '{}'",
            function_name
        )
    })?;

    let runtime = config.runtime().map(|r| r.as_str().to_string());
    let architecture = if config.architectures().is_empty() {
        Some("x86_64".to_string())
    } else {
        config
            .architectures()
            .first()
            .map(|arch| arch.as_str().to_string())
    };

    println!("\nConfiguration:");
    println!(
        "  {:20}: {} MB",
        "Memory Size".dimmed(),
        memory_size.unwrap_or(128)
    );
    println!(
        "  {:20}: {}",
        "Runtime".dimmed(),
        runtime.as_deref().unwrap_or("unknown")
    );
    println!(
        "  {:20}: {}",
        "Architecture".dimmed(),
        architecture.as_deref().unwrap_or("unknown")
    );
    println!("  {:20}: {}", "Concurrency".dimmed(), concurrent);
    println!("  {:20}: {}", "Rounds".dimmed(), rounds);
    if let Some(proxy) = proxy_function {
        println!("  {:20}: {}", "Using Proxy Function".dimmed(), proxy);
    }
    if !environment.is_empty() {
        println!("  {:20}:", "Environment".dimmed());
        for (key, value) in environment {
            println!("    {}={}", key, value);
        }
    }

    // Print telemetry configuration
    println!("\nTelemetry:");
    if let (Ok(endpoint), Ok(service)) = (
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"),
        std::env::var("OTEL_SERVICE_NAME"),
    ) {
        println!("  {:20}: {}", "Service".dimmed(), service);
        println!("  {:20}: {}", "Endpoint".dimmed(), endpoint);
        println!(
            "  {:20}: {}",
            "Protocol".dimmed(),
            std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
                .unwrap_or_else(|_| "http/protobuf (default)".to_string())
        );

        // Region is required for AWS endpoints
        if endpoint.contains(".amazonaws.com") {
            let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            println!(
                "  {:20}: {}{}",
                "Region".dimmed(),
                region,
                if region == "us-east-1" { " *" } else { "" }
            );
        }
    } else {
        println!("  OpenTelemetry is not configured (OTEL_EXPORTER_OTLP_ENDPOINT and OTEL_SERVICE_NAME are required)");
    }

    // Convert environment variables
    let env: Vec<(String, String)> = environment
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Save original configuration if we're going to modify it
    let original_config = if memory_size.is_some() || !environment.is_empty() {
        Some(lambda::get_function_config(client, function_name).await?)
    } else {
        None
    };

    // Update function configuration if needed
    if memory_size.is_some() || !environment.is_empty() {
        println!("\nUpdating function configuration...");
        lambda::update_function_config(client, function_name, memory_size, &env).await?;
        println!("✓ Function configuration updated");
    }

    // Create a future for the benchmark execution
    let config = FunctionBenchmarkConfig::new(
        function_name.to_string(),
        memory_size,
        concurrent,
        rounds,
        payload.map(|s| s.to_string()),
        output_dir.unwrap_or("default").to_string(),
        env,
        proxy_function.map(|s| s.to_string()),
    );

    let result = async {
        // First pass - get server metrics and cold start
        println!("\nCollecting server metrics...");
        let (mut results, mut successes, mut failures, mut errors) =
            run_benchmark_pass(client, &config, false).await?;
        println!("✓ Server metrics collected");

        // If client metrics requested, do a second pass for warm starts only
        if client_metrics_mode {
            println!("\nCollecting client metrics...");
            // Run without logs to get accurate client metrics
            let (client_results, client_successes, client_failures, client_errors) =
                run_benchmark_pass(client, &config, true).await?;
            results.client_measurements = client_results.warm_starts;
            successes += client_successes;
            failures += client_failures;
            errors.extend(client_errors);
            println!("✓ Client metrics collected\n");
        }

        // Print results
        console::print_benchmark_results(function_name, &results);

        // Calculate and print success rate
        let total = successes + failures;
        let success_rate = if successes > 0 {
            100.0 * (successes as f64) / (total as f64)
        } else {
            0.0
        };
        println!("\n"); // Add separator before success/failure report
        if failures > 0 {
            println!("--- Invocation Errors (showing up to 10) ---");
            for (i, err) in errors.iter().take(10).enumerate() {
                println!("{}. {}", i + 1, err);
            }
            if errors.len() > 10 {
                println!("...and {} more errors.", errors.len() - 10);
            }
            println!("--- End Errors ---\n");
        }
        if (success_rate - 100.0).abs() < f64::EPSILON {
            println!("{}: {}", function_name, "Success rate: 100%".green());
        } else {
            println!(
                "{}: {}",
                function_name,
                format!("Success rate: {:.1}%", success_rate).red()
            );
        }

        // Save results
        if let Some(dir) = output_dir {
            save_report(
                BenchmarkReport {
                    config: BenchmarkConfig {
                        function_name: function_name.to_string(),
                        memory_size,
                        concurrent_invocations: concurrent,
                        rounds,
                        timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                        runtime,
                        architecture,
                        environment: environment
                            .iter()
                            .map(|(k, v)| EnvVar {
                                key: k.to_string(),
                                value: v.to_string(),
                            })
                            .collect(),
                    },
                    cold_starts: results
                        .cold_starts
                        .iter()
                        .filter_map(|m| m.to_cold_start())
                        .collect(),
                    warm_starts: results
                        .warm_starts
                        .iter()
                        .map(|m| m.to_warm_start())
                        .collect(),
                    client_measurements: results
                        .client_measurements
                        .iter()
                        .map(|m| m.to_client_metrics())
                        .collect(),
                },
                dir,
            )
            .await?;
        }

        Ok(())
    }
    .await;

    // Restore original configuration if we modified it
    if let Some(original) = original_config {
        // Always try to restore, even if the benchmark failed or was interrupted
        if let Err(e) = lambda::restore_function_config(client, function_name, &original).await {
            eprintln!("Warning: Failed to restore function configuration: {}", e);
        }
    }

    // Now return the benchmark result
    result
}

pub async fn run_stack_benchmark(
    lambda_client: &LambdaClient,
    cf_client: &CloudFormationClient,
    config: StackBenchmarkConfig,
) -> Result<()> {
    println!(
        "Analyzing stack: {}. This might take a moment...",
        config.stack_name
    );

    // Use list_stack_resources to get all resources in the stack
    let mut all_stack_resources = Vec::new();
    let mut next_token: Option<String> = None;
    loop {
        let resp = cf_client
            .list_stack_resources()
            .stack_name(&config.stack_name)
            .set_next_token(next_token)
            .send()
            .await?;

        // Correctly handle the slice returned by stack_resource_summaries()
        let summaries_slice: &[aws_sdk_cloudformation::types::StackResourceSummary] =
            resp.stack_resource_summaries();
        all_stack_resources.extend(summaries_slice.to_vec());

        next_token = resp.next_token().map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }

    let mut function_identifiers_to_benchmark = Vec::new();

    for resource_summary in all_stack_resources {
        if resource_summary.resource_type() == Some("AWS::Lambda::Function") {
            if let Some(physical_id) = resource_summary.physical_resource_id() {
                let id_matches = config.select_regex.as_ref().map_or_else(
                    || physical_id.contains(&config.select_pattern),
                    |re_str| match Regex::new(re_str) {
                        Ok(re) => re.is_match(physical_id),
                        Err(e) => {
                            eprintln!(
                                "Invalid regex '{}': {}. Skipping match for this resource.",
                                re_str, e
                            );
                            false
                        }
                    },
                );

                if id_matches {
                    function_identifiers_to_benchmark.push(physical_id.to_string());
                }
            }
        }
    }

    if function_identifiers_to_benchmark.is_empty() {
        println!(
            "{}",
            format!(
                "No Lambda functions found in stack '{}' matching select criteria '{}' (or regex: {:?}). Searched all 'AWS::Lambda::Function' resources.",
                config.stack_name,
                config.select_pattern,
                config.select_regex
            )
            .yellow()
        );
        return Ok(());
    }

    // Early feedback: Print the functions that will be benchmarked
    println!(
        "\n{}",
        "The following Lambda functions will be benchmarked:".green()
    );
    for func_id in &function_identifiers_to_benchmark {
        println!("  - {}", func_id);
    }
    println!(); // Add a blank line for better readability

    let total_functions = function_identifiers_to_benchmark.len();
    println!("Total functions to benchmark: {}", total_functions);

    for (index, function_arn_or_name) in function_identifiers_to_benchmark.iter().enumerate() {
        if is_interrupted() {
            println!("\nInterrupted, skipping remaining functions");
            return Err(anyhow!("Benchmark interrupted by user"));
        }
        println!(
            "\n[{}/{}] Benchmarking: {} {}",
            index + 1,
            total_functions,
            function_arn_or_name.bold(),
            config
                .memory_size
                .map_or_else(String::new, |m| format!("({}MB)", m))
        );

        let function_specific_output_dir = config.output_dir.as_ref().map(|base_output_dir| {
            let path = PathBuf::from(base_output_dir);
            path.to_string_lossy().into_owned()
        });

        if let Err(e) = run_function_benchmark(
            lambda_client,
            function_arn_or_name,
            config.memory_size,
            config.concurrent_invocations as u32,
            config.rounds as u32,
            config.payload.as_deref(),
            function_specific_output_dir.as_deref(), // Corrected: Option<String> to Option<&str>
            &config
                .environment
                .iter()
                .map(|e| (e.key.as_str(), e.value.as_str()))
                .collect::<Vec<_>>(),
            true,                             // client_metrics_mode is true for stack benchmarks
            config.proxy_function.as_deref(), // Corrected: Option<String> to Option<&str>
        )
        .await
        {
            eprintln!(
                "Error running benchmark for {}: {}",
                function_arn_or_name, e
            );
            // Decide if we should continue with other functions or stop
        }

        let progress_percentage = ((index + 1) as f64 / total_functions as f64) * 100.0;
        println!("{:.2}% complete", progress_percentage);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        BenchmarkConfig, BenchmarkReport, ClientMetrics, ColdStartMetrics, EnvVar, WarmStartMetrics,
    };
    use chrono::Local;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_function_benchmark_config_new() {
        let function_name = "test_func";
        let memory_size = Some(512);
        let concurrent = 10;
        let rounds = 5;
        let payload = Some("{}".to_string());
        let output_dir = "test_output";
        let environment = vec![("KEY".to_string(), "VALUE".to_string())];
        let proxy_function = Some("proxy_func".to_string());

        let config = FunctionBenchmarkConfig::new(
            function_name,
            memory_size,
            concurrent,
            rounds,
            payload.clone(),
            output_dir,
            environment.clone(),
            proxy_function.clone(),
        );

        assert_eq!(config.function_name, function_name);
        assert_eq!(config.memory_size, memory_size);
        assert_eq!(config.concurrent, concurrent);
        assert_eq!(config.rounds, rounds);
        assert_eq!(config.payload, payload);
        assert_eq!(config.output_dir, output_dir);
        assert_eq!(config.environment, environment);
        assert_eq!(config.proxy_function, proxy_function);
    }

    #[tokio::test]
    async fn test_save_report_happy_path() {
        let temp_dir = tempdir().unwrap();
        let output_dir_path = temp_dir.path().join("benchmark_reports");
        let output_dir_str = output_dir_path.to_str().unwrap();

        let report = BenchmarkReport {
            config: BenchmarkConfig {
                function_name: "my_test_lambda".to_string(),
                memory_size: Some(256),
                concurrent_invocations: 1,
                rounds: 1,
                timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                runtime: Some("nodejs18.x".to_string()),
                architecture: Some("arm64".to_string()),
                environment: vec![EnvVar {
                    key: "TEST_ENV".to_string(),
                    value: "TEST_VAL".to_string(),
                }],
            },
            cold_starts: vec![ColdStartMetrics {
                timestamp: "ts_cold".to_string(),
                init_duration: 100.0,
                duration: 200.0,
                extension_overhead: 10.0,
                total_cold_start_duration: Some(310.0),
                billed_duration: 300,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            }],
            warm_starts: vec![WarmStartMetrics {
                timestamp: "ts_warm".to_string(),
                duration: 50.0,
                extension_overhead: 5.0,
                billed_duration: 50,
                max_memory_used: 100,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            }],
            client_measurements: vec![ClientMetrics {
                timestamp: "ts_client".to_string(),
                client_duration: 30.0,
                memory_size: 256,
            }],
        };

        let save_result = save_report(report.clone(), output_dir_str).await;
        assert!(
            save_result.is_ok(),
            "Failed to save report: {:?}",
            save_result.err()
        );

        let expected_memory_dir = Path::new(output_dir_str).join("256mb");
        assert!(
            expected_memory_dir.exists(),
            "Memory specific directory was not created"
        );
        assert!(
            expected_memory_dir.is_dir(),
            "Memory specific path is not a directory"
        );

        let expected_file_path = expected_memory_dir.join("my_test_lambda.json");
        assert!(
            expected_file_path.exists(),
            "Report file was not created at {:?}",
            expected_file_path
        );
        assert!(expected_file_path.is_file(), "Report path is not a file");

        let file_content = fs::read_to_string(expected_file_path).unwrap();
        let saved_report: BenchmarkReport = serde_json::from_str(&file_content).unwrap();

        // Basic check, ideally compare all fields or use a proper diffing library for structs
        assert_eq!(
            saved_report.config.function_name,
            report.config.function_name
        );
        assert_eq!(saved_report.config.memory_size, report.config.memory_size);
        assert_eq!(saved_report.cold_starts.len(), 1);
        assert_eq!(saved_report.warm_starts.len(), 1);
        assert_eq!(saved_report.client_measurements.len(), 1);
        assert_eq!(
            saved_report.cold_starts[0].init_duration,
            report.cold_starts[0].init_duration
        );

        // Clean up
        temp_dir.close().unwrap();
    }

    #[tokio::test]
    async fn test_save_report_default_memory() {
        let temp_dir = tempdir().unwrap();
        let output_dir_path = temp_dir.path().join("benchmark_reports_default");
        let output_dir_str = output_dir_path.to_str().unwrap();

        let report = BenchmarkReport {
            config: BenchmarkConfig {
                function_name: "my_default_lambda".to_string(),
                memory_size: None, // Test default memory case
                concurrent_invocations: 1,
                rounds: 1,
                timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                runtime: Some("python3.9".to_string()),
                architecture: Some("x86_64".to_string()),
                environment: vec![],
            },
            cold_starts: vec![],
            warm_starts: vec![],
            client_measurements: vec![],
        };

        let save_result = save_report(report.clone(), output_dir_str).await;
        assert!(save_result.is_ok());

        let expected_memory_dir = Path::new(output_dir_str).join("default");
        assert!(
            expected_memory_dir.exists(),
            "Default memory directory was not created"
        );

        let expected_file_path = expected_memory_dir.join("my_default_lambda.json");
        assert!(
            expected_file_path.exists(),
            "Report file was not created for default memory"
        );

        temp_dir.close().unwrap();
    }
}
