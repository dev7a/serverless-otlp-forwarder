//! Collector configuration management for the Lambda OTLP forwarder.
//!
//! This module handles:
//! - Loading collector configurations from AWS Secrets Manager
//! - Matching log records to appropriate collectors
//! - Managing collector authentication details
//!
//! Collectors configuration is cached and refreshed periodically.

use anyhow::{Context, Result};
use aws_sdk_secretsmanager::types::Filter;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tracing::instrument;
use url::Url;

/// Global storage for cached collectors configuration
static COLLECTORS: OnceLock<Arc<CollectorsCache>> = OnceLock::new();

/// Represents a single collector configuration.
/// Each collector has a name, endpoint, and optional authentication details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Collector {
    /// Unique name identifying the collector
    pub(crate) name: String,
    /// Base URL endpoint for the collector
    pub(crate) endpoint: String,
    /// Optional authentication string. Special values:
    /// - "sigv4" or "iam": Use AWS SigV4 signing
    /// - "header_name=value": Add a custom header
    /// - null or empty: No authentication
    pub(crate) auth: Option<String>,
}

/// Container for managing multiple collector configurations.
#[derive(Debug)]
pub(crate) struct Collectors {
    items: Vec<Collector>,
}

/// Cache wrapper for Collectors with TTL tracking
#[derive(Debug)]
pub(crate) struct CollectorsCache {
    inner: Collectors,
    last_refresh: Instant,
    ttl: Duration,
}

