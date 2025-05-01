import base64
import gzip
import json
import os
import sys
import logging
from abc import ABC, abstractmethod
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Dict, Optional

from opentelemetry.exporter.otlp.proto.common.trace_encoder import encode_spans
from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.sdk.trace.export import SpanExporter, SpanExportResult

from .constants import EnvVars, Defaults, LogLevel, OutputType
from .version import VERSION

# Set up logger
logger = logging.getLogger(__name__)


class Output(ABC):
    """Interface for output handling."""

    @abstractmethod
    def write_line(self, line: str) -> bool:
        """
        Write a line to the output.

        Args:
            line: The line to write

        Returns:
            bool: True if successful, False otherwise
        """
        pass


class StdOutput(Output):
    """Standard output implementation that writes to stdout."""

    def write_line(self, line: str) -> bool:
        """
        Write a line to stdout.

        Args:
            line: The line to write

        Returns:
            bool: True if successful, False otherwise
        """
        try:
            print(line)
            return True
        except Exception as e:
            logger.error(f"Failed to write to stdout: {e}")
            return False


class NamedPipeOutput(Output):
    """Output implementation that writes to a named pipe."""

    def __init__(self) -> None:
        """Initialize the named pipe output."""
        self.pipe_path = Path(Defaults.PIPE_PATH)

        # Check if pipe exists once during initialization
        self.pipe_exists = self.pipe_path.exists()
        if not self.pipe_exists:
            logger.warning(
                f"Named pipe does not exist: {self.pipe_path}, will fall back to stdout"
            )

    def write_line(self, line: str) -> bool:
        """
        Write a line to the named pipe.

        Args:
            line: The line to write

        Returns:
            bool: True if successful, False otherwise
        """
        if not self.pipe_exists:
            # Fall back to stdout if pipe doesn't exist
            return StdOutput().write_line(line)

        try:
            with open(self.pipe_path, "w") as pipe:
                pipe.write(line + "\n")
            return True
        except Exception as e:
            logger.warning(f"Failed to write to pipe: {e}, falling back to stdout")
            return StdOutput().write_line(line)


def create_output(output_type: OutputType) -> Output:
    """
    Create an output implementation based on the specified type.

    Args:
        output_type: The output type (stdout or pipe)

    Returns:
        Output: The output implementation
    """
    if output_type == OutputType.PIPE:
        return NamedPipeOutput()
    return StdOutput()


def parse_log_level(value: str) -> Optional[LogLevel]:
    """
    Parse log level from string.

    Args:
        value: The string value to parse

    Returns:
        Optional[LogLevel]: The parsed LogLevel or None if invalid
    """
    try:
        normalized = value.lower()
        if normalized == "debug":
            return LogLevel.DEBUG
        if normalized == "info":
            return LogLevel.INFO
        if normalized in ("warn", "warning"):
            return LogLevel.WARN
        if normalized == "error":
            return LogLevel.ERROR
    except Exception:
        pass
    return None


