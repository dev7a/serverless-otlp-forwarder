//! Attribute extraction for OpenTelemetry spans in AWS Lambda functions.
//!
//! This module provides functionality for extracting OpenTelemetry span attributes from AWS Lambda
//! events. It includes:
//! - Built-in support for common AWS event types (API Gateway, ALB)
//! - Extensible trait system for custom event types
//! - Automatic W3C Trace Context propagation
//! - Support for span links and custom attributes
//!
//! # Architecture
//!
//! The module uses a trait-based approach for attribute extraction:
//!
//! 1. **Event Processing**: Each supported event type implements the `SpanAttributesExtractor` trait
//! 2. **Attribute Collection**: Standard attributes are collected based on event type
//! 3. **Context Propagation**: W3C Trace Context headers are automatically extracted
//! 4. **Custom Attributes**: Additional attributes can be added through custom implementations
//!
//! # Automatic Attributes
//!
//! The module automatically extracts and sets several types of attributes:
//!
//! ## Resource Attributes
//! - `cloud.provider`: Set to "aws"
//! - `cloud.region`: From AWS_REGION
//! - `faas.name`: From AWS_LAMBDA_FUNCTION_NAME
//! - `faas.version`: From AWS_LAMBDA_FUNCTION_VERSION
//! - `faas.instance`: From AWS_LAMBDA_LOG_STREAM_NAME
//! - `faas.max_memory`: From AWS_LAMBDA_FUNCTION_MEMORY_SIZE
//! - `service.name`: From OTEL_SERVICE_NAME or function name
//!
//! ## Span Attributes
//! - `faas.coldstart`: True only on first invocation
//! - `faas.invocation_id`: From Lambda request ID
//! - `cloud.account.id`: From function ARN
//! - `cloud.resource_id`: Complete function ARN
//! - `otel.kind`: "SERVER" by default
//! - `otel.status_code`/`message`: From response processing
//!
//! ## HTTP Attributes (for supported event types)
//! - `faas.trigger`: Set to "http" for API/ALB events
//! - `http.status_code`: From response
//! - `http.route`: Route key or resource path
//! - `http.method`: HTTP method
//! - `url.path`: Request path
//! - `url.query`: Query parameters if present
//! - `url.scheme`: Protocol (https)
//! - `network.protocol.version`: HTTP version
//! - `client.address`: Client IP address
//! - `user_agent.original`: User agent string
//! - `server.address`: Server hostname
//!
//! # Built-in Support
//!
//! The following AWS event types are supported out of the box:
//! - API Gateway v1/v2 (HTTP API and REST API)
//! - Application Load Balancer
//!
//! Each implementation follows OpenTelemetry semantic conventions for HTTP spans:
//! - `http.request.method`: The HTTP method (e.g., "GET", "POST")
//! - `url.path`: The request path
//! - `url.query`: The query string (if present)
//! - `url.scheme`: The protocol scheme ("https" for API Gateway, configurable for ALB)
//! - `network.protocol.version`: The HTTP protocol version
//! - `http.route`: The route pattern or resource path
//! - `client.address`: The client's IP address
//! - `user_agent.original`: The user agent string
//! - `server.address`: The server's domain name or host
//!
//! # Performance Considerations
//!
//! - Attribute extraction is done lazily when spans are created
//! - String allocations are minimized where possible
//! - Header extraction filters invalid UTF-8 values
//!
use aws_lambda_events::event::alb::AlbTargetGroupRequest;
use aws_lambda_events::event::apigw::{ApiGatewayProxyRequest, ApiGatewayV2httpRequest};
use bon::Builder;
use lambda_runtime::Context;
use opentelemetry::trace::{Link, Status};
use opentelemetry::Value;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fmt::{self, Display};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use urlencoding;

