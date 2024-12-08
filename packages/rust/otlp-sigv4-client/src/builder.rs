//! Builder pattern implementation for SigV4Client

use aws_credential_types::Credentials;
use opentelemetry_http::HttpClient;
use std::fmt::Debug;

use crate::{SigV4Client, SigV4Error, SigningPredicate};

/// Builder for configuring and creating a SigV4Client
pub struct SigV4ClientBuilder<T: HttpClient> {
    inner: Option<T>,
    credentials: Option<Credentials>,
    region: Option<String>,
    service: Option<String>,
    should_sign_predicate: Option<SigningPredicate>,
}

impl<T: HttpClient + Debug> Debug for SigV4ClientBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigV4ClientBuilder")
            .field("inner", &self.inner)
            .field("credentials", &self.credentials)
            .field("region", &self.region)
            .field("service", &self.service)
            .field("should_sign_predicate", &format_args!("<function>"))
            .finish()
    }
}

impl<T: HttpClient> Default for SigV4ClientBuilder<T> {
    fn default() -> Self {
        Self {
            inner: None,
            credentials: None,
            region: None,
            service: None,
            should_sign_predicate: None,
        }
    }
}

impl<T: HttpClient> SigV4ClientBuilder<T> {
    /// Creates a new SigV4ClientBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the underlying HTTP client
    pub fn with_client(mut self, client: T) -> Self {
        self.inner = Some(client);
        self
    }

    /// Sets the AWS credentials
    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Sets the AWS region (defaults to "us-east-1")
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Sets the AWS service name (e.g. "xray")
    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    /// Sets a predicate to determine whether a request should be signed
    ///
    /// The predicate is a closure that takes a reference to the request and returns a boolean:
    /// - `true` means the request should be signed
    /// - `false` means the request should not be signed
    ///
    /// # Example
    ///
    /// ```
    /// # use aws_credential_types::Credentials;
    /// # use otlp_sigv4_client::SigV4ClientBuilder;
    /// # use reqwest::Client;
    /// let credentials = Credentials::new(
    ///     "access_key",
    ///     "secret_key",
    ///     None,
    ///     None,
    ///     "example"
    /// );
    ///
    /// let client = SigV4ClientBuilder::new()
    ///     .with_client(Client::new())
    ///     .with_credentials(credentials)
    ///     .with_region("us-west-2")
    ///     .with_service("xray")
    ///     .with_signing_predicate(Box::new(|request| {
    ///         // Only sign requests to AWS endpoints
    ///         request.uri().host().map_or(false, |host| {
    ///             host.ends_with(".amazonaws.com")
    ///         })
    ///     }))
    ///     .build()
    ///     .expect("Failed to build client");
    /// ```
    pub fn with_signing_predicate(mut self, predicate: SigningPredicate) -> Self {
        self.should_sign_predicate = Some(predicate);
        self
    }

    /// Builds the SigV4Client with the configured parameters
    ///
    /// # Errors
    ///
    /// Returns a `SigV4Error` if:
    /// - The HTTP client is not provided
    /// - The AWS credentials are not provided
    pub fn build(self) -> Result<SigV4Client<T>, SigV4Error> {
        let inner = self.inner.ok_or(SigV4Error::MissingClient)?;
        let credentials = self.credentials.ok_or(SigV4Error::MissingCredentials)?;
        let region = self
            .region
            .or_else(|| std::env::var("AWS_REGION").ok())
            .unwrap_or_else(|| "us-east-1".to_string());
        let service = self.service.unwrap_or_else(|| "xray".to_string());

        Ok(SigV4Client::new(
            inner,
            credentials,
            region,
            service,
            self.should_sign_predicate,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use aws_credential_types::Credentials;
    use bytes::Bytes;
    use http::{Request, Response};

    #[derive(Debug, Clone)]
    struct MockHttpClient;

    #[async_trait]
    impl HttpClient for MockHttpClient {
        async fn send(
            &self,
            _request: Request<Vec<u8>>,
        ) -> Result<Response<Bytes>, Box<dyn std::error::Error + Send + Sync>> {
            Ok(Response::builder()
                .status(200)
                .body(Bytes::from_static(b"test"))
                .unwrap())
        }
    }

    #[test]
    fn test_builder_missing_client() {
        let credentials = Credentials::new("test", "test", None, None, "test");

        let result = SigV4ClientBuilder::<MockHttpClient>::new()
            .with_credentials(credentials)
            .build();

        assert!(matches!(result.err(), Some(SigV4Error::MissingClient)));
    }

    #[test]
    fn test_builder_missing_credentials() {
        let result = SigV4ClientBuilder::new()
            .with_client(MockHttpClient)
            .build();

        assert!(matches!(result.err(), Some(SigV4Error::MissingCredentials)));
    }

    #[test]
    fn test_builder_success() {
        let credentials = Credentials::new("test", "test", None, None, "test");

        let result = SigV4ClientBuilder::new()
            .with_client(MockHttpClient)
            .with_credentials(credentials)
            .with_region("us-west-2")
            .with_service("xray")
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_default_values() {
        let credentials = Credentials::new("test", "test", None, None, "test");

        let client = SigV4ClientBuilder::new()
            .with_client(MockHttpClient)
            .with_credentials(credentials)
            .build()
            .unwrap();

        assert_eq!(client.region, "us-east-1");
        assert_eq!(client.service, "xray");
    }

    #[test]
    fn test_builder_fluent_interface() {
        let credentials = Credentials::new("test", "test", None, None, "test");

        let result = SigV4ClientBuilder::new()
            .with_client(MockHttpClient)
            .with_credentials(credentials.clone())
            .with_region("us-west-2")
            .with_service("xray")
            .with_credentials(credentials) // Test that we can override values
            .build();

        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.region, "us-west-2");
        assert_eq!(client.service, "xray");
    }

    #[test]
    fn test_builder_with_signing_predicate() {
        let credentials = Credentials::new("test", "test", None, None, "test");
        let predicate = Box::new(|request: &Request<Vec<u8>>| {
            request
                .uri()
                .host()
                .map_or(false, |host| host.ends_with(".amazonaws.com"))
        });

        let result = SigV4ClientBuilder::new()
            .with_client(MockHttpClient)
            .with_credentials(credentials)
            .with_signing_predicate(predicate)
            .build();

        assert!(result.is_ok());
    }
}
