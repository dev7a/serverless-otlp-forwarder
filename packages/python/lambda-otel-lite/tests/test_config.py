"""Tests for the config module."""

import os
import pytest
from unittest.mock import patch, MagicMock

from lambda_otel_lite.config import get_bool_env, get_int_env, get_str_env


@pytest.fixture
def clean_env():
    """Fixture to clean environment variables before each test."""
    # Clear any environment variables that might affect the tests
    if "TEST_BOOL" in os.environ:
        del os.environ["TEST_BOOL"]
    if "TEST_INT" in os.environ:
        del os.environ["TEST_INT"]
    if "TEST_STR" in os.environ:
        del os.environ["TEST_STR"]
    yield


@patch("lambda_otel_lite.config.create_logger")
def test_get_bool_env_with_true(mock_create_logger, clean_env):
    """Test that get_bool_env returns True when the environment variable is 'true'."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_BOOL"] = "true"
    assert get_bool_env("TEST_BOOL") is True


@patch("lambda_otel_lite.config.create_logger")
def test_get_bool_env_with_false(mock_create_logger, clean_env):
    """Test that get_bool_env returns False when the environment variable is 'false'."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_BOOL"] = "false"
    assert get_bool_env("TEST_BOOL") is False


@patch("lambda_otel_lite.config.create_logger")
def test_get_bool_env_with_case_insensitive(mock_create_logger, clean_env):
    """Test that get_bool_env handles case-insensitive values."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_BOOL"] = "TRUE"
    assert get_bool_env("TEST_BOOL") is True

    os.environ["TEST_BOOL"] = "False"
    assert get_bool_env("TEST_BOOL") is False


@patch("lambda_otel_lite.config.create_logger")
def test_get_bool_env_with_config_value(mock_create_logger, clean_env):
    """Test that get_bool_env returns the config value when the environment variable is not set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert get_bool_env("TEST_BOOL", True) is True
    assert get_bool_env("TEST_BOOL", False) is False


@patch("lambda_otel_lite.config.create_logger")
def test_get_bool_env_with_default(mock_create_logger, clean_env):
    """Test that get_bool_env returns the default value when neither env var nor config is set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert get_bool_env("TEST_BOOL") is False  # Default is False
    assert get_bool_env("TEST_BOOL", None, True) is True


@patch("lambda_otel_lite.config.logger")
def test_get_bool_env_with_invalid_value(mock_logger, clean_env):
    """Test that get_bool_env logs a warning and returns the config value when the env var is invalid."""
    os.environ["TEST_BOOL"] = "invalid"
    assert get_bool_env("TEST_BOOL", True) is True
    assert get_bool_env("TEST_BOOL") is False
    mock_logger.warn.assert_called()


@patch("lambda_otel_lite.config.create_logger")
def test_get_bool_env_with_empty_value(mock_create_logger, clean_env):
    """Test that get_bool_env returns the config value when the env var is empty."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_BOOL"] = ""
    assert get_bool_env("TEST_BOOL", True) is True
    assert get_bool_env("TEST_BOOL") is False


@patch("lambda_otel_lite.config.create_logger")
def test_get_bool_env_with_whitespace(mock_create_logger, clean_env):
    """Test that get_bool_env handles whitespace in the environment variable."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_BOOL"] = "  true  "
    assert get_bool_env("TEST_BOOL") is True


@patch("lambda_otel_lite.config.create_logger")
def test_get_int_env_with_valid_value(mock_create_logger, clean_env):
    """Test that get_int_env returns the correct value when the environment variable is set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_INT"] = "123"
    assert get_int_env("TEST_INT") == 123

    os.environ["TEST_INT"] = "0"
    assert get_int_env("TEST_INT") == 0

    os.environ["TEST_INT"] = "-456"
    assert get_int_env("TEST_INT") == -456


