"""Tests for the propagation module."""

import os
from typing import Dict, Iterator, List, Optional, Set

import pytest
from unittest.mock import MagicMock, patch

from opentelemetry.context import Context
from opentelemetry.propagators.composite import CompositePropagator
from opentelemetry.propagators.textmap import Setter

from lambda_otel_lite.constants import EnvVars
from lambda_otel_lite.propagation import (
    LambdaXRayPropagator,
    NoopPropagator,
    create_propagator,
    has_valid_span,
    setup_propagator,
)


# Concrete implementation of TextMapGetter for testing
class ConcreteTextMapGetter:
    """Concrete implementation of TextMapGetter for testing."""

    def get(self, carrier: Dict[str, str], key: str) -> Optional[List[str]]:
        """Get a value from the carrier."""
        if value := carrier.get(key):
            return [value]
        return None

    def keys(self, carrier: Dict[str, str]) -> List[str]:
        """Get all keys from the carrier."""
        return list(carrier.keys())


# Mock the AwsXRayLambdaPropagator
class MockAwsXRayLambdaPropagator:
    """Mock implementation of AwsXRayLambdaPropagator for testing."""

    def __init__(self) -> None:
        """Initialize the mock propagator."""
        self.extract = MagicMock(return_value=Context())
        self.inject = MagicMock()
        self._fields = ["x-amzn-trace-id"]

    @property
    def fields(self) -> Set[str]:
        """Get fields."""
        return set(self._fields)


class MockTraceContextTextMapPropagator:
    """Mock implementation of TraceContextTextMapPropagator for testing."""

    def __init__(self) -> None:
        """Initialize the mock propagator."""
        self.extract = MagicMock(return_value=Context())
        self.inject = MagicMock()
        self._fields = ["traceparent", "tracestate"]

    @property
    def fields(self) -> Set[str]:
        """Get fields."""
        return set(self._fields)


@pytest.fixture
def clean_env() -> Iterator[None]:
    """Fixture to clean environment variables before each test."""
    # Clear any environment variables that might affect the tests
    if EnvVars.OTEL_PROPAGATORS in os.environ:
        del os.environ[EnvVars.OTEL_PROPAGATORS]
    if "_X_AMZN_TRACE_ID" in os.environ:
        del os.environ["_X_AMZN_TRACE_ID"]
    yield


@pytest.fixture
def mock_propagators() -> Iterator[MagicMock]:
    """Fixture to mock propagator classes."""
    # Create a mock for the AWS X-Ray Lambda propagator
    mock_aws_xray_lambda = MockAwsXRayLambdaPropagator()

    # Patch our LambdaXRayPropagator to inherit from the mock
    with (
        # Patch AwsXRayLambdaPropagator that our class inherits from
        patch(
            "opentelemetry.propagators.aws.aws_xray_propagator.AwsXRayLambdaPropagator",
            return_value=mock_aws_xray_lambda,
        ),
        # Patch the TraceContextTextMapPropagator
        patch(
            "opentelemetry.trace.propagation.tracecontext.TraceContextTextMapPropagator",
            MockTraceContextTextMapPropagator,
        ),
        # Patch our logger
        patch("lambda_otel_lite.propagation.logger") as mock_logger,
    ):
        # Make the mock available to tests
        yield mock_logger


def test_has_valid_span() -> None:
    """Test the has_valid_span function."""
    # Mock the get_current_span function
    with patch(
        "lambda_otel_lite.propagation.get_current_span"
    ) as mock_get_current_span:
        # Case 1: No span
        mock_get_current_span.return_value = None
        assert has_valid_span(Context()) is False

        # Case 2: Span with no context
        mock_span = MagicMock()
        mock_span.get_span_context.return_value = None
        mock_get_current_span.return_value = mock_span
        assert has_valid_span(Context()) is False

        # Case 3: Span with valid context
        mock_span_context = MagicMock()
        mock_span_context.trace_id = "trace_id"
        mock_span_context.span_id = "span_id"
        mock_span.get_span_context.return_value = mock_span_context
        mock_get_current_span.return_value = mock_span
        assert has_valid_span(Context()) is True

        # Case 4: Span with invalid context (no trace_id)
        mock_span_context = MagicMock()
        mock_span_context.trace_id = None
        mock_span_context.span_id = "span_id"
        mock_span.get_span_context.return_value = mock_span_context
        mock_get_current_span.return_value = mock_span
        assert has_valid_span(Context()) is False

        # Case 5: Span with invalid context (no span_id)
        mock_span_context = MagicMock()
        mock_span_context.trace_id = "trace_id"
        mock_span_context.span_id = None
        mock_span.get_span_context.return_value = mock_span_context
        mock_get_current_span.return_value = mock_span
        assert has_valid_span(Context()) is False