/// Common trigger types for Lambda functions.
///
/// These variants follow OpenTelemetry semantic conventions:
/// - `Datasource`: Database triggers
/// - `Http`: HTTP/API triggers
/// - `PubSub`: Message/event triggers
/// - `Timer`: Schedule/cron triggers
/// - `Other`: Fallback for unknown types
///
/// Custom trigger types can be used for more specific cases by using
/// the string value directly in SpanAttributes.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerType {
    /// Database trigger
    Datasource,
    /// HTTP/API trigger
    Http,
    /// Message/event trigger
    PubSub,
    /// Schedule/cron trigger
    Timer,
    /// Other/unknown trigger
    Other,
}

impl Display for TriggerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerType::Datasource => write!(f, "datasource"),
            TriggerType::Http => write!(f, "http"),
            TriggerType::PubSub => write!(f, "pubsub"),
            TriggerType::Timer => write!(f, "timer"),
            TriggerType::Other => write!(f, "other"),
        }
    }
}

impl Default for TriggerType {
    fn default() -> Self {
        Self::Other
    }
}

/// Data extracted from a Lambda event for span creation.
///
/// This struct contains all the information needed to create and configure an OpenTelemetry span,
/// including custom attributes, span kind, links, and context propagation headers.
///
/// # Span Kind
///
/// The `kind` field accepts standard OpenTelemetry span kinds:
/// - "SERVER" (default): Inbound request handling
/// - "CLIENT": Outbound calls
/// - "PRODUCER": Message/event production
/// - "CONSUMER": Message/event consumption
/// - "INTERNAL": Internal operations
///
/// # Span Name
///
/// For HTTP spans, the `span_name` is automatically generated from the HTTP method and route:
/// - API Gateway V2: "GET /users/{id}" (with $default mapped to "/")
/// - API Gateway V1: "POST /orders"
/// - ALB: "PUT /items/123"
///
/// # Attributes
///
/// Standard HTTP attributes following OpenTelemetry semantic conventions:
/// - `http.request.method`: The HTTP method
/// - `url.path`: The request path
/// - `url.query`: The query string (if present)
/// - `url.scheme`: The protocol scheme
/// - `network.protocol.version`: The HTTP protocol version
/// - `http.route`: The route pattern or resource path
/// - `client.address`: The client's IP address
/// - `user_agent.original`: The user agent string
/// - `server.address`: The server's domain name or host
///
/// # Context Propagation
///
/// The `carrier` field supports W3C Trace Context headers:
/// - `traceparent`: Contains trace ID, span ID, and trace flags
/// - `tracestate`: Vendor-specific trace information
///
/// # Examples
///
/// Basic usage with custom attributes:
///
/// ```no_run
/// use lambda_otel_lite::SpanAttributes;
/// use std::collections::HashMap;
/// use opentelemetry::Value;
///
/// let mut attributes = HashMap::new();
/// attributes.insert("custom.field".to_string(), Value::String("value".into()));
///
/// let span_attrs = SpanAttributes::builder()
///     .attributes(attributes)
///     .build();
/// ```
///
#[derive(Builder)]
pub struct SpanAttributes {
    /// Optional span kind (defaults to SERVER if not provided)
    /// Valid values: "SERVER", "CLIENT", "PRODUCER", "CONSUMER", "INTERNAL"
    pub kind: Option<String>,

    /// Optional span name.
    /// For HTTP spans, this should be "{http.method} {http.route}"
    /// Example: "GET /users/:id"
    pub span_name: Option<String>,

    /// Custom attributes to add to the span.
    /// Follow OpenTelemetry semantic conventions for naming:
    /// <https://opentelemetry.io/docs/specs/semconv/>
    #[builder(default)]
    pub attributes: HashMap<String, Value>,

    /// Optional span links for connecting related traces.
    /// Useful for batch processing or joining multiple workflows.
    #[builder(default)]
    pub links: Vec<Link>,

    /// Optional carrier headers for context propagation (W3C Trace Context format).
    /// Common headers:
    /// - traceparent: Contains trace ID, span ID, and trace flags
    /// - tracestate: Vendor-specific trace information
    pub carrier: Option<HashMap<String, String>>,

