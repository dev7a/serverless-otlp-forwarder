"""Tests for the traced_handler implementation."""

import os
from dataclasses import dataclass
from unittest.mock import Mock, patch

import pytest
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.trace import SpanKind
from opentelemetry.propagate import inject
from opentelemetry.trace.propagation.tracecontext import TraceContextTextMapPropagator
from opentelemetry import context as context_api
from opentelemetry.trace import set_span_in_context, NonRecordingSpan, SpanContext, TraceFlags

from lambda_otel_lite import ProcessorMode
from lambda_otel_lite.handler import traced_handler, _extract_span_attributes


@dataclass
class MockLambdaContext:
    """Mock AWS Lambda context."""
    invoked_function_arn: str = "arn:aws:lambda:us-west-2:123456789012:function:test-function"
    aws_request_id: str = "test-request-id"


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


@pytest.fixture
def mock_env():
    """Set up mock environment variables."""
    env_vars = {
        "AWS_LAMBDA_REQUEST_ID": "test-request-id",
    }
    with patch.dict(os.environ, env_vars):
        yield env_vars


@pytest.fixture
def mock_context():
    """Create a mock Lambda context."""
    return MockLambdaContext()


def test_traced_handler_sync_mode(mock_tracer, mock_provider, mock_env, mock_context):
    """Test traced_handler in sync mode."""
    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", context=mock_context):
            pass

        # Verify span creation with basic attributes
        mock_tracer.start_as_current_span.assert_called_once_with(
            "test_handler",
            context=None,
            kind=SpanKind.SERVER,
            attributes={
                "faas.invocation_id": mock_env["AWS_LAMBDA_REQUEST_ID"],
                "cloud.resource_id": mock_context.invoked_function_arn,
                "cloud.account.id": "123456789012",
                "faas.trigger": "other",
            },
            links=None,
            start_time=None,
            record_exception=True,
            set_status_on_exception=True,
            end_on_exit=True,
        )

        # Verify force flush in sync mode
        mock_provider.force_flush.assert_called_once()


def test_traced_handler_async_mode(mock_tracer, mock_provider, mock_env):
    """Test traced_handler in async mode."""
    with (
        patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.ASYNC),
        patch("lambda_otel_lite.handler._handler_ready") as mock_ready,
        patch("lambda_otel_lite.handler._handler_complete") as mock_complete,
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


def test_traced_handler_finalize_mode(mock_tracer, mock_provider, mock_env):
    """Test traced_handler in finalize mode."""
    with (
        patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.FINALIZE),
        patch("lambda_otel_lite.handler._handler_ready") as mock_ready,
        patch("lambda_otel_lite.handler._handler_complete") as mock_complete,
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


def test_traced_handler_cold_start(mock_tracer, mock_provider, mock_env):
    """Test cold start attribute setting."""
    with (
        patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC),
        patch("lambda_otel_lite.handler._is_cold_start", True),
    ):
        with traced_handler(mock_tracer, mock_provider, "test_handler"):
            span = mock_tracer.start_as_current_span.return_value.__enter__.return_value
            span.set_attribute.assert_called_once_with("faas.cold_start", True)


def test_traced_handler_not_cold_start(mock_tracer, mock_provider, mock_env):
    """Test no cold start attribute after first invocation."""
    with (
        patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC),
        patch("lambda_otel_lite.handler._is_cold_start", False),
    ):
        with traced_handler(mock_tracer, mock_provider, "test_handler"):
            span = mock_tracer.start_as_current_span.return_value.__enter__.return_value
            span.set_attribute.assert_not_called()


def test_traced_handler_with_attributes(mock_tracer, mock_provider, mock_env, mock_context):
    """Test traced_handler with custom attributes."""
    attributes = {"test.key": "test.value"}

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", context=mock_context, attributes=attributes):
            pass

        # Verify custom attributes are merged with span attributes
        expected_attributes = {
            "faas.invocation_id": mock_env["AWS_LAMBDA_REQUEST_ID"],
            "cloud.resource_id": mock_context.invoked_function_arn,
            "cloud.account.id": "123456789012",
            "test.key": "test.value",
            "faas.trigger": "other",
        }

        mock_tracer.start_as_current_span.assert_called_once_with(
            "test_handler",
            context=None,
            kind=SpanKind.SERVER,
            attributes=expected_attributes,
            links=None,
            start_time=None,
            record_exception=True,
            set_status_on_exception=True,
            end_on_exit=True,
        )


