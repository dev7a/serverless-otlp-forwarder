"""Tests for the Lambda extension implementation."""

import os
from unittest.mock import Mock, call, patch

import pytest
from opentelemetry.sdk.trace import TracerProvider

from lambda_otel_lite.extension import ProcessorMode, init_extension


@pytest.fixture
def mock_tracer_provider():
    """Create a mock TracerProvider."""
    provider = Mock(spec=TracerProvider)
    provider.force_flush.return_value = None
    provider.shutdown.return_value = None
    return provider


@pytest.fixture
def mock_http_client():
    """Create a mock HTTP client."""
    mock_response = Mock()
    mock_response.status = 200
    mock_response.headers = {"Lambda-Extension-Identifier": "test-id"}

    mock_client = Mock()
    mock_client.request.return_value = mock_response
    return mock_client


def test_extension_init_sync_mode(mock_tracer_provider):
    """Test that extension is not initialized in sync mode."""
    with patch.dict(os.environ, {"AWS_LAMBDA_RUNTIME_API": "test"}):
        init_extension(ProcessorMode.SYNC, mock_tracer_provider)
        # No extension initialization in sync mode
        mock_tracer_provider.force_flush.assert_not_called()


def test_extension_init_no_runtime(mock_tracer_provider):
    """Test that extension is not initialized without runtime API."""
    with patch.dict(os.environ, clear=True):
        init_extension(ProcessorMode.ASYNC, mock_tracer_provider)
        # No extension initialization without runtime API
        mock_tracer_provider.force_flush.assert_not_called()


@patch("lambda_otel_lite.extension.urllib3.PoolManager")
@patch("lambda_otel_lite.extension.threading.Thread")
def test_extension_init_async_mode(mock_thread, mock_pool, mock_tracer_provider, mock_http_client):
    """Test extension initialization in async mode."""
    mock_pool.return_value = mock_http_client

    with patch.dict(os.environ, {"AWS_LAMBDA_RUNTIME_API": "test"}):
        init_extension(ProcessorMode.ASYNC, mock_tracer_provider)

        # Verify extension registration
        assert mock_http_client.request.call_args_list[0] == call(
            "POST",
            "http://test/2020-01-01/extension/register",
            headers={"Lambda-Extension-Name": "internal"},
            json={"events": ["INVOKE"]},
        )

        # Verify thread started
        mock_thread.assert_called_once()
        assert mock_thread.return_value.start.called


@patch("lambda_otel_lite.extension.urllib3.PoolManager")
@patch("lambda_otel_lite.extension.threading.Thread")
def test_extension_init_finalize_mode(
    mock_thread, mock_pool, mock_tracer_provider, mock_http_client
):
    """Test extension initialization in finalize mode."""
    # Reset extension initialization state
    import lambda_otel_lite.extension

    lambda_otel_lite.extension._extension_initialized = False

    # Set up mock HTTP client
    mock_pool.return_value = mock_http_client
    mock_response = Mock()
    mock_response.headers = {"Lambda-Extension-Identifier": "test-id"}
    mock_http_client.request.return_value = mock_response

    # Create a mock thread instance
    mock_thread_instance = Mock()
    mock_thread.return_value = mock_thread_instance

    with patch.dict(os.environ, {"AWS_LAMBDA_RUNTIME_API": "test"}):
        init_extension(ProcessorMode.FINALIZE, mock_tracer_provider)

        # Verify extension registration
        mock_http_client.request.assert_called_with(
            "POST",
            "http://test/2020-01-01/extension/register",
            headers={"Lambda-Extension-Name": "internal"},
            json={"events": []},
        )

        # Verify thread started with correct function
        mock_thread.assert_called_once()
        mock_thread_instance.start.assert_called_once()


@patch("lambda_otel_lite.extension.urllib3.PoolManager")
@patch("lambda_otel_lite.extension.threading.Thread")
def test_extension_init_with_shutdown_callback(
    mock_thread, mock_pool, mock_tracer_provider, mock_http_client
):
    """Test extension initialization with shutdown callback."""
    # Reset extension initialization state
    import lambda_otel_lite.extension

    lambda_otel_lite.extension._extension_initialized = False

    # Set up mock HTTP client
    mock_pool.return_value = mock_http_client
    mock_response = Mock()
    mock_response.headers = {"Lambda-Extension-Identifier": "test-id"}
    mock_http_client.request.return_value = mock_response

    # Create a mock thread instance
    mock_thread_instance = Mock()
    mock_thread.return_value = mock_thread_instance

    mock_callback = Mock()

    with patch.dict(os.environ, {"AWS_LAMBDA_RUNTIME_API": "test"}):
        init_extension(ProcessorMode.FINALIZE, mock_tracer_provider, on_shutdown=mock_callback)

        # Verify thread creation
        mock_thread.assert_called_once()

        # Get the thread target and args
        thread_args = mock_thread.call_args
        assert len(thread_args) == 2  # args and kwargs
        assert "target" in thread_args[1]
        assert "args" in thread_args[1]

        # Call the target function to simulate shutdown
        target_func = thread_args[1]["target"]
        target_func(*thread_args[1]["args"])

        # Verify callback was called
        mock_callback.assert_called_once()


@patch("lambda_otel_lite.extension.urllib3.PoolManager")
@patch("lambda_otel_lite.extension.threading.Thread")
def test_extension_no_double_init(mock_thread, mock_pool, mock_tracer_provider, mock_http_client):
    """Test that extension is not initialized twice."""
    mock_pool.return_value = mock_http_client

    with patch.dict(os.environ, {"AWS_LAMBDA_RUNTIME_API": "test"}):
        init_extension(ProcessorMode.ASYNC, mock_tracer_provider)
        mock_thread.reset_mock()

        # Try to initialize again
        init_extension(ProcessorMode.ASYNC, mock_tracer_provider)

        # Verify no second initialization
        mock_thread.assert_not_called()
