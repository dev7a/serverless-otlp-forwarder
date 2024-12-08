use anyhow::{Context, Result};
use aws_sdk_lambda::Client as LambdaClient;
use base64::{engine::general_purpose, Engine};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;
use tracing::info;

use crate::types::*;

pub async fn update_function_memory(
    client: &LambdaClient,
    function_name: &str,
    memory_size: i32,
) -> Result<()> {
    client
        .update_function_configuration()
        .function_name(function_name)
        .memory_size(memory_size)
        .send()
        .await
        .context("Failed to update function memory")?;

    Ok(())
}

pub fn extract_metrics(logs: &str) -> Result<InvocationMetrics> {
    // Find the last platform.report line
    let report = logs
        .lines()
        .filter_map(|line| {
            // Try to parse each line as JSON
            serde_json::from_str::<PlatformReport>(line).ok()
        })
        .filter(|report| report.report_type == "platform.report")
        .last()
        .context("No platform.report found in logs")?;

    Ok(InvocationMetrics {
        init_duration: report.record.metrics.init_duration_ms,
        duration: report.record.metrics.duration_ms,
        billed_duration: report.record.metrics.billed_duration_ms,
        memory_size: report.record.metrics.memory_size_mb,
        max_memory_used: report.record.metrics.max_memory_used_mb,
    })
}

pub async fn invoke_function(
    client: &LambdaClient,
    function_name: &str,
    payload: Option<&str>,
) -> Result<InvocationMetrics> {
    let mut req = client
        .invoke()
        .function_name(function_name)
        .log_type(aws_sdk_lambda::types::LogType::Tail);

    if let Some(p) = payload {
        req = req.payload(aws_sdk_lambda::primitives::Blob::new(p));
    }

    let output = req.send().await?;

    // Extract and decode base64 logs
    let logs = output.log_result().context("No logs returned")?;
    let decoded_logs = general_purpose::STANDARD
        .decode(logs)
        .expect("Failed to decode base64 payload");

    extract_metrics(&String::from_utf8(decoded_logs).expect("Failed to decode logs"))
}

pub struct MetricsStats {
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p95: f64,
    pub mean: f64,
}

pub fn calculate_stats(values: &[f64]) -> MetricsStats {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let len = sorted.len();
    let p95_idx = (len as f64 * 0.95) as usize;

    MetricsStats {
        min: sorted.first().copied().unwrap_or(0.0),
        max: sorted.last().copied().unwrap_or(0.0),
        p50: sorted[len / 2],
        p95: sorted[p95_idx],
        mean: sorted.iter().sum::<f64>() / len as f64,
    }
}

fn print_stats(stats: &MetricsStats, metric: &str) {
    println!(
        "{:<15} {:>9.3} ms  {:>9.3} ms  {:>9.3} ms  {:>9.3} ms  {:>9.3} ms",
        metric, stats.min, stats.p50, stats.p95, stats.max, stats.mean
    );
}

fn print_memory_stats(stats: &MetricsStats, metric: &str) {
    println!(
        "{:<15} {:>9.1} MB  {:>9.1} MB  {:>9.1} MB  {:>9.1} MB  {:>9.1} MB",
        metric, stats.min, stats.p50, stats.p95, stats.max, stats.mean
    );
}

