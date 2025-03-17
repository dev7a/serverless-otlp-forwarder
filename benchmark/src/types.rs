use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_json;
use std::fs;
use anyhow::Context;

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
                if !key.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) 
                    || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ColdStartMetrics {
    pub timestamp: String,
    pub init_duration: f64,
    pub duration: f64,
    pub net_duration: f64,
    pub billed_duration: i64,
    pub max_memory_used: i64,
    pub memory_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WarmStartMetrics {
    pub timestamp: String,
    pub duration: f64,
    pub net_duration: f64,
    pub billed_duration: i64,
    pub max_memory_used: i64,
    pub memory_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
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
    pub net_duration: f64,
    pub billed_duration: i64,
    pub max_memory_used: i64,
    pub memory_size: i64,
    pub init_duration: Option<f64>,
}

impl InvocationMetrics {
    pub fn to_cold_start(&self) -> Option<ColdStartMetrics> {
        self.init_duration.map(|init| ColdStartMetrics {
            timestamp: self.timestamp.clone(),
            init_duration: init,
            duration: self.duration,
            net_duration: self.net_duration,
            billed_duration: self.billed_duration,
            max_memory_used: self.max_memory_used,
            memory_size: self.memory_size,
        })
    }

    pub fn to_warm_start(&self) -> WarmStartMetrics {
        WarmStartMetrics {
            timestamp: self.timestamp.clone(),
            duration: self.duration,
            net_duration: self.net_duration,
            billed_duration: self.billed_duration,
            max_memory_used: self.max_memory_used,
            memory_size: self.memory_size,
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

#[derive(Debug, Serialize, Deserialize)]
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
    pub pattern: Option<String>,
    pub memory_size: Option<i32>,
    pub concurrent_invocations: usize,
    pub rounds: usize,
    pub output_dir: String,
    pub payload: Option<String>,
    pub parallel: bool,
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

/// Global configuration for batch benchmarking.
/// These settings apply to all tests unless overridden at the test level.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    /// Global title for the benchmark suite.
    /// This will be used as the main page title.
    #[serde(default)]
    pub title: Option<String>,

    /// Global description for the entire benchmark suite.
    /// This can be used to provide overall context for all tests.
    #[serde(default)]
    pub description: Option<String>,

    /// Memory sizes (in MB) to test for each function.
    /// Defaults to [128, 512, 1024].
    #[serde(default = "default_memory_sizes")]
    pub memory_sizes: Vec<i32>,

    /// Number of concurrent invocations for each test.
    /// Defaults to 1.
    #[serde(default = "default_concurrent")]
    pub concurrent: u32,

    /// Number of rounds for warm start measurements.
    /// Each round will execute the specified number of concurrent invocations.
    /// Defaults to 1.
    #[serde(default = "default_rounds")]
    pub rounds: u32,

    /// Whether to run function tests in parallel.
    /// When true, multiple functions can be tested simultaneously.
    /// Defaults to false.
    #[serde(default)]
    pub parallel: bool,

    /// Default CloudFormation stack name.
    /// This can be overridden at the test level.
    #[serde(default = "default_stack_name")]
    pub stack_name: String,

    /// Global environment variables that will be merged with test-specific ones.
    /// Test-level environment variables take precedence over global ones.
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Optional proxy function ARN or name to use for all tests
    /// This can be overridden at the test level
    #[serde(default)]
    pub proxy_function: Option<String>,
}

impl GlobalConfig {
    /// Creates a new GlobalConfig with default values.
    /// This is equivalent to using Default::default() but more explicit.
    pub fn new() -> Self {
        Self {
            title: None,
            description: None,
            memory_sizes: default_memory_sizes(),
            concurrent: default_concurrent(),
            rounds: default_rounds(),
            parallel: false,
            stack_name: default_stack_name(),
            environment: HashMap::new(),
            proxy_function: None,
        }
    }
}

fn default_memory_sizes() -> Vec<i32> {
    vec![128, 512, 1024]
}

fn default_concurrent() -> u32 {
    1
}

fn default_rounds() -> u32 {
    1
}

fn default_stack_name() -> String {
    "benchmark".to_string()
}

/// Configuration for a single test in the batch.
/// Each test represents a Lambda function to benchmark with specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTestConfig {
    /// Human-readable title for the test.
    /// This will be used in reports and visualizations.
    pub title: String,

    /// Detailed description of what the test measures.
    /// This helps document the purpose of each benchmark.
    pub description: String,

    /// Unique name for this test variation.
    /// This will be used in the directory structure to differentiate between
    /// multiple runs of the same function with different configurations.
    pub name: String,

    /// Name of the CloudFormation stack containing the Lambda function.
    /// If not specified, the global stack_name will be used.
    #[serde(default)]
    pub stack_name: Option<String>,

    /// Pattern to select the function within the stack.
    /// This should match the function's logical ID or a unique part of its name.
    pub selector: String,

    /// Payload for function invocations.
    /// Can be either a path to a JSON file or an inline JSON object.
    #[serde(default)]
    pub payload: PayloadConfig,

    /// Environment variables to set on the Lambda function during testing.
    /// These will be restored to their original values after testing.
    #[serde(default)]
    pub environment: HashMap<String, String>,

    // Optional overrides of global settings
    /// Override the global memory sizes for this specific test.
    /// If not specified, the global setting will be used.
    pub memory_sizes: Option<Vec<i32>>,

    /// Override the global concurrent invocations for this specific test.
    /// If not specified, the global setting will be used.
    pub concurrent: Option<u32>,

    /// Override the global number of rounds for this specific test.
    /// If not specified, the global setting will be used.
    pub rounds: Option<u32>,

    /// Override the global parallel execution setting for this specific test.
    /// If not specified, the global setting will be used.
    pub parallel: Option<bool>,

    /// Override the global proxy function for this specific test
    /// If not specified, the global setting will be used
    pub proxy_function: Option<String>,
}

impl BatchTestConfig {
    /// Merge environment variables from global config with test-specific ones.
    /// Test-level variables take precedence over global ones.
    pub fn merge_environment(&self, global_env: &HashMap<String, String>) -> HashMap<String, String> {
        let mut merged = global_env.clone();
        // Override/add test-specific environment variables
        merged.extend(self.environment.clone());
        merged
    }

    /// Get the effective proxy function, considering both global and test-level settings
    pub fn get_proxy_function(&self, global: &GlobalConfig) -> Option<String> {
        self.proxy_function.clone().or_else(|| global.proxy_function.clone())
    }
}

/// Configuration for function payload.
/// Can be either a path to a JSON file or an inline JSON object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PayloadConfig {
    /// Path to a JSON file containing the payload
    Path(String),
    /// Inline JSON object to use as payload
    Inline(serde_json::Value),
}

impl Default for PayloadConfig {
    fn default() -> Self {
        PayloadConfig::Inline(serde_json::json!({}))
    }
}

impl PayloadConfig {
    /// Get the payload as a JSON string.
    /// If the payload is a file path, reads and parses the file.
    pub fn to_json_string(&self) -> anyhow::Result<String> {
        match self {
            PayloadConfig::Path(path) => {
                let content = fs::read_to_string(path)
                    .context(format!("Failed to read payload file: {}", path))?;
                // Validate JSON
                serde_json::from_str::<serde_json::Value>(&content)
                    .context("Invalid JSON in payload file")?;
                Ok(content)
            }
            PayloadConfig::Inline(value) => Ok(value.to_string()),
        }
    }
}

/// Complete configuration for batch benchmarking.
/// This is the top-level structure that will be parsed from the YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Global settings that apply to all tests unless overridden.
    #[serde(default)]
    pub global: GlobalConfig,

    /// List of tests to run as part of this batch.
    pub tests: Vec<BatchTestConfig>,
}

/// Metadata about a specific test run.
/// This information is stored alongside the test results and used for reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMetadata {
    /// Human-readable title of the test
    pub title: String,

    /// Detailed description of the test's purpose
    pub description: String,

    /// Name of the CloudFormation stack containing the function
    pub stack_name: String,

    /// Pattern used to select the function
    pub selector: String,

    /// When the test was executed
    pub timestamp: String,

    /// Memory sizes that were tested
    pub memory_sizes: Vec<i32>,

    /// Number of concurrent invocations used
    pub concurrent: u32,

    /// Number of warm start rounds executed
    pub rounds: u32,

    /// Whether parallel execution was enabled
    pub parallel: bool,

    /// Environment variables that were set during testing
    pub environment: HashMap<String, String>,
}

/// Global index of all executed tests.
/// This is stored as index.yaml in the results directory and provides
/// a complete overview of all benchmark runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestIndex {
    /// When the index was last updated
    pub timestamp: String,

    /// Metadata for all tests that have been run
    pub tests: Vec<TestMetadata>,
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
