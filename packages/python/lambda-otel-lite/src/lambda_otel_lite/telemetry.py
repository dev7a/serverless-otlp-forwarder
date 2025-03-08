"""
Telemetry initialization for lambda-otel-lite.

This module provides the initialization function for OpenTelemetry in AWS Lambda.
"""

import os
from collections.abc import Sequence

from opentelemetry import trace
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import SpanProcessor, TracerProvider
from otlp_stdout_span_exporter import OTLPStdoutSpanExporter

from . import ProcessorMode, __version__, processor_mode
from .extension import handler_complete_event, init_extension
from .logger import create_logger
from .processor import LambdaSpanProcessor

# Setup logging
logger = create_logger("telemetry")


class TelemetryCompletionHandler:
    """Handles coordination between the handler and extension for span flushing.

    This handler is responsible for ensuring that spans are properly exported before
    the Lambda function completes. It MUST be used to signal when spans should be exported.

    The behavior varies by processing mode:
    - Sync: Forces immediate export in the handler thread
    - Async: Signals the extension to export after the response is sent
    - Finalize: Defers to span processor (used with BatchSpanProcessor)
    """

    def __init__(self, tracer_provider: TracerProvider, mode: ProcessorMode):
        """Initialize the completion handler.

        Args:
            tracer_provider: The TracerProvider to use for tracing
            mode: The processor mode that determines export behavior
        """
        self._tracer_provider = tracer_provider
        self._mode = mode
        # Cache the tracer instance at construction time
        self._tracer = self._tracer_provider.get_tracer(
            __package__,
            __version__,
            schema_url=None,
            attributes={
                "library.language": "python",
                "library.type": "instrumentation",
                "library.runtime": "aws_lambda",
            },
        )

    @property
    def tracer_provider(self) -> TracerProvider:
        """Get the tracer provider."""
        return self._tracer_provider

    def get_tracer(self) -> trace.Tracer:
        """Get a tracer instance for creating spans.

        Returns a cached tracer instance configured with this package's instrumentation scope
        (name and version) and Lambda-specific attributes. The tracer is configured
        with the provider's settings and will automatically use the correct span processor
        based on the processing mode.

        The tracer is configured with instrumentation scope attributes that identify:
        - library.language: The implementation language (python)
        - library.type: The type of library (instrumentation)
        - library.runtime: The runtime environment (aws_lambda)

        These attributes are different from resource attributes:
        - Resource attributes describe the entity producing telemetry (the Lambda function)
        - Instrumentation scope attributes describe the library doing the instrumentation

        Returns:
            A tracer instance for creating spans
        """
        return self._tracer

    def complete(self) -> None:
        """Complete telemetry processing for the current invocation.

        This method must be called to ensure spans are exported. The behavior depends
        on the processing mode:

        - Sync mode: Blocks until spans are flushed. Any errors during flush are logged
          but do not affect the handler response.

        - Async mode: Schedules span export via the extension after the response is sent.
          This is non-blocking and optimizes perceived latency.

        - Finalize mode: No-op as export is handled by the span processor configuration
          (e.g., BatchSpanProcessor with custom export triggers).

        Multiple calls to this method are safe but have no additional effect.
        """
        if self._mode == ProcessorMode.SYNC:
            try:
                self._tracer_provider.force_flush()
            except Exception as e:
                logger.warn("Error flushing telemetry:", e)
        elif self._mode == ProcessorMode.ASYNC:
            # Signal the extension to export spans
            handler_complete_event.set()
        # In finalize mode, do nothing - handled by processor


