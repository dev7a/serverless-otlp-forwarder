//! Structured event recording for OpenTelemetry spans in AWS Lambda functions.
//!
//! This module provides functionality for recording structured events within OpenTelemetry spans.
//! Events are queryable data points that provide additional context and business logic markers
//! within the execution of Lambda functions.
//!
//! # Key Features
//!
//! - **Dual API**: Both function-based and builder-based interfaces
//! - **Level-based filtering**: Events can be filtered by severity level
//! - **Structured attributes**: Attach custom key-value pairs to events
//! - **OpenTelemetry compliance**: Uses standard OpenTelemetry event semantics
//! - **Lambda-optimized**: Designed for AWS Lambda execution patterns
//! - **Performance-conscious**: Early filtering to minimize overhead
//!
//! # Use Cases
//!
//! Events are particularly useful for:
//! - **Business logic markers**: Track significant application state changes
//! - **Audit trails**: Record user actions and system decisions
//! - **Debugging context**: Add structured data for troubleshooting
//! - **Performance insights**: Mark important execution milestones
//! - **Compliance logging**: Record security and regulatory events
//!
//! # API Styles
//!
//! ## Function-based API (Direct)
//!
//! ```rust
//! use lambda_otel_lite::events::{record_event, EventLevel};
//! use opentelemetry::KeyValue;
//!
//! record_event(
//!     EventLevel::Info,
//!     "User logged in",
//!     vec![
//!         KeyValue::new("user_id", "123"),
//!         KeyValue::new("method", "oauth"),
//!     ],
//!     None, // timestamp
//! );
//! ```
//!
//! ## Builder-based API (Ergonomic)
//!
//! ```rust
//! use lambda_otel_lite::events::{event, EventLevel};
//!
//! // Simple event
//! event()
//!     .level(EventLevel::Info)
//!     .message("User logged in")
//!     .call();
//!
//! // Event with individual attributes
//! event()
//!     .level(EventLevel::Info)
//!     .message("User logged in")
//!     .attribute("user_id", "123")
//!     .attribute("method", "oauth")
//!     .call();
//! ```
//!
//! # Environment Configuration
//!
//! The event level can be controlled via the `AWS_LAMBDA_LOG_LEVEL` environment variable
//! (with fallback to `LOG_LEVEL`), same as the internal logging system:
//! - `TRACE`: All events (most verbose)
//! - `DEBUG`: Debug, Info, Warn, Error events
//! - `INFO`: Info, Warn, Error events (default)
//! - `WARN`: Warn, Error events only
//! - `ERROR`: Error events only

use crate::constants::defaults;
use bon::builder;
use opentelemetry::KeyValue;
use std::sync::OnceLock;
use std::{env, time::SystemTime};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Event severity levels for filtering and categorization.
///
/// These levels follow the same semantics as standard logging levels
/// and are used both for filtering events and setting their severity in OpenTelemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventLevel {
    /// Trace-level events (most verbose)
    Trace = 1,
    /// Debug-level events
    Debug = 5,
    /// Informational events (default)
    Info = 9,
    /// Warning events
    Warn = 13,
    /// Error events (least verbose)
    Error = 17,
}

impl From<EventLevel> for tracing::Level {
    fn from(level: EventLevel) -> Self {
        match level {
            EventLevel::Trace => tracing::Level::TRACE,
            EventLevel::Debug => tracing::Level::DEBUG,
            EventLevel::Info => tracing::Level::INFO,
            EventLevel::Warn => tracing::Level::WARN,
            EventLevel::Error => tracing::Level::ERROR,
        }
    }
}

impl From<EventLevel> for u8 {
    fn from(level: EventLevel) -> Self {
        level as u8
    }
}

/// Convert event level to standard text representation
fn level_text(level: EventLevel) -> &'static str {
    match level {
        EventLevel::Trace => "TRACE",
        EventLevel::Debug => "DEBUG",
        EventLevel::Info => "INFO",
        EventLevel::Warn => "WARN",
        EventLevel::Error => "ERROR",
    }
}

/// Cached minimum event level for performance
static MIN_LEVEL: OnceLock<EventLevel> = OnceLock::new();

/// Get the minimum event level from environment configuration
fn get_min_level() -> EventLevel {
    *MIN_LEVEL.get_or_init(|| get_min_level_with_env_vars("AWS_LAMBDA_LOG_LEVEL", "LOG_LEVEL"))
}

/// Helper function for testing that allows custom environment variable names
fn get_min_level_with_env_vars(primary_var: &str, fallback_var: &str) -> EventLevel {
    // Use the same environment variable logic as the logger module for consistency
    let level = env::var(primary_var)
        .or_else(|_| env::var(fallback_var))
        .unwrap_or_else(|_| defaults::EVENT_LEVEL.to_string())
        .to_uppercase();

    match level.as_str() {
        "ERROR" => EventLevel::Error,
        "WARN" => EventLevel::Warn,
        "INFO" => EventLevel::Info,
        "DEBUG" => EventLevel::Debug,
        "TRACE" => EventLevel::Trace,
        _ => EventLevel::Info, // Default fallback
    }
}

