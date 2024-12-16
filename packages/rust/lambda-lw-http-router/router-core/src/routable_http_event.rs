use aws_lambda_events::{
    alb::AlbTargetGroupRequest,
    apigw::{ApiGatewayProxyRequest, ApiGatewayV2httpRequest, ApiGatewayWebsocketProxyRequest},
    http::HeaderMap,
};
use lambda_runtime::tracing::Span;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::RwLock;
use tracing_opentelemetry::OpenTelemetrySpanExt;

lazy_static! {
    /// Default set of safe HTTP headers to capture in spans.
    ///
    /// Following OpenTelemetry semantic conventions for HTTP headers:
    /// - Includes common, non-sensitive headers useful for debugging and monitoring
    /// - Excludes sensitive headers like `Authorization` and `Cookie` to avoid security risks
    /// - Can be overridden via `configure_captured_*_headers()` if different headers are needed
    /// - `User-Agent` is handled separately via `user_agent.original` attribute
    /// - `Content-Length` is handled separately via `http.request.body.size` and `http.response.body.size` attributes
    static ref COMMON_HEADERS: Vec<&'static str> = vec![
        "accept",
        "accept-encoding",
        "accept-language",
        "cache-control",
        "content-encoding",
        "content-language",
        "content-type",
        "etag",
        "forwarded",
        "if-match",
        "if-modified-since",
        "if-none-match",
        "if-unmodified-since",
        "last-modified",
        "location",
        "origin",
        "range",
        "vary",
        "x-forwarded-for",
        "x-forwarded-host",
        "x-forwarded-proto",
    ];
    static ref CAPTURED_REQUEST_HEADERS: RwLock<HashSet<String>> = {
        let mut headers = HashSet::new();
        headers.extend(COMMON_HEADERS.iter().map(|&h| h.to_string()));
        RwLock::new(headers)
    };
    static ref CAPTURED_RESPONSE_HEADERS: RwLock<HashSet<String>> = {
        let mut headers = HashSet::new();
        headers.extend(COMMON_HEADERS.iter().map(|&h| h.to_string()));
        RwLock::new(headers)
    };
}

/// Configure which request headers should be captured in spans
#[allow(dead_code)]
pub fn configure_captured_request_headers(headers: &[&str]) {
    let mut captured = CAPTURED_REQUEST_HEADERS.write().unwrap();
    captured.clear();
    captured.extend(headers.iter().map(|&h| h.to_lowercase()));
}

/// Configure which response headers should be captured in spans
#[allow(dead_code)]
pub fn configure_captured_response_headers(headers: &[&str]) {
    let mut captured = CAPTURED_RESPONSE_HEADERS.write().unwrap();
    captured.clear();
    captured.extend(headers.iter().map(|&h| h.to_lowercase()));
}