def get_lambda_resource(custom_resource: Resource | None = None) -> Resource:
    """Create a Resource instance with AWS Lambda attributes and OTEL environment variables.

    This function combines AWS Lambda environment attributes with any OTEL resource attributes
    specified via environment variables (OTEL_RESOURCE_ATTRIBUTES and OTEL_SERVICE_NAME).

    Returns:
        Resource instance with AWS Lambda and OTEL environment attributes
    """
    # Start with Lambda attributes
    attributes: dict[str, str | int | float | bool] = {"cloud.provider": "aws"}

    def parse_numeric_env(key: str, env_var: str, default: str) -> None:
        """Parse numeric environment variable with default."""
        try:
            attributes[key] = int(os.environ.get(env_var, default))
        except ValueError:
            logger.warn(
                "Invalid numeric value for %s: %s", key, os.environ.get(env_var)
            )

    def parse_memory_value(key: str, value: str | None, default: str) -> None:
        """Parse memory value from MB to bytes."""
        try:
            attributes[key] = int(value or default) * 1024 * 1024  # Convert MB to bytes
        except ValueError:
            logger.warn("Invalid memory value for %s: %s", key, value)

    # Map environment variables to attribute names
    env_mappings = {
        "AWS_REGION": "cloud.region",
        "AWS_LAMBDA_FUNCTION_NAME": "faas.name",
        "AWS_LAMBDA_FUNCTION_VERSION": "faas.version",
        "AWS_LAMBDA_LOG_STREAM_NAME": "faas.instance",
        "AWS_LAMBDA_FUNCTION_MEMORY_SIZE": "faas.max_memory",
    }

    # Add attributes only if they exist in environment
    for env_var, attr_name in env_mappings.items():
        if value := os.environ.get(env_var):
            if attr_name == "faas.max_memory":
                parse_memory_value(attr_name, value, "128")
            else:
                attributes[attr_name] = value

    # Add service name (guaranteed to have a value)
    service_name = os.environ.get(
        "OTEL_SERVICE_NAME",
        os.environ.get("AWS_LAMBDA_FUNCTION_NAME", "unknown_service"),
    )
    attributes["service.name"] = service_name

    # Add telemetry configuration attributes
    attributes["lambda_otel_lite.extension.span_processor_mode"] = os.environ.get(
        "LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", "sync"
    )

    # Parse numeric configuration values
    parse_numeric_env(
        "lambda_otel_lite.lambda_span_processor.queue_size",
        "LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE",
        "2048",
    )
    parse_numeric_env(
        "lambda_otel_lite.lambda_span_processor.batch_size",
        "LAMBDA_SPAN_PROCESSOR_BATCH_SIZE",
        "512",
    )
    parse_numeric_env(
        "lambda_otel_lite.otlp_stdout_span_exporter.compression_level",
        "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL",
        "6",
    )

    # OTEL_RESOURCE_ATTRIBUTES are automatically parsed by the Resource create method
    # Create resource and merge with custom resource if provided
    resource = Resource(attributes)

    if custom_resource:
        # Merge in reverse order so custom resource takes precedence
        resource = resource.merge(custom_resource)

    final_resource = Resource.create().merge(resource)
    return final_resource


def init_telemetry(
    *,
    resource: Resource | None = None,
    span_processors: Sequence[SpanProcessor] | None = None,
) -> tuple[trace.Tracer, TelemetryCompletionHandler]:
    """Initialize OpenTelemetry with manual OTLP stdout configuration.

    This function provides a flexible way to initialize OpenTelemetry for AWS Lambda,
    with sensible defaults that work well in most cases but allowing customization
    where needed.

    Args:
        resource: Optional custom Resource. Defaults to Lambda resource detection.
        span_processors: Optional sequence of SpanProcessors. If None, a default LambdaSpanProcessor
            with OTLPStdoutSpanExporter will be used. If provided, these processors will be
            the only ones used, in the order provided.

    Returns:
        Tuple containing:
            - tracer: Tracer instance for manual instrumentation
            - completion_handler: Handler for managing telemetry lifecycle
    """
    # Setup resource
    resource = resource or get_lambda_resource()

    # Create tracer provider
    tracer_provider = TracerProvider(resource=resource)

    # Setup processors
    if span_processors is None:
        # Default case: Add LambdaSpanProcessor with OTLPStdoutSpanExporter
        tracer_provider.add_span_processor(
            LambdaSpanProcessor(
                OTLPStdoutSpanExporter(
                    gzip_level=int(
                        os.environ.get(
                            "OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL", "6"
                        )
                    )
                ),
                max_queue_size=int(
                    os.environ.get("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE", "2048")
                ),
                max_export_batch_size=int(
                    os.environ.get("LAMBDA_SPAN_PROCESSOR_BATCH_SIZE", "512")
                ),
            )
        )
    else:
        # Custom case: Add user-provided processors in order
        for processor in span_processors:
            tracer_provider.add_span_processor(processor)

    # Set as global tracer provider
    trace.set_tracer_provider(tracer_provider)

    # Get current mode and check extension status
    mode = processor_mode
    # Initialize extension for async and finalize modes
    if mode in [ProcessorMode.ASYNC, ProcessorMode.FINALIZE]:
        init_extension(mode, tracer_provider)

    # Create completion handler
    completion_handler = TelemetryCompletionHandler(tracer_provider, mode)

    # Return tracer and completion handler
    return completion_handler.get_tracer(), completion_handler
