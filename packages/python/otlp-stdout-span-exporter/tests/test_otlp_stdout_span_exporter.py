"""Tests for the OTLPStdoutSpanExporter."""

import json
import os
from collections.abc import Generator
from unittest.mock import Mock, patch

import pytest
from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.sdk.trace.export import SpanExportResult

from otlp_stdout_span_exporter import OTLPStdoutSpanExporter
from otlp_stdout_span_exporter.constants import EnvVars
from otlp_stdout_span_exporter.version import VERSION


# Mock the encode_spans function
@pytest.fixture
def mock_encode_spans() -> Generator[Mock, None, None]:
    with patch("otlp_stdout_span_exporter.exporter.encode_spans") as mock:
        mock_proto = Mock()
        mock_proto.SerializeToString.return_value = b"mock-serialized-data"
        mock.return_value = mock_proto
        yield mock


# Mock gzip compression
@pytest.fixture
def mock_gzip() -> Generator[Mock, None, None]:
    with patch("gzip.compress") as mock:
        mock.return_value = b"mock-compressed-data"
        yield mock


# Mock print function
@pytest.fixture
def mock_print() -> Generator[Mock, None, None]:
    with patch("builtins.print") as mock:
        yield mock


# Mock logger to capture warnings
@pytest.fixture
def mock_logger() -> Generator[Mock, None, None]:
    with patch("otlp_stdout_span_exporter.exporter.logger") as mock:
        yield mock


@pytest.fixture
def clean_env() -> Generator[None, None, None]:
    """Clean environment variables before each test."""
    original_env = dict(os.environ)

    # Clear relevant environment variables
    env_vars = [
        EnvVars.SERVICE_NAME,
        EnvVars.AWS_LAMBDA_FUNCTION_NAME,
        EnvVars.OTLP_HEADERS,
        EnvVars.OTLP_TRACES_HEADERS,
        EnvVars.COMPRESSION_LEVEL,
    ]
    for var in env_vars:
        if var in os.environ:
            del os.environ[var]

    yield

    # Restore original environment
    os.environ.clear()
    os.environ.update(original_env)


def test_default_values(clean_env: None, mock_print: Mock) -> None:
    """Test default values when no config is provided."""
    exporter = OTLPStdoutSpanExporter()
    assert exporter._gzip_level == 6
    assert exporter._endpoint == "http://localhost:4318/v1/traces"
    assert exporter._service_name == "unknown-service"
    assert exporter._headers == {}


def test_service_name_from_env(clean_env: None, mock_print: Mock) -> None:
    """Test service name from environment variables."""
    os.environ[EnvVars.SERVICE_NAME] = "test-service"
    exporter = OTLPStdoutSpanExporter()
    assert exporter._service_name == "test-service"


def test_service_name_fallback(clean_env: None, mock_print: Mock) -> None:
    """Test service name fallback to AWS_LAMBDA_FUNCTION_NAME."""
    os.environ[EnvVars.AWS_LAMBDA_FUNCTION_NAME] = "lambda-function"
    exporter = OTLPStdoutSpanExporter()
    assert exporter._service_name == "lambda-function"


def test_custom_gzip_level(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test custom gzip compression level."""
    exporter = OTLPStdoutSpanExporter(gzip_level=9)
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=9)


def test_gzip_level_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test gzip compression level from environment variable."""
    os.environ[EnvVars.COMPRESSION_LEVEL] = "3"
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=3)


def test_env_precedence_over_constructor(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test that environment variables take precedence over constructor parameters."""
    os.environ[EnvVars.COMPRESSION_LEVEL] = "3"
    exporter = OTLPStdoutSpanExporter(gzip_level=9)
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=3)


def test_invalid_gzip_level_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_logger: Mock,
) -> None:
    """Test handling of invalid gzip level in environment variable."""
    # Test with non-numeric value
    os.environ[EnvVars.COMPRESSION_LEVEL] = "invalid"
    exporter = OTLPStdoutSpanExporter(gzip_level=4)
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=4)
    mock_logger.warning.assert_called_once()


def test_out_of_range_gzip_level_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_logger: Mock,
) -> None:
    """Test handling of out-of-range gzip level in environment variable."""
    # Test with out-of-range value
    os.environ[EnvVars.COMPRESSION_LEVEL] = "15"
    exporter = OTLPStdoutSpanExporter(gzip_level=4)
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=4)
    mock_logger.warning.assert_called_once()


def test_export_success(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test successful export operation."""
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    # Verify output format
    output = json.loads(mock_print.call_args[0][0])
    assert (
        output
        == {
            "__otel_otlp_stdout": VERSION,
            "source": "unknown-service",
            "endpoint": "http://localhost:4318/v1/traces",
            "method": "POST",
            "content-type": "application/x-protobuf",
            "content-encoding": "gzip",
            "payload": "bW9jay1jb21wcmVzc2VkLWRhdGE=",  # base64 encoded 'mock-compressed-data'
            "base64": True,
        }
    )


def test_export_failure(
    clean_env: None, mock_encode_spans: Mock, mock_print: Mock
) -> None:
    """Test export failure handling."""
    mock_encode_spans.return_value.SerializeToString.return_value = None
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.FAILURE


def test_header_parsing(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test header parsing from environment variables."""
    os.environ[EnvVars.OTLP_HEADERS] = "api-key=secret123,custom-header=value"
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {"api-key": "secret123", "custom-header": "value"}


def test_header_precedence(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test that trace-specific headers take precedence."""
    os.environ[EnvVars.OTLP_HEADERS] = "api-key=secret123,shared-key=general"
    os.environ[EnvVars.OTLP_TRACES_HEADERS] = "shared-key=specific,trace-key=value123"
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {
        "api-key": "secret123",
        "shared-key": "specific",  # TRACES_HEADERS value takes precedence
        "trace-key": "value123",
    }


def test_header_whitespace_handling(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test header parsing with whitespace."""
    os.environ[EnvVars.OTLP_HEADERS] = " api-key = secret123 , custom-header = value "
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {"api-key": "secret123", "custom-header": "value"}


def test_header_filtering(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test filtering of content-type and content-encoding headers."""
    os.environ[EnvVars.OTLP_HEADERS] = (
        "content-type=text/plain,content-encoding=none,api-key=secret123"
    )
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {"api-key": "secret123"}


def test_header_multiple_equals(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
) -> None:
    """Test handling of headers with multiple equal signs in value."""
    os.environ[EnvVars.OTLP_HEADERS] = "bearer-token=abc=123=xyz"
    exporter = OTLPStdoutSpanExporter()
    spans: list[ReadableSpan] = []

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {"bearer-token": "abc=123=xyz"}


def test_force_flush(clean_env: None) -> None:
    """Test force_flush operation."""
    exporter = OTLPStdoutSpanExporter()
    assert exporter.force_flush() is True


def test_shutdown(clean_env: None) -> None:
    """Test shutdown operation."""
    exporter = OTLPStdoutSpanExporter()
    exporter.shutdown()  # Should not raise any exceptions