/// Record a structured event within the current OpenTelemetry span (function-based API).
///
/// This is the direct function interface for recording events. For a more ergonomic
/// builder-based API, see the [`event()`] function.
///
/// # Arguments
///
/// * `level` - The severity level of the event
/// * `message` - Human-readable description of the event
/// * `attributes` - Additional structured attributes as key-value pairs
/// * `timestamp` - Optional custom timestamp (uses current time if None)
///
/// # Examples
///
/// ```rust
/// use lambda_otel_lite::events::{record_event, EventLevel};
/// use opentelemetry::KeyValue;
///
/// // Simple event
/// record_event(
///     EventLevel::Info,
///     "User logged in",
///     vec![],
///     None,
/// );
///
/// // Event with attributes and custom timestamp
/// record_event(
///     EventLevel::Warn,
///     "Rate limit approaching",
///     vec![
///         KeyValue::new("user_id", "123"),
///         KeyValue::new("requests_remaining", 10),
///     ],
///     Some(std::time::SystemTime::now()),
/// );
/// ```
pub fn record_event(
    level: EventLevel,
    message: impl AsRef<str>,
    attributes: Vec<KeyValue>,
    timestamp: Option<SystemTime>,
) {
    record_event_impl(level, message.as_ref(), attributes, timestamp);
}

/// Create an event builder for ergonomic event construction (builder-based API).
///
/// This returns a builder that allows you to configure the event through method chaining
/// before calling `.call()` to record it. For a direct function interface, see [`record_event()`].
///
/// # Examples
///
/// ```rust
/// use lambda_otel_lite::events::{event, EventLevel};
///
/// // Basic event
/// event()
///     .level(EventLevel::Info)
///     .message("User action completed")
///     .call();
///
/// // Event with individual attributes
/// event()
///     .level(EventLevel::Info)
///     .message("User logged in")
///     .attribute("user_id", "123")
///     .attribute("count", 42)
///     .attribute("is_admin", true)
///     .call();
/// ```
#[builder]
pub fn event(
    #[builder(field)] attributes: Vec<KeyValue>,

    #[builder(default = EventLevel::Info)] level: EventLevel,

    #[builder(into, default = "")] message: String,

    timestamp: Option<SystemTime>,
) {
    record_event_impl(level, &message, attributes, timestamp);
}

/// Internal implementation that both APIs call
fn record_event_impl(
    level: EventLevel,
    message: &str,
    attributes: Vec<KeyValue>,
    timestamp: Option<SystemTime>,
) {
    // Early return if event level is below threshold
    if level < get_min_level() {
        return;
    }

    // Get the current span and check if it's valid
    let span = tracing::Span::current();
    if span.is_disabled() {
        return;
    }

    // Create the event attributes with OpenTelemetry semantic conventions
    let mut event_attributes = Vec::with_capacity(attributes.len() + 3);
    event_attributes.extend_from_slice(&[
        KeyValue::new("event.severity_text", level_text(level)),
        KeyValue::new("event.severity_number", u8::from(level) as i64),
    ]);

    // Add the message as event.body if provided
    if !message.is_empty() {
        event_attributes.push(KeyValue::new("event.body", message.to_string()));
    }

    // Add custom attributes
    event_attributes.extend(attributes);

    // Add the event to the span
    if let Some(ts) = timestamp {
        span.add_event_with_timestamp("event", ts, event_attributes);
    } else {
        span.add_event("event", event_attributes);
    }
}

