use crate::telemetry::TelemetryData;
use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_ENCODING, CONTENT_TYPE};
use reqwest::Client as ReqwestClient;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, instrument, warn, Span};
use url::Url;

const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4318/v1/traces";
const OTLP_TRACES_PATH: &str = "/v1/traces";
const DEFAULT_OTLP_EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// Parses OTLP headers from a comma-separated key=value string.
fn parse_otlp_headers(headers_str: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    if headers_str.is_empty() {
        return Ok(headers);
    }
    for pair_str in headers_str.split(',') {
        let pair_str = pair_str.trim();
        if pair_str.is_empty() {
            continue;
        }
        match pair_str.split_once('=') {
            Some((key_str, value_str)) => {
                let key = key_str.trim();
                let value = value_str.trim();
                if key.is_empty() {
                    warn!("Empty header key found in OTLP headers string part: '{}' from full string: '{}'", pair_str, headers_str);
                    continue;
                }
                match HeaderName::from_str(key) {
                    Ok(header_name) => match HeaderValue::from_str(value) {
                        Ok(header_value) => {
                            headers.append(header_name, header_value);
                        }
                        Err(e) => {
                            warn!(
                                "Invalid header value '{}' for key '{}': {}. Skipping header.",
                                value, key, e
                            );
                        }
                    },
                    Err(e) => {
                        warn!("Invalid header name '{}': {}. Skipping header.", key, e);
                    }
                }
            }
            None => {
                warn!("Malformed header pair (missing '='): '{}' in OTLP headers string: '{}'. Skipping.", pair_str, headers_str);
            }
        }
    }
    Ok(headers)
}

/// Resolves OTLP headers from environment variables.
/// Priotity: OTEL_EXPORTER_OTLP_TRACES_HEADERS, then OTEL_EXPORTER_OTLP_HEADERS.
fn resolve_otlp_headers() -> Result<HeaderMap> {
    let traces_headers_var = env::var("OTEL_EXPORTER_OTLP_TRACES_HEADERS");
    let generic_headers_var = env::var("OTEL_EXPORTER_OTLP_HEADERS");

    match traces_headers_var {
        Ok(headers_str) if !headers_str.is_empty() => {
            debug!("Using OTEL_EXPORTER_OTLP_TRACES_HEADERS: {}", headers_str);
            return parse_otlp_headers(&headers_str);
        }
        _ => { // Fall through if TRACES_HEADERS is not set or empty
        }
    }

    match generic_headers_var {
        Ok(headers_str) if !headers_str.is_empty() => {
            debug!("Using OTEL_EXPORTER_OTLP_HEADERS: {}", headers_str);
            return parse_otlp_headers(&headers_str);
        }
        _ => { // Fall through if HEADERS is not set or empty
        }
    }

    Ok(HeaderMap::new()) // No headers from env vars
}

/// Resolves the OTLP endpoint URL based on OpenTelemetry environment variables.
/// Priorities:
/// 1. OTEL_EXPORTER_OTLP_TRACES_ENDPOINT (used as is)
/// 2. OTEL_EXPORTER_OTLP_ENDPOINT (base URL, /v1/traces might be appended)
/// 3. Default: http://localhost:4318/v1/traces
fn resolve_otlp_endpoint() -> Result<Url> {
    if let Ok(traces_endpoint) = env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") {
        if !traces_endpoint.is_empty() {
            debug!(
                "Using OTEL_EXPORTER_OTLP_TRACES_ENDPOINT: {}",
                traces_endpoint
            );
            return Url::parse(&traces_endpoint).with_context(|| {
                format!("Invalid URL from OTEL_EXPORTER_OTLP_TRACES_ENDPOINT: {traces_endpoint}")
            });
        }
    }

    if let Ok(generic_endpoint) = env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        if !generic_endpoint.is_empty() {
            debug!("Using OTEL_EXPORTER_OTLP_ENDPOINT: {}", generic_endpoint);
            let mut url = Url::parse(&generic_endpoint).with_context(|| {
                format!("Invalid URL from OTEL_EXPORTER_OTLP_ENDPOINT: {generic_endpoint}")
            })?;

            let current_path = url.path();
            if !current_path.ends_with(OTLP_TRACES_PATH) {
                let new_path = if current_path == "/" || current_path.is_empty() {
                    OTLP_TRACES_PATH.to_string()
                } else {
                    format!("{}{}", current_path.trim_end_matches('/'), OTLP_TRACES_PATH)
                };
                url.set_path(&new_path);
            }
            return Ok(url);
        }
    }

    debug!("Using default OTLP endpoint: {}", DEFAULT_OTLP_ENDPOINT);
    Url::parse(DEFAULT_OTLP_ENDPOINT)
        .with_context(|| format!("Failed to parse default OTLP endpoint: {DEFAULT_OTLP_ENDPOINT}"))
}