    /// The type of trigger for this Lambda invocation.
    /// Common values: "datasource", "http", "pubsub", "timer", "other"
    /// Custom values can be used for more specific triggers.
    #[builder(default = TriggerType::Other.to_string())]
    pub trigger: String,
}

impl Default for SpanAttributes {
    fn default() -> Self {
        Self {
            kind: None,
            span_name: None,
            attributes: HashMap::new(),
            links: Vec::new(),
            carrier: None,
            trigger: TriggerType::Other.to_string(),
        }
    }
}

/// Extract status code from response if it's an HTTP response.
///
/// This function attempts to extract an HTTP status code from a response Value.
/// It looks for a top-level "statusCode" field and returns its value if present
/// and valid.
pub fn get_status_code(response: &JsonValue) -> Option<i64> {
    response
        .as_object()
        .and_then(|obj| obj.get("statusCode"))
        .and_then(|v| v.as_i64())
}

/// Set response attributes on the span based on the response value.
///
/// This function extracts and sets response-related attributes on the span,
/// including status code and error status for HTTP responses.
pub fn set_response_attributes(span: &Span, response: &JsonValue) {
    if let Some(status_code) = get_status_code(response) {
        span.set_attribute("http.status_code", status_code.to_string());

        // Set span status based on status code
        if status_code >= 500 {
            span.set_status(Status::error(format!("HTTP {} response", status_code)));
        } else {
            span.set_status(Status::Ok);
        }
        span.set_attribute("http.response.status_code", status_code.to_string());
    }
}

/// Set common attributes on the span based on the Lambda context.
///
/// This function sets standard Lambda-related attributes on the span using
/// information from the Lambda context.
pub fn set_common_attributes(span: &Span, context: &Context, is_cold_start: bool) {
    // Set basic attributes
    span.set_attribute("faas.invocation_id", context.request_id.to_string());
    span.set_attribute(
        "cloud.resource_id",
        context.invoked_function_arn.to_string(),
    );
    if is_cold_start {
        span.set_attribute("faas.coldstart", true);
    }

    // Extract and set AWS account ID
    if let Some(account_id) = context.invoked_function_arn.split(':').nth(4) {
        span.set_attribute("cloud.account.id", account_id.to_string());
    }

    // Set AWS region if available
    if let Some(region) = context.invoked_function_arn.split(':').nth(3) {
        span.set_attribute("cloud.region", region.to_string());
    }

    // Set function name and version
    span.set_attribute(
        "faas.name",
        std::env::var("AWS_LAMBDA_FUNCTION_NAME").unwrap_or_default(),
    );
    span.set_attribute(
        "faas.version",
        std::env::var("AWS_LAMBDA_FUNCTION_VERSION").unwrap_or_default(),
    );
}

