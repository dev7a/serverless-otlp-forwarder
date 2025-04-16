"""Tests for the ProcessorMode enum and its methods."""

import os
import pytest
from unittest.mock import patch, MagicMock

from lambda_otel_lite import ProcessorMode
from lambda_otel_lite.constants import EnvVars


@pytest.fixture
def clean_env():
    """Fixture to clean environment variables before each test."""
    # Clear any environment variables that might affect the tests
    if EnvVars.PROCESSOR_MODE in os.environ:
        del os.environ[EnvVars.PROCESSOR_MODE]
    if "TEST_MODE" in os.environ:
        del os.environ["TEST_MODE"]
    if "CUSTOM_ENV_VAR" in os.environ:
        del os.environ["CUSTOM_ENV_VAR"]
    yield


def test_enum_values():
    """Test that the enum values are correct."""
    assert ProcessorMode.SYNC == "sync"
    assert ProcessorMode.ASYNC == "async"
    assert ProcessorMode.FINALIZE == "finalize"


def test_from_env_with_valid_value(clean_env):
    """Test that from_env returns the correct value when the environment variable is set."""
    os.environ["TEST_MODE"] = "sync"
    assert ProcessorMode.from_env("TEST_MODE") == ProcessorMode.SYNC

    os.environ["TEST_MODE"] = "async"
    assert ProcessorMode.from_env("TEST_MODE") == ProcessorMode.ASYNC

    os.environ["TEST_MODE"] = "finalize"
    assert ProcessorMode.from_env("TEST_MODE") == ProcessorMode.FINALIZE


def test_from_env_with_case_insensitive_value(clean_env):
    """Test that from_env handles case-insensitive values."""
    os.environ["TEST_MODE"] = "SYNC"
    assert ProcessorMode.from_env("TEST_MODE") == ProcessorMode.SYNC

    os.environ["TEST_MODE"] = "Async"
    assert ProcessorMode.from_env("TEST_MODE") == ProcessorMode.ASYNC


def test_from_env_with_default(clean_env):
    """Test that from_env returns the default value when the environment variable is not set."""
    assert ProcessorMode.from_env("TEST_MODE", ProcessorMode.SYNC) == ProcessorMode.SYNC
    assert (
        ProcessorMode.from_env("TEST_MODE", ProcessorMode.ASYNC) == ProcessorMode.ASYNC
    )


def test_from_env_with_invalid_value(clean_env):
    """Test that from_env raises ValueError when the environment variable has an invalid value."""
    os.environ["TEST_MODE"] = "invalid"
    with pytest.raises(ValueError):
        ProcessorMode.from_env("TEST_MODE")


def test_from_env_with_empty_value(clean_env):
    """Test that from_env returns the default value when the environment variable is empty."""
    os.environ["TEST_MODE"] = ""
    assert ProcessorMode.from_env("TEST_MODE", ProcessorMode.SYNC) == ProcessorMode.SYNC


def test_from_env_with_whitespace(clean_env):
    """Test that from_env handles whitespace in the environment variable."""
    os.environ["TEST_MODE"] = "  sync  "
    assert ProcessorMode.from_env("TEST_MODE") == ProcessorMode.SYNC


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_env_var(mock_create_logger, clean_env):
    """Test that resolve returns the environment variable value when it's set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ[EnvVars.PROCESSOR_MODE] = "async"
    assert ProcessorMode.resolve() == ProcessorMode.ASYNC
    assert ProcessorMode.resolve(ProcessorMode.SYNC) == ProcessorMode.ASYNC


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_config_value(mock_create_logger, clean_env):
    """Test that resolve returns the config value when the environment variable is not set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert ProcessorMode.resolve(ProcessorMode.ASYNC) == ProcessorMode.ASYNC
    assert ProcessorMode.resolve(ProcessorMode.FINALIZE) == ProcessorMode.FINALIZE


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_default(mock_create_logger, clean_env):
    """Test that resolve returns the default value when neither env var nor config is set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert ProcessorMode.resolve() == ProcessorMode.SYNC


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_invalid_env_var(mock_create_logger, clean_env):
    """Test that resolve logs a warning and returns the config value when the env var is invalid."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ[EnvVars.PROCESSOR_MODE] = "invalid"
    assert ProcessorMode.resolve(ProcessorMode.ASYNC) == ProcessorMode.ASYNC
    assert ProcessorMode.resolve() == ProcessorMode.SYNC
    mock_logger.warn.assert_called()


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_empty_env_var(mock_create_logger, clean_env):
    """Test that resolve returns the config value when the env var is empty."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ[EnvVars.PROCESSOR_MODE] = ""
    assert ProcessorMode.resolve(ProcessorMode.ASYNC) == ProcessorMode.ASYNC
    assert ProcessorMode.resolve() == ProcessorMode.SYNC


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_whitespace_env_var(mock_create_logger, clean_env):
    """Test that resolve handles whitespace in the environment variable."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ[EnvVars.PROCESSOR_MODE] = "  async  "
    assert ProcessorMode.resolve() == ProcessorMode.ASYNC


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_case_insensitive_env_var(mock_create_logger, clean_env):
    """Test that resolve handles case-insensitive values in the environment variable."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ[EnvVars.PROCESSOR_MODE] = "ASYNC"
    assert ProcessorMode.resolve() == ProcessorMode.ASYNC


@patch("lambda_otel_lite.logger.create_logger")
def test_resolve_with_custom_env_var(mock_create_logger, clean_env):
    """Test that resolve handles a custom environment variable name."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["CUSTOM_ENV_VAR"] = "async"
    assert (
        ProcessorMode.resolve(ProcessorMode.SYNC, "CUSTOM_ENV_VAR")
        == ProcessorMode.ASYNC
    )