pub async fn run_benchmark(
    client: &aws_sdk_lambda::Client,
    function_name: String,
    memory: Option<i32>,
    concurrent_invocations: usize,
    rounds: usize,
) -> Result<BenchmarkReport> {
    let config = BenchmarkConfig {
        function_name: function_name
            .split(':')
            .last()
            .unwrap_or(&function_name)
            .to_string(),
        memory_size: memory,
        concurrent_invocations: concurrent_invocations as u32,
        rounds: rounds as u32,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let m = MultiProgress::new();

    // Setup progress bar style
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .unwrap();

    // Function configuration progress
    let config_pb = m.add(ProgressBar::new_spinner());
    config_pb.set_style(spinner_style.clone());
    config_pb.set_prefix("[1/3] Setup");
    config_pb.enable_steady_tick(Duration::from_millis(100));

    if let Some(memory_size) = memory {
        config_pb.set_message("Updating function configuration...");
        update_function_memory(client, &function_name, memory_size).await?;

        config_pb.set_message("Waiting for function update to complete...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        config_pb.finish_with_message("✓ Function configuration updated");
    } else {
        config_pb.finish_with_message("✓ Using existing function configuration");
    }

    // Cold starts progress
    let cold_pb = m.add(ProgressBar::new_spinner());
    cold_pb.set_style(spinner_style.clone());
    cold_pb.set_prefix("[2/3] Cold Starts");
    cold_pb.enable_steady_tick(Duration::from_millis(100));

    let mut cold_starts = Vec::new();
    cold_pb.set_message(format!(
        "Running {} parallel invocations...",
        concurrent_invocations
    ));

    let results = stream::iter(1..=concurrent_invocations)
        .map(|i| {
            let client = client.clone();
            let function_name = function_name.clone();
            tokio::spawn(async move {
                info!("Starting invocation {i}/{concurrent_invocations}");
                invoke_function(&client, &function_name, None).await
            })
        })
        .buffer_unordered(concurrent_invocations)
        .collect::<Vec<_>>()
        .await;

    cold_starts.extend(
        results
            .into_iter()
            .filter_map(|r| r.ok().and_then(|r| r.ok())),
    );
    cold_pb.finish_with_message(format!(
        "✓ Completed {} cold start invocations",
        cold_starts.len()
    ));

    // Warm starts progress
    let warm_pb = m.add(ProgressBar::new_spinner());
    warm_pb.set_style(spinner_style);
    warm_pb.set_prefix("[3/3] Warm Starts");
    warm_pb.enable_steady_tick(Duration::from_millis(100));

    let mut warm_starts = Vec::new();
    for round in 1..=rounds {
        warm_pb.set_message(format!(
            "Round {}/{} with {} parallel invocations...",
            round, rounds, concurrent_invocations
        ));

        let results = stream::iter(1..=concurrent_invocations)
            .map(|i| {
                let client = client.clone();
                let function_name = function_name.clone();
                tokio::spawn(async move {
                    info!("Starting invocation {i}/{concurrent_invocations}");
                    invoke_function(&client, &function_name, None).await
                })
            })
            .buffer_unordered(concurrent_invocations)
            .collect::<Vec<_>>()
            .await;

        warm_starts.extend(
            results
                .into_iter()
                .filter_map(|r| r.ok().and_then(|r| r.ok())),
        );
    }
    warm_pb.finish_with_message(format!(
        "✓ Completed {} warm start invocations",
        warm_starts.len()
    ));

    // Wait for all progress bars to finish
    m.clear().unwrap();

    let report = BenchmarkReport {
        config,
        cold_starts,
        warm_starts,
    };

    // Print results
    if !report.cold_starts.is_empty() {
        println!(
            "\n🥶 Cold Start Metrics ({} invocations) | Memory Size: {} MB",
            report.cold_starts.len(),
            report.cold_starts[0].memory_size
        );
        println!(
            "\n{:<15} {:>12} {:>12} {:>12} {:>12} {:>12}",
            "Metric", "Min", "P50", "P95", "Max", "Mean"
        );
        println!("{:-<87}", "");

        let init_durations: Vec<f64> = report
            .cold_starts
            .iter()
            .filter_map(|m| m.init_duration)
            .collect();
        if !init_durations.is_empty() {
            print_stats(&calculate_stats(&init_durations), "Init Duration");
        }

        let durations: Vec<f64> = report.cold_starts.iter().map(|m| m.duration).collect();
        print_stats(&calculate_stats(&durations), "Duration");

        let billed_durations: Vec<f64> = report
            .cold_starts
            .iter()
            .map(|m| m.billed_duration as f64)
            .collect();
        print_stats(&calculate_stats(&billed_durations), "Billed Duration");

        let memory_used: Vec<f64> = report
            .cold_starts
            .iter()
            .map(|m| m.max_memory_used as f64)
            .collect();
        print_memory_stats(&calculate_stats(&memory_used), "Memory Used");
    }

    if !report.warm_starts.is_empty() {
        println!(
            "\n🔥️ Warm Start Metrics ({} invocations) | Memory Size: {} MB",
            report.warm_starts.len(),
            report.warm_starts[0].memory_size
        );
        println!(
            "\n{:<15} {:>12} {:>12} {:>12} {:>12} {:>12}",
            "Metric", "Min", "P50", "P95", "Max", "Mean"
        );
        println!("{:-<87}", "");

        let durations: Vec<f64> = report.warm_starts.iter().map(|m| m.duration).collect();
        let billed_durations: Vec<f64> = report
            .warm_starts
            .iter()
            .map(|m| m.billed_duration as f64)
            .collect();
        let memory_used: Vec<f64> = report
            .warm_starts
            .iter()
            .map(|m| m.max_memory_used as f64)
            .collect();

        if !durations.is_empty() {
            print_stats(&calculate_stats(&durations), "Duration");
            print_stats(&calculate_stats(&billed_durations), "Billed Duration");
            print_memory_stats(&calculate_stats(&memory_used), "Memory Used");
        }
    }

    Ok(report)
}
