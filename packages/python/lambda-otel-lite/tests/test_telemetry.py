"""Tests for the telemetry module."""

import os
import re
from unittest.mock import patch

import pytest
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import IdGenerator

from lambda_otel_lite.telemetry import (
    TelemetryCompletionHandler,
    get_lambda_resource,
    init_telemetry,
)


@pytest.fixture
def mock_env():
    """Fixture to provide a clean environment for each test."""
    from opentelemetry import trace
    from opentelemetry.util._once import Once

    # Clear any existing tracer provider
    trace._TRACER_PROVIDER = None
    trace._TRACER_PROVIDER_SET_ONCE = Once()
    with patch.dict(os.environ, {}, clear=True):
        yield


def test_get_lambda_resource_basic(mock_env):
    """Test basic Lambda resource creation with minimal environment."""
    os.environ.update(
        {
            "AWS_LAMBDA_FUNCTION_NAME": "test-function",
            "AWS_REGION": "us-west-2",
        }
    )

    resource = get_lambda_resource()
    assert isinstance(resource, Resource)
    attrs = resource.attributes

    assert attrs["service.name"] == "test-function"
    assert attrs["faas.name"] == "test-function"
    assert attrs["cloud.provider"] == "aws"
    assert attrs["cloud.region"] == "us-west-2"


def test_get_lambda_resource_with_otel_service_name(mock_env):
    """Test that OTEL_SERVICE_NAME overrides AWS_LAMBDA_FUNCTION_NAME for service.name."""
    os.environ.update(
        {
            "AWS_LAMBDA_FUNCTION_NAME": "test-function",
            "OTEL_SERVICE_NAME": "custom-service",
        }
    )

    resource = get_lambda_resource()
    assert resource.attributes["service.name"] == "custom-service"
    assert resource.attributes["faas.name"] == "test-function"


def test_get_lambda_resource_with_resource_attributes(mock_env):
    """Test that OTEL_RESOURCE_ATTRIBUTES are properly parsed."""
    os.environ.update(
        {
            "AWS_LAMBDA_FUNCTION_NAME": "test-function",
            "OTEL_RESOURCE_ATTRIBUTES": "key1=value1,key2=value2,custom.provider=gcp",
        }
    )

    resource = get_lambda_resource()
    attrs = resource.attributes

    assert attrs["key1"] == "value1"
    assert attrs["key2"] == "value2"
    assert attrs["custom.provider"] == "gcp"
    # Lambda attributes should remain unchanged
    assert attrs["cloud.provider"] == "aws"


def test_get_lambda_resource_with_url_encoded_attributes(mock_env):
    """Test that URL-encoded values in OTEL_RESOURCE_ATTRIBUTES are properly decoded."""
    os.environ.update(
        {
            "AWS_LAMBDA_FUNCTION_NAME": "test-function",
            "OTEL_RESOURCE_ATTRIBUTES": "deployment.env=prod%20env,custom.name=my%20service",
        }
    )

    resource = get_lambda_resource()
    attrs = resource.attributes

    assert attrs["deployment.env"] == "prod env"
    assert attrs["custom.name"] == "my service"
    assert attrs["service.name"] == "test-function"


def test_get_lambda_resource_no_service_name(mock_env):
    """Test that service.name defaults to 'unknown_service' when no name is provided."""
    resource = get_lambda_resource()
    attrs = resource.attributes

    assert attrs["service.name"] == "unknown_service"
    assert "faas.name" not in attrs
    assert attrs["cloud.provider"] == "aws"


def test_get_lambda_resource_only_otel_service_name(mock_env):
    """Test that when only OTEL_SERVICE_NAME is set, faas.name is not set."""
    os.environ["OTEL_SERVICE_NAME"] = "custom-service"

    resource = get_lambda_resource()
    attrs = resource.attributes

    assert attrs["service.name"] == "custom-service"
    assert "faas.name" not in attrs
    assert attrs["cloud.provider"] == "aws"


def test_init_telemetry_basic(mock_env):
    """Test basic telemetry initialization."""
    os.environ["AWS_LAMBDA_FUNCTION_NAME"] = "test-function"

    tracer, handler = init_telemetry()

    # Verify tracer is properly configured
    assert tracer is not None
    assert isinstance(handler, TelemetryCompletionHandler)