/// Parses an OTLP timeout string (expected to be milliseconds) into a Duration.
fn parse_otlp_timeout_millis(duration_ms_str: &str) -> Result<Duration> {
    let millis = duration_ms_str.parse::<u64>().with_context(|| {
        format!("Invalid OTLP timeout value: '{duration_ms_str}'. Expected integer milliseconds.")
    })?;
    Ok(Duration::from_millis(millis))
}

/// Resolves the OTLP export timeout from environment variables.
/// Value is expected to be in milliseconds.
fn resolve_otlp_timeout() -> Duration {
    let traces_timeout_var = env::var("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT");
    let generic_timeout_var = env::var("OTEL_EXPORTER_OTLP_TIMEOUT");

    let timeout_str_to_parse = match traces_timeout_var {
        Ok(val) if !val.is_empty() => {
            debug!("Using OTEL_EXPORTER_OTLP_TRACES_TIMEOUT: {}", val);
            Some(val)
        }
        _ => match generic_timeout_var {
            Ok(val) if !val.is_empty() => {
                debug!("Using OTEL_EXPORTER_OTLP_TIMEOUT: {}", val);
                Some(val)
            }
            _ => None,
        },
    };

    if let Some(s) = timeout_str_to_parse {
        match parse_otlp_timeout_millis(&s) {
            Ok(duration) => {
                debug!("Parsed OTLP export timeout duration: {:?}", duration);
                return duration;
            }
            Err(e) => {
                warn!(
                    "Failed to parse OTLP timeout value '{}': {}. Using default timeout.",
                    s, e
                );
            }
        }
    }
    debug!(
        "Using default OTLP export timeout: {:?}",
        DEFAULT_OTLP_EXPORT_TIMEOUT
    );
    DEFAULT_OTLP_EXPORT_TIMEOUT
}

/// Trait for an HTTP client capable of sending OTLP telemetry batches for the forwarder.
#[async_trait]
pub trait HttpOtlpForwarderClient: Send + Sync {
    async fn post_telemetry(
        &self,
        target_url: Url,
        headers: HeaderMap,
        payload: Bytes,
        timeout: Duration,
    ) -> Result<reqwest::Response>;
}

#[async_trait]
impl HttpOtlpForwarderClient for ReqwestClient {
    async fn post_telemetry(
        &self,
        target_url: Url,
        headers: HeaderMap,
        payload: Bytes,
        timeout: Duration,
    ) -> Result<reqwest::Response> {
        self.post(target_url)
            .headers(headers)
            .body(payload)
            .timeout(timeout)
            .send()
            .await
            .context("HTTP request failed during OTLP export")
    }
}

/// A convenience type alias for Arc\<dyn HttpOtlpForwarderClient\>
pub type HttpClient = Arc<dyn HttpOtlpForwarderClient + Send + Sync>;

