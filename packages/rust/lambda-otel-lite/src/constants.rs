//! Constants for the lambda-otel-lite package.
//!
//! This file centralizes all constants to ensure consistency across the codebase
//! and provide a single source of truth for configuration parameters.

/// Environment variable names for configuration.
pub mod env_vars {
    /// Mode for the Lambda Extension span processor (sync or async).
    pub const PROCESSOR_MODE: &str = "LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE";

    /// Maximum number of spans to queue in the LambdaSpanProcessor.
    pub const QUEUE_SIZE: &str = "LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE";

    /// Maximum batch size for span export.
    pub const BATCH_SIZE: &str = "LAMBDA_SPAN_PROCESSOR_BATCH_SIZE";

    /// Compression level for OTLP stdout span exporter.
    pub const COMPRESSION_LEVEL: &str = "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL";

    /// Service name for telemetry.
    pub const SERVICE_NAME: &str = "OTEL_SERVICE_NAME";

    /// Resource attributes in KEY=VALUE,KEY2=VALUE2 format.
    pub const RESOURCE_ATTRIBUTES: &str = "OTEL_RESOURCE_ATTRIBUTES";

    /// AWS Lambda function name (used as fallback service name).
    pub const AWS_LAMBDA_FUNCTION_NAME: &str = "AWS_LAMBDA_FUNCTION_NAME";

    /// Comma-separated list of context propagators to use.
    /// Valid values: tracecontext, xray, xray-lambda, none
    pub const PROPAGATORS: &str = "OTEL_PROPAGATORS";

    /// Controls whether to enable the fmt layer for logging regardless of code settings.
    /// Set to "true" to force enable logging output.
    pub const ENABLE_FMT_LAYER: &str = "LAMBDA_TRACING_ENABLE_FMT_LAYER";
}

/// Default values for configuration parameters.
pub mod defaults {
    /// Default maximum queue size for LambdaSpanProcessor.
    pub const QUEUE_SIZE: usize = 2048;

    /// Default maximum batch size for LambdaSpanProcessor.
    pub const BATCH_SIZE: usize = 512;

    /// Default compression level for OTLP stdout span exporter.
    pub const COMPRESSION_LEVEL: u8 = 6;

    /// Default service name if not provided.
    pub const SERVICE_NAME: &str = "unknown_service";

    /// Default processor mode.
    pub const PROCESSOR_MODE: &str = "sync";

    /// Default value for enabling fmt layer from environment.
    pub const ENABLE_FMT_LAYER: bool = false;
}

/// Resource attribute keys used in the Lambda resource.
pub mod resource_attributes {
    /// Resource attribute key for processor mode.
    pub const PROCESSOR_MODE: &str = "lambda_otel_lite.extension.span_processor_mode";

    /// Resource attribute key for queue size.
    pub const QUEUE_SIZE: &str = "lambda_otel_lite.lambda_span_processor.queue_size";

    /// Resource attribute key for batch size.
    pub const BATCH_SIZE: &str = "lambda_otel_lite.lambda_span_processor.batch_size";

    /// Resource attribute key for compression level.
    pub const COMPRESSION_LEVEL: &str =
        "lambda_otel_lite.otlp_stdout_span_exporter.compression_level";
}