def test_init_telemetry_with_custom_resource(mock_env):
    """Test telemetry initialization with a custom resource."""
    custom_resource = Resource.create({"custom.attr": "value"})

    tracer, handler = init_telemetry(resource=custom_resource)
    assert isinstance(handler, TelemetryCompletionHandler)
    assert handler.tracer_provider.resource.attributes["custom.attr"] == "value"


def test_init_telemetry_with_custom_processor(mock_env):
    """Test telemetry initialization with a custom processor."""
    from opentelemetry.sdk.trace import SpanProcessor

    class CustomProcessor(SpanProcessor):
        def __init__(self):
            self.was_used = False

        def on_start(self, span, parent_context):
            self.was_used = True

        def on_end(self, span):
            pass

        def shutdown(self):
            pass

        def force_flush(self, timeout_millis=30000):
            pass

    custom_processor = CustomProcessor()
    tracer, handler = init_telemetry(span_processors=[custom_processor])

    # Create a span to trigger the processor
    with tracer.start_span("test"):
        pass

    assert custom_processor.was_used


def test_init_telemetry_with_custom_id_generator(mock_env):
    """Test telemetry initialization with a custom ID generator."""

    class MockXRayIdGenerator(IdGenerator):
        def __init__(self):
            self.trace_id_called = False
            self.span_id_called = False

        def generate_trace_id(self):
            self.trace_id_called = True
            # Return a trace ID with a timestamp in the first 32 bits (8 hex chars)
            # X-Ray format: <timestamp in seconds>-<random part>
            import time

            timestamp_hex = format(int(time.time()), "08x")
            random_part = "a" * 24  # Use a fixed value to easily verify it
            return int(timestamp_hex + random_part, 16)

        def generate_span_id(self):
            self.span_id_called = True
            # Return a fixed span ID for testing
            return 0x1234567890ABCDEF

    # Create a custom X-Ray ID generator
    id_generator = MockXRayIdGenerator()

    # Initialize telemetry with the custom ID generator
    tracer, handler = init_telemetry(id_generator=id_generator)

    # Create a span to trigger ID generation
    span = tracer.start_span("test_span")
    span_context = span.get_span_context()

    # Verify that the ID generator was used
    assert id_generator.trace_id_called
    assert id_generator.span_id_called

    # Verify that the trace ID format matches X-Ray format
    # X-Ray trace IDs have a timestamp in the first 8 hex characters
    trace_id_hex = format(span_context.trace_id, "032x")

    # The first 8 chars should be a valid timestamp (in the last day)
    timestamp_chars = trace_id_hex[:8]
    assert re.match(r"^[0-9a-f]{8}$", timestamp_chars) is not None

    # The rest should match our fixed random part
    random_part = trace_id_hex[8:]
    assert random_part == "a" * 24

    # Verify span ID
    span_id_hex = format(span_context.span_id, "016x")
    assert span_id_hex == "1234567890abcdef"

    span.end()


def test_get_lambda_resource_with_all_attributes(mock_env):
    """Test that get_lambda_resource sets all expected attributes."""
    os.environ.update(
        {
            "AWS_REGION": "us-west-2",
            "AWS_LAMBDA_FUNCTION_NAME": "test-function",
            "AWS_LAMBDA_FUNCTION_VERSION": "$LATEST",
            "AWS_LAMBDA_LOG_STREAM_NAME": "2024/02/16/[$LATEST]1234567890",
            "AWS_LAMBDA_FUNCTION_MEMORY_SIZE": "128",
            "OTEL_SERVICE_NAME": "custom-service",
            "OTEL_RESOURCE_ATTRIBUTES": "team=platform,env=dev",
        }
    )

    resource = get_lambda_resource()
    attributes = resource.attributes

    # Test cloud and FAAS attributes
    assert attributes["cloud.provider"] == "aws"
    assert attributes["cloud.region"] == "us-west-2"
    assert attributes["faas.name"] == "test-function"
    assert attributes["faas.version"] == "$LATEST"
    assert attributes["faas.instance"] == "2024/02/16/[$LATEST]1234567890"
    # 128 MB = 128 * 1024 * 1024 bytes
    assert attributes["faas.max_memory"] == 128 * 1024 * 1024

    # Test service name
    assert attributes["service.name"] == "custom-service"

    # Test custom attributes
    assert attributes["team"] == "platform"
    assert attributes["env"] == "dev"