def test_traced_handler_with_http_trigger(mock_tracer, mock_provider, mock_env, mock_context):
    """Test traced_handler with HTTP event."""
    event = {
        "httpMethod": "POST",
    }

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", event, mock_context):
            pass

        expected_attributes = {
            "faas.invocation_id": mock_env["AWS_LAMBDA_REQUEST_ID"],
            "cloud.resource_id": mock_context.invoked_function_arn,
            "cloud.account.id": "123456789012",
            "faas.trigger": "http",
            "http.method": "POST",
            "http.route": "",
            "http.target": "",
        }

        mock_tracer.start_as_current_span.assert_called_once_with(
            "test_handler",
            context=None,
            kind=SpanKind.SERVER,
            attributes=expected_attributes,
            links=None,
            start_time=None,
            record_exception=True,
            set_status_on_exception=True,
            end_on_exit=True,
        )


def test_traced_handler_with_invalid_arn(mock_tracer, mock_provider, mock_env):
    """Test traced_handler with invalid function ARN."""
    context = MockLambdaContext(invoked_function_arn="invalid:arn")

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", context=context):
            pass

        # Should only set invocation_id and faas.trigger when ARN is invalid
        mock_tracer.start_as_current_span.assert_called_once_with(
            "test_handler",
            context=None,
            kind=SpanKind.SERVER,
            attributes={
                "faas.invocation_id": mock_env["AWS_LAMBDA_REQUEST_ID"],
                "faas.trigger": "other",
            },
            links=None,
            start_time=None,
            record_exception=True,
            set_status_on_exception=True,
            end_on_exit=True,
        )


def test_traced_handler_with_http_headers_context(mock_tracer, mock_provider, mock_env):
    """Test traced_handler with context in HTTP headers."""
    # Create a real trace context
    span_context = SpanContext(
        trace_id=0x123456789ABCDEF0123456789ABCDEF0,
        span_id=0x123456789ABCDEF0,
        is_remote=True,
        trace_flags=TraceFlags(TraceFlags.SAMPLED),
    )
    span = NonRecordingSpan(span_context)
    context = set_span_in_context(span)
    
    # Create a carrier with trace context
    carrier = {}
    TraceContextTextMapPropagator().inject(carrier, context=context)
    
    event = {
        "headers": carrier,
        "httpMethod": "GET"
    }

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", event=event):
            pass

        # Verify span creation with extracted context
        mock_tracer.start_as_current_span.assert_called_once()
        call_args = mock_tracer.start_as_current_span.call_args[1]
        assert call_args["context"] is not None
        assert call_args["attributes"]["faas.trigger"] == "http"
        assert call_args["attributes"]["http.method"] == "GET"


def test_traced_handler_with_invalid_carrier(mock_tracer, mock_provider, mock_env):
    """Test traced_handler with invalid carrier extraction."""
    def invalid_extractor(event: dict) -> dict:
        raise ValueError("Invalid carrier")

    event = {"some": "data"}

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(
            mock_tracer,
            mock_provider,
            "test_handler",
            event=event,
            get_carrier=invalid_extractor
        ):
            pass

        # Verify span creation without context (extraction failed)
        mock_tracer.start_as_current_span.assert_called_once()
        call_args = mock_tracer.start_as_current_span.call_args[1]
        assert call_args["context"] is None


def test_traced_handler_parent_context_precedence(mock_tracer, mock_provider, mock_env):
    """Test that explicit parent_context takes precedence over extracted context."""
    # Create a real trace context for the carrier
    span_context = SpanContext(
        trace_id=0x123456789ABCDEF0123456789ABCDEF0,
        span_id=0x123456789ABCDEF0,
        is_remote=True,
        trace_flags=TraceFlags(TraceFlags.SAMPLED),
    )
    span = NonRecordingSpan(span_context)
    carrier_context = set_span_in_context(span)
    
    # Create a carrier with trace context
    carrier = {}
    TraceContextTextMapPropagator().inject(carrier, context=carrier_context)
    
    event = {
        "headers": carrier
    }

    # Create a different context to use as parent_context
    explicit_context = context_api.Context()

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(
            mock_tracer,
            mock_provider,
            "test_handler",
            event=event,
            parent_context=explicit_context
        ):
            pass

        # Verify explicit context was used
        mock_tracer.start_as_current_span.assert_called_once()
        call_args = mock_tracer.start_as_current_span.call_args[1]
        assert call_args["context"] == explicit_context


