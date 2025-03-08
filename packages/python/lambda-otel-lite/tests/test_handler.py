"""Tests for the traced handler implementation."""

import os
from collections.abc import Generator
from dataclasses import dataclass
from typing import Any
from unittest.mock import Mock

import pytest
from opentelemetry import trace
from opentelemetry.trace import (
    SpanKind,
    StatusCode,
)

from lambda_otel_lite.extractors import SpanAttributes, TriggerType
from lambda_otel_lite.handler import create_traced_handler
from lambda_otel_lite.telemetry import TelemetryCompletionHandler


@dataclass
class MockLambdaContext:
    """Mock AWS Lambda context."""

    invoked_function_arn: str = (
        "arn:aws:lambda:us-west-2:123456789012:function:test-function"
    )
    aws_request_id: str = "test-request-id"


@pytest.fixture
def mock_tracer() -> trace.Tracer:
    """Create a mock tracer."""
    tracer = Mock(spec=trace.Tracer)
    span = Mock()
    context_manager = Mock()
    context_manager.__enter__ = Mock(return_value=span)
    context_manager.__exit__ = Mock(return_value=None)
    tracer.start_as_current_span.return_value = context_manager
    return tracer


@pytest.fixture
def mock_completion_handler(mock_tracer: trace.Tracer) -> TelemetryCompletionHandler:
    """Create a mock completion handler."""
    handler = Mock(spec=TelemetryCompletionHandler)
    handler.get_tracer.return_value = mock_tracer
    return handler


@pytest.fixture
def mock_env() -> Generator[dict[str, str], None, None]:
    """Mock environment variables."""
    original_env = dict(os.environ)
    os.environ.clear()
    os.environ.update(
        {
            "LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE": "sync",
        }
    )
    yield os.environ
    os.environ.clear()
    os.environ.update(original_env)


@pytest.fixture
def mock_context() -> MockLambdaContext:
    """Create a mock Lambda context."""
    return MockLambdaContext()


def test_traced_handler_sync_mode(
    mock_completion_handler: TelemetryCompletionHandler,
    mock_env: dict[str, str],
    mock_context: MockLambdaContext,
) -> None:
    """Test traced handler in sync mode."""
    # Create traced handler
    traced = create_traced_handler(
        name="test-handler",
        completion_handler=mock_completion_handler,
    )

    # Define handler function
    @traced
    def handler(event: dict[str, Any], context: Any) -> dict[str, Any]:
        # Access current span via OpenTelemetry API
        current_span = trace.get_current_span()
        current_span.set_attribute("custom", "value")
        return {"statusCode": 200}

    # Call handler
    result = handler({}, mock_context)

    # Verify result
    assert result == {"statusCode": 200}

    # Verify span was created
    mock_tracer = mock_completion_handler.get_tracer()
    mock_tracer.start_as_current_span.assert_called_once()
    mock_completion_handler.complete.assert_called_once()


def test_traced_handler_with_custom_extractor(
    mock_completion_handler: TelemetryCompletionHandler,
    mock_env: dict[str, str],
    mock_context: MockLambdaContext,
) -> None:
    """Test traced handler with custom attribute extractor."""

    def custom_extractor(event: Any, context: Any) -> SpanAttributes:
        return SpanAttributes(
            trigger=TriggerType.HTTP,
            attributes={"custom": "attribute"},
            span_name="custom-span",
            kind=SpanKind.SERVER,
        )

    # Create traced handler with custom extractor
    traced = create_traced_handler(
        name="test-handler",
        completion_handler=mock_completion_handler,
        attributes_extractor=custom_extractor,
    )

    # Define handler function
    @traced
    def handler(event: dict[str, Any], context: Any) -> dict[str, Any]:
        return {"statusCode": 200}

    # Call handler
    result = handler({}, mock_context)

    # Verify result
    assert result == {"statusCode": 200}

    # Verify span was created with custom attributes
    mock_tracer = mock_completion_handler.get_tracer()
    mock_tracer.start_as_current_span.assert_called_once()
    call_args = mock_tracer.start_as_current_span.call_args
    assert call_args.kwargs["name"] == "custom-span"
    assert call_args.kwargs["kind"] == SpanKind.SERVER
    assert call_args.kwargs["attributes"] == {"custom": "attribute"}


def test_traced_handler_with_http_response(
    mock_completion_handler: TelemetryCompletionHandler,
    mock_env: dict[str, str],
) -> None:
    """Test traced handler with HTTP response."""
    print("\n=== Starting HTTP response test ===")

    # Create traced handler
    traced = create_traced_handler(
        name="test-handler",
        completion_handler=mock_completion_handler,
    )

    # Create a spy for the span
    from unittest.mock import MagicMock

    span = MagicMock()
    context_manager = MagicMock()
    context_manager.__enter__ = MagicMock(return_value=span)
    context_manager.__exit__ = MagicMock(return_value=None)

    # Configure the mock tracer to return our spy
    mock_tracer = mock_completion_handler.get_tracer()
    mock_tracer.start_as_current_span.return_value = context_manager

    # Define handler function
    @traced
    def handler(event: dict[str, Any], context: Any) -> dict[str, Any]:
        result = {"statusCode": 500, "body": "error"}
        return result

    # Call handler
    result = handler({}, None)
    # Verify result
    assert result == {"statusCode": 500, "body": "error"}

    # Verify span was created
    mock_tracer.start_as_current_span.assert_called_once()

    # Verify the status was set correctly
    for i, call_args in enumerate(span.set_status.call_args_list):
        status = call_args[0][0]
        print(
            f"  Call {i}: status_code={status.status_code}, description={status.description}"
        )

    # Get the last status that was set
    assert span.set_status.call_count > 0, "set_status was never called"
    last_status = span.set_status.call_args[0][0]

    assert last_status.status_code == StatusCode.ERROR, (
        f"Expected ERROR status but got {last_status.status_code}"
    )
    assert last_status.description == "HTTP 500 response"


def test_traced_handler_with_error(
    mock_completion_handler: TelemetryCompletionHandler,
    mock_env: dict[str, str],
) -> None:
    """Test traced handler with error."""
    # Create traced handler
    traced = create_traced_handler(
        name="test-handler",
        completion_handler=mock_completion_handler,
    )

    # Define handler function that raises an error
    @traced
    def handler(event: dict[str, Any], context: Any) -> dict[str, Any]:
        raise ValueError("test error")

    # Call handler and expect error
    with pytest.raises(ValueError, match="test error"):
        handler({}, None)

    # Verify span was created and error was recorded
    mock_tracer = mock_completion_handler.get_tracer()
    mock_tracer.start_as_current_span.assert_called_once()
    span = mock_tracer.start_as_current_span().__enter__()
    span.record_exception.assert_called_once()

    # Get the actual status that was set
    actual_status = span.set_status.call_args[0][0]
    assert actual_status.status_code == StatusCode.ERROR
    assert actual_status.description == "test error"
