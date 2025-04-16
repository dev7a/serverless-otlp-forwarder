"""
Telemetry initialization for lambda-otel-lite.

This module provides the initialization function for OpenTelemetry in AWS Lambda.
"""

# No need to import os directly as we'll use the config helpers
from collections.abc import Sequence

from opentelemetry import trace
from opentelemetry.propagate import set_global_textmap
from opentelemetry.propagators.composite import CompositePropagator
from opentelemetry.propagators.textmap import TextMapPropagator
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import SpanProcessor, TracerProvider
from opentelemetry.sdk.trace.id_generator import IdGenerator
from otlp_stdout_span_exporter import OTLPStdoutSpanExporter

from . import ProcessorMode, __version__
from .constants import Defaults, EnvVars
from .config import get_int_env
from .extension import handler_complete_event, init_extension
from .logger import create_logger
from .processor import LambdaSpanProcessor
from .propagation import setup_propagator
from .resource import get_lambda_resource

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


def init_telemetry(
    *,
    resource: Resource | None = None,
    span_processors: Sequence[SpanProcessor] | None = None,
    propagators: Sequence[TextMapPropagator] | None = None,
    id_generator: IdGenerator | None = None,
    processor_mode: ProcessorMode | None = None,
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
        propagators: Optional sequence of TextMapPropagators. If None, the default
            global propagators (W3C TraceContext and Baggage) will be used. If provided,
            these propagators will be combined into a composite propagator and set as the
            global propagator.
        id_generator: Optional ID generator. If None, the default W3C-compatible ID generator
            will be used. Set to an XRayIdGenerator instance to use X-Ray compatible IDs.
        processor_mode: Optional processor mode to control how spans are processed and exported.
            Environment variable LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE takes precedence if set.
            If neither environment variable nor this option is set, defaults to 'sync'.

    Returns:
        Tuple containing:
            - tracer: Tracer instance for manual instrumentation
            - completion_handler: Handler for managing telemetry lifecycle
    """
    # Setup resource
    resource = resource or get_lambda_resource()

    # Setup propagators
    if propagators is not None:
        # If custom propagators are provided, use them
        composite_propagator = CompositePropagator(propagators)
        set_global_textmap(composite_propagator)
        logger.debug(
            f"Set custom propagators: {[type(p).__name__ for p in propagators]}"
        )
    else:
        # Otherwise, use the default propagator setup based on environment variables
        setup_propagator()

    # Create tracer provider
    tracer_provider = TracerProvider(resource=resource, id_generator=id_generator)

    # Setup processors with environment variables having precedence
    if span_processors is None:
        # Get configuration values with proper precedence using helper functions

        # Get compression level (0-9)
        compression_level = get_int_env(
            EnvVars.COMPRESSION_LEVEL,
            None,
            Defaults.COMPRESSION_LEVEL,
            lambda value: 0 <= value <= 9,
        )

        # Get queue size
        queue_size = get_int_env(
            EnvVars.QUEUE_SIZE, None, Defaults.QUEUE_SIZE, lambda value: value > 0
        )

        # Get batch size
        batch_size = get_int_env(
            EnvVars.BATCH_SIZE, None, Defaults.BATCH_SIZE, lambda value: value > 0
        )

        # Default case: Add LambdaSpanProcessor with OTLPStdoutSpanExporter
        tracer_provider.add_span_processor(
            LambdaSpanProcessor(
                OTLPStdoutSpanExporter(gzip_level=compression_level),
                max_queue_size=queue_size,
                max_export_batch_size=batch_size,
            )
        )
    else:
        # Custom case: Add user-provided processors in order
        for processor in span_processors:
            tracer_provider.add_span_processor(processor)

    # Set as global tracer provider
    trace.set_tracer_provider(tracer_provider)

    # Resolve processor mode with proper precedence: env var > config > default
    mode = ProcessorMode.resolve(
        config_mode=processor_mode, env_var=EnvVars.PROCESSOR_MODE
    )
    # Initialize extension for async and finalize modes
    if mode in [ProcessorMode.ASYNC, ProcessorMode.FINALIZE]:
        init_extension(mode, tracer_provider)

    # Create completion handler
    completion_handler = TelemetryCompletionHandler(tracer_provider, mode)

    # Return tracer and completion handler
    return completion_handler.get_tracer(), completion_handler