def test_lambda_xray_propagator_extract_from_carrier(
    mock_propagators: MagicMock,
) -> None:
    """Test that LambdaXRayPropagator extracts context from carrier."""
    # Create propagator instance
    propagator = LambdaXRayPropagator()

    # Setup test data
    carrier: Dict[str, str] = {
        "x-amzn-trace-id": "Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1"
    }
    getter = ConcreteTextMapGetter()
    ctx = Context()

    # Mock has_valid_span to return True
    with patch("lambda_otel_lite.propagation.has_valid_span") as mock_has_valid_span:
        mock_has_valid_span.return_value = True

        # Call the method under test
        result = propagator.extract(carrier, ctx, getter)

        # When inheriting from AwsXRayLambdaPropagator, the super().extract method is called
        # which is the extract method of the mocked base class
        assert result == Context()


def test_lambda_xray_propagator_extract_from_env_var(
    mock_propagators: MagicMock, clean_env: None
) -> None:
    """Test that LambdaXRayPropagator extracts context from environment variable."""
    # Create propagator instance
    propagator = LambdaXRayPropagator()

    # Setup test data
    carrier: Dict[str, str] = {}
    getter = ConcreteTextMapGetter()
    ctx = Context()

    # Mock has_valid_span to return False then True
    with patch("lambda_otel_lite.propagation.has_valid_span") as mock_has_valid_span:
        mock_has_valid_span.side_effect = [False, True]

        # Set the environment variable
        os.environ["_X_AMZN_TRACE_ID"] = (
            "Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1"
        )

        # Call the method under test
        result = propagator.extract(carrier, ctx, getter)

        # Verify that a span was extracted from the env var
        from opentelemetry.trace import get_current_span

        span = get_current_span(result)
        span_context = span.get_span_context()
        assert span_context.is_valid
        assert span_context.trace_id == 0x5759E988BD862E3FE1BE46A994272793
        assert span_context.span_id == 0x53995C3F42CD8AD8
        assert span_context.is_remote


def test_lambda_xray_propagator_inject(mock_propagators: MagicMock) -> None:
    """Test that LambdaXRayPropagator injects context into carrier."""
    # Create propagator instance
    propagator = LambdaXRayPropagator()

    # Setup test data
    carrier: Dict[str, str] = {}
    setter = MagicMock(spec=Setter)
    ctx = Context()

    # Call the method under test
    propagator.inject(carrier, ctx, setter)


def test_lambda_xray_propagator_fields(mock_propagators: MagicMock) -> None:
    """Test that LambdaXRayPropagator returns fields."""
    # Create propagator instance
    propagator = LambdaXRayPropagator()

    # Call the method under test
    result = propagator.fields

    # assert is a set
    assert isinstance(result, set)


def test_create_propagator_with_env_var(
    mock_propagators: MagicMock, clean_env: None
) -> None:
    """Test that create_propagator creates a propagator based on environment variable."""
    # Set environment variable
    os.environ[EnvVars.OTEL_PROPAGATORS] = "tracecontext,xray"

    # Call the method under test
    result = create_propagator()

    # Verify result
    assert isinstance(result, CompositePropagator)
    assert len(result._propagators) == 2  # pylint: disable=protected-access


def test_create_propagator_with_xray_lambda(
    mock_propagators: MagicMock, clean_env: None
) -> None:
    """Test that create_propagator creates LambdaXRayPropagator when xray-lambda is specified."""
    # Set environment variable
    os.environ[EnvVars.OTEL_PROPAGATORS] = "xray-lambda"

    # Call the method under test
    result = create_propagator()

    # Verify result
    assert isinstance(result, CompositePropagator)
    assert len(result._propagators) == 1  # pylint: disable=protected-access
    assert isinstance(result._propagators[0], LambdaXRayPropagator)  # pylint: disable=protected-access


def test_create_propagator_with_none(
    mock_propagators: MagicMock, clean_env: None
) -> None:
    """Test that create_propagator creates NoopPropagator when none is specified."""
    # Set environment variable
    os.environ[EnvVars.OTEL_PROPAGATORS] = "none"

    # Call the method under test
    result = create_propagator()

    # Verify result
    assert isinstance(result, NoopPropagator)


def test_create_propagator_with_default(
    mock_propagators: MagicMock, clean_env: None
) -> None:
    """Test that create_propagator uses default propagators when environment variable is not set."""
    # Call the method under test
    result = create_propagator()

    # Verify result
    assert isinstance(result, CompositePropagator)
    assert len(result._propagators) == 2  # pylint: disable=protected-access
    assert isinstance(result._propagators[0], MockTraceContextTextMapPropagator)  # pylint: disable=protected-access
    assert isinstance(result._propagators[1], LambdaXRayPropagator)  # pylint: disable=protected-access


def test_setup_propagator(mock_propagators: MagicMock) -> None:
    """Test that setup_propagator sets the global propagator."""
    with patch(
        "lambda_otel_lite.propagation.set_global_textmap"
    ) as mock_set_global_textmap:
        # Call the method under test
        setup_propagator()

        # Verify that set_global_textmap was called
        mock_set_global_textmap.assert_called_once()