/// Trait for types that can provide span attributes.
///
/// This trait enables automatic extraction of OpenTelemetry span attributes from event types.
/// The tracing layer automatically detects and uses implementations of this trait when
/// processing Lambda events.
///
/// # Implementation Guidelines
///
/// When implementing this trait, follow these best practices:
///
/// 1. **Attribute Naming**:
///    - Follow [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/specs/semconv/)
///    - Use lowercase with dots for namespacing (e.g., "http.method")
///    - Keep names concise but descriptive
///
/// 2. **Span Kind**:
///    - Set appropriate kind based on event type:
///      - "SERVER" for inbound requests (default)
///      - "CONSUMER" for event/message processing
///      - "CLIENT" for outbound calls
///
/// 3. **Context Propagation**:
///    - Extract W3C Trace Context headers if available
///    - Handle both `traceparent` and `tracestate`
///    - Validate header values when possible
///
/// 4. **Performance**:
///    - Minimize string allocations
///    - Avoid unnecessary cloning
///    - Filter out invalid or unnecessary headers
///
/// # Examples
///
/// Basic implementation for a custom event:
///
/// ```no_run
/// use lambda_otel_lite::{SpanAttributes, SpanAttributesExtractor};
/// use std::collections::HashMap;
/// use opentelemetry::Value;
///
/// struct CustomEvent {
///     operation: String,
///     trace_parent: Option<String>,
/// }
///
/// impl SpanAttributesExtractor for CustomEvent {
///     fn extract_span_attributes(&self) -> SpanAttributes {
///         let mut attributes = HashMap::new();
///         attributes.insert("operation".to_string(), Value::String(self.operation.clone().into()));
///
///         // Add trace context if available
///         let mut carrier = HashMap::new();
///         if let Some(header) = &self.trace_parent {
///             carrier.insert("traceparent".to_string(), header.clone());
///         }
///
///         SpanAttributes::builder()
///             .attributes(attributes)
///             .carrier(carrier)
///             .build()
///     }
/// }
/// ```
pub trait SpanAttributesExtractor {
    /// Extract span attributes from this type.
    ///
    /// This method should extract any relevant information from the implementing type
    /// that should be included in the OpenTelemetry span. This includes:
    /// - Custom attributes describing the event
    /// - Span kind if different from SERVER
    /// - Headers for context propagation
    /// - Links to related traces
    ///
    /// # Returns
    ///
    /// Returns a `SpanAttributes` instance containing all extracted information.
    /// If extraction fails in any way, it should return a default instance rather
    /// than failing.
    fn extract_span_attributes(&self) -> SpanAttributes;
}

/// Implementation for API Gateway V2 HTTP API events.
///
/// Extracts standard HTTP attributes following OpenTelemetry semantic conventions:
/// - `http.request.method`: The HTTP method
/// - `url.path`: The request path (from raw_path)
/// - `url.query`: The query string if present (from raw_query_string)
/// - `url.scheme`: The protocol scheme (always "https" for API Gateway)
/// - `network.protocol.version`: The HTTP protocol version
/// - `http.route`: The API Gateway route key (e.g. "$default" or "GET /users/{id}")
/// - `client.address`: The client's IP address (from source_ip)
/// - `user_agent.original`: The user agent header
/// - `server.address`: The domain name
///
/// Also extracts W3C Trace Context headers for distributed tracing.
impl SpanAttributesExtractor for ApiGatewayV2httpRequest {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let mut attributes = HashMap::new();
        let method = self.request_context.http.method.to_string();
        let path = self.raw_path.as_deref().unwrap_or("/");

        // Add HTTP attributes following OTel semantic conventions
        attributes.insert(
            "http.request.method".to_string(),
            Value::String(method.clone().into()),
        );

        // Use raw_path directly for url.path
        if let Some(raw_path) = &self.raw_path {
            attributes.insert(
                "url.path".to_string(),
                Value::String(raw_path.to_string().into()),
            );
        }

        // Use raw_query_string directly for url.query
        if let Some(query) = &self.raw_query_string {
            if !query.is_empty() {
                attributes.insert(
                    "url.query".to_string(),
                    Value::String(query.to_string().into()),
                );
            }
        }

        if let Some(protocol) = &self.request_context.http.protocol {
            let protocol_lower = protocol.to_lowercase();
            if protocol_lower.starts_with("http/") {
                attributes.insert(
                    "network.protocol.version".to_string(),
                    Value::String(
                        protocol_lower
                            .trim_start_matches("http/")
                            .to_string()
                            .into(),
                    ),
                );
            }
            attributes.insert(
                "url.scheme".to_string(),
                Value::String("https".to_string().into()),
            ); // API Gateway is always HTTPS
        }

        // Add route key as http.route
        if let Some(route_key) = &self.route_key {
            attributes.insert(
                "http.route".to_string(),
                Value::String(route_key.to_string().into()),
            );
        }

        // Extract headers for context propagation
        let carrier = self
            .headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();