/// Custom methods for the event builder to support individual attribute calls
impl<S: event_builder::State> EventBuilder<S> {
    /// Add a single attribute to the event.
    ///
    /// This method accepts any value that can be converted to an OpenTelemetry `Value`,
    /// including strings, numbers, booleans, and other supported types.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use lambda_otel_lite::events::{event, EventLevel};
    ///
    /// event()
    ///     .level(EventLevel::Info)
    ///     .message("User action")
    ///     .attribute("user_id", "123")        // String
    ///     .attribute("count", 42)             // Integer
    ///     .attribute("score", 98.5)           // Float
    ///     .attribute("is_admin", true)        // Boolean
    ///     .call();
    /// ```
    pub fn attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<opentelemetry::Value>,
    ) -> Self {
        self.attributes
            .push(KeyValue::new(key.into(), value.into()));
        self
    }

    /// Add multiple attributes at once from a vector.
    ///
    /// This is useful when you have a collection of attributes to add.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use lambda_otel_lite::events::{event, EventLevel};
    /// use opentelemetry::KeyValue;
    ///
    /// event()
    ///     .level(EventLevel::Info)
    ///     .message("Batch operation")
    ///     .add_attributes(vec![
    ///         KeyValue::new("batch_id", "batch_123"),
    ///         KeyValue::new("item_count", 100),
    ///     ])
    ///     .call();
    /// ```
    pub fn add_attributes(mut self, attrs: Vec<KeyValue>) -> Self {
        self.attributes.extend(attrs);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sealed_test::prelude::*;
    use std::env;

    #[test]
    fn test_event_level_ordering() {
        assert!(EventLevel::Trace < EventLevel::Debug);
        assert!(EventLevel::Debug < EventLevel::Info);
        assert!(EventLevel::Info < EventLevel::Warn);
        assert!(EventLevel::Warn < EventLevel::Error);
    }

    #[test]
    fn test_event_level_to_tracing_level() {
        assert_eq!(
            tracing::Level::TRACE,
            tracing::Level::from(EventLevel::Trace)
        );
        assert_eq!(
            tracing::Level::DEBUG,
            tracing::Level::from(EventLevel::Debug)
        );
        assert_eq!(tracing::Level::INFO, tracing::Level::from(EventLevel::Info));
        assert_eq!(tracing::Level::WARN, tracing::Level::from(EventLevel::Warn));
        assert_eq!(
            tracing::Level::ERROR,
            tracing::Level::from(EventLevel::Error)
        );
    }

    #[test]
    fn test_event_level_to_u8() {
        assert_eq!(1u8, u8::from(EventLevel::Trace));
        assert_eq!(5u8, u8::from(EventLevel::Debug));
        assert_eq!(9u8, u8::from(EventLevel::Info));
        assert_eq!(13u8, u8::from(EventLevel::Warn));
        assert_eq!(17u8, u8::from(EventLevel::Error));
    }

    #[test]
    fn test_level_text() {
        assert_eq!("TRACE", level_text(EventLevel::Trace));
        assert_eq!("DEBUG", level_text(EventLevel::Debug));
        assert_eq!("INFO", level_text(EventLevel::Info));
        assert_eq!("WARN", level_text(EventLevel::Warn));
        assert_eq!("ERROR", level_text(EventLevel::Error));
    }

    #[sealed_test]
    fn test_get_min_level_aws_lambda_log_level() {
        env::set_var("AWS_LAMBDA_LOG_LEVEL", "DEBUG");
        env::remove_var("LOG_LEVEL");

        let level = get_min_level_with_env_vars("AWS_LAMBDA_LOG_LEVEL", "LOG_LEVEL");
        assert_eq!(level, EventLevel::Debug);
    }

    #[sealed_test]
    fn test_get_min_level_log_level_fallback() {
        env::remove_var("AWS_LAMBDA_LOG_LEVEL");
        env::set_var("LOG_LEVEL", "WARN");

        let level = get_min_level_with_env_vars("AWS_LAMBDA_LOG_LEVEL", "LOG_LEVEL");
        assert_eq!(level, EventLevel::Warn);
    }

    #[sealed_test]
    fn test_get_min_level_default() {
        env::remove_var("AWS_LAMBDA_LOG_LEVEL");
        env::remove_var("LOG_LEVEL");

        let level = get_min_level_with_env_vars("AWS_LAMBDA_LOG_LEVEL", "LOG_LEVEL");
        assert_eq!(level, EventLevel::Info);
    }

    #[sealed_test]
    fn test_get_min_level_invalid() {
        env::set_var("AWS_LAMBDA_LOG_LEVEL", "INVALID");
        env::remove_var("LOG_LEVEL");

        let level = get_min_level_with_env_vars("AWS_LAMBDA_LOG_LEVEL", "LOG_LEVEL");
        assert_eq!(level, EventLevel::Info);
    }

    #[sealed_test]
    fn test_get_min_level_case_insensitive() {
        env::set_var("AWS_LAMBDA_LOG_LEVEL", "error");
        env::remove_var("LOG_LEVEL");

        let level = get_min_level_with_env_vars("AWS_LAMBDA_LOG_LEVEL", "LOG_LEVEL");
        assert_eq!(level, EventLevel::Error);
    }

    #[test]
    fn test_record_event_function_api() {
        // Test the direct function API
        record_event(
            EventLevel::Info,
            "Test event",
            vec![KeyValue::new("test_key", "test_value")],
            None,
        );
    }

    #[test]
    fn test_event_builder_api_basic() {
        // Test the basic builder API compiles and works
        event()
            .level(EventLevel::Info)
            .message("test message")
            .call();
    }

    #[test]
    fn test_event_builder_individual_attributes() {
        // Test the individual attribute API
        event()
            .level(EventLevel::Info)
            .message("test message")
            .attribute("user_id", "123")
            .attribute("count", 42)
            .attribute("is_admin", true)
            .attribute("score", 98.5)
            .call();
    }

    #[test]
    fn test_event_builder_mixed_attributes() {
        // Test mixing individual attributes with batch attributes
        use opentelemetry::KeyValue;

        event()
            .level(EventLevel::Warn)
            .message("mixed attributes test")
            .attribute("single_attr", "value")
            .add_attributes(vec![
                KeyValue::new("batch1", "value1"),
                KeyValue::new("batch2", "value2"),
            ])
            .attribute("another_single", 999)
            .call();
    }

    #[test]
    fn test_both_apis_work() {
        // Test that both APIs can be used together
        record_event(EventLevel::Info, "Function API", vec![], None);
        event()
            .level(EventLevel::Info)
            .message("Builder API")
            .call();
    }
}
