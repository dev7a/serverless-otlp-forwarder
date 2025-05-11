use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

/// Environment variable key-value pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

impl std::str::FromStr for EnvVar {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, '=').collect();
        match parts.as_slice() {
            [key, value] => {
                // Validate key format
                if !key.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                    || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    anyhow::bail!("Invalid environment variable name: {}. Must start with a letter and contain only letters, numbers, and underscores", key);
                }

                Ok(EnvVar {
                    key: key.to_string(),
                    value: value.to_string(),
                })
            }
            _ => anyhow::bail!("Invalid environment variable format. Must be KEY=VALUE"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub function_name: String,
    pub memory_size: Option<i32>,
    pub concurrent_invocations: u32,
    pub rounds: u32,
    pub timestamp: String,
    pub runtime: Option<String>,
    pub architecture: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment: Vec<EnvVar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartMetrics {
    pub timestamp: String,
    pub init_duration: f64,
    pub duration: f64,
    pub extension_overhead: f64,
    pub total_cold_start_duration: Option<f64>,
    pub billed_duration: i64,
    pub max_memory_used: i64,
    pub memory_size: i64,
    pub response_latency_ms: Option<f64>,
    pub response_duration_ms: Option<f64>,
    pub runtime_overhead_ms: Option<f64>,
    pub produced_bytes: Option<i64>,
    pub runtime_done_metrics_duration_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmStartMetrics {
    pub timestamp: String,
    pub duration: f64,
    pub extension_overhead: f64,
    pub billed_duration: i64,
    pub max_memory_used: i64,
    pub memory_size: i64,
    pub response_latency_ms: Option<f64>,
    pub response_duration_ms: Option<f64>,
    pub runtime_overhead_ms: Option<f64>,
    pub produced_bytes: Option<i64>,
    pub runtime_done_metrics_duration_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMetrics {
    pub timestamp: String,
    pub client_duration: f64,
    pub memory_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvocationMetrics {
    pub timestamp: String,
    pub client_duration: f64,
    pub duration: f64,
    pub extension_overhead: f64,
    pub billed_duration: i64,
    pub max_memory_used: i64,
    pub memory_size: i64,
    pub init_duration: Option<f64>,
    pub total_cold_start_duration: Option<f64>,

    // New fields for platform.runtimeDone metrics
    pub response_latency_ms: Option<f64>,
    pub response_duration_ms: Option<f64>,
    pub runtime_overhead_ms: Option<f64>,
    pub produced_bytes: Option<i64>,
    pub runtime_done_metrics_duration_ms: Option<f64>,
}

impl InvocationMetrics {
    pub fn to_cold_start(&self) -> Option<ColdStartMetrics> {
        self.init_duration.map(|init| ColdStartMetrics {
            timestamp: self.timestamp.clone(),
            init_duration: init,
            duration: self.duration,
            extension_overhead: self.extension_overhead,
            total_cold_start_duration: self.total_cold_start_duration,
            billed_duration: self.billed_duration,
            max_memory_used: self.max_memory_used,
            memory_size: self.memory_size,
            response_latency_ms: self.response_latency_ms,
            response_duration_ms: self.response_duration_ms,
            runtime_overhead_ms: self.runtime_overhead_ms,
            produced_bytes: self.produced_bytes,
            runtime_done_metrics_duration_ms: self.runtime_done_metrics_duration_ms,
        })
    }

    pub fn to_warm_start(&self) -> WarmStartMetrics {
        WarmStartMetrics {
            timestamp: self.timestamp.clone(),
            duration: self.duration,
            extension_overhead: self.extension_overhead,
            billed_duration: self.billed_duration,
            max_memory_used: self.max_memory_used,
            memory_size: self.memory_size,
            response_latency_ms: self.response_latency_ms,
            response_duration_ms: self.response_duration_ms,
            runtime_overhead_ms: self.runtime_overhead_ms,
            produced_bytes: self.produced_bytes,
            runtime_done_metrics_duration_ms: self.runtime_done_metrics_duration_ms,
        }
    }

    pub fn to_client_metrics(&self) -> ClientMetrics {
        ClientMetrics {
            timestamp: self.timestamp.clone(),
            client_duration: self.client_duration,
            memory_size: self.memory_size,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub config: BenchmarkConfig,
    pub cold_starts: Vec<ColdStartMetrics>,
    pub warm_starts: Vec<WarmStartMetrics>,
    pub client_measurements: Vec<ClientMetrics>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformReport {
    pub time: String,
    #[serde(rename = "type")]
    pub report_type: String,
    pub record: ReportRecord,
}

#[derive(Debug, Deserialize)]
pub struct ReportRecord {
    pub metrics: ReportMetrics,
    #[serde(default)]
    pub spans: Vec<Span>,
}

#[derive(Debug, Deserialize)]
pub struct ReportMetrics {
    #[serde(rename = "durationMs")]
    pub duration_ms: f64,
    #[serde(rename = "billedDurationMs")]
    pub billed_duration_ms: i64,
    #[serde(rename = "memorySizeMB")]
    pub memory_size_mb: i64,
    #[serde(rename = "maxMemoryUsedMB")]
    pub max_memory_used_mb: i64,
    #[serde(rename = "initDurationMs")]
    pub init_duration_ms: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Span {
    pub name: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: f64,
}

/// Configuration for stack benchmarking
#[derive(Debug, Clone)]
pub struct StackBenchmarkConfig {
    pub stack_name: String,
    pub select_pattern: String,       // Value from --select (required)
    pub select_regex: Option<String>, // Value from --select-regex (optional)
    pub memory_size: Option<i32>,
    pub concurrent_invocations: usize,
    pub rounds: usize,
    pub output_dir: Option<String>, // Path like "base_dir/group_name" or "group_name"
    pub payload: Option<String>,
    pub environment: Vec<EnvVar>,
    pub client_metrics_mode: bool,
    pub proxy_function: Option<String>,
}

/// Original function configuration to restore after testing
#[derive(Debug, Clone)]
pub struct OriginalConfig {
    pub memory_size: i32,
    pub environment: HashMap<String, String>,
}

/// Request payload for the proxy function
#[derive(Debug, Serialize)]
pub struct ProxyRequest {
    /// Target Lambda function to invoke
    pub target: String,
    /// Payload to send to the target function
    pub payload: serde_json::Value,
}

/// Response from the proxy function
#[derive(Debug, Deserialize)]
pub struct ProxyResponse {
    /// Time taken for the invocation in milliseconds
    pub invocation_time_ms: f64,
    /// Response from the target function
    pub response: serde_json::Value,
}

// Added structs for platform.runtimeDone
#[derive(Deserialize, Debug, Clone)]
pub struct PlatformRuntimeDoneReport {
    pub time: String,
    #[serde(rename = "type")]
    pub event_type: String, // Should be "platform.runtimeDone"
    pub record: RuntimeDoneRecord,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RuntimeDoneRecord {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub status: String,
    // pub tracing: serde_json::Value, // Omitted for now
    pub spans: Vec<RuntimeDoneSpan>,
    pub metrics: RuntimeDoneRecordMetrics,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RuntimeDoneSpan {
    pub name: String,
    // pub start: String, // Omitted for now
    #[serde(rename = "durationMs")]
    pub duration_ms: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RuntimeDoneRecordMetrics {
    #[serde(rename = "durationMs")]
    pub duration_ms: f64,
    #[serde(rename = "producedBytes")]
    pub produced_bytes: i64,
}
