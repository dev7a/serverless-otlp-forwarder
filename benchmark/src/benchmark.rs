use anyhow::{anyhow, Result};
use aws_sdk_cloudformation::Client as CloudFormationClient;
use aws_sdk_lambda::primitives::Blob;
use aws_sdk_lambda::{error::ProvideErrorMetadata, Client as LambdaClient};
use base64::{engine::general_purpose, Engine};
use chrono::Local;
use futures::stream::{self, StreamExt, TryStreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use opentelemetry::trace::SpanKind;
use opentelemetry_http::HeaderInjector;
use reqwest::header::HeaderMap;
use serde_json::Value;
use statrs::statistics::{Data, Distribution, Max, Min, OrderStatistics};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

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

#[tracing::instrument(
    skip_all,
    fields(
        otel.name = %format!("invoke {}", function_name),
        otel.kind = ?SpanKind::Client,
    ),
)]
pub async fn invoke_function(
    client: &LambdaClient,
    function_name: &str,
    memory_size: Option<i32>,
    payload: Option<&str>,
    _environment: &[(String, String)],
    client_metrics_mode: bool,
    proxy_function: Option<&str>,
) -> Result<InvocationMetrics> {
    let span = Span::current();

    // Set initial span attributes
    span.set_attribute("function.name", function_name.to_string());
    span.set_attribute("function.memory_size", memory_size.unwrap_or(128) as i64);
    if let Some(proxy) = proxy_function {
        span.set_attribute("function.proxy", proxy.to_string());
    }
    if let Some(payload) = payload {
        span.set_attribute("function.payload", payload.to_string());
    }
    let mut req = client.invoke();

    // Only request logs if not skipping
    if !client_metrics_mode {
        req = req.log_type(aws_sdk_lambda::types::LogType::Tail);
    }

    // Inject trace context into payload
    let mut final_payload = if let Some(p) = payload {
        // If payload exists, parse it
        serde_json::from_str(p)?
    } else {
        // If no payload, create empty object
        serde_json::Value::Object(serde_json::Map::new())
    };

    // Create a map for trace headers
    let mut trace_headers = HeaderMap::new();
    let mut injector = HeaderInjector(&mut trace_headers);
    let cx = span.context();
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut injector);
    });

    let mut otel_context = serde_json::Map::new();
    let mut has_trace_context = false;

    // Extract headers into context map
    for (header_name, header_types) in [
        ("traceparent", true),
        ("tracestate", false),
        ("X-Amzn-Trace-Id", true),
    ] {
        if let Some(header_value) = trace_headers.get(header_name) {
            if header_types {
                // If this header type indicates trace context presence
                has_trace_context = true;
            }

            if let Ok(value_str) = header_value.to_str() {
                otel_context.insert(
                    header_name.to_string(),
                    Value::String(value_str.to_string()),
                );
            }
        }
    }

    // Add to payload if we have trace context
    if has_trace_context && !otel_context.is_empty() {
        if let Value::Object(ref mut payload_map) = final_payload {
            // Use entry API to simplify
            let headers = payload_map
                .entry("headers")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));

            if let Value::Object(ref mut headers_map) = headers {
                headers_map.extend(otel_context);
            }
        }
    }

    // Start client-side timing only if we're measuring client metrics
    let start = if client_metrics_mode {
        Some(std::time::Instant::now())
    } else {
        None
    };

    // Get X-Ray header as a string if it exists
    let xray_header_value = trace_headers
        .get("X-Amzn-Trace-Id")
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string());

    let result = if client_metrics_mode && proxy_function.is_some() {
        // When doing client measurements and proxy is available, use it
        let proxy = proxy_function.unwrap();
        let proxy_request = ProxyRequest {
            target: function_name.to_string(),
            payload: final_payload,
        };

        let req_builder = req
            .function_name(proxy)
            .payload(Blob::new(serde_json::to_vec(&proxy_request)?));

        // Add X-Ray header to the HTTP request if available
        if let Some(header_value) = xray_header_value.clone() {
            req_builder
                .customize()
                .mutate_request(move |http_req| {
                    http_req
                        .headers_mut()
                        .insert("X-Amzn-Trace-Id", header_value.clone());
                })
                .send()
                .await
        } else {
            req_builder.send().await
        }
    } else {
        // Direct invocation for:
        // 1. Server metrics (skip_logs = false)
        // 2. Client measurements without proxy
        let req_builder = req
            .function_name(function_name)
            .payload(Blob::new(final_payload.to_string()));

        // Add X-Ray header to the HTTP request if available
        if let Some(header_value) = xray_header_value {
            req_builder
                .customize()
                .mutate_request(move |http_req| {
                    http_req
                        .headers_mut()
                        .insert("X-Amzn-Trace-Id", header_value.clone());
                })
                .send()
                .await
        } else {
            req_builder.send().await
        }
    };

    match result {
        Ok(output) => {
            // Calculate client-side duration if we're measuring it
            let client_duration = start
                .map(|s| s.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(0.0);

            // Record duration in span
            span.set_attribute("client.duration_ms", client_duration);

            if client_metrics_mode {
                // When skipping logs, just return client duration with current timestamp
                Ok(InvocationMetrics {
                    timestamp: chrono::Utc::now()
                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                        .to_string(),
                    client_duration: if proxy_function.is_some() {
                        let proxy_response: ProxyResponse = serde_json::from_slice(
                            output
                                .payload()
                                .ok_or_else(|| anyhow!("No response from proxy function"))?
                                .as_ref(),
                        )?;
                        proxy_response.invocation_time_ms
                    } else {
                        client_duration
                    },
                    init_duration: None,
                    duration: 0.0,
                    net_duration: 0.0,
                    billed_duration: 0,
                    memory_size: memory_size.unwrap_or(128) as i64,
                    max_memory_used: 0,
                })
            } else {
                // Extract and decode base64 logs
                let logs = output
                    .log_result()
                    .ok_or_else(|| anyhow!("No logs returned"))?;
                let decoded_logs = String::from_utf8(
                    general_purpose::STANDARD
                        .decode(logs)
                        .expect("Failed to decode base64 payload"),
                )
                .expect("Failed to decode logs");
                // Check for function error and include decoded logs in error message
                if let Some(func_error) = output.function_error() {
                    span.set_attribute("error", true);
                    span.set_attribute("error.type", func_error.to_string());
                    return Err(anyhow!(
                        "Function invocation failed: {}.\nLogs:\n{}",
                        func_error,
                        decoded_logs
                    ));
                }

                // Get server-side metrics
                let mut metrics = extract_metrics(&decoded_logs)?;

                // Add metrics to span
                span.set_attribute("function.duration_ms", metrics.duration);
                span.set_attribute("function.billed_duration_ms", metrics.billed_duration);
                span.set_attribute("function.net_duration_ms", metrics.net_duration);
                if let Some(init) = metrics.init_duration {
                    span.set_attribute("function.init_duration_ms", init);
                }

                // Add client-side duration if we measured it
                metrics.client_duration = client_duration;
                Ok(metrics)
            }
        }
        Err(err) => {
            span.set_attribute("error", true);
            let error_details = match err {
                aws_sdk_lambda::error::SdkError::ServiceError(context) => {
                    let msg = format!(
                        "Service error: {} ({})",
                        context.err().message().unwrap_or_default(),
                        context.err().code().unwrap_or_default()
                    );
                    span.set_attribute("error.type", "service_error");
                    span.set_attribute("error.message", msg.clone());
                    msg
                }
                other_err => {
                    let msg = format!("SDK error: {}", other_err);
                    span.set_attribute("error.type", "sdk_error");
                    span.set_attribute("error.message", msg.clone());
                    msg
                }
            };

            Err(anyhow!("Failed to invoke function: {}", error_details))
        }
    }
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
        .ok_or_else(|| anyhow!("No platform.report found in logs"))?;

    // Find extension overhead if present
    let extension_overhead = report
        .record
        .spans
        .iter()
        .find(|span| span.name == "extensionOverhead")
        .map_or(0.0, |span| span.duration_ms);

    // Calculate net duration by subtracting extension overhead
    let duration = report.record.metrics.duration_ms;
    let net_duration = duration - extension_overhead;

    Ok(InvocationMetrics {
        timestamp: report.time.clone(),
        client_duration: 0.0, // This will be set by invoke_function
        init_duration: report.record.metrics.init_duration_ms,
        duration,
        net_duration,
        billed_duration: report.record.metrics.billed_duration_ms,
        memory_size: report.record.metrics.memory_size_mb,
        max_memory_used: report.record.metrics.max_memory_used_mb,
    })
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
) -> Result<BenchmarkResults> {
    use tokio::signal;

    let mut results = BenchmarkResults {
        cold_starts: Vec::new(),
        warm_starts: Vec::new(),
        client_measurements: Vec::new(),
    };

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
            invoke_function(
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
            let metrics = handle.await??;
            results.cold_starts.push(metrics);
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
        return Ok(results);
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
                invoke_function(
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
                let metrics = handle.await??;
                results.warm_starts.push(metrics);
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
            return Ok(results);
        }

        if let Some(pb) = &progress {
            pb.inc(1);
        }
    }

    // Finish progress bar
    if let Some(pb) = progress {
        pb.finish_and_clear();
    }

    Ok(results)
}