/// Sends a batch of OTLP telemetry data.
/// The TelemetryData payload is assumed to be a compacted, possibly compressed, OTLP protobuf batch.
#[instrument(name="http_sender/send_telemetry_batch", skip_all, fields(
    otel.kind = "client",
    http.method = "POST",
    http.url = %telemetry_data.endpoint,
    http.status_code
    // error details will be added on error
))]
pub async fn send_telemetry_batch(
    client: &impl HttpOtlpForwarderClient,
    telemetry_data: TelemetryData,
) -> Result<()> {
    let resolved_target_url = resolve_otlp_endpoint()?;
    Span::current().record("http.url", resolved_target_url.as_str());
    let timeout = resolve_otlp_timeout();

    let mut headers = resolve_otlp_headers()?;
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&telemetry_data.content_type)
            .context("Invalid Content-Type in TelemetryData")?,
    );
    if let Some(encoding) = &telemetry_data.content_encoding {
        headers.insert(
            CONTENT_ENCODING,
            HeaderValue::from_str(encoding).context("Invalid Content-Encoding in TelemetryData")?,
        );
    } else {
        headers.remove(CONTENT_ENCODING);
    }

    let payload_bytes = Bytes::from(telemetry_data.payload); // Convert Vec<u8> to Bytes

    debug!(
        name = "sending telemetry batch",
        target_url = %resolved_target_url,
        timeout_ms = timeout.as_millis(),
        headers = ?headers,
        payload_size_bytes = payload_bytes.len(), // Use length of Bytes
        "Request details"
    );

    let response = match client
        .post_telemetry(resolved_target_url.clone(), headers, payload_bytes, timeout)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            Span::current().record("otel.status_code", "ERROR");
            Span::current().record("error", true);
            Span::current().record("error.message", e.to_string());
            warn!(target_url = %resolved_target_url, error = %e, "OTLP HTTP post_telemetry failed");
            return Err(e);
        }
    };

    let status = response.status();
    Span::current().record("http.status_code", status.as_u16());

    if !status.is_success() {
        Span::current().record("otel.status_code", "ERROR");
        let error_body = response.text().await.unwrap_or_else(|e| {
            warn!("Failed to read error response body: {}", e);
            format!("Failed to read response body. Status: {status}")
        });
        Span::current().record("error.message", error_body.clone());
        warn!(
            target_url = %resolved_target_url,
            status = %status,
            response_body = %error_body,
            "OTLP export failed with non-success status"
        );
        return Err(anyhow::anyhow!(
            "OTLP export failed to {}. Status: {}. Body: {}",
            resolved_target_url,
            status,
            error_body
        ));
    }

    debug!(target_url = %resolved_target_url, status = %status, "Telemetry batch sent successfully");
    Ok(())
}

#[cfg(feature = "instrumented-client")]
pub mod instrumented {
    use super::*;
    use reqwest_middleware::ClientWithMiddleware;

    /// A pre-configured HTTP client that wraps ClientWithMiddleware and implements HttpOtlpForwarderClient
    pub struct InstrumentedHttpClient {
        inner: ClientWithMiddleware,
    }

    impl InstrumentedHttpClient {
        /// Creates a new instrumented HTTP client from a ClientWithMiddleware
        ///
        /// # Example
        /// ```rust,ignore
        /// use reqwest::Client;
        /// use reqwest_middleware::ClientBuilder;
        /// use reqwest_tracing::TracingMiddleware;
        /// use serverless_otlp_forwarder_core::InstrumentedHttpClient;
        ///
        /// let base_client = Client::new();
        /// let middleware_client = ClientBuilder::new(base_client)
        ///     .with(TracingMiddleware::default())
        ///     .build();
        /// let instrumented_client = InstrumentedHttpClient::new(middleware_client);
        /// ```
        pub fn new(client: ClientWithMiddleware) -> Self {
            Self { inner: client }
        }
    }

    #[async_trait]
    impl HttpOtlpForwarderClient for InstrumentedHttpClient {
        async fn post_telemetry(
            &self,
            target_url: Url,
            headers: HeaderMap,
            payload: Bytes,
            timeout: Duration,
        ) -> Result<reqwest::Response> {
            self.inner
                .post(target_url)
                .headers(headers)
                .body(payload)
                .timeout(timeout)
                .send()
                .await
                .context("HTTP request failed during instrumented OTLP export")
        }
    }
}

/// Utility functions for creating HTTP clients with common configurations
pub mod client_builder {
    use super::*;