impl CollectorsCache {
    fn new(collectors: Collectors) -> Self {
        // Get TTL from environment variable or default to 300 seconds
        let ttl_seconds = env::var("COLLECTORS_CACHE_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);

        tracing::debug!("Using collectors cache TTL of {} seconds", ttl_seconds);

        Self {
            inner: collectors,
            last_refresh: Instant::now(),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    fn is_stale(&self) -> bool {
        self.last_refresh.elapsed() >= self.ttl
    }
}

impl Collectors {
    /// Creates a new Collectors instance with the provided items.
    fn new(items: Vec<Collector>) -> Self {
        Self { items }
    }

    /// Check if collectors cache is initialized globally.
    pub(crate) fn is_initialized() -> bool {
        COLLECTORS.get().is_some()
    }

    /// Initialize or refresh collectors cache from AWS Secrets Manager.
    ///
    /// This method will:
    /// 1. Check if the cache needs refreshing
    /// 2. Load collector configurations from Secrets Manager if needed
    /// 3. Update the cache with new configurations
    #[instrument(skip(client))]
    pub(crate) async fn init(client: &SecretsManagerClient) -> Result<()> {
        // If cache exists, check if it's stale
        if let Some(cache) = COLLECTORS.get() {
            if cache.is_stale() {
                tracing::info!("Cache expired, refreshing collectors configuration");
                let items = fetch_collectors(client).await?;
                let new_cache = Arc::new(CollectorsCache::new(Collectors::new(items)));
                // Replace the old cache - Arc handles cleanup of old instance
                let _ = COLLECTORS.set(new_cache);
                tracing::info!("Refreshed collectors configuration");
            }
            return Ok(());
        }

        // Initial cache load
        let items = fetch_collectors(client).await?;
        let cache = Arc::new(CollectorsCache::new(Collectors::new(items)));
        COLLECTORS
            .set(cache)
            .map_err(|_| anyhow::anyhow!("Collectors cache already initialized"))?;

        tracing::info!(
            "Initialized collectors cache with {} collectors",
            COLLECTORS.get().unwrap().inner.items.len()
        );
        Ok(())
    }

    /// Finds a collector matching the given endpoint
    #[instrument(skip_all)]
    pub(crate) fn find_matching(endpoint: &str) -> Option<Collector> {
        let cache = COLLECTORS.get().expect("Collectors cache not initialized");
        cache
            .inner
            .items
            .iter()
            .find(|c| endpoint.starts_with(&c.endpoint))
            .cloned()
    }

    /// Returns all collectors with endpoints configured for the given signal path
    #[instrument(skip_all)]
    pub(crate) fn get_signal_endpoints(original_endpoint: &str) -> Result<Vec<Collector>> {
        let cache = COLLECTORS.get().expect("Collectors cache not initialized");

        cache
            .inner
            .items
            .iter()
            .map(|collector| {
                let endpoint = collector.construct_signal_endpoint(original_endpoint)?;
                Ok(Collector {
                    name: collector.name.clone(),
                    endpoint,
                    auth: collector.auth.clone(),
                })
            })
            .collect()
    }
}

impl Collector {
    /// Constructs the full endpoint URL for this collector by combining its base endpoint
    /// with the signal path from the original request
    fn construct_signal_endpoint(&self, original_endpoint: &str) -> Result<String> {
        // Extract the signal path (e.g., "/v1/traces") from the original endpoint
        let signal_path = Url::parse(original_endpoint)
            .context("Invalid original endpoint URL")?
            .path()
            .to_string();

        let mut base = Url::parse(&self.endpoint)
            .with_context(|| format!("Invalid collector base endpoint: {}", self.endpoint))?;

        // Ensure the base path ends with a slash if it's not empty
        if !base.path().is_empty() && !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }

        // Remove leading slash from signal path if present
        let signal_path = signal_path.trim_start_matches('/');

        // Combine paths
        base.set_path(&format!("{}{}", base.path(), signal_path));
        Ok(base.to_string())
    }
}

/// Fetches collectors configuration from AWS Secrets Manager
#[instrument(skip(client))]
async fn fetch_collectors(client: &SecretsManagerClient) -> Result<Vec<Collector>> {
    let prefix = env::var("COLLECTORS_SECRETS_KEY_PREFIX")
        .context("COLLECTORS_SECRETS_KEY_PREFIX must be set")?;

    tracing::info!("Loading collectors secrets with prefix: {}", prefix);

    // Create a filter for the name prefix
    let filter = Filter::builder().key("name".into()).values(prefix).build();

    let response = client
        .batch_get_secret_value()
        .filters(filter)
        .send()
        .await?;

    // Check for API errors
    let errors = response.errors();
    if !errors.is_empty() {
        for error in errors {
            let error_msg = format!(
                "Failed to fetch secret {}: {} - {}",
                error.secret_id().unwrap_or("unknown"),
                error.error_code().unwrap_or("unknown error"),
                error.message().unwrap_or("no error message")
            );
            tracing::error!("{}", error_msg);
        }
        // If there are errors but also some valid secrets, we'll continue
        // Otherwise, return an error
        if response.secret_values().is_empty() {
            return Err(anyhow::anyhow!(
                "Failed to fetch any secrets. Check the logs for details."
            ));
        }
    }

    let mut collectors = Vec::new();

    // Process each secret in the response
    for secret in response.secret_values() {
        if let Some(secret_string) = secret.secret_string() {
            match serde_json::from_str::<Collector>(secret_string) {
                Ok(collector) => {
                    tracing::debug!(
                        "Successfully loaded collector '{}' from secret {}",
                        collector.name,
                        secret.name().unwrap_or("unknown")
                    );
                    collectors.push(collector);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse collector from secret {}: {}",
                        secret.name().unwrap_or("unknown"),
                        e
                    );
                }
            }
        }
    }

    if collectors.is_empty() {
        return Err(anyhow::anyhow!("No valid collectors found"));
    }

    tracing::info!("Loaded {} collectors from secrets", collectors.len());
    Ok(collectors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_collector_deserialization() {
        // Valid collector with auth
        let valid_json = json!({
            "name": "example-collector",
            "endpoint": "https://collector.example.com",
            "auth": "x-api-key=your-api-key"
        });
        let collector: Collector = serde_json::from_value(valid_json).unwrap();
        assert_eq!(collector.auth, Some("x-api-key=your-api-key".to_string()));

        // Valid collector without auth
        let valid_no_auth = json!({
            "name": "example-collector",
            "endpoint": "https://collector.example.com"
        });
        let collector: Collector = serde_json::from_value(valid_no_auth).unwrap();
        assert_eq!(collector.auth, None);

        // Valid collector with null auth
        let valid_null_auth = json!({
            "name": "example-collector",
            "endpoint": "https://collector.example.com",
            "auth": null
        });
        let collector: Collector = serde_json::from_value(valid_null_auth).unwrap();
        assert_eq!(collector.auth, None);

        // Note: Empty string is preserved as Some("") since that's how serde handles it
        let valid_empty_auth = json!({
            "name": "example-collector",
            "endpoint": "https://collector.example.com",
            "auth": ""
        });
        let collector: Collector = serde_json::from_value(valid_empty_auth).unwrap();
        assert_eq!(collector.auth, Some("".to_string()));
    }

    #[test]
    fn test_construct_signal_endpoint() {
        let collector = Collector {
            name: "test".to_string(),
            endpoint: "https://collector.example.com".to_string(),
            auth: None,
        };

        // Test with simple path
        let result = collector
            .construct_signal_endpoint("https://original.com/v1/traces")
            .unwrap();
        assert_eq!(result, "https://collector.example.com/v1/traces");

        // Test with base path in collector endpoint
        let collector_with_path = Collector {
            name: "test".to_string(),
            endpoint: "https://collector.example.com/base".to_string(),
            auth: None,
        };
        let result = collector_with_path
            .construct_signal_endpoint("https://original.com/v1/traces")
            .unwrap();
        assert_eq!(result, "https://collector.example.com/base/v1/traces");

        // Test with trailing slash in collector endpoint
        let collector_with_slash = Collector {
            name: "test".to_string(),
            endpoint: "https://collector.example.com/".to_string(),
            auth: None,
        };
        let result = collector_with_slash
            .construct_signal_endpoint("https://original.com/v1/traces")
            .unwrap();
        assert_eq!(result, "https://collector.example.com/v1/traces");

        // Test with invalid URLs
        let collector = Collector {
            name: "test".to_string(),
            endpoint: "not a url".to_string(),
            auth: None,
        };
        assert!(collector
            .construct_signal_endpoint("https://original.com/v1/traces")
            .is_err());

        let collector = Collector {
            name: "test".to_string(),
            endpoint: "https://collector.example.com".to_string(),
            auth: None,
        };
        assert!(collector.construct_signal_endpoint("not a url").is_err());
    }

    #[test]
    fn test_collector_cache_ttl() {
        std::env::set_var("COLLECTORS_CACHE_TTL_SECONDS", "2");

        let collectors = Collectors::new(vec![Collector {
            name: "test".to_string(),
            endpoint: "https://collector.example.com".to_string(),
            auth: None,
        }]);

        let cache = CollectorsCache::new(collectors);
        assert!(!cache.is_stale());

        // Sleep for 3 seconds to exceed TTL
        std::thread::sleep(Duration::from_secs(3));
        assert!(cache.is_stale());
    }
}
