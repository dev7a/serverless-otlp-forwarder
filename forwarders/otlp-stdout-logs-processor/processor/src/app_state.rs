//! Shared application state for Lambda functions
//!
//! This module provides a shared AppState structure that can be used
//! across different Lambda functions to manage common resources.

use anyhow::Result;
use aws_credential_types::{provider::ProvideCredentials, Credentials};
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use lambda_runtime::Error as LambdaError;
use reqwest::Client as ReqwestClient;

/// Shared application state across Lambda invocations
pub struct AppState {
    pub http_client: ReqwestClient,
    pub credentials: Credentials,
    pub secrets_client: SecretsManagerClient,
    pub region: String,
}

impl AppState {
    /// Create a new AppState instance
    pub async fn new() -> Result<Self, LambdaError> {
        let config = aws_config::load_from_env().await;
        let credentials = config
            .credentials_provider()
            .expect("No credentials provider found")
            .provide_credentials()
            .await?;
        let region = config.region().expect("No region found").to_string();

        Ok(Self {
            http_client: ReqwestClient::new(),
            credentials,
            secrets_client: SecretsManagerClient::new(&config),
            region,
        })
    }
}
