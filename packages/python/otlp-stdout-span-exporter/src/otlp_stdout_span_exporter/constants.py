"""Constants for the otlp-stdout-span-exporter package.

This file centralizes all constants to ensure consistency across the codebase
and provide a single source of truth for configuration parameters.
"""


class EnvVars:
    """Environment variable names for configuration."""

    # OTLP Stdout Span Exporter configuration
    COMPRESSION_LEVEL = "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL"

    # Service name configuration
    SERVICE_NAME = "OTEL_SERVICE_NAME"
    AWS_LAMBDA_FUNCTION_NAME = "AWS_LAMBDA_FUNCTION_NAME"

    # Headers configuration
    OTLP_HEADERS = "OTEL_EXPORTER_OTLP_HEADERS"
    OTLP_TRACES_HEADERS = "OTEL_EXPORTER_OTLP_TRACES_HEADERS"


class Defaults:
    """Default values for configuration parameters."""

    COMPRESSION_LEVEL = 6
    SERVICE_NAME = "unknown-service"
    ENDPOINT = "http://localhost:4318/v1/traces"


class ResourceAttributes:
    """Resource attribute keys used in the Lambda resource."""

    COMPRESSION_LEVEL = "lambda_otel_lite.otlp_stdout_span_exporter.compression_level"