    /// Creates a simple ReqwestClient that implements HttpOtlpForwarderClient
    pub fn simple() -> ReqwestClient {
        ReqwestClient::new()
    }

    /// Creates a ReqwestClient with custom timeout
    pub fn with_timeout(timeout: Duration) -> ReqwestClient {
        ReqwestClient::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to build HTTP client")
    }

    #[cfg(feature = "instrumented-client")]
    /// Creates an instrumented client with tracing middleware
    pub fn instrumented() -> crate::InstrumentedHttpClient {
        use reqwest_middleware::ClientBuilder;
        use reqwest_tracing::TracingMiddleware;

        let base_client = ReqwestClient::new();
        let middleware_client = ClientBuilder::new(base_client)
            .with(TracingMiddleware::default())
            .build();
        crate::InstrumentedHttpClient::new(middleware_client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::TelemetryData;
    use reqwest::Client as ReqwestClient;
    use sealed_test::prelude::*;
    use std::time::Duration as StdDuration;
    use wiremock::matchers::{body_bytes, header, method, path};
    use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

    // Helper struct to ensure env vars are cleaned up.
    struct EnvVarGuard {
        name: String,
        original_value: Option<String>,
    }

    impl EnvVarGuard {
        #[allow(dead_code)]
        fn set(name: &str, value: &str) -> Self {
            let original_value = env::var(name).ok();
            env::set_var(name, value);
            Self {
                name: name.to_string(),
                original_value,
            }
        }

        #[allow(dead_code)]
        fn remove(name: &str) -> Self {
            let original_value = env::var(name).ok();
            env::remove_var(name);
            Self {
                name: name.to_string(),
                original_value,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(val) = &self.original_value {
                env::set_var(&self.name, val);
            } else {
                env::remove_var(&self.name);
            }
        }
    }

    fn test_client() -> ReqwestClient {
        ReqwestClient::new()
    }

    #[tokio::test]
    async fn test_parse_otlp_headers_empty() {
        let headers = parse_otlp_headers("").unwrap();
        assert!(headers.is_empty());
    }

    #[tokio::test]
    async fn test_parse_otlp_headers_single() {
        let headers = parse_otlp_headers("key1=value1").unwrap();
        assert_eq!(headers.get("key1").unwrap(), "value1");
    }

    #[tokio::test]
    async fn test_parse_otlp_headers_multiple() {
        let headers = parse_otlp_headers("key1=value1,key2=value2, key3 = value3 ").unwrap();
        assert_eq!(headers.get("key1").unwrap(), "value1");
        assert_eq!(headers.get("key2").unwrap(), "value2");
        assert_eq!(headers.get("key3").unwrap(), "value3");
    }

    #[tokio::test]
    async fn test_parse_otlp_headers_invalid_pair() {
        let headers = parse_otlp_headers("key1=value1,invalid,key2=value2").unwrap();
        assert_eq!(headers.get("key1").unwrap(), "value1");
        assert_eq!(headers.get("key2").unwrap(), "value2");
        assert!(headers.get("invalid").is_none());
        assert_eq!(headers.len(), 2);
    }

    #[tokio::test]
    async fn test_parse_otlp_headers_empty_key_value() {
        let headers = parse_otlp_headers("key1=, =value2 , key3=value3").unwrap();
        assert_eq!(headers.get("key1").unwrap(), "");
        assert_eq!(headers.get("key3").unwrap(), "value3");
        assert_eq!(headers.len(), 2);
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_headers_none_set() {
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_HEADERS");
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_HEADERS");
        let headers = resolve_otlp_headers().unwrap();
        assert!(headers.is_empty());
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_headers_traces_set() {
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_HEADERS", "tracekey=traceval");
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_HEADERS");
        let headers = resolve_otlp_headers().unwrap();
        assert_eq!(headers.get("tracekey").unwrap(), "traceval");
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_headers_generic_set() {
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_HEADERS");
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_HEADERS", "generalkey=generalval");
        let headers = resolve_otlp_headers().unwrap();
        assert_eq!(headers.get("generalkey").unwrap(), "generalval");
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_headers_traces_takes_precedence() {
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_HEADERS", "tracekey=traceval");
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_HEADERS", "generalkey=generalval");
        let headers = resolve_otlp_headers().unwrap();
        assert_eq!(headers.get("tracekey").unwrap(), "traceval");
        assert!(headers.get("generalkey").is_none());
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_endpoint_default() {
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT");
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_ENDPOINT");
        assert_eq!(
            resolve_otlp_endpoint().unwrap().to_string(),
            DEFAULT_OTLP_ENDPOINT
        );
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_endpoint_traces_var() {
        let custom_endpoint = "http://custom-traces.local:4318/v1/traces";
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", custom_endpoint);
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_ENDPOINT");
        assert_eq!(
            resolve_otlp_endpoint().unwrap().to_string(),
            custom_endpoint
        );
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_endpoint_traces_var_no_path() {
        let custom_endpoint = "http://custom-traces.local:4318";
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", custom_endpoint);
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_ENDPOINT");
        let expected_url = if custom_endpoint.ends_with('/') {
            custom_endpoint.to_string()
        } else {
            format!("{custom_endpoint}/")
        };
        assert_eq!(resolve_otlp_endpoint().unwrap().to_string(), expected_url);
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_endpoint_generic_var_no_path() {
        let base_endpoint = "http://generic.local:4318";
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT");
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_ENDPOINT", base_endpoint);
        let expected_url = format!("{}/v1/traces", base_endpoint.trim_end_matches('/'));
        assert_eq!(resolve_otlp_endpoint().unwrap().to_string(), expected_url);
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_endpoint_generic_var_with_path() {
        let base_endpoint = "http://generic.local:4318/custom/path";
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT");
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_ENDPOINT", base_endpoint);
        let expected_url = format!("{}/v1/traces", base_endpoint.trim_end_matches('/'));
        assert_eq!(resolve_otlp_endpoint().unwrap().to_string(), expected_url);
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_endpoint_generic_var_with_traces_path_already() {
        let full_endpoint = "http://generic.local:4318/v1/traces";
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT");
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_ENDPOINT", full_endpoint);
        assert_eq!(resolve_otlp_endpoint().unwrap().to_string(), full_endpoint);
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_endpoint_traces_takes_precedence() {
        let traces_specific = "http://traces-specific.local:4318/v1/traces";
        let generic_val = "http://generic-ignored.local:4318";
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", traces_specific);
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_ENDPOINT", generic_val);
        assert_eq!(
            resolve_otlp_endpoint().unwrap().to_string(),
            traces_specific
        );
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_timeout_default() {
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT");
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TIMEOUT");
        assert_eq!(resolve_otlp_timeout(), DEFAULT_OTLP_EXPORT_TIMEOUT);
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_timeout_traces_var_millis_val() {
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT", "1500");
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TIMEOUT");
        assert_eq!(resolve_otlp_timeout(), Duration::from_millis(1500));
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_timeout_generic_var_millis_val() {
        let _g1 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT");
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TIMEOUT", "7000");
        assert_eq!(resolve_otlp_timeout(), Duration::from_millis(7000));
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_timeout_traces_takes_precedence_millis_val() {
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT", "3000");
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TIMEOUT", "12000");
        assert_eq!(resolve_otlp_timeout(), Duration::from_millis(3000));
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_timeout_invalid_value_uses_default() {
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT", "invalid");
        let _g2 = EnvVarGuard::remove("OTEL_EXPORTER_OTLP_TIMEOUT");
        assert_eq!(resolve_otlp_timeout(), DEFAULT_OTLP_EXPORT_TIMEOUT);
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_resolve_otlp_timeout_invalid_value_suffixed_uses_default() {
        let _g1 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT", "5s");
        assert_eq!(resolve_otlp_timeout(), DEFAULT_OTLP_EXPORT_TIMEOUT);
    }

    struct SlowServerMatcher {
        delay: StdDuration,
    }
    impl Match for SlowServerMatcher {
        fn matches(&self, _request: &Request) -> bool {
            std::thread::sleep(self.delay);
            true
        }
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_send_telemetry_batch_respects_timeout() {
        let server = MockServer::start().await;
        let client = ReqwestClient::builder().build().unwrap();
        let telemetry = TelemetryData::default();
        Mock::given(SlowServerMatcher {
            delay: StdDuration::from_millis(200),
        })
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
        let _g1 = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}{}", server.uri(), OTLP_TRACES_PATH),
        );
        let _g2 = EnvVarGuard::set("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT", "100");
        let result = send_telemetry_batch(&client, telemetry).await;
        assert!(
            result.is_err(),
            "Expected send_telemetry_batch to fail due to timeout"
        );
        let err = result.unwrap_err();

        let is_timeout_error = err.chain().any(|cause| {
            if let Some(req_err) = cause.downcast_ref::<reqwest::Error>() {
                req_err.is_timeout()
            } else {
                cause.to_string().to_lowercase().contains("timeout")
                    || cause.to_string().to_lowercase().contains("timed out")
            }
        });
        assert!(
            is_timeout_error,
            "Error was not a timeout error. Actual error: {:?}\nCause chain: {:#?}",
            err,
            err.chain().collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_send_telemetry_batch_success_with_env_headers() {
        let server = MockServer::start().await;
        let client = test_client();
        let telemetry = TelemetryData {
            payload: b"payload_for_headers_test".to_vec(),
            content_type: "application/x-protobuf".to_string(),
            content_encoding: None,
            ..Default::default()
        };
        Mock::given(method("POST"))
            .and(path(OTLP_TRACES_PATH))
            .and(header(CONTENT_TYPE, "application/x-protobuf"))
            .and(header("customkey", "customvalue"))
            .and(header("anotherkey", "anotherval"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let _g1 = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}{}", server.uri(), OTLP_TRACES_PATH),
        );
        let _g2 = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_HEADERS",
            "customkey=customvalue,anotherkey=anotherval",
        );
        let result = send_telemetry_batch(&client, telemetry).await;
        assert!(
            result.is_ok(),
            "send_telemetry_batch failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_send_telemetry_batch_success() {
        let server = MockServer::start().await;
        let client = test_client();
        let telemetry = TelemetryData {
            payload: b"test_payload".to_vec(),
            content_type: "application/x-protobuf".to_string(),
            content_encoding: Some("gzip".to_string()),
            ..Default::default()
        };
        Mock::given(method("POST"))
            .and(path(OTLP_TRACES_PATH))
            .and(body_bytes(telemetry.payload.clone()))
            .and(header(CONTENT_TYPE, "application/x-protobuf"))
            .and(header(CONTENT_ENCODING, "gzip"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let _g1 = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}{}", server.uri(), OTLP_TRACES_PATH),
        );
        let result = send_telemetry_batch(&client, telemetry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_send_telemetry_batch_no_content_encoding() {
        let server = MockServer::start().await;
        let client = test_client();
        let telemetry = TelemetryData {
            payload: b"test_payload_no_encoding".to_vec(),
            content_type: "application/x-protobuf".to_string(),
            content_encoding: None,
            ..Default::default()
        };
        Mock::given(method("POST"))
            .and(path(OTLP_TRACES_PATH))
            .and(body_bytes(telemetry.payload.clone()))
            .and(header(CONTENT_TYPE, "application/x-protobuf"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let _g1 = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}{}", server.uri(), OTLP_TRACES_PATH),
        );
        let result = send_telemetry_batch(&client, telemetry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[sealed_test]
    async fn test_send_telemetry_batch_server_error() {
        let server = MockServer::start().await;
        let client = test_client();
        let telemetry = TelemetryData::default();
        Mock::given(method("POST"))
            .and(path(OTLP_TRACES_PATH))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Error"))
            .expect(1)
            .mount(&server)
            .await;
        let _g1 = EnvVarGuard::set(
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            &format!("{}{}", server.uri(), OTLP_TRACES_PATH),
        );
        let result = send_telemetry_batch(&client, telemetry).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Status: 500"));
        assert!(err_msg.contains("Internal Error"));
    }
}