        // Add source IP and user agent
        if let Some(source_ip) = &self.request_context.http.source_ip {
            attributes.insert(
                "client.address".to_string(),
                Value::String(source_ip.to_string().into()),
            );
        }
        if let Some(user_agent) = self.headers.get("user-agent").and_then(|h| h.to_str().ok()) {
            attributes.insert(
                "user_agent.original".to_string(),
                Value::String(user_agent.to_string().into()),
            );
        }

        // Add domain name if available
        if let Some(domain_name) = &self.request_context.domain_name {
            attributes.insert(
                "server.address".to_string(),
                Value::String(domain_name.to_string().into()),
            );
        }

        SpanAttributes::builder()
            .attributes(attributes)
            .carrier(carrier)
            .span_name(format!("{} {}", method, path))
            .trigger(TriggerType::Http.to_string())
            .build()
    }
}

/// Implementation for API Gateway V1 REST API events.
///
/// Extracts standard HTTP attributes following OpenTelemetry semantic conventions:
/// - `http.request.method`: The HTTP method
/// - `url.path`: The request path
/// - `url.query`: The query string (constructed from multi_value_query_string_parameters)
/// - `url.scheme`: The protocol scheme (always "https" for API Gateway)
/// - `network.protocol.version`: The HTTP protocol version
/// - `http.route`: The API Gateway resource path
/// - `client.address`: The client's IP address (from identity.source_ip)
/// - `user_agent.original`: The user agent header
/// - `server.address`: The domain name
///
/// Also extracts W3C Trace Context headers for distributed tracing.
impl SpanAttributesExtractor for ApiGatewayProxyRequest {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let mut attributes = HashMap::new();
        let method = self.http_method.to_string();
        let route = self.resource.as_deref().unwrap_or("/");

        // Add HTTP attributes following OTel semantic conventions
        attributes.insert(
            "http.request.method".to_string(),
            Value::String(method.clone().into()),
        );

        // Use path directly
        if let Some(path) = &self.path {
            attributes.insert(
                "url.path".to_string(),
                Value::String(path.to_string().into()),
            );
        }

        // Use multi_value_query_string_parameters and format query string manually
        if !self.multi_value_query_string_parameters.is_empty() {
            let mut query_parts = Vec::new();
            for key in self
                .multi_value_query_string_parameters
                .iter()
                .map(|(k, _)| k)
            {
                if let Some(values) = self.multi_value_query_string_parameters.all(key) {
                    for value in values {
                        query_parts.push(format!(
                            "{}={}",
                            urlencoding::encode(key),
                            urlencoding::encode(value)
                        ));
                    }
                }
            }
            if !query_parts.is_empty() {
                let query = query_parts.join("&");
                attributes.insert("url.query".to_string(), Value::String(query.into()));
            }
        }

        if let Some(protocol) = &self.request_context.protocol {
            let protocol_lower = protocol.to_lowercase();
            if protocol_lower.starts_with("http/") {
                attributes.insert(
                    "network.protocol.version".to_string(),
                    Value::String(
                        protocol_lower
                            .trim_start_matches("http/")
                            .to_string()
                            .into(),
                    ),
                );
            }
            attributes.insert(
                "url.scheme".to_string(),
                Value::String("https".to_string().into()),
            ); // API Gateway is always HTTPS
        }

        // Add route
        attributes.insert(
            "http.route".to_string(),
            Value::String(route.to_string().into()),
        );

        // Extract headers for context propagation
        let carrier = self
            .headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();

        // Add source IP and user agent
        if let Some(source_ip) = &self.request_context.identity.source_ip {
            attributes.insert(
                "client.address".to_string(),
                Value::String(source_ip.to_string().into()),
            );
        }
        if let Some(user_agent) = self.headers.get("user-agent").and_then(|h| h.to_str().ok()) {
            attributes.insert(
                "user_agent.original".to_string(),
                Value::String(user_agent.to_string().into()),
            );
        }

        // Add domain name if available
        if let Some(domain_name) = &self.request_context.domain_name {
            attributes.insert(
                "server.address".to_string(),
                Value::String(domain_name.to_string().into()),
            );
        }