@patch("lambda_otel_lite.config.create_logger")
def test_get_int_env_with_config_value(mock_create_logger, clean_env):
    """Test that get_int_env returns the config value when the environment variable is not set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert get_int_env("TEST_INT", 42) == 42


@patch("lambda_otel_lite.config.create_logger")
def test_get_int_env_with_default(mock_create_logger, clean_env):
    """Test that get_int_env returns the default value when neither env var nor config is set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert get_int_env("TEST_INT") == 0  # Default is 0
    assert get_int_env("TEST_INT", None, 42) == 42


@patch("lambda_otel_lite.config.logger")
def test_get_int_env_with_invalid_value(mock_logger, clean_env):
    """Test that get_int_env logs a warning and returns the config value when the env var is invalid."""
    os.environ["TEST_INT"] = "invalid"
    assert get_int_env("TEST_INT", 42) == 42
    assert get_int_env("TEST_INT") == 0
    mock_logger.warn.assert_called()


@patch("lambda_otel_lite.config.create_logger")
def test_get_int_env_with_empty_value(mock_create_logger, clean_env):
    """Test that get_int_env returns the config value when the env var is empty."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_INT"] = ""
    assert get_int_env("TEST_INT", 42) == 42
    assert get_int_env("TEST_INT") == 0


@patch("lambda_otel_lite.config.create_logger")
def test_get_int_env_with_whitespace(mock_create_logger, clean_env):
    """Test that get_int_env handles whitespace in the environment variable."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_INT"] = "  123  "
    assert get_int_env("TEST_INT") == 123


@patch("lambda_otel_lite.config.logger")
def test_get_int_env_with_validator(mock_logger, clean_env):
    """Test that get_int_env applies the validator function if provided."""
    os.environ["TEST_INT"] = "123"
    assert get_int_env("TEST_INT", 42, 0, lambda x: x > 100) == 123
    assert get_int_env("TEST_INT", 42, 0, lambda x: x > 200) == 42
    mock_logger.warn.assert_called_once()


@patch("lambda_otel_lite.config.create_logger")
def test_get_str_env_with_valid_value(mock_create_logger, clean_env):
    """Test that get_str_env returns the correct value when the environment variable is set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_STR"] = "hello"
    assert get_str_env("TEST_STR") == "hello"


@patch("lambda_otel_lite.config.create_logger")
def test_get_str_env_with_config_value(mock_create_logger, clean_env):
    """Test that get_str_env returns the config value when the environment variable is not set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert get_str_env("TEST_STR", "world") == "world"


@patch("lambda_otel_lite.config.create_logger")
def test_get_str_env_with_default(mock_create_logger, clean_env):
    """Test that get_str_env returns the default value when neither env var nor config is set."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    assert get_str_env("TEST_STR") == ""  # Default is empty string
    assert get_str_env("TEST_STR", None, "default") == "default"


@patch("lambda_otel_lite.config.create_logger")
def test_get_str_env_with_empty_value(mock_create_logger, clean_env):
    """Test that get_str_env returns the config value when the env var is empty."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_STR"] = ""
    assert get_str_env("TEST_STR", "world") == "world"
    assert get_str_env("TEST_STR") == ""


@patch("lambda_otel_lite.config.create_logger")
def test_get_str_env_with_whitespace(mock_create_logger, clean_env):
    """Test that get_str_env handles whitespace in the environment variable."""
    mock_logger = MagicMock()
    mock_create_logger.return_value = mock_logger

    os.environ["TEST_STR"] = "  hello  "
    assert get_str_env("TEST_STR") == "hello"


@patch("lambda_otel_lite.config.logger")
def test_get_str_env_with_validator(mock_logger, clean_env):
    """Test that get_str_env applies the validator function if provided."""
    os.environ["TEST_STR"] = "hello"
    assert get_str_env("TEST_STR", "world", "", lambda x: len(x) > 3) == "hello"
    assert get_str_env("TEST_STR", "world", "", lambda x: len(x) > 5) == "world"
    mock_logger.warn.assert_called_once()