/// Get the original function configuration
async fn get_function_config(client: &LambdaClient, function_name: &str) -> Result<OriginalConfig> {
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

    Ok(OriginalConfig {
        memory_size: config.memory_size().unwrap_or(128) as i32,
        environment: config
            .environment()
            .and_then(|e| e.variables())
            .map(|vars| vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default(),
    })
}

/// Update function configuration
async fn update_function_config(
    client: &LambdaClient,
    function_name: &str,
    memory_size: Option<i32>,
    environment: &[(String, String)],
) -> Result<()> {
    // Get current configuration
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

    // Get current configuration
    let current_config = function.configuration().ok_or_else(|| {
        anyhow!(
            "Failed to get function configuration for '{}'",
            function_name
        )
    })?;

    // Build update configuration
    let mut update = client
        .update_function_configuration()
        .function_name(function_name);

    // Add memory if specified
    if let Some(memory) = memory_size {
        update = update.memory_size(memory);
    }

    // Prepare environment variables
    let mut env_vars = HashMap::new();

    // First, copy existing environment variables
    if let Some(current_env) = current_config.environment().and_then(|e| e.variables()) {
        env_vars.extend(current_env.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    // Then add/update new variables
    for (key, value) in environment {
        env_vars.insert(key.clone(), value.clone());
    }

    // Only set environment if we have variables
    if !env_vars.is_empty() {
        update = update.environment(
            aws_sdk_lambda::types::Environment::builder()
                .set_variables(Some(env_vars))
                .build(),
        );
    }

    // Send update
    match update.send().await {
        Ok(_) => {
            // Wait for function update to complete
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok(())
        }
        Err(err) => {
            let error_details = match err {
                aws_sdk_lambda::error::SdkError::ServiceError(context) => format!(
                    "Service error: {} ({})",
                    context.err().message().unwrap_or_default(),
                    context.err().code().unwrap_or_default()
                ),
                other_err => format!("SDK error: {}", other_err),
            };

            Err(anyhow!(
                "Failed to update function configuration: {}",
                error_details
            ))
        }
    }
}

/// Restore original function configuration
async fn restore_function_config(
    client: &LambdaClient,
    function_name: &str,
    original: &OriginalConfig,
) -> Result<()> {
    println!("\nRestoring function configuration...");
    update_function_config(
        client,
        function_name,
        Some(original.memory_size),
        &original
            .environment
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
    )
    .await?;
    println!("✓ Function configuration restored");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run_function_benchmark(
    client: &LambdaClient,
    function_name: &str,
    memory_size: Option<i32>,
    concurrent: u32,
    rounds: u32,
    payload: Option<&str>,
    output_dir: &str,
    environment: &[(&str, &str)],
    client_metrics_mode: bool,
    proxy_function: Option<&str>,
) -> Result<()> {
    println!("\nStarting benchmark for: {}", function_name);

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

    println!("Configuration:");
    println!("  Memory: {} MB", memory_size.unwrap_or(128));
    println!("  Runtime: {}", runtime.as_deref().unwrap_or("unknown"));
    println!(
        "  Architecture: {}",
        architecture.as_deref().unwrap_or("unknown")
    );
    println!("  Concurrency: {}", concurrent);
    println!("  Rounds: {}", rounds);
    if let Some(proxy) = proxy_function {
        println!("  Using Proxy Function: {}", proxy);
    }
    if !environment.is_empty() {
        println!("  Environment:");
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
        println!("  Service: {}", service);
        println!("  Endpoint: {}", endpoint);
        println!(
            "  Protocol: {}",
            std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
                .unwrap_or_else(|_| "http/protobuf (default)".to_string())
        );

        // Region is required for AWS endpoints
        if endpoint.contains(".amazonaws.com") {
            let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            println!(
                "  Region: {}{}",
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
        Some(get_function_config(client, function_name).await?)
    } else {
        None
    };

    // Update function configuration if needed
    if memory_size.is_some() || !environment.is_empty() {
        println!("\nUpdating function configuration...");
        update_function_config(client, function_name, memory_size, &env).await?;
        println!("✓ Function configuration updated");
    }

    // Create a future for the benchmark execution
    let config = FunctionBenchmarkConfig::new(
        function_name.to_string(),
        memory_size,
        concurrent,
        rounds,
        payload.map(|s| s.to_string()),
        output_dir.to_string(),
        env,
        proxy_function.map(|s| s.to_string()),
    );

    let result = async {
        // First pass - get server metrics and cold start
        println!("\nCollecting server metrics...");
        let mut results = run_benchmark_pass(client, &config, false).await?;
        println!("✓ Server metrics collected");

        // If client metrics requested, do a second pass for warm starts only
        if client_metrics_mode {
            println!("\nCollecting client metrics...");
            // Run without logs to get accurate client metrics
            let client_results = run_benchmark_pass(client, &config, true).await?;
            results.client_measurements = client_results.warm_starts;
            println!("✓ Client metrics collected");
        }

        // Print results
        print_benchmark_results(&results);

        // Save results
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
            output_dir,
        )
        .await?;

        Ok(())
    }
    .await;

    // Restore original configuration if we modified it
    if let Some(original) = original_config {
        // Always try to restore, even if the benchmark failed or was interrupted
        if let Err(e) = restore_function_config(client, function_name, &original).await {
            eprintln!("Warning: Failed to restore function configuration: {}", e);
        }
    }

    // Now return the benchmark result
    result
}

// Move the results printing to a separate function
fn print_benchmark_results(results: &BenchmarkResults) {
    if !results.cold_starts.is_empty()
        && results
            .cold_starts
            .iter()
            .any(|m| m.init_duration.is_some())
    {
        println!(
            "\n🥶 Cold Start Metrics ({} invocations) | Memory Size: {} MB",
            results.cold_starts.len(),
            results.cold_starts[0].memory_size
        );
        println!(
            "\n{:<15} {:>12} {:>12} {:>12} {:>12} {:>12}",
            "Metric", "Min", "Max", "Mean", "P50", "P95"
        );
        println!("{:-<87}", "");

        let init_durations: Vec<f64> = results
            .cold_starts
            .iter()
            .filter_map(|m| m.init_duration)
            .collect();
        if !init_durations.is_empty() {
            print_stats(&calculate_stats(&init_durations), "Init Duration");
        }

        let durations: Vec<f64> = results.cold_starts.iter().map(|m| m.duration).collect();
        print_stats(&calculate_stats(&durations), "Server Duration");

        let net_durations: Vec<f64> = results.cold_starts.iter().map(|m| m.net_duration).collect();
        print_stats(&calculate_stats(&net_durations), "Net Duration");

        let billed_durations: Vec<f64> = results
            .cold_starts
            .iter()
            .map(|m| m.billed_duration as f64)
            .collect();
        print_stats(&calculate_stats(&billed_durations), "Billed Duration");

        let memory_used: Vec<f64> = results
            .cold_starts
            .iter()
            .map(|m| m.max_memory_used as f64)
            .collect();
        print_memory_stats(&calculate_stats(&memory_used), "Memory Used");
    }

    if !results.warm_starts.is_empty() {
        println!(
            "\n🔥 Warm Start Metrics ({} invocations) | Memory Size: {} MB",
            results.warm_starts.len(),
            results.warm_starts[0].memory_size
        );
        println!(
            "\n{:<15} {:>12} {:>12} {:>12} {:>12} {:>12}",
            "Metric", "Min", "Max", "Mean", "P50", "P95"
        );
        println!("{:-<87}", "");

        let durations: Vec<f64> = results.warm_starts.iter().map(|m| m.duration).collect();
        print_stats(&calculate_stats(&durations), "Server Duration");

        let net_durations: Vec<f64> = results.warm_starts.iter().map(|m| m.net_duration).collect();
        print_stats(&calculate_stats(&net_durations), "Net Duration");

        let billed_durations: Vec<f64> = results
            .warm_starts
            .iter()
            .map(|m| m.billed_duration as f64)
            .collect();
        print_stats(&calculate_stats(&billed_durations), "Billed Duration");

        let memory_used: Vec<f64> = results
            .warm_starts
            .iter()
            .map(|m| m.max_memory_used as f64)
            .collect();
        print_memory_stats(&calculate_stats(&memory_used), "Memory Used");
    }

    if !results.client_measurements.is_empty() {
        println!(
            "\n⏱️ Client Metrics ({} invocations) | Memory Size: {} MB",
            results.client_measurements.len(),
            results.client_measurements[0].memory_size
        );
        println!(
            "\n{:<15} {:>12} {:>12} {:>12} {:>12} {:>12}",
            "Metric", "Min", "Max", "Mean", "P50", "P95"
        );
        println!("{:-<87}", "");

        let client_durations: Vec<f64> = results
            .client_measurements
            .iter()
            .map(|m| m.client_duration)
            .collect();
        print_stats(&calculate_stats(&client_durations), "Client Duration");
    }
}

pub struct MetricsStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
}

pub fn calculate_stats(values: &[f64]) -> MetricsStats {
    if values.is_empty() {
        return MetricsStats {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            p50: 0.0,
            p95: 0.0,
        };
    }
    // Use the statrs Data structure which properly handles statistics
    let mut data = Data::new(values.to_vec());
    MetricsStats {
        min: data.min(),
        max: data.max(),
        mean: data.mean().unwrap_or(0.0),
        p50: data.median(), // 50th percentile (properly handles even-length arrays)
        p95: data.percentile(95), // 95th percentile with proper interpolation
    }
}

fn print_stats(stats: &MetricsStats, metric: &str) {
    println!(
        "{:<15} {:>9.3} ms  {:>9.3} ms  {:>9.3} ms  {:>9.3} ms  {:>9.3} ms",
        metric, stats.min, stats.max, stats.mean, stats.p50, stats.p95
    );
}

fn print_memory_stats(stats: &MetricsStats, metric: &str) {
    println!(
        "{:<15} {:>9.1} MB  {:>9.1} MB  {:>9.1} MB  {:>9.1} MB  {:>9.1} MB",
        metric, stats.min, stats.max, stats.mean, stats.p50, stats.p95
    );
}

/// Extract function name from ARN
fn extract_function_name(arn: &str) -> &str {
    arn.split(':').last().unwrap_or(arn)
}

pub async fn run_stack_benchmark(
    lambda_client: &LambdaClient,
    cf_client: &CloudFormationClient,
    config: StackBenchmarkConfig,
) -> Result<()> {
    // Get stack outputs
    let response = cf_client
        .describe_stacks()
        .stack_name(&config.stack_name)
        .send()
        .await?;

    let stacks = response
        .stacks
        .ok_or_else(|| anyhow::anyhow!("Stack '{}' not found", config.stack_name))?;

    let outputs = stacks
        .first()
        .ok_or_else(|| anyhow::anyhow!("Stack '{}' not found", config.stack_name))?
        .outputs
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Stack '{}' has no outputs", config.stack_name))?;

    // Filter outputs for Lambda function ARNs and apply pattern matching if select is provided
    let functions: Vec<_> = outputs
        .iter()
        .filter(|output| {
            output
                .output_value()
                .map(|v| {
                    v.contains("arn:aws:lambda:")
                        && config
                            .pattern
                            .as_ref()
                            .map_or(true, |pattern| v.contains(pattern))
                })
                .unwrap_or(false)
        })
        .map(|output| output.output_value().unwrap())
        .collect();

    if functions.is_empty() {
        return Err(anyhow::anyhow!(
            "No Lambda functions found matching the criteria"
        ));
    }

    println!(
        "\nFound {} functions in stack '{}'",
        functions.len(),
        config.stack_name
    );

    if config.parallel {
        stream::iter(functions)
            .map(|function_arn| {
                let client = lambda_client.clone();
                let env = config.environment.clone();
                let function_name = extract_function_name(function_arn).to_string();
                let memory_size = config.memory_size;
                let concurrent = config.concurrent_invocations as u32;
                let rounds = config.rounds as u32;
                let payload = config.payload.as_deref();
                let output_dir = config.output_dir.clone();
                let client_metrics_mode = config.client_metrics_mode;
                let proxy_function = config.proxy_function.as_deref();
                async move {
                    if is_interrupted() {
                        println!("\nInterrupted, skipping function: {}", function_name);
                        return Err(anyhow!("Benchmark interrupted by user"));
                    }
                    run_function_benchmark(
                        &client,
                        &function_name,
                        memory_size,
                        concurrent,
                        rounds,
                        payload,
                        &output_dir,
                        &env.iter()
                            .map(|e| (e.key.as_str(), e.value.as_str()))
                            .collect::<Vec<_>>(),
                        client_metrics_mode,
                        proxy_function,
                    )
                    .await
                }
            })
            .buffer_unordered(4)
            .try_for_each(|_| async { Ok(()) })
            .await?;
    } else {
        for function_arn in functions {
            if is_interrupted() {
                println!("\nInterrupted, skipping remaining functions");
                return Err(anyhow!("Benchmark interrupted by user"));
            }
            let function_name = extract_function_name(function_arn);
            run_function_benchmark(
                lambda_client,
                function_name,
                config.memory_size,
                config.concurrent_invocations as u32,
                config.rounds as u32,
                config.payload.as_deref(),
                &config.output_dir,
                &config
                    .environment
                    .iter()
                    .map(|e| (e.key.as_str(), e.value.as_str()))
                    .collect::<Vec<_>>(),
                config.client_metrics_mode,
                config.proxy_function.as_deref(),
            )
            .await?;
        }
    }

    Ok(())
}