        SpanAttributes::builder()
            .attributes(attributes)
            .carrier(carrier)
            .span_name(format!("{} {}", method, route))
            .trigger(TriggerType::Http.to_string())
            .build()
    }
}

/// Implementation for Application Load Balancer target group events.
///
/// Extracts standard HTTP attributes following OpenTelemetry semantic conventions:
/// - `http.request.method`: The HTTP method
/// - `url.path`: The request path
/// - `url.query`: The query string (constructed from multi_value_query_string_parameters)
/// - `url.scheme`: The protocol scheme (defaults to "http")
/// - `network.protocol.version`: The HTTP protocol version (always "1.1" for ALB)
/// - `http.route`: The request path
/// - `client.address`: The client's IP address (from x-forwarded-for header)
/// - `user_agent.original`: The user agent header
/// - `server.address`: The host header
/// - `alb.target_group_arn`: The ARN of the target group
///
/// Also extracts W3C Trace Context headers for distributed tracing.
impl SpanAttributesExtractor for AlbTargetGroupRequest {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let mut attributes = HashMap::new();
        let method = self.http_method.to_string();
        let route = self.path.as_deref().unwrap_or("/");

        // Add HTTP attributes following OTel semantic conventions
        attributes.insert(
            "http.request.method".to_string(),
            Value::String(method.clone().into()),
        );

        // Use path directly
        if let Some(path) = &self.path {
            attributes.insert(
                "url.path".to_string(),
                Value::String(path.to_string().into()),
            );
        }

        // Use multi_value_query_string_parameters and format query string manually
        if !self.multi_value_query_string_parameters.is_empty() {
            let mut query_parts = Vec::new();
            for key in self
                .multi_value_query_string_parameters
                .iter()
                .map(|(k, _)| k)
            {
                if let Some(values) = self.multi_value_query_string_parameters.all(key) {
                    for value in values {
                        query_parts.push(format!(
                            "{}={}",
                            urlencoding::encode(key),
                            urlencoding::encode(value)
                        ));
                    }
                }
            }
            if !query_parts.is_empty() {
                let query = query_parts.join("&");
                attributes.insert("url.query".to_string(), Value::String(query.into()));
            }
        }

        // ALB can be HTTP or HTTPS, default to HTTP if not specified
        attributes.insert(
            "url.scheme".to_string(),
            Value::String("http".to_string().into()),
        );
        attributes.insert(
            "network.protocol.version".to_string(),
            Value::String("1.1".to_string().into()),
        ); // ALB uses HTTP/1.1

        // Add ALB specific attributes
        if let Some(target_group_arn) = &self.request_context.elb.target_group_arn {
            attributes.insert(
                "alb.target_group_arn".to_string(),
                Value::String(target_group_arn.to_string().into()),
            );
        }

        // Extract headers for context propagation
        let carrier = self
            .headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();

        // Add source IP and user agent
        if let Some(source_ip) = &self
            .headers
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
        {
            if let Some(client_ip) = source_ip.split(',').next() {
                attributes.insert(
                    "client.address".to_string(),
                    Value::String(client_ip.trim().to_string().into()),
                );
            }
        }
        if let Some(user_agent) = self.headers.get("user-agent").and_then(|h| h.to_str().ok()) {
            attributes.insert(
                "user_agent.original".to_string(),
                Value::String(user_agent.to_string().into()),
            );
        }

        // Add domain name if available
        if let Some(host) = self.headers.get("host").and_then(|h| h.to_str().ok()) {
            attributes.insert(
                "server.address".to_string(),
                Value::String(host.to_string().into()),
            );
        }

        SpanAttributes::builder()
            .attributes(attributes)
            .carrier(carrier)
            .span_name(format!("{} {}", method, route))
            .trigger(TriggerType::Http.to_string())
            .build()
    }
}

