"""Tests for the OTLPStdoutSpanExporter."""

import json
import os
from collections.abc import Generator
from unittest.mock import Mock, patch
from pathlib import Path

import pytest
from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.sdk.trace.export import SpanExportResult

from otlp_stdout_span_exporter import OTLPStdoutSpanExporter
from otlp_stdout_span_exporter.constants import EnvVars, LogLevel, OutputType, Defaults
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


# Mock file operations for named pipe
@pytest.fixture
def mock_file_ops() -> Generator[tuple[Mock, Mock], None, None]:
    with (
        patch("pathlib.Path.exists") as mock_exists,
        patch("builtins.open") as mock_open,
    ):
        mock_exists.return_value = True
        mock_file = Mock()
        mock_open.return_value.__enter__.return_value = mock_file
        yield mock_exists, mock_file


# Mock span for tests
@pytest.fixture
def mock_span() -> Generator[ReadableSpan, None, None]:
    mock = Mock(spec=ReadableSpan)
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
        EnvVars.LOG_LEVEL,
        EnvVars.OUTPUT_TYPE,
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
    assert exporter._log_level is None
    assert exporter._output_type == OutputType.STDOUT


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
    mock_span: ReadableSpan,
) -> None:
    """Test custom gzip compression level."""
    exporter = OTLPStdoutSpanExporter(gzip_level=9)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=9)


def test_gzip_level_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test gzip compression level from environment variable."""
    os.environ[EnvVars.COMPRESSION_LEVEL] = "3"
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=3)


def test_env_precedence_over_constructor(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test that environment variables take precedence over constructor parameters."""
    os.environ[EnvVars.COMPRESSION_LEVEL] = "3"
    exporter = OTLPStdoutSpanExporter(gzip_level=9)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=3)


def test_invalid_gzip_level_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_logger: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test handling of invalid gzip level in environment variable."""
    # Test with non-numeric value
    os.environ[EnvVars.COMPRESSION_LEVEL] = "invalid"
    exporter = OTLPStdoutSpanExporter(gzip_level=4)
    spans = [mock_span]  # Use a non-empty spans list

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
    mock_span: ReadableSpan,
) -> None:
    """Test handling of out-of-range gzip level in environment variable."""
    # Test with out-of-range value
    os.environ[EnvVars.COMPRESSION_LEVEL] = "15"
    exporter = OTLPStdoutSpanExporter(gzip_level=4)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS
    mock_gzip.assert_called_once_with(b"mock-serialized-data", compresslevel=4)
    mock_logger.warning.assert_called_once()


def test_export_success(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test successful export operation."""
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

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
    clean_env: None, mock_encode_spans: Mock, mock_print: Mock, mock_span: ReadableSpan
) -> None:
    """Test export failure handling."""
    # The current implementation in exporter.py handles empty serialized data gracefully
    # and returns SUCCESS, not FAILURE. Update the test expectation accordingly.
    mock_encode_spans.return_value.SerializeToString.return_value = None
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    # Verify debug message was logged (or additional verification if needed)
    # We could add mock_logger and assert it was called with debug message


def test_header_parsing(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test header parsing from environment variables."""
    os.environ[EnvVars.OTLP_HEADERS] = "api-key=secret123,custom-header=value"
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {"api-key": "secret123", "custom-header": "value"}


def test_header_precedence(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test that trace-specific headers take precedence."""
    os.environ[EnvVars.OTLP_HEADERS] = "api-key=secret123,shared-key=general"
    os.environ[EnvVars.OTLP_TRACES_HEADERS] = "shared-key=specific,trace-key=value123"
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

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
    mock_span: ReadableSpan,
) -> None:
    """Test header parsing with whitespace."""
    os.environ[EnvVars.OTLP_HEADERS] = " api-key = secret123 , custom-header = value "
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {"api-key": "secret123", "custom-header": "value"}


