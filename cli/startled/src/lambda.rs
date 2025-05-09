use crate::types::{
    InvocationMetrics, PlatformReport, PlatformRuntimeDoneReport, ProxyRequest, ProxyResponse,
};
use anyhow::{anyhow, Result};
use aws_sdk_lambda::primitives::Blob;
use aws_sdk_lambda::{error::ProvideErrorMetadata, error::SdkError, Client as LambdaClient};
use base64::Engine;
use opentelemetry::trace::SpanKind;
use opentelemetry_http::HeaderInjector;
use reqwest::header::HeaderMap;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

// Helper types needed for metrics extraction and config
#[derive(Clone)]
pub struct OriginalConfig {
    pub memory_size: i32,
    pub environment: Vec<(String, String)>,
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
        serde_json::from_str(p)?
    } else {
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

    for (header_name, header_types) in [
        ("traceparent", true),
        ("tracestate", false),
        ("X-Amzn-Trace-Id", true),
    ] {
        if let Some(header_value) = trace_headers.get(header_name) {
            if header_types {
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

    if has_trace_context && !otel_context.is_empty() {
        if let Value::Object(ref mut payload_map) = final_payload {
            let headers = payload_map
                .entry("headers")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Value::Object(ref mut headers_map) = headers {
                headers_map.extend(otel_context);
            }
        }
    }

    let start = if client_metrics_mode {
        Some(std::time::Instant::now())
    } else {
        None
    };

    let xray_header_value = trace_headers
        .get("X-Amzn-Trace-Id")
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string());

    let result = if client_metrics_mode && proxy_function.is_some() {
        let proxy = proxy_function.unwrap();
        let proxy_request = ProxyRequest {
            target: function_name.to_string(),
            payload: final_payload,
        };
        let req_builder = req
            .function_name(proxy)
            .payload(Blob::new(serde_json::to_vec(&proxy_request)?));
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
        let req_builder = req
            .function_name(function_name)
            .payload(Blob::new(final_payload.to_string()));
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
            let client_duration = start
                .map(|s| s.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(0.0);
            span.set_attribute("function.client.duration_ms", client_duration);
            if client_metrics_mode {
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
                    extension_overhead: 0.0,
                    total_cold_start_duration: None,
                    billed_duration: 0,
                    memory_size: memory_size.unwrap_or(128) as i64,
                    max_memory_used: 0,
                    response_latency_ms: None,
                    response_duration_ms: None,
                    runtime_overhead_ms: None,
                    produced_bytes: None,
                    runtime_done_metrics_duration_ms: None,
                })
            } else {
                let logs = output
                    .log_result()
                    .ok_or_else(|| anyhow!("No logs returned"))?;
                let decoded_logs = String::from_utf8(
                    base64::engine::general_purpose::STANDARD
                        .decode(logs)
                        .expect("Failed to decode base64 payload"),
                )
                .expect("Failed to decode logs");
                if let Some(func_error) = output.function_error() {
                    span.set_attribute("error", true);
                    span.set_attribute("error.type", func_error.to_string());
                    return Err(anyhow!(
                        "Function invocation failed: {}.\nLogs:\n{}",
                        func_error,
                        decoded_logs
                    ));
                }
                let mut metrics = extract_metrics(&decoded_logs)?;
                span.set_attribute("function.duration_ms", metrics.duration);
                span.set_attribute("function.billed_duration_ms", metrics.billed_duration);
                span.set_attribute("function.extension_overhead_ms", metrics.extension_overhead);
                if let Some(init) = metrics.init_duration {
                    span.set_attribute("function.init_duration_ms", init);
                }
                if let Some(total) = metrics.total_cold_start_duration {
                    span.set_attribute("function.total_cold_start_duration_ms", total);
                }
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
    let mut platform_report_data: Option<PlatformReport> = None;
    let mut runtime_done_report_data: Option<PlatformRuntimeDoneReport> = None;

    // Iterate lines in reverse to find the last occurrence of each report type
    for line in logs.lines().rev() {
        // Try to parse as PlatformReport
        if platform_report_data.is_none() {
            if let Ok(report) = serde_json::from_str::<PlatformReport>(line) {
                if report.report_type == "platform.report" {
                    platform_report_data = Some(report);
                }
            }
        }

        // Try to parse as PlatformRuntimeDoneReport
        if runtime_done_report_data.is_none() {
            if let Ok(report) = serde_json::from_str::<PlatformRuntimeDoneReport>(line) {
                // Ensure it's the correct type, field name in struct is event_type
                if report.event_type == "platform.runtimeDone" {
                    runtime_done_report_data = Some(report);
                }
            }
        }

        // Optimization: if both are found, no need to parse further lines
        if platform_report_data.is_some() && runtime_done_report_data.is_some() {
            break;
        }
    }

    let report = platform_report_data.ok_or_else(|| anyhow!("No platform.report found in logs"))?;

    // Metrics from platform.report (existing logic)
    let extension_overhead = report
        .record
        .spans
        .iter()
        .find(|span| span.name == "extensionOverhead")
        .map_or(0.0, |span| span.duration_ms);
    let duration = report.record.metrics.duration_ms;
    let init_duration = report.record.metrics.init_duration_ms;
    let total_cold_start_duration = init_duration.map(|init| init + duration);

    // Initialize new metrics fields as None
    let mut response_latency_ms: Option<f64> = None;
    let mut response_duration_ms: Option<f64> = None;
    let mut runtime_overhead_ms: Option<f64> = None; // Simplified variable name
    let mut produced_bytes: Option<i64> = None;
    let mut runtime_done_metrics_duration_ms: Option<f64> = None;

    // Extract metrics from platform.runtimeDone if found
    if let Some(rd_report) = runtime_done_report_data {
        for span in rd_report.record.spans {
            match span.name.as_str() {
                "responseLatency" => response_latency_ms = Some(span.duration_ms),
                "responseDuration" => response_duration_ms = Some(span.duration_ms),
                "runtimeOverhead" => runtime_overhead_ms = Some(span.duration_ms), // Use simplified name
                _ => {} // Ignore other spans
            }
        }
        produced_bytes = Some(rd_report.record.metrics.produced_bytes);
        runtime_done_metrics_duration_ms = Some(rd_report.record.metrics.duration_ms);
    }

    Ok(InvocationMetrics {
        timestamp: report.time.clone(),
        client_duration: 0.0, // This is typically set outside this function for the non-client_metrics_mode path
        init_duration,
        duration, // Function execution duration from platform.report
        extension_overhead,
        total_cold_start_duration,
        billed_duration: report.record.metrics.billed_duration_ms,
        memory_size: report.record.metrics.memory_size_mb,
        max_memory_used: report.record.metrics.max_memory_used_mb,

        // Populate new fields
        response_latency_ms,
        response_duration_ms,
        runtime_overhead_ms, // Assign from simplified local variable
        produced_bytes,
        runtime_done_metrics_duration_ms,
    })
}

pub async fn get_function_config(
    client: &LambdaClient,
    function_name: &str,
) -> Result<OriginalConfig> {
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

pub async fn update_function_config(
    client: &LambdaClient,
    function_name: &str,
    memory_size: Option<i32>,
    environment: &[(String, String)],
) -> Result<()> {
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
    let current_config = function.configuration().ok_or_else(|| {
        anyhow!(
            "Failed to get function configuration for '{}'",
            function_name
        )
    })?;
    let mut update = client
        .update_function_configuration()
        .function_name(function_name);
    if let Some(memory) = memory_size {
        update = update.memory_size(memory);
    }
    // Get the current environment variables
    let mut env_vars = HashMap::new();
    if let Some(current_env) = current_config.environment().and_then(|e| e.variables()) {
        env_vars.extend(current_env.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    // Add the new environment variables
    for (key, value) in environment {
        env_vars.insert(key.clone(), value.clone());
    }
    if !env_vars.is_empty() {
        update = update.environment(
            aws_sdk_lambda::types::Environment::builder()
                .set_variables(Some(env_vars))
                .build(),
        );
    }
    // Set log format to JSON and system log level to DEBUG
    update = update.logging_config(
        aws_sdk_lambda::types::LoggingConfig::builder()
            .system_log_level(aws_sdk_lambda::types::SystemLogLevel::Debug)
            .log_format(aws_sdk_lambda::types::LogFormat::Json)
            .build(),
    );

    match update.send().await {
        Ok(_) => {
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

pub async fn restore_function_config(
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

pub async fn check_function_exists(client: &LambdaClient, function_name: &str) -> Result<()> {
    use aws_sdk_lambda::operation::get_function::GetFunctionError;
    match client
        .get_function()
        .function_name(function_name)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(SdkError::ServiceError(service_err)) => {
            let inner_err = service_err.into_err();
            if matches!(&inner_err, GetFunctionError::ResourceNotFoundException(_)) {
                Err(anyhow!("Lambda function '{}' not found.", function_name))
            } else {
                Err(anyhow!(
                    "AWS service error checking function '{}': {:?}",
                    function_name,
                    inner_err
                ))
            }
        }
        Err(other_err) => Err(anyhow!(
            "Error checking Lambda function '{}': {}",
            function_name,
            other_err
        )),
    }
}