def test_get_lambda_resource_defaults(mock_env):
    """Test that get_lambda_resource uses defaults when env vars are missing."""
    resource = get_lambda_resource()
    attributes = resource.attributes

    assert "cloud.provider" in attributes
    assert attributes["cloud.provider"] == "aws"
    assert attributes["service.name"] == "unknown_service"


def test_get_lambda_resource_with_custom_resource(mock_env):
    """Test that get_lambda_resource merges with custom resource."""
    custom_resource = Resource.create(
        {
            "custom.attribute": "value",
            "service.name": "override-service",
        }
    )

    with patch.dict(
        os.environ,
        {
            "AWS_REGION": "us-west-2",
            "AWS_LAMBDA_FUNCTION_NAME": "test-function",
            "OTEL_SERVICE_NAME": "env-service",
        },
    ):
        resource = get_lambda_resource(custom_resource)
        attributes = resource.attributes

        # Custom attributes should be preserved
        assert attributes["custom.attribute"] == "value"
        # Service name from custom resource should take precedence
        assert attributes["service.name"] == "override-service"
        # Lambda attributes should still be present
        assert attributes["cloud.provider"] == "aws"
        assert attributes["cloud.region"] == "us-west-2"
        assert attributes["faas.name"] == "test-function"


def test_get_lambda_resource_with_malformed_attributes(mock_env):
    """Test that get_lambda_resource handles malformed OTEL_RESOURCE_ATTRIBUTES."""
    with patch.dict(
        os.environ,
        {
            "OTEL_RESOURCE_ATTRIBUTES": "invalid,key=value,another=invalid=format",
        },
    ):
        resource = get_lambda_resource()
        attributes = resource.attributes

        # Should only parse valid key=value pairs
        assert attributes["key"] == "value"
        # The SDK accepts values containing equals signs
        assert attributes["another"] == "invalid=format"
        # But completely malformed entries (no equals) are skipped
        assert "invalid" not in attributes


def test_init_telemetry_resource_attributes(mock_env):
    """Test that init_telemetry correctly sets up resource attributes."""
    os.environ.update(
        {
            "AWS_REGION": "us-west-2",
            "AWS_LAMBDA_FUNCTION_NAME": "test-function",
            "AWS_LAMBDA_FUNCTION_VERSION": "$LATEST",
            "AWS_LAMBDA_LOG_STREAM_NAME": "2024/02/16/[$LATEST]1234567890",
            "AWS_LAMBDA_FUNCTION_MEMORY_SIZE": "128",
            "OTEL_SERVICE_NAME": "custom-service",
            "OTEL_RESOURCE_ATTRIBUTES": "team=platform,env=dev",
        }
    )

    tracer, handler = init_telemetry()
    attrs = handler.tracer_provider.resource.attributes

    assert attrs["cloud.provider"] == "aws"
    assert attrs["cloud.region"] == "us-west-2"
    assert attrs["faas.name"] == "test-function"
    assert attrs["service.name"] == "custom-service"
    assert attrs["team"] == "platform"
    assert attrs["env"] == "dev"


def test_init_telemetry_with_custom_propagators(mock_env):
    """Test telemetry initialization with custom propagators."""
    from opentelemetry.propagate import get_global_textmap
    from opentelemetry.propagators.composite import CompositePropagator
    from opentelemetry.trace.propagation.tracecontext import (
        TraceContextTextMapPropagator,
    )

    # Create a mock propagator for testing
    class MockPropagator(TraceContextTextMapPropagator):
        def __init__(self):
            super().__init__()
            self.extract_called = False
            self.inject_called = False

        def extract(self, carrier, context=None, getter=None):
            self.extract_called = True
            return super().extract(carrier, context, getter)

        def inject(self, carrier, context=None, setter=None):
            self.inject_called = True
            super().inject(carrier, context, setter)

    # Create a custom propagator
    mock_propagator = MockPropagator()

    # Initialize telemetry with the custom propagator
    tracer, handler = init_telemetry(propagators=[mock_propagator])

    # Verify the global propagator was set
    global_propagator = get_global_textmap()

    # The global propagator should be a CompositePropagator containing our mock
    assert isinstance(global_propagator, CompositePropagator)

    # Test that our propagator is used for extraction/injection
    carrier = {}
    global_propagator.inject(carrier)
    global_propagator.extract(carrier)

    # Verify our mock propagator was called
    assert mock_propagator.inject_called
    assert mock_propagator.extract_called