def test_traced_handler_no_context_extraction(mock_tracer, mock_provider, mock_env):
    """Test traced_handler with no context available."""
    event = {
        "data": "some-data"  # No headers or other context
    }

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", event=event):
            pass

        # Verify span creation without context
        mock_tracer.start_as_current_span.assert_called_once()
        call_args = mock_tracer.start_as_current_span.call_args[1]
        assert call_args["context"] is None


def test_extract_span_attributes_with_context(mock_context):
    """Test that span attributes are correctly extracted from context."""
    mock_context.invoked_function_arn = "arn:aws:lambda:us-west-2:123456789012:function:test"
    mock_context.aws_request_id = "test-request-id"
    
    attributes = _extract_span_attributes(context=mock_context)
    
    assert attributes["faas.invocation_id"] == "test-request-id"
    assert attributes["cloud.account.id"] == "123456789012"
    assert attributes["cloud.resource_id"] == mock_context.invoked_function_arn
    assert attributes["faas.trigger"] == "other"


def test_extract_span_attributes_without_context():
    """Test that span attributes are empty without context."""
    attributes = _extract_span_attributes()
    assert "faas.invocation_id" not in attributes
    assert "cloud.account.id" not in attributes
    assert "cloud.resource_id" not in attributes


def test_extract_span_attributes_with_partial_context(mock_context):
    """Test that span attributes handle missing context attributes."""
    # Only set aws_request_id
    mock_context.aws_request_id = "test-request-id"
    mock_context.invoked_function_arn = None  # Clear the ARN
    
    attributes = _extract_span_attributes(context=mock_context)
    
    assert attributes["faas.invocation_id"] == "test-request-id"
    assert "cloud.account.id" not in attributes
    assert "cloud.resource_id" not in attributes
    assert attributes["faas.trigger"] == "other"


def test_traced_handler_with_other_trigger(mock_tracer, mock_provider, mock_env, mock_context):
    """Test traced_handler with non-HTTP event."""
    event = {
        "Records": [
            {
                "eventSource": "aws:sqs",
            }
        ]
    }

    with patch("lambda_otel_lite.handler._processor_mode", ProcessorMode.SYNC):
        with traced_handler(mock_tracer, mock_provider, "test_handler", event, mock_context):
            pass

        expected_attributes = {
            "faas.invocation_id": mock_env["AWS_LAMBDA_REQUEST_ID"],
            "cloud.resource_id": mock_context.invoked_function_arn,
            "cloud.account.id": "123456789012",
            "faas.trigger": "other",
        }

        mock_tracer.start_as_current_span.assert_called_once_with(
            "test_handler",
            context=None,
            kind=SpanKind.SERVER,
            attributes=expected_attributes,
            links=None,
            start_time=None,
            record_exception=True,
            set_status_on_exception=True,
            end_on_exit=True,
        )


def test_extract_span_attributes_with_other_trigger():
    """Test that non-HTTP events get 'other' trigger type."""
    event = {
        "Records": [{"eventSource": "aws:sqs"}]  # SQS event
    }
    
    attributes = _extract_span_attributes(event=event)
    
    assert attributes["faas.trigger"] == "other"


def test_extract_span_attributes_without_event():
    """Test that faas.trigger is set to 'other' without event."""
    attributes = _extract_span_attributes()
    assert attributes["faas.trigger"] == "other"


def test_extract_span_attributes_with_simple_event():
    """Test that simple dictionary events get 'other' trigger type."""
    event = {
        "depth": 2,
        "iterations": 2,
    }
    
    attributes = _extract_span_attributes(event=event)
    assert attributes["faas.trigger"] == "other"
