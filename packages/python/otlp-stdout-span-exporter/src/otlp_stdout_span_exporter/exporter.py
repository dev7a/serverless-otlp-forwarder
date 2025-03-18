import base64
import gzip
import json
import os
import sys
import logging
from collections.abc import Sequence
from typing import Any

from opentelemetry.exporter.otlp.proto.common.trace_encoder import encode_spans
from opentelemetry.sdk.trace import ReadableSpan
from opentelemetry.sdk.trace.export import SpanExporter, SpanExportResult

from .constants import EnvVars, Defaults
from .version import VERSION

# Set up logger
logger = logging.getLogger(__name__)


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

    Environment Variables:
    - OTEL_SERVICE_NAME: Service name to use in output
    - AWS_LAMBDA_FUNCTION_NAME: Fallback service name (if OTEL_SERVICE_NAME not set)
    - OTEL_EXPORTER_OTLP_HEADERS: Global headers for OTLP export
    - OTEL_EXPORTER_OTLP_TRACES_HEADERS: Trace-specific headers (takes precedence)
    - OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL: GZIP compression level (0-9). Defaults to 6.

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
      "base64": true
    }
    ```
    """

    def __init__(self, *, gzip_level: int | None = None) -> None:
        """
        Creates a new OTLPStdoutSpanExporter

        Args:
            gzip_level: GZIP compression level (0-9). Defaults to 6.
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

        self._endpoint = Defaults.ENDPOINT
        self._service_name = os.environ.get(EnvVars.SERVICE_NAME) or os.environ.get(
            EnvVars.AWS_LAMBDA_FUNCTION_NAME, Defaults.SERVICE_NAME
        )
        self._headers = self._parse_headers()

    def _parse_headers(self) -> dict[str, str]:
        """
        Parse headers from environment variables.
        Headers should be in the format: key1=value1,key2=value2
        Filters out content-type and content-encoding as they are fixed.
        If both OTLP_TRACES_HEADERS and OTLP_HEADERS are defined, merges them with
        OTLP_TRACES_HEADERS taking precedence.

        Returns:
            dict: Header key-value pairs
        """
        headers: dict[str, str] = {}
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

    def _parse_header_string(self, header_str: str) -> dict[str, str]:
        """
        Parse a header string in the format key1=value1,key2=value2

        Args:
            header_str: The header string to parse

        Returns:
            dict: Header key-value pairs
        """
        headers: dict[str, str] = {}
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
        and writing to stdout as a structured JSON object.

        Args:
            spans: The spans to export

        Returns:
            SpanExportResult indicating success or failure
        """
        try:
            # Serialize spans to protobuf format
            serialized_data = encode_spans(spans).SerializeToString()
            if not serialized_data:
                return SpanExportResult.FAILURE

            # Compress the serialized data using GZIP
            compressed_data = gzip.compress(
                serialized_data, compresslevel=self._gzip_level
            )

            # Create the output object with metadata and payload
            output: dict[str, Any] = {
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

            # Write the formatted output to stdout
            print(json.dumps(output))
            return SpanExportResult.SUCCESS

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