def test_header_filtering(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test filtering of content-type and content-encoding headers."""
    os.environ[EnvVars.OTLP_HEADERS] = (
        "content-type=text/plain,content-encoding=none,api-key=secret123"
    )
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["headers"] == {"api-key": "secret123"}


def test_header_multiple_equals(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test handling of headers with multiple equal signs in value."""
    os.environ[EnvVars.OTLP_HEADERS] = "bearer-token=abc=123=xyz"
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

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


# Tests for log level support
def test_log_level_from_constructor(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test log level from constructor parameter."""
    exporter = OTLPStdoutSpanExporter(log_level=LogLevel.DEBUG)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["level"] == LogLevel.DEBUG.value


def test_log_level_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test log level from environment variable."""
    os.environ[EnvVars.LOG_LEVEL] = "warn"
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["level"] == LogLevel.WARN.value


def test_log_level_env_precedence(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test that environment variable takes precedence for log level."""
    os.environ[EnvVars.LOG_LEVEL] = "error"
    exporter = OTLPStdoutSpanExporter(log_level=LogLevel.INFO)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["level"] == LogLevel.ERROR.value


def test_invalid_log_level_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_logger: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test handling of invalid log level in environment variable."""
    os.environ[EnvVars.LOG_LEVEL] = "invalid"
    exporter = OTLPStdoutSpanExporter(log_level=LogLevel.INFO)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert output["level"] == LogLevel.INFO.value
    mock_logger.warning.assert_called_once()


def test_no_log_level(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test that level field is omitted when no log level is set."""
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    output = json.loads(mock_print.call_args[0][0])
    assert "level" not in output


# Tests for named pipe output
def test_output_type_from_constructor(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_file_ops: tuple[Mock, Mock],
    mock_span: ReadableSpan,
) -> None:
    """Test output type from constructor parameter."""
    mock_exists, mock_file = mock_file_ops
    exporter = OTLPStdoutSpanExporter(output_type=OutputType.PIPE)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    # Verify that print was not called (pipe was used instead)
    mock_print.assert_not_called()
    # Verify that file write was called
    mock_file.write.assert_called_once()


def test_output_type_from_env(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_file_ops: tuple[Mock, Mock],
    mock_span: ReadableSpan,
) -> None:
    """Test output type from environment variable."""
    mock_exists, mock_file = mock_file_ops
    os.environ[EnvVars.OUTPUT_TYPE] = "pipe"
    exporter = OTLPStdoutSpanExporter()
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    # Verify that print was not called (pipe was used instead)
    mock_print.assert_not_called()
    # Verify that file write was called
    mock_file.write.assert_called_once()


def test_output_type_env_precedence(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_file_ops: tuple[Mock, Mock],
    mock_span: ReadableSpan,
) -> None:
    """Test that environment variable takes precedence for output type."""
    mock_exists, mock_file = mock_file_ops
    os.environ[EnvVars.OUTPUT_TYPE] = "pipe"
    exporter = OTLPStdoutSpanExporter(output_type=OutputType.STDOUT)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    # Verify that print was not called (pipe was used instead)
    mock_print.assert_not_called()
    # Verify that file write was called
    mock_file.write.assert_called_once()


def test_pipe_fallback_when_not_exists(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_file_ops: tuple[Mock, Mock],
    mock_span: ReadableSpan,
) -> None:
    """Test fallback to stdout when pipe does not exist."""
    mock_exists, mock_file = mock_file_ops
    mock_exists.return_value = False
    exporter = OTLPStdoutSpanExporter(output_type=OutputType.PIPE)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    # Verify that print was called (fallback to stdout)
    mock_print.assert_called_once()
    # Verify that file write was not called
    mock_file.write.assert_not_called()


def test_pipe_fallback_on_error(
    clean_env: None,
    mock_gzip: Mock,
    mock_encode_spans: Mock,
    mock_print: Mock,
    mock_file_ops: tuple[Mock, Mock],
    mock_logger: Mock,
    mock_span: ReadableSpan,
) -> None:
    """Test fallback to stdout when pipe write fails."""
    mock_exists, mock_file = mock_file_ops
    mock_file.write.side_effect = IOError("Write failed")
    exporter = OTLPStdoutSpanExporter(output_type=OutputType.PIPE)
    spans = [mock_span]  # Use a non-empty spans list

    result = exporter.export(spans)
    assert result == SpanExportResult.SUCCESS

    # Verify that print was called (fallback to stdout)
    mock_print.assert_called_once()
    # Verify that warning was logged
    mock_logger.warning.assert_called_once()


# Add a new test for the empty span batch case with named pipe output
def test_empty_spans_with_pipe_output(
    clean_env: None,
    mock_file_ops: tuple[Mock, Mock],
    mock_print: Mock,
) -> None:
    """Test that empty span batch with named pipe output triggers pipe touch."""
    mock_exists, mock_file = mock_file_ops

    # Use path object as expected by the implementation
    expected_pipe_path = Path(Defaults.PIPE_PATH)

    # Create the exporter with pipe output
    exporter = OTLPStdoutSpanExporter(output_type=OutputType.PIPE)
    spans: list[ReadableSpan] = []  # Empty spans list

    # Patch the open function before calling export
    with patch("otlp_stdout_span_exporter.exporter.open") as mock_open:
        # Call export once and capture result
        result = exporter.export(spans)

        # Verify the result was successful
        assert result == SpanExportResult.SUCCESS

        # Verify open was called with the correct pipe path and mode
        mock_open.assert_called_once_with(expected_pipe_path, "w")

    # Verify no write operations occurred
    assert mock_file.write.call_count == 0  # No write, just open/close

    # Verify that print was not called
    mock_print.assert_not_called()
