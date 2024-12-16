//! A SigV4-compatible HTTP client wrapper for OpenTelemetry OTLP exporters.
//!
//! This crate provides a wrapper that adds AWS SigV4 signing capabilities to any OpenTelemetry
//! HTTP client implementation. It's particularly useful when sending telemetry data to AWS services
//! that require SigV4 authentication. This crate is part of the
//! [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder/) project, which provides
//! a comprehensive solution for OpenTelemetry telemetry collection in AWS Lambda environments.
//!
//! # Features
//!
//! - `reqwest` - Enables support for the reqwest HTTP client
//! - `hyper` - Enables support for the hyper HTTP client
//!
//! # Example
//!
//! ```no_run
//! use aws_credential_types::Credentials;
//! use otlp_sigv4_client::SigV4ClientBuilder;
//! use opentelemetry_otlp::{HttpExporterBuilder, WithHttpConfig};
//! use reqwest::Client as ReqwestClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let credentials = Credentials::new(
//!         "access_key",
//!         "secret_key",
//!         None,
//!         None,
//!         "example",
//!     );
//!
//!     let sigv4_client = SigV4ClientBuilder::new()
//!         .with_client(ReqwestClient::new())
//!         .with_credentials(credentials)
//!         .with_region("us-west-2")
//!         .with_service("xray")
//!         .build()?;
//!
//!     let _exporter = HttpExporterBuilder::default()
//!         .with_http_client(sigv4_client)
//!         .build_span_exporter()?;
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use aws_credential_types::Credentials;
use bytes::Bytes;
use http::{Request, Response};
use opentelemetry_http::{HttpClient, HttpError};
use reqwest::header::HeaderMap;
use std::fmt::Debug;
use thiserror::Error;

/// Type alias for the request signing predicate
pub type SigningPredicate = Box<dyn Fn(&Request<Vec<u8>>) -> bool + Send + Sync>;

mod builder;
pub mod signing;

pub use builder::SigV4ClientBuilder;
pub use signing::sign_request;

/// Errors that can occur during SigV4 client operations
#[derive(Error, Debug)]
pub enum SigV4Error {
    #[error("AWS credentials not provided")]
    MissingCredentials,

    #[error("HTTP client not provided")]
    MissingClient,

    #[error("Failed to sign request: {0}")]
    SigningError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("HTTP error: {0}")]
    HttpError(#[from] http::Error),
}

/// A decorator that adds SigV4 signing capabilities to any HttpClient implementation
pub struct SigV4Client<T: HttpClient> {
    inner: T,
    credentials: Credentials,
    region: String,
    service: String,
    should_sign_predicate: Option<SigningPredicate>,
}

impl<T: HttpClient + Debug> Debug for SigV4Client<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigV4Client")
            .field("inner", &self.inner)
            .field("credentials", &self.credentials)
            .field("region", &self.region)
            .field("service", &self.service)
            .field("should_sign_predicate", &format_args!("<function>"))
            .finish()
    }
}

impl<T: HttpClient> SigV4Client<T> {
    /// Creates a new SigV4Client with the given parameters
    pub(crate) fn new(
        inner: T,
        credentials: Credentials,
        region: impl Into<String>,
        service: impl Into<String>,
        should_sign_predicate: Option<SigningPredicate>,
    ) -> Self {
        Self {
            inner,
            credentials,
            region: region.into(),
            service: service.into(),
            should_sign_predicate,
        }
    }

