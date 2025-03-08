"""
Lambda extension implementation for lambda-otel-lite.

This module provides the internal Lambda extension functionality for asynchronous
span processing and graceful shutdown handling.
"""

import http.client
import json
import os
import signal
import sys
import threading
from collections.abc import Callable
from typing import Any

from opentelemetry.sdk.trace import TracerProvider

from . import ProcessorMode
from .logger import create_logger

# Setup logging
logger = create_logger("extension")

# Extension state
_extension_initialized: bool = False
handler_complete_event: threading.Event = threading.Event()
handler_complete_event.clear()

# HTTP connection state
_http_conn: http.client.HTTPConnection | None = None


def _close_http_connection() -> None:
    """Close the HTTP connection and reset the global state."""
    global _http_conn
    if _http_conn is not None:
        try:
            _http_conn.close()
        except Exception as e:
            logger.warn("Error closing HTTP connection:", e)
        finally:
            _http_conn = None


def _get_http_connection() -> http.client.HTTPConnection:
    """Get or create the HTTP connection to the Lambda Runtime API."""
    global _http_conn
    if _http_conn is None:
        runtime_api = os.getenv("AWS_LAMBDA_RUNTIME_API", "")
        _http_conn = http.client.HTTPConnection(
            runtime_api,
            timeout=2.0,  # Set a reasonable timeout
        )
    return _http_conn


def _make_request(
    method: str,
    path: str,
    headers: dict[str, str],
    body: str | None = None,
) -> tuple[int, dict[str, str], bytes]:
    """Make an HTTP request and ensure the response is fully read."""
    try:
        conn = _get_http_connection()
        conn.request(method, path, body, headers)
        response = conn.getresponse()
        response_body = response.read()  # Always read the full body
        return response.status, dict(response.getheaders()), response_body
    except (http.client.HTTPException, OSError) as e:
        logger.warn("HTTP error during request:", e)
        _close_http_connection()  # Reset connection on error
        raise


def shutdown_telemetry(tracer_provider: TracerProvider, signum: int, _: Any) -> None:
    """Handle SIGTERM by flushing spans and shutting down."""
    logger.debug("SIGTERM received (%d), flushing traces and shutting down", signum)
    tracer_provider.force_flush()
    tracer_provider.shutdown()  # type: ignore[no-untyped-call]

    # Clean up HTTP connection
    _close_http_connection()

    sys.exit(0)


def init_extension(
    mode: ProcessorMode,
    tracer_provider: TracerProvider,
    on_shutdown: Callable[[], None] | None = None,
) -> None:
    """Initialize the internal extension for receiving Lambda events."""
    global _extension_initialized

    # If extension is already initialized or we're in sync mode, return
    if (
        _extension_initialized
        or mode == ProcessorMode.SYNC
        or not os.getenv("AWS_LAMBDA_RUNTIME_API")
    ):
        return

    # Register SIGTERM handler
    signal.signal(
        signal.SIGTERM,
        lambda signum, frame: shutdown_telemetry(tracer_provider, signum, frame),
    )

    # Extension API paths
    register_path = "/2020-01-01/extension/register"
    next_path = "/2020-01-01/extension/event/next"

    def lambda_internal_extension(extension_id: str) -> None:
        """Extension loop for async mode - processes spans after each invoke"""
        logger.debug("Entering event loop for extension id: '%s'", extension_id)

        while True:
            logger.debug("Requesting next event: %s", next_path)
            try:
                status, _, _ = _make_request(
                    "GET",
                    next_path,
                    headers={"Lambda-Extension-Identifier": extension_id},
                )
                if status == 200:
                    logger.debug("Received next event, waiting for completion")
                    # Wait for handler completion
                    handler_complete_event.wait()
                    # Reset handler completion event
                    handler_complete_event.clear()
                    # Flush spans after every request
                    logger.debug("Received completion, flushing traces")
                    tracer_provider.force_flush()
                else:
                    logger.error("Unexpected status code from next: %d", status)
            except (http.client.HTTPException, OSError) as e:
                logger.warn("HTTP error in extension loop:", e)

    def wait_for_shutdown(extension_id: str) -> None:
        """Extension loop for finalize mode - just waits for SIGTERM"""
        logger.debug("Waiting for shutdown, extension id: '%s'", extension_id)

        try:
            status, _, _ = _make_request(
                "GET",
                next_path,
                headers={"Lambda-Extension-Identifier": extension_id},
            )
            if status == 200:
                logger.debug("Received shutdown event")
                if on_shutdown:
                    on_shutdown()
            else:
                logger.error("Unexpected status code from next: %d", status)
        except (http.client.HTTPException, OSError) as e:
            logger.warn("HTTP error in shutdown wait:", e)

    # Start by registering the extension
    events = ["INVOKE"] if mode == ProcessorMode.ASYNC else []

    try:
        status, headers, _ = _make_request(
            "POST",
            register_path,
            headers={
                "Lambda-Extension-Name": "internal",
                "Content-Type": "application/json",
            },
            body=json.dumps({"events": events}),
        )
        if status != 200:
            raise RuntimeError(f"Extension registration failed with status: {status}")

        extension_id = headers.get("Lambda-Extension-Identifier")
        if not extension_id:
            raise ValueError("No extension ID received in registration response")

        logger.debug(
            "Internal extension '%s' registered for mode: %s", extension_id, mode.value
        )

        # Start extension thread based on mode
        threading.Thread(
            target=lambda_internal_extension
            if mode == ProcessorMode.ASYNC
            else wait_for_shutdown,
            args=(extension_id,),
        ).start()

        _extension_initialized = True
    except (http.client.HTTPException, OSError) as e:
        logger.error("Failed to register extension:", e)
        raise
