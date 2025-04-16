"""Constants for the lambda-otel-lite package.

This file centralizes all constants to ensure consistency across the codebase
and provide a single source of truth for configuration parameters.
"""

import logging

# Create a logger
logger = logging.getLogger(__name__)


class EnvVars:
    """Environment variable names for configuration.

    These environment variables control the behavior of the lambda-otel-lite package.
    """

    # Span processor configuration
    PROCESSOR_MODE = "LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE"
    QUEUE_SIZE = "LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE"
    BATCH_SIZE = "LAMBDA_SPAN_PROCESSOR_BATCH_SIZE"

    # OTLP Stdout Span Exporter configuration
    COMPRESSION_LEVEL = "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL"

    # Service name configuration
    SERVICE_NAME = "OTEL_SERVICE_NAME"

    # Resource attributes
    RESOURCE_ATTRIBUTES = "OTEL_RESOURCE_ATTRIBUTES"

    # Propagator configuration
    OTEL_PROPAGATORS = "OTEL_PROPAGATORS"


class Defaults:
    """Default values for configuration parameters.

    These values are used when the corresponding environment variables are not set.
    """

    QUEUE_SIZE = 2048
    BATCH_SIZE = 512
    COMPRESSION_LEVEL = 6
    SERVICE_NAME = "unknown_service"
    PROCESSOR_MODE = "sync"


class ResourceAttributes:
    """Resource attribute keys used in the Lambda resource.

    These keys are used to add resource attributes to the telemetry data.
    """

    PROCESSOR_MODE = "lambda_otel_lite.extension.span_processor_mode"
    QUEUE_SIZE = "lambda_otel_lite.lambda_span_processor.queue_size"
    BATCH_SIZE = "lambda_otel_lite.lambda_span_processor.batch_size"
    COMPRESSION_LEVEL = "lambda_otel_lite.otlp_stdout_span_exporter.compression_level"
