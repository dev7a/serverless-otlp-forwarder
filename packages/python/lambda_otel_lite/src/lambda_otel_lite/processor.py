"""
Core processor implementation for lambda-otel-lite.

This module provides the LambdaSpanProcessor implementation.
"""

from queue import Full, Queue

from opentelemetry.context import Context
from opentelemetry.sdk.trace import ReadableSpan, SpanProcessor
from opentelemetry.sdk.trace.export import SpanExporter
from opentelemetry.trace import Span

from .logger import create_logger

# Setup logging
logger = create_logger("processor")


class LambdaSpanProcessor(SpanProcessor):
    """Lambda-optimized SpanProcessor implementation.

    Queues spans for processing by the extension thread, providing efficient
    handling for AWS Lambda's execution model without the overhead of
    worker threads or complex batching logic.
    """

    def __init__(
        self,
        span_exporter: SpanExporter,
        max_queue_size: int = 2048,
        max_export_batch_size: int = 512,
    ):
        """Initialize the LambdaSpanProcessor.

        Args:
            span_exporter: The SpanExporter to use for exporting spans
            max_queue_size: Maximum number of spans to queue (default: 2048)
            max_export_batch_size: Maximum number of spans to export in each batch (default: 512)
        """
        self.span_exporter = span_exporter
        self.max_queue_size = max_queue_size
        self.max_export_batch_size = max_export_batch_size
        self.span_queue: Queue[ReadableSpan] = Queue(maxsize=self.max_queue_size)
        self._shutdown = False
        self._dropped_spans_count = 0

    def on_start(self, span: Span, parent_context: Context | None = None) -> None:
        """Called when a span starts. No-op in this implementation."""
        pass

    def on_end(self, span: ReadableSpan) -> None:
        """Called when a span ends. Queues the span for export if sampled."""
        if not span.context.trace_flags.sampled or self._shutdown:
            return

        try:
            self.span_queue.put_nowait(span)
            if self._dropped_spans_count > 0:
                logger.warn(
                    "Recovered from dropping spans: %d spans were dropped",
                    self._dropped_spans_count,
                )
                self._dropped_spans_count = 0
        except Full:
            self._dropped_spans_count += 1
            if self._dropped_spans_count == 1 or self._dropped_spans_count % 100 == 0:
                logger.warn(
                    "Dropping spans: %d spans dropped because buffer is full",
                    self._dropped_spans_count,
                )
        except Exception as ex:
            logger.error("Unexpected error while queueing span:", ex)

    def process_spans(self) -> None:
        """Process all queued spans in batches.

        Called by the extension thread to process and export spans.
        """
        if self._shutdown:
            return

        while not self.span_queue.empty():
            spans_to_export: list[ReadableSpan] = []
            # Get up to max_export_batch_size spans
            for _ in range(min(self.max_export_batch_size, self.span_queue.qsize())):
                try:
                    spans_to_export.append(self.span_queue.get_nowait())
                except Exception:
                    break

            if spans_to_export:
                logger.debug("Processing batch of %d spans", len(spans_to_export))
                try:
                    self.span_exporter.export(spans_to_export)
                except Exception as ex:
                    logger.error("Exception while exporting spans:", ex)

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