/// A trait for HTTP events that can be routed by the router.
///
/// This trait defines the minimum requirements for an HTTP event to be
/// usable with the routing system, as well as OpenTelemetry semantic conventions
/// for HTTP spans. Any event type that implements this trait can be used with
/// the router and will automatically get standardized span attributes.
///
/// # Examples
///
/// ```rust
/// use lambda_lw_http_router_core::RoutableHttpEvent;
/// use http::HeaderMap;
///
/// #[derive(Clone)]  // Required by RoutableHttpEvent
/// struct CustomHttpEvent {
///     path: String,
///     method: String,
///     headers: HeaderMap,
/// }
///
/// impl RoutableHttpEvent for CustomHttpEvent {
///     fn path(&self) -> Option<String> {
///         Some(self.path.clone())
///     }
///     
///     fn http_method(&self) -> String {
///         self.method.clone()
///     }
///
///     fn http_headers(&self) -> Option<&HeaderMap> {
///         Some(&self.headers)
///     }
/// }
/// ```
pub trait RoutableHttpEvent: Send + Sync + Clone + 'static {
    /// Returns the raw path of the HTTP request
    fn path(&self) -> Option<String>;

    /// Returns the HTTP method of the request
    fn http_method(&self) -> String;

    /// Returns the API Gateway resource pattern if available, otherwise None
    fn route(&self) -> Option<String> {
        None
    }

    /// Returns pre-parsed path parameters if available
    fn path_parameters(&self) -> Option<&HashMap<String, String>> {
        None
    }

    /// Returns the query string
    fn url_query(&self) -> Option<String> {
        None
    }

    /// Returns the client IP address
    fn client_address(&self) -> Option<String> {
        None
    }

    /// Returns the request headers
    fn http_headers(&self) -> Option<&HeaderMap> {
        None
    }

    /// Returns the response headers
    fn response_headers(&self) -> Option<&HeaderMap> {
        None
    }

    /// Returns the user agent string
    fn user_agent(&self) -> Option<String> {
        self.http_headers()
            .and_then(|h| h.get("user-agent"))
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    }

    /// Returns the request scheme (http/https)
    fn url_scheme(&self) -> String {
        "https".to_string() // Lambda is always HTTPS
    }

    /// Returns the server address
    fn server_address(&self) -> Option<String> {
        self.http_headers()
            .and_then(|headers| headers.get("host"))
            .and_then(|v| v.to_str().ok())
            .map(|host| host.split(':').next().unwrap_or(host).to_string())
    }

    fn server_port(&self) -> Option<u16> {
        Some(443) // Lambda is always HTTPS
    }

    /// Sets OpenTelemetry semantic convention attributes on the current span
    fn set_otel_http_attributes(
        &self,
        span: &Span,
        route_pattern: &str,
        lambda_context: &lambda_runtime::Context,
    ) {
        let span_name = format!("{} {}", self.http_method(), route_pattern);
        span.record("otel.name", &span_name);
        span.record("otel.kind", "SERVER");

        // HTTP request attributes
        span.set_attribute("http.request.method", self.http_method());
        span.set_attribute("http.route", route_pattern.to_string());

        // Capture configured request headers
        if let Some(headers) = self.http_headers() {
            let captured = CAPTURED_REQUEST_HEADERS.read().unwrap();
            if !captured.is_empty() {
                for name in captured.iter() {
                    let values = headers.get_all(name).iter().collect::<Vec<_>>();
                    if !values.is_empty() {
                        let header_values: Vec<String> = values
                            .iter()
                            .filter_map(|v| v.to_str().ok())
                            .map(String::from)
                            .collect();
                        span.set_attribute(
                            format!("http.request.header.{}", name),
                            header_values.join(","),
                        );
                    }
                }
            }
        }

        // Capture configured response headers
        if let Some(headers) = self.response_headers() {
            let captured = CAPTURED_RESPONSE_HEADERS.read().unwrap();
            if !captured.is_empty() {
                for name in captured.iter() {
                    let values = headers.get_all(name).iter().collect::<Vec<_>>();
                    if !values.is_empty() {
                        let header_values: Vec<String> = values
                            .iter()
                            .filter_map(|v| v.to_str().ok())
                            .map(String::from)
                            .collect();
                        span.set_attribute(
                            format!("http.response.header.{}", name),
                            header_values.join(","),
                        );
                    }
                }
            }
        }

        // URL attributes
        span.set_attribute("url.path", self.path().unwrap_or_else(|| "/".to_string()));
        span.set_attribute("url.scheme", self.url_scheme());
        if let Some(query) = self.url_query() {
            span.set_attribute("url.query", query);
        }

        // Server attributes
        if let Some(addr) = self.server_address() {
            span.set_attribute("server.address", addr);
        }
        if let Some(port) = self.server_port() {
            span.set_attribute("server.port", port as i64);
        }

        // Client attributes
        if let Some(addr) = self.client_address() {
            span.set_attribute("client.address", addr);
        }
        if let Some(agent) = self.user_agent() {
            span.set_attribute("user_agent.original", agent);
        }

        // Network attributes
        span.set_attribute("network.protocol.name", "http");
        span.set_attribute("network.protocol.version", "1.1");

        // Lambda-specific attributes
        span.set_attribute("faas.invocation_id", lambda_context.request_id.to_string());
        if let Some(account_id) = lambda_context.invoked_function_arn.split(':').nth(4) {
            span.set_attribute("cloud.account.id", account_id.to_string());
        }
        span.set_attribute(
            "aws.lambda.invoked_arn",
            lambda_context.invoked_function_arn.to_string(),
        );
    }
}

impl RoutableHttpEvent for ApiGatewayV2httpRequest {
    fn path(&self) -> Option<String> {
        self.raw_path.clone()
    }

    fn http_method(&self) -> String {
        self.request_context.http.method.to_string()
    }

    fn url_query(&self) -> Option<String> {
        self.raw_query_string.clone()
    }

    fn client_address(&self) -> Option<String> {
        self.request_context.http.source_ip.clone()
    }

    fn http_headers(&self) -> Option<&HeaderMap> {
        Some(&self.headers)
    }
}

impl RoutableHttpEvent for ApiGatewayProxyRequest {
    fn path(&self) -> Option<String> {
        self.path.clone()
    }

    fn http_method(&self) -> String {
        self.http_method.to_string()
    }

    fn route(&self) -> Option<String> {
        self.resource.clone()
    }

    fn path_parameters(&self) -> Option<&HashMap<String, String>> {
        Some(&self.path_parameters)
    }

    fn url_query(&self) -> Option<String> {
        if self.query_string_parameters.is_empty() {
            None
        } else {
            Some(
                self.query_string_parameters
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&"),
            )
        }
    }

    fn client_address(&self) -> Option<String> {
        self.request_context.identity.source_ip.clone()
    }

    fn http_headers(&self) -> Option<&HeaderMap> {
        Some(&self.headers)
    }
}

impl RoutableHttpEvent for AlbTargetGroupRequest {
    fn path(&self) -> Option<String> {
        self.path.clone()
    }

    fn http_method(&self) -> String {
        self.http_method.to_string()
    }

    fn url_query(&self) -> Option<String> {
        if self.query_string_parameters.is_empty() {
            None
        } else {
            Some(
                self.query_string_parameters
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&"),
            )
        }
    }

    fn client_address(&self) -> Option<String> {
        self.headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|ips| {
                ips.split(',')
                    .next() // ALB puts the original client IP first
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
    }

    fn http_headers(&self) -> Option<&HeaderMap> {
        Some(&self.headers)
    }
}

impl RoutableHttpEvent for ApiGatewayWebsocketProxyRequest {
    fn path(&self) -> Option<String> {
        self.path.clone()
    }

    fn http_method(&self) -> String {
        self.http_method
            .clone()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "GET".to_string())
    }

    fn url_query(&self) -> Option<String> {
        if self.query_string_parameters.is_empty() {
            None
        } else {
            Some(
                self.query_string_parameters
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&"),
            )
        }
    }

    fn client_address(&self) -> Option<String> {
        self.request_context.identity.source_ip.clone()
    }

    fn http_headers(&self) -> Option<&HeaderMap> {
        Some(&self.headers)
    }
}
