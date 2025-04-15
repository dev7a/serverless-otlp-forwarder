"""Constants for the otlp-stdout-span-exporter package.

This file centralizes all constants to ensure consistency across the codebase
and provide a single source of truth for configuration parameters.
"""

from enum import Enum


class EnvVars:
    """Environment variable names for configuration."""

    # OTLP Stdout Span Exporter configuration
    COMPRESSION_LEVEL = "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL"
    LOG_LEVEL = "OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL"
    OUTPUT_TYPE = "OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE"

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
    OUTPUT_TYPE = "stdout"
    PIPE_PATH = "/tmp/otlp-stdout-span-exporter.pipe"


class ResourceAttributes:
    """Resource attribute keys used in the Lambda resource."""

    COMPRESSION_LEVEL = "lambda_otel_lite.otlp_stdout_span_exporter.compression_level"


class LogLevel(str, Enum):
    """Log level for the exported spans."""

    DEBUG = "DEBUG"
    INFO = "INFO"
    WARN = "WARN"
    ERROR = "ERROR"


class OutputType(str, Enum):
    """Output type for the exporter."""

    STDOUT = "stdout"
    PIPE = "pipe"