/// Default implementation for serde_json::Value.
///
/// This implementation provides a fallback for when the event type is not known
/// or when working with raw JSON data. It returns default attributes with
/// the trigger type set to "other".
/// If there's a headers field, it will be used to populate the carrier.
impl SpanAttributesExtractor for serde_json::Value {
    fn extract_span_attributes(&self) -> SpanAttributes {
        let carrier = self
            .get("headers")
            .and_then(|headers| headers.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k.to_string(), v.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        SpanAttributes::builder()
            .carrier(carrier)
            .trigger(TriggerType::Other.to_string())
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lambda_events::http::Method;

    #[test]
    fn test_trigger_types() {
        let attrs = SpanAttributes::default();
        assert_eq!(attrs.trigger, TriggerType::Other.to_string());

        let attrs = SpanAttributes::builder()
            .trigger(TriggerType::Http.to_string())
            .build();
        assert_eq!(attrs.trigger, TriggerType::Http.to_string());

        let attrs = SpanAttributes::builder().build();
        assert_eq!(attrs.trigger, TriggerType::Other.to_string());
    }

    #[test]
    fn test_apigw_v2_extraction() {
        let request = ApiGatewayV2httpRequest {
            raw_path: Some("/test".to_string()),
            route_key: Some("GET /test".to_string()),
            headers: aws_lambda_events::http::HeaderMap::new(),
            request_context: aws_lambda_events::apigw::ApiGatewayV2httpRequestContext {
                http: aws_lambda_events::apigw::ApiGatewayV2httpRequestContextHttpDescription {
                    method: Method::GET,
                    path: Some("/test".to_string()),
                    protocol: Some("HTTP/1.1".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let attrs = request.extract_span_attributes();

        assert_eq!(
            attrs.attributes.get("http.request.method"),
            Some(&Value::String("GET".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("url.path"),
            Some(&Value::String("/test".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("http.route"),
            Some(&Value::String("GET /test".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("url.scheme"),
            Some(&Value::String("https".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("network.protocol.version"),
            Some(&Value::String("1.1".to_string().into()))
        );
    }

    #[test]
    fn test_apigw_v1_extraction() {
        let request = ApiGatewayProxyRequest {
            path: Some("/test".to_string()),
            http_method: Method::GET,
            resource: Some("/test".to_string()),
            headers: aws_lambda_events::http::HeaderMap::new(),
            request_context: aws_lambda_events::apigw::ApiGatewayProxyRequestContext {
                protocol: Some("HTTP/1.1".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let attrs = request.extract_span_attributes();

        assert_eq!(
            attrs.attributes.get("http.request.method"),
            Some(&Value::String("GET".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("url.path"),
            Some(&Value::String("/test".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("http.route"),
            Some(&Value::String("/test".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("url.scheme"),
            Some(&Value::String("https".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("network.protocol.version"),
            Some(&Value::String("1.1".to_string().into()))
        );
    }

    #[test]
    fn test_alb_extraction() {
        let request = AlbTargetGroupRequest {
            path: Some("/test".to_string()),
            http_method: Method::GET,
            headers: aws_lambda_events::http::HeaderMap::new(),
            request_context: aws_lambda_events::alb::AlbTargetGroupRequestContext {
                elb: aws_lambda_events::alb::ElbContext {
                    target_group_arn: Some("arn:aws:elasticloadbalancing:...".to_string()),
                },
            },
            ..Default::default()
        };

        let attrs = request.extract_span_attributes();

        assert_eq!(
            attrs.attributes.get("http.request.method"),
            Some(&Value::String("GET".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("url.path"),
            Some(&Value::String("/test".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("url.scheme"),
            Some(&Value::String("http".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("network.protocol.version"),
            Some(&Value::String("1.1".to_string().into()))
        );
        assert_eq!(
            attrs.attributes.get("alb.target_group_arn"),
            Some(&Value::String(
                "arn:aws:elasticloadbalancing:...".to_string().into()
            ))
        );
    }
}
