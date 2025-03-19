//! Constants for the otlp-stdout-span-exporter package.
//!
//! This file centralizes all constants to ensure consistency across the codebase
//! and provide a single source of truth for configuration parameters.

/// Environment variable names for configuration.
pub mod env_vars {
    /// GZIP compression level for OTLP stdout span exporter (0-9).
    pub const COMPRESSION_LEVEL: &str = "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL";

    /// Service name for telemetry.
    pub const SERVICE_NAME: &str = "OTEL_SERVICE_NAME";

    /// AWS Lambda function name (used as fallback service name).
    pub const AWS_LAMBDA_FUNCTION_NAME: &str = "AWS_LAMBDA_FUNCTION_NAME";

    /// Global headers for OTLP export.
    pub const OTLP_HEADERS: &str = "OTEL_EXPORTER_OTLP_HEADERS";

    /// Trace-specific headers (takes precedence over OTLP_HEADERS).
    pub const OTLP_TRACES_HEADERS: &str = "OTEL_EXPORTER_OTLP_TRACES_HEADERS";
}

/// Default values for configuration parameters.
pub mod defaults {
    /// Default GZIP compression level (0-9).
    pub const COMPRESSION_LEVEL: u8 = 6;

    /// Default service name if not provided.
    pub const SERVICE_NAME: &str = "unknown-service";

    /// Default endpoint for OTLP export.
    pub const ENDPOINT: &str = "http://localhost:4318/v1/traces";
}

/// Resource attribute keys used in the Lambda resource.
pub mod resource_attributes {
    /// Resource attribute key for compression level.
    pub const COMPRESSION_LEVEL: &str =
        "lambda_otel_lite.otlp_stdout_span_exporter.compression_level";
}