    async fn sign_request(
        &self,
        request: Request<Vec<u8>>,
    ) -> Result<Request<Vec<u8>>, SigV4Error> {
        // Check if we should sign this request
        if let Some(predicate) = &self.should_sign_predicate {
            if !predicate(&request) {
                return Ok(request);
            }
        }

        let (parts, body) = request.into_parts();

        let endpoint = format!(
            "{}://{}{}",
            parts.uri.scheme_str().unwrap_or("https"),
            parts.uri.authority().map(|a| a.as_str()).unwrap_or(""),
            parts.uri.path()
        );

        // Convert http::HeaderMap to reqwest::HeaderMap
        let mut reqwest_headers = HeaderMap::new();
        for (name, value) in parts.headers.iter() {
            reqwest_headers.insert(
                reqwest::header::HeaderName::from_bytes(name.as_ref()).unwrap(),
                reqwest::header::HeaderValue::from_bytes(value.as_bytes()).unwrap(),
            );
        }

        let signed_headers = signing::sign_request(
            &self.credentials,
            &endpoint,
            &reqwest_headers,
            &body,
            &self.region,
            &self.service,
        )
        .map_err(SigV4Error::SigningError)?;

        // Rebuild request with signed headers
        let mut builder = Request::builder().method(parts.method).uri(parts.uri);

        // Convert reqwest::HeaderMap to http::HeaderMap and preserve original headers
        let mut http_headers = parts.headers;
        for (name, value) in signed_headers.iter() {
            if let Ok(header_name) = http::header::HeaderName::from_bytes(name.as_ref()) {
                if let Ok(header_value) = http::header::HeaderValue::from_bytes(value.as_bytes()) {
                    http_headers.insert(header_name, header_value);
                }
            }
        }

        // Set the headers on the builder
        *builder.headers_mut().unwrap() = http_headers;

        Ok(builder.body(body)?)
    }
}

#[async_trait]
impl<T: HttpClient> HttpClient for SigV4Client<T> {
    async fn send(&self, request: Request<Vec<u8>>) -> Result<Response<Bytes>, HttpError> {
        let signed_request = self
            .sign_request(request)
            .await
            .map_err(|e| Box::new(e) as HttpError)?;

        self.inner.send(signed_request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_credential_types::Credentials;
    use http::{Request, Response, StatusCode};
    use std::sync::Arc;

    #[derive(Debug)]
    struct MockHttpClient {
        response: Arc<Response<Bytes>>,
    }

    impl MockHttpClient {
        fn new(response: Response<Bytes>) -> Self {
            Self {
                response: Arc::new(response),
            }
        }
    }

    #[async_trait]
    impl HttpClient for MockHttpClient {
        async fn send(&self, _request: Request<Vec<u8>>) -> Result<Response<Bytes>, HttpError> {
            Ok(Response::builder()
                .status(self.response.status())
                .body(self.response.body().clone())
                .unwrap())
        }
    }

    #[tokio::test]
    async fn test_sigv4_client_signs_request() {
        // Create a mock response
        let mock_response = Response::builder()
            .status(StatusCode::OK)
            .body(Bytes::from("test response"))
            .unwrap();

        // Create the mock client
        let mock_client = MockHttpClient::new(mock_response);

        // Create test credentials
        let credentials = Credentials::new("test_key", "test_secret", None, None, "test");

        // Create the SigV4 client
        let sigv4_client = SigV4Client::new(mock_client, credentials, "us-east-1", "xray", None);

        // Create a test request
        let request = Request::builder()
            .method("POST")
            .uri("https://xray.us-east-1.amazonaws.com/")
            .body(Vec::new())
            .unwrap();

        // Send the request
        let response = sigv4_client.send(request).await.unwrap();

        // Verify the response
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), &Bytes::from("test response"));
    }

    #[tokio::test]
    async fn test_sigv4_client_preserves_headers() {
        // Create a mock response
        let mock_response = Response::builder()
            .status(StatusCode::OK)
            .body(Bytes::from("test response"))
            .unwrap();

        // Create the mock client
        let mock_client = MockHttpClient::new(mock_response);

        // Create test credentials
        let credentials = Credentials::new("test_key", "test_secret", None, None, "test");

        // Create the SigV4 client
        let sigv4_client = SigV4Client::new(mock_client, credentials, "us-east-1", "xray", None);

        // Create a test request with custom headers
        let request = Request::builder()
            .method("POST")
            .uri("https://xray.us-east-1.amazonaws.com/")
            .header("X-Custom-Header", "test-value")
            .body(Vec::new())
            .unwrap();

        // Sign the request
        let signed_request = sigv4_client.sign_request(request).await.unwrap();

        // Verify that the original header is preserved
        assert!(signed_request.headers().contains_key("X-Custom-Header"));
        assert_eq!(
            signed_request.headers().get("X-Custom-Header").unwrap(),
            "test-value"
        );

        // Verify that AWS SigV4 headers are added
        assert!(signed_request.headers().contains_key("x-amz-date"));
        assert!(signed_request.headers().contains_key("authorization"));
    }
}
