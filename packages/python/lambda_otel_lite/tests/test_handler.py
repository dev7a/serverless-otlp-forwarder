"""Tests for the traced_handler implementation."""

from unittest.mock import Mock, patch

import pytest
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.trace import SpanKind

from lambda_otel_lite.processor import ProcessorMode, traced_handler


@pytest.fixture
def mock_tracer():
    """Create a mock tracer."""
    tracer = Mock(spec=trace.Tracer)
    span = Mock()
    context_manager = Mock()
    context_manager.__enter__ = Mock(return_value=span)
    context_manager.__exit__ = Mock(return_value=None)
    tracer.start_as_current_span.return_value = context_manager
    return tracer


@pytest.fixture
def mock_provider():
    """Create a mock tracer provider."""
    provider = Mock(spec=TracerProvider)
    provider.force_flush.return_value = None
    return provider


def test_traced_handler_sync_mode(mock_tracer, mock_provider):
    """Test traced_handler in sync mode."""
    with patch("lambda_otel_lite.processor._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler"):
            pass

        # Verify span creation
        mock_tracer.start_as_current_span.assert_called_once_with(
            "test_handler",
            kind=SpanKind.INTERNAL,
            attributes=None,
            links=None,
            start_time=None,
            record_exception=True,
            set_status_on_exception=True,
            end_on_exit=True,
        )

        # Verify force flush in sync mode
        mock_provider.force_flush.assert_called_once()


def test_traced_handler_async_mode(mock_tracer, mock_provider):
    """Test traced_handler in async mode."""
    with (
        patch("lambda_otel_lite.processor._processor_mode", ProcessorMode.ASYNC),
        patch("lambda_otel_lite.processor._handler_ready") as mock_ready,
        patch("lambda_otel_lite.processor._handler_complete") as mock_complete,
    ):
        with traced_handler(mock_tracer, mock_provider, "test_handler"):
            # Verify handler ready wait
            mock_ready.wait.assert_called_once()
            mock_ready.clear.assert_called_once()
            mock_complete.set.assert_not_called()

        # Verify completion signal
        mock_complete.set.assert_called_once()
        # No force flush in async mode
        mock_provider.force_flush.assert_not_called()


def test_traced_handler_finalize_mode(mock_tracer, mock_provider):
    """Test traced_handler in finalize mode."""
    with (
        patch("lambda_otel_lite.processor._processor_mode", ProcessorMode.FINALIZE),
        patch("lambda_otel_lite.processor._handler_ready") as mock_ready,
        patch("lambda_otel_lite.processor._handler_complete") as mock_complete,
    ):
        with traced_handler(mock_tracer, mock_provider, "test_handler"):
            # No handler ready wait in finalize mode
            mock_ready.wait.assert_not_called()
            mock_ready.clear.assert_not_called()
            mock_complete.set.assert_not_called()

        # No completion signal in finalize mode
        mock_complete.set.assert_not_called()
        # No force flush in finalize mode
        mock_provider.force_flush.assert_not_called()


def test_traced_handler_cold_start(mock_tracer, mock_provider):
    """Test cold start attribute setting."""
    with (
        patch("lambda_otel_lite.processor._processor_mode", ProcessorMode.SYNC),
        patch("lambda_otel_lite.processor._is_cold_start", True),
    ):
        with traced_handler(mock_tracer, mock_provider, "test_handler"):
            span = mock_tracer.start_as_current_span.return_value.__enter__.return_value
            span.set_attribute.assert_called_once_with("faas.cold_start", True)


def test_traced_handler_not_cold_start(mock_tracer, mock_provider):
    """Test no cold start attribute after first invocation."""
    with (
        patch("lambda_otel_lite.processor._processor_mode", ProcessorMode.SYNC),
        patch("lambda_otel_lite.processor._is_cold_start", False),
    ):
        with traced_handler(mock_tracer, mock_provider, "test_handler"):
            span = mock_tracer.start_as_current_span.return_value.__enter__.return_value
            span.set_attribute.assert_not_called()


def test_traced_handler_with_attributes(mock_tracer, mock_provider):
    """Test traced_handler with custom attributes."""
    attributes = {"test.key": "test.value"}

    with patch("lambda_otel_lite.processor._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", attributes=attributes):
            pass

        mock_tracer.start_as_current_span.assert_called_once_with(
            "test_handler",
            kind=SpanKind.INTERNAL,
            attributes=attributes,
            links=None,
            start_time=None,
            record_exception=True,
            set_status_on_exception=True,
            end_on_exit=True,
        )
