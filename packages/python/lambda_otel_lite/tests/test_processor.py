"""Tests for the LambdaSpanProcessor implementation."""

from unittest.mock import Mock, patch

import pytest
from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.sdk.trace.export import SpanExporter
from opentelemetry.trace import SpanKind, Status, StatusCode

from lambda_otel_lite.processor import LambdaSpanProcessor


@pytest.fixture
def mock_exporter():
    """Create a mock span exporter."""
    exporter = Mock(spec=SpanExporter)
    exporter.export.return_value = None
    return exporter


@pytest.fixture
def mock_span():
    """Create a mock ReadableSpan."""
    span = Mock(spec=ReadableSpan)
    context = Mock()
    trace_flags = Mock()
    trace_flags.sampled = True
    context.trace_flags = trace_flags
    span.context = context
    span.name = "test_span"
    span.kind = SpanKind.INTERNAL
    span.status = Status(StatusCode.OK)
    span.attributes = {}
    return span


def test_processor_init():
    """Test processor initialization."""
    exporter = Mock(spec=SpanExporter)
    processor = LambdaSpanProcessor(exporter)

    assert processor.span_exporter == exporter
    assert processor.span_queue.maxsize == 2048
    assert not processor._shutdown


def test_processor_queue_span(mock_exporter, mock_span):
    """Test queuing a span."""
    processor = LambdaSpanProcessor(mock_exporter)
    processor.on_end(mock_span)

    assert not processor.span_queue.empty()
    assert processor.span_queue.get_nowait() == mock_span


def test_processor_queue_full(mock_exporter, mock_span):
    """Test handling a full queue."""
    processor = LambdaSpanProcessor(mock_exporter, max_queue_size=1)

    # Fill the queue
    processor.on_end(mock_span)

    # Try to add another span
    with patch("lambda_otel_lite.processor.logger") as mock_logger:
        processor.on_end(mock_span)
        mock_logger.exception.assert_called_once()


def test_processor_process_spans(mock_exporter, mock_span):
    """Test processing spans."""
    processor = LambdaSpanProcessor(mock_exporter)
    processor.on_end(mock_span)

    processor.process_spans()

    mock_exporter.export.assert_called_once()
    assert mock_exporter.export.call_args[0][0] == [mock_span]
    assert processor.span_queue.empty()


def test_processor_shutdown(mock_exporter, mock_span):
    """Test processor shutdown."""
    processor = LambdaSpanProcessor(mock_exporter)

    # Add a span to the queue
    processor.on_end(mock_span)

    # Shutdown should process spans and call export
    processor.shutdown()

    assert processor._shutdown
    mock_exporter.export.assert_called_once_with([mock_span])
    mock_exporter.shutdown.assert_called_once()


def test_processor_force_flush(mock_exporter, mock_span):
    """Test force flush operation."""
    processor = LambdaSpanProcessor(mock_exporter)
    processor.on_end(mock_span)

    result = processor.force_flush()

    assert result is True
    mock_exporter.export.assert_called_once()
    assert processor.span_queue.empty()


def test_processor_force_flush_after_shutdown(mock_exporter):
    """Test force flush after shutdown."""
    processor = LambdaSpanProcessor(mock_exporter)

    # Add a span and shutdown
    mock_span = Mock(spec=ReadableSpan)
    context = Mock()
    trace_flags = Mock()
    trace_flags.sampled = True
    context.trace_flags = trace_flags
    mock_span.context = context

    processor.on_end(mock_span)
    assert processor.span_queue.qsize() == 1

    # Shutdown should process spans
    processor.shutdown()
    assert processor.span_queue.qsize() == 0
    mock_exporter.export.assert_called_once_with([mock_span])

    # Try force flush after shutdown
    result = processor.force_flush()
    assert result is False

    # No additional export calls after shutdown
    mock_exporter.export.assert_called_once()
