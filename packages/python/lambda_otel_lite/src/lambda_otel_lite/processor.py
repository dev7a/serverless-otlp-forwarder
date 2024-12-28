"""
Core processor implementation for lambda-otel-lite.

This module provides the LambdaSpanProcessor and telemetry initialization functions.
"""

import logging
import os
from contextlib import contextmanager
from queue import Queue
from typing import Any

from opentelemetry import trace
from opentelemetry.context import Context, attach, detach, set_value
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import ReadableSpan, TracerProvider
from opentelemetry.sdk.trace.export import SpanExporter, SpanProcessor
from opentelemetry.trace import Span, SpanKind
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource

from . import ProcessorMode
from .extension import _handler_complete, _handler_ready, debug_timing, init_extension

# Setup logging
logger = logging.getLogger(__name__)
logger.setLevel(os.getenv("AWS_LAMBDA_LOG_LEVEL", os.getenv("LOG_LEVEL", "INFO")).upper())

# Global state
_tracer_provider: TracerProvider | None = None
_is_cold_start: bool = True
_processor_mode: ProcessorMode = ProcessorMode.from_env(
    "LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", ProcessorMode.SYNC
)


class LambdaSpanProcessor(SpanProcessor):
    """Lambda-optimized SpanProcessor implementation.

    Queues spans for processing by the extension thread, providing efficient
    handling for AWS Lambda's execution model without the overhead of
    worker threads or complex batching logic.
    """

    # Key for suppressing instrumentation during span export
    _SUPPRESS_INSTRUMENTATION_KEY = "suppress_instrumentation"

    def __init__(self, span_exporter: SpanExporter, max_queue_size: int = 2048):
        """Initialize the LambdaSpanProcessor.

        Args:
            span_exporter: The SpanExporter to use for exporting spans
            max_queue_size: Maximum number of spans to queue (default: 2048)
        """
        self.span_exporter = span_exporter
        self.span_queue: Queue[ReadableSpan] = Queue(maxsize=max_queue_size)
        self._shutdown = False

    def on_start(self, span: Span, parent_context: Context | None = None) -> None:
        """Called when a span starts. No-op in this implementation."""
        pass

    def on_end(self, span: ReadableSpan) -> None:
        """Called when a span ends. Queues the span for export if sampled."""
        if not span.context.trace_flags.sampled or self._shutdown:
            return

        try:
            self.span_queue.put_nowait(span)
        except Exception as ex:
            logger.exception("Failed to queue span: %s", ex)

    def process_spans(self) -> None:
        """Process all queued spans.

        Called by the extension thread to process and export spans.
        """
        if self._shutdown:
            return

        spans_to_export: list[ReadableSpan] = []
        while not self.span_queue.empty():
            try:
                spans_to_export.append(self.span_queue.get_nowait())
            except Exception:
                break

        if spans_to_export:
            logger.debug("Processing %d spans", len(spans_to_export))
            token = attach(set_value(self._SUPPRESS_INSTRUMENTATION_KEY, True))
            try:
                with debug_timing(logger, "span_export"):
                    self.span_exporter.export(spans_to_export)
            except Exception as ex:
                logger.exception("Exception while exporting spans: %s", ex)
            finally:
                detach(token)

    def shutdown(self) -> None:
        """Shuts down the processor and exports any remaining spans."""
        self.process_spans()  # Process any remaining spans
        self.span_exporter.shutdown()
        self._shutdown = True

    def force_flush(self, timeout_millis: int = 0) -> bool:
        """Forces a flush of any pending spans."""
        if self._shutdown:
            return False

        self.process_spans()
        return True


def init_telemetry(
    name: str,
    resource: Resource | None = None,
    span_processor: SpanProcessor | None = None,
    exporter: SpanExporter | None = None,
) -> tuple[trace.Tracer, TracerProvider]:
    """Initialize OpenTelemetry with manual OTLP stdout configuration.

    This function provides a flexible way to initialize OpenTelemetry for AWS Lambda,
    with sensible defaults that work well in most cases but allowing customization
    where needed.

    Args:
        name: Name for the tracer (e.g., 'my-service', 'payment-processor')
        resource: Optional custom Resource. Defaults to Lambda resource detection
        span_processor: Optional custom SpanProcessor. Defaults to LambdaSpanProcessor
        exporter: Optional custom SpanExporter. Defaults to OTLPSpanExporter with stdout

    Returns:
        tuple: (tracer, provider) instances
    """
    global _tracer_provider

    # Setup resource
    resource = resource or get_lambda_resource()
    _tracer_provider = TracerProvider(resource=resource)

    # Setup exporter and processor
    if span_processor is None:
        exporter = exporter or OTLPSpanExporter(session=StdoutAdapter().get_session())
        span_processor = LambdaSpanProcessor(
            exporter, max_queue_size=int(os.getenv("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE", "2048"))
        )

    _tracer_provider.add_span_processor(span_processor)
    trace.set_tracer_provider(_tracer_provider)

    # Initialize extension for async and finalize modes
    if _processor_mode in [ProcessorMode.ASYNC, ProcessorMode.FINALIZE]:
        init_extension(_processor_mode, _tracer_provider)

    return trace.get_tracer(name), _tracer_provider


@contextmanager
def traced_handler(
    tracer: trace.Tracer,
    tracer_provider: TracerProvider,
    name: str,
    kind: SpanKind = SpanKind.INTERNAL,
    attributes: dict[str, Any] | None = None,
    links: list[Any] | None = None,
    start_time: int | None = None,
    record_exception: bool = True,
    set_status_on_exception: bool = True,
    end_on_exit: bool = True,
) -> None:
    """Context manager for tracing Lambda handlers.

    Example:
        ```python
        def handler(event, context):
            with traced_handler(tracer, provider, "my-handler",
                              attributes={"event.type": event["type"]}):
                # The span is available as the current span in the context
                # No need to access it directly
                process_event(event)
        ```

    Args:
        tracer: OpenTelemetry tracer instance
        tracer_provider: OpenTelemetry tracer provider instance
        name: Name of the span
        kind: Kind of span (default: INTERNAL)
        attributes: Optional span attributes
        links: Optional span links
        start_time: Optional span start time
        record_exception: Whether to record exceptions (default: True)
        set_status_on_exception: Whether to set status on exceptions (default: True)
        end_on_exit: Whether to end span on exit (default: True)
    """
    global _is_cold_start
    try:
        # Only wait for handler ready in async mode
        if _processor_mode == ProcessorMode.ASYNC:
            _handler_ready.wait()
            _handler_ready.clear()

        with tracer.start_as_current_span(
            name,
            kind=kind,
            attributes=attributes,
            links=links,
            start_time=start_time,
            record_exception=record_exception,
            set_status_on_exception=set_status_on_exception,
            end_on_exit=end_on_exit,
        ) as span:
            if _is_cold_start:
                span.set_attribute("faas.cold_start", True)
                _is_cold_start = False
            yield
    finally:
        if _processor_mode == ProcessorMode.SYNC:
            # In sync mode, force flush before returning
            with debug_timing(logger, "force_flush call"):
                tracer_provider.force_flush()
        elif _processor_mode == ProcessorMode.ASYNC:
            # In async mode, signal completion to extension
            _handler_complete.set()
        # In finalize mode, do nothing - let the processor handle flushing
