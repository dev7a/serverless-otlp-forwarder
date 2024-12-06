use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub function_name: String,
    pub memory_size: Option<i32>,
    pub concurrent_invocations: u32,
    pub rounds: u32,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvocationMetrics {
    pub duration: f64,
    pub billed_duration: i64,
    pub max_memory_used: i64,
    pub memory_size: i64,
    pub init_duration: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub config: BenchmarkConfig,
    pub cold_starts: Vec<InvocationMetrics>,
    pub warm_starts: Vec<InvocationMetrics>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformReport {
    #[serde(rename = "type")]
    pub report_type: String,
    pub record: ReportRecord,
}

#[derive(Debug, Deserialize)]
pub struct ReportRecord {
    pub metrics: ReportMetrics,
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