class OTLPStdoutSpanExporter(SpanExporter):
    """
    An OpenTelemetry span exporter that writes spans to stdout in OTLP format.

    This exporter is particularly useful in serverless environments like AWS Lambda
    where writing to stdout is a common pattern for exporting telemetry data.

    Features:
    - Uses OTLP Protobuf serialization for efficient encoding
    - Applies GZIP compression with configurable levels
    - Detects service name from environment variables
    - Supports custom headers via environment variables
    - Supports log level for filtering in log aggregation systems
    - Supports writing to stdout or named pipe

    Environment Variables:
    - OTEL_SERVICE_NAME: Service name to use in output
    - AWS_LAMBDA_FUNCTION_NAME: Fallback service name (if OTEL_SERVICE_NAME not set)
    - OTEL_EXPORTER_OTLP_HEADERS: Global headers for OTLP export
    - OTEL_EXPORTER_OTLP_TRACES_HEADERS: Trace-specific headers (takes precedence)
    - OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL: GZIP compression level (0-9). Defaults to 6.
    - OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL: Log level (debug, info, warn, error)
    - OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE: Output type (stdout, pipe)

    Output Format:
    ```json
    {
      "__otel_otlp_stdout": "0.1.0",
      "source": "my-service",
      "endpoint": "http://localhost:4318/v1/traces",
      "method": "POST",
      "content-type": "application/x-protobuf",
      "content-encoding": "gzip",
      "headers": {
        "api-key": "secret123",
        "custom-header": "value"
      },
      "payload": "<base64-encoded-gzipped-protobuf>",
      "base64": true,
      "level": "INFO"
    }
    ```
    """

    def __init__(
        self,
        *,
        gzip_level: Optional[int] = None,
        log_level: Optional[LogLevel] = None,
        output_type: Optional[OutputType] = None,
    ) -> None:
        """
        Creates a new OTLPStdoutSpanExporter

        Args:
            gzip_level: GZIP compression level (0-9). Defaults to 6.
            log_level: Log level for the exported spans.
            output_type: Output type (stdout or pipe).
        """
        super().__init__()

        # Set gzip_level with proper precedence (env var > constructor param > default)
        env_value = os.environ.get(EnvVars.COMPRESSION_LEVEL)
        if env_value is not None:
            try:
                parsed_value = int(env_value)
                if 0 <= parsed_value <= 9:
                    self._gzip_level = parsed_value
                else:
                    logger.warning(
                        f"Invalid value in {EnvVars.COMPRESSION_LEVEL}: {env_value} (must be 0-9), "
                        f"using fallback"
                    )
                    self._gzip_level = (
                        gzip_level
                        if gzip_level is not None
                        else Defaults.COMPRESSION_LEVEL
                    )
            except ValueError:
                logger.warning(
                    f"Failed to parse {EnvVars.COMPRESSION_LEVEL}: {env_value}, using fallback"
                )
                self._gzip_level = (
                    gzip_level if gzip_level is not None else Defaults.COMPRESSION_LEVEL
                )
        else:
            # No environment variable, use parameter or default
            self._gzip_level = (
                gzip_level if gzip_level is not None else Defaults.COMPRESSION_LEVEL
            )

        # Set log level with proper precedence (env var > constructor param)
        self._log_level = None
        log_level_env = os.environ.get(EnvVars.LOG_LEVEL)
        if log_level_env is not None:
            parsed_log_level = parse_log_level(log_level_env)
            if parsed_log_level is not None:
                self._log_level = parsed_log_level
            else:
                logger.warning(
                    f"Invalid log level in {EnvVars.LOG_LEVEL}: {log_level_env}, "
                    f"log level will not be included in output"
                )
                self._log_level = log_level
        else:
            # No environment variable, use parameter
            self._log_level = log_level

        # Set output type with proper precedence (env var > constructor param > default)
        output_type_env = os.environ.get(EnvVars.OUTPUT_TYPE)
        if output_type_env is not None:
            if output_type_env.lower() == "pipe":
                self._output_type = OutputType.PIPE
            else:
                self._output_type = OutputType.STDOUT
        elif output_type is not None:
            self._output_type = output_type
        else:
            self._output_type = OutputType.STDOUT

        self._endpoint = Defaults.ENDPOINT
        self._service_name = os.environ.get(EnvVars.SERVICE_NAME) or os.environ.get(
            EnvVars.AWS_LAMBDA_FUNCTION_NAME, Defaults.SERVICE_NAME
        )
        self._headers = self._parse_headers()
        self._output = create_output(self._output_type)

    def _parse_headers(self) -> Dict[str, str]:
        """
        Parse headers from environment variables.
        Headers should be in the format: key1=value1,key2=value2
        Filters out content-type and content-encoding as they are fixed.
        If both OTLP_TRACES_HEADERS and OTLP_HEADERS are defined, merges them with
        OTLP_TRACES_HEADERS taking precedence.

        Returns:
            dict: Header key-value pairs
        """
        headers: Dict[str, str] = {}
        header_vars = [
            os.environ.get(EnvVars.OTLP_HEADERS),  # General headers first
            os.environ.get(
                EnvVars.OTLP_TRACES_HEADERS
            ),  # Trace-specific headers override
        ]

        for header_str in header_vars:
            if header_str:
                headers.update(self._parse_header_string(header_str))

        return headers

    def _parse_header_string(self, header_str: str) -> Dict[str, str]:
        """
        Parse a header string in the format key1=value1,key2=value2

        Args:
            header_str: The header string to parse

        Returns:
            dict: Header key-value pairs
        """
        headers: Dict[str, str] = {}
        for pair in header_str.split(","):
            if "=" not in pair:
                continue
            key, *value_parts = pair.strip().split("=")
            key = key.strip().lower()
            if key and value_parts and key not in ["content-type", "content-encoding"]:
                headers[key] = "=".join(value_parts).strip()
        return headers

    def export(self, spans: Sequence[ReadableSpan]) -> SpanExportResult:
        """
        Exports the spans by serializing them to OTLP Protobuf format, compressing with GZIP,
        and writing to the configured output as a structured JSON object.

        Args:
            spans: The spans to export

        Returns:
            SpanExportResult indicating success or failure
        """
        # Check for empty batch first to avoid duplicate checks
        if not spans:
            # For empty spans with named pipe, perform the "pipe touch" operation
            if isinstance(self._output, NamedPipeOutput):
                try:
                    # Perform the "pipe touch" operation: open for writing and immediately close
                    with open(self._output.pipe_path, "w"):
                        pass  # Just need to open and close
                    return SpanExportResult.SUCCESS
                except Exception as e:
                    logger.error(f"Error touching pipe: {e}")
                    return SpanExportResult.FAILURE
            # For stdout output with empty spans, return success without writing anything
            return SpanExportResult.SUCCESS

        # Process non-empty batches
        try:
            # Serialize spans to protobuf format
            serialized_data = encode_spans(spans).SerializeToString()

            # Handle case where serialization returns empty data
            if not serialized_data:
                logger.debug(
                    "encode_spans resulted in empty data, likely invalid batch."
                )
                return SpanExportResult.SUCCESS

            # Compress the serialized data using GZIP
            compressed_data = gzip.compress(
                serialized_data, compresslevel=self._gzip_level
            )

            # Create the output object with metadata and payload
            output: Dict[str, Any] = {
                "__otel_otlp_stdout": VERSION,
                "source": self._service_name,
                "endpoint": self._endpoint,
                "method": "POST",
                "content-type": "application/x-protobuf",
                "content-encoding": "gzip",
                "payload": base64.b64encode(compressed_data).decode("utf-8"),
                "base64": True,
            }

            # Add headers section only if there are custom headers
            if self._headers:
                output["headers"] = self._headers

            # Add log level if configured
            if self._log_level is not None:
                output["level"] = self._log_level.value

            # Write the formatted output to the configured output
            if self._output.write_line(json.dumps(output)):
                return SpanExportResult.SUCCESS
            else:
                return SpanExportResult.FAILURE

        except Exception as e:
            # Log the error but don't raise it
            print(f"Error in OTLPStdoutSpanExporter: {e}", file=sys.stderr)
            return SpanExportResult.FAILURE

    def force_flush(self, timeout_millis: float = 30000) -> bool:
        """
        Force flush is a no-op for this exporter as it writes immediately

        Args:
            timeout_millis: The maximum amount of time to wait for force flush to complete

        Returns:
            bool: True, as there's nothing to flush
        """
        return True

    def shutdown(self) -> None:
        """
        Shuts down the exporter. This is a no-op as stdout doesn't need cleanup.
        """
        pass
