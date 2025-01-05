"""
Core processor implementation for lambda-otel-lite.

This module provides the LambdaSpanProcessor implementation.
"""

import logging
import os
from queue import Queue

from opentelemetry.context import Context, attach, detach, set_value
from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.sdk.trace.export import SpanExporter, SpanProcessor
from opentelemetry.trace import Span

from .extension import debug_timing

# Setup logging
logger = logging.getLogger(__name__)
logger.setLevel(os.getenv("AWS_LAMBDA_LOG_LEVEL", os.getenv("LOG_LEVEL", "INFO")).upper())


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
