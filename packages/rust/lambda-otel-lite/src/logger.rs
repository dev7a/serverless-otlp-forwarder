//! Logging utilities for lambda-otel-lite.
//!
//! This module provides a simple logging interface with level filtering and prefixing.
//!
//! # Example
//! ```
//! use lambda_otel_lite::logger::Logger;
//!
//! // Create a logger for your module
//! let logger = Logger::new("my_module");
//! logger.info("Starting module");
//! ```
//!
//! # Static Logger Example
//! ```
//! use lambda_otel_lite::logger::Logger;
//!
//! // Define a static logger for your module
//! static LOGGER: Logger = Logger::const_new("my_module");
//!
//! // Use it directly
//! LOGGER.info("Starting module");
//! ```

use std::env;
use std::sync::OnceLock;

// Default log level used for const initialization
const DEFAULT_LOG_LEVEL: &str = "info";

// Global log level cache
static LOG_LEVEL: OnceLock<&'static str> = OnceLock::new();

/// Get the log level from environment variables
fn get_log_level() -> &'static str {
    LOG_LEVEL.get_or_init(|| {
        let level = env::var("AWS_LAMBDA_LOG_LEVEL")
            .or_else(|_| env::var("LOG_LEVEL"))
            .unwrap_or_else(|_| "info".to_string())
            .to_lowercase();

        match level.as_str() {
            "none" | "error" | "warn" | "info" | "debug" => Box::leak(level.into_boxed_str()),
            _ => "info",
        }
    })
}

/// Logger with level filtering and consistent prefixing
#[derive(Clone)]
pub struct Logger {
    prefix: &'static str,
    log_level_fn: fn() -> &'static str,
}

impl Logger {
    /// Create a new logger with the given prefix
    pub fn new(prefix: impl Into<String>) -> Self {
        // Convert the prefix to a &'static str
        let static_prefix = Box::leak(prefix.into().into_boxed_str());

        Self {
            prefix: static_prefix,
            log_level_fn: get_log_level,
        }
    }

    /// Create a new logger with the given prefix that can be used in const contexts
    pub const fn const_new(prefix: &'static str) -> Self {
        Self {
            prefix,
            log_level_fn: || DEFAULT_LOG_LEVEL,
        }
    }

    // Get the current log level
    fn log_level(&self) -> &'static str {
        (self.log_level_fn)()
    }

    fn should_log(&self, level: &str) -> bool {
        match self.log_level() {
            "none" => false,
            "error" => level == "error",
            "warn" => matches!(level, "error" | "warn"),
            "info" => matches!(level, "error" | "warn" | "info"),
            "debug" => matches!(level, "error" | "warn" | "info" | "debug"),
            _ => matches!(level, "error" | "warn" | "info"),
        }
    }

    fn format_message(&self, message: &str) -> String {
        format!("[{}] {}", self.prefix, message)
    }

    /// Log a debug message
    pub fn debug(&self, message: impl AsRef<str>) {
        if self.should_log("debug") {
            println!("{}", self.format_message(message.as_ref()));
        }
    }

    /// Log an info message
    pub fn info(&self, message: impl AsRef<str>) {
        if self.should_log("info") {
            println!("{}", self.format_message(message.as_ref()));
        }
    }

    /// Log a warning message
    pub fn warn(&self, message: impl AsRef<str>) {
        if self.should_log("warn") {
            eprintln!("{}", self.format_message(message.as_ref()));
        }
    }

    /// Log an error message
    pub fn error(&self, message: impl AsRef<str>) {
        if self.should_log("error") {
            eprintln!("{}", self.format_message(message.as_ref()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn logger_with_level(level: &'static str) -> Logger {
        match level {
            "none" => Logger {
                prefix: "test",
                log_level_fn: || "none",
            },
            "error" => Logger {
                prefix: "test",
                log_level_fn: || "error",
            },
            "warn" => Logger {
                prefix: "test",
                log_level_fn: || "warn",
            },
            "info" => Logger {
                prefix: "test",
                log_level_fn: || "info",
            },
            "debug" => Logger {
                prefix: "test",
                log_level_fn: || "debug",
            },
            _ => unreachable!("unsupported level"),
        }
    }

    #[test]
    #[serial]
    fn test_log_levels() {
        let none = logger_with_level("none");
        assert!(!none.should_log("error"));

        let error = logger_with_level("error");
        assert!(error.should_log("error"));
        assert!(!error.should_log("warn"));

        let warn = logger_with_level("warn");
        assert!(warn.should_log("error"));
        assert!(warn.should_log("warn"));
        assert!(!warn.should_log("info"));

        let info = logger_with_level("info");
        assert!(info.should_log("error"));
        assert!(info.should_log("warn"));
        assert!(info.should_log("info"));
        assert!(!info.should_log("debug"));
        assert!(!info.should_log("invalid"));

        let debug = logger_with_level("debug");
        assert!(debug.should_log("error"));
        assert!(debug.should_log("warn"));
        assert!(debug.should_log("info"));
        assert!(debug.should_log("debug"));
    }

    #[test]
    #[serial]
    fn test_format_message() {
        let logger = Logger::new("test");

        assert_eq!(logger.format_message("hello"), "[test] hello");
    }

    #[test]
    fn test_const_logger_uses_default_info_level() {
        static LOGGER: Logger = Logger::const_new("const-test");

        assert_eq!(LOGGER.log_level(), "info");
        assert!(LOGGER.should_log("info"));
        assert!(!LOGGER.should_log("debug"));
        assert_eq!(LOGGER.format_message("hello"), "[const-test] hello");
    }
}
