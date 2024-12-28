"""
Lambda extension implementation for lambda-otel-lite.

This module provides the internal Lambda extension functionality for asynchronous
span processing and graceful shutdown handling.
"""

import logging
import os
import signal
import sys
import threading
from collections.abc import Callable
from contextlib import contextmanager
from typing import Any

import urllib3
from opentelemetry.sdk.trace import TracerProvider

from . import ProcessorMode

# Setup logging
logger = logging.getLogger(__name__)

# Extension state
_extension_initialized: bool = False
_handler_ready: threading.Event = threading.Event()
_handler_complete: threading.Event = threading.Event()
_flush_counter: int = 0
_flush_threshold: int = int(os.getenv("LAMBDA_EXTENSION_SPAN_PROCESSOR_FREQUENCY", "1"))


def shutdown_telemetry(tracer_provider: TracerProvider, signum: int, _: Any) -> None:
    """Handle SIGTERM by flushing spans and shutting down.

    Args:
        tracer_provider: The TracerProvider to flush and shutdown
        signum: The signal number received
        _: Unused frame argument
    """
    logger.debug(f"[runtime] SIGTERM received ({signum}), flushing traces and shutting down")
    tracer_provider.force_flush()
    tracer_provider.shutdown()
    sys.exit(0)


def init_extension(
    mode: ProcessorMode,
    tracer_provider: TracerProvider,
    on_shutdown: Callable[[], None] | None = None,
) -> None:
    """Initialize the internal extension for receiving Lambda events.

    Args:
        mode: The processor mode (async or finalize)
        tracer_provider: The TracerProvider for span flushing
        on_shutdown: Optional callback to run before shutdown
    """
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
        signal.SIGTERM, lambda signum, frame: shutdown_telemetry(tracer_provider, signum, frame)
    )

    # Extension API endpoints
    register_url = f"http://{os.getenv('AWS_LAMBDA_RUNTIME_API')}/2020-01-01/extension/register"
    next_url = f"http://{os.getenv('AWS_LAMBDA_RUNTIME_API')}/2020-01-01/extension/event/next"

    # Create HTTP client for extension
    http = urllib3.PoolManager()

    def lambda_internal_extension(extension_id: str) -> None:
        """Extension loop for async mode - processes spans after each invoke"""
        global _flush_counter
        logger.info(f"[runtime] enter event loop for extension id: '{extension_id}'")

        while True:
            logger.debug(f"[runtime] extension's request: {next_url}")
            response = http.request(
                "GET",
                next_url,
                headers={"Lambda-Extension-Identifier": extension_id},
            )
            if response.status == 200:
                logger.debug(f"[runtime] extension's response: {response.status}")

                # Signal handler is ready
                _handler_ready.set()
                # Wait for handler completion
                _handler_complete.wait()
                # Reset handler completion event
                _handler_complete.clear()

                # Increment flush counter
                _flush_counter += 1
                # If flush counter is a multiple of flush threshold, flush spans
                if _flush_counter % _flush_threshold == 0:
                    with debug_timing(logger, "force_flush call"):
                        tracer_provider.force_flush()
                        logger.debug("[runtime] flushing traces after request #%d", _flush_counter)

    def wait_for_shutdown(extension_id: str) -> None:
        """Extension loop for finalize mode - just waits for SIGTERM"""
        logger.info(f"[runtime] waiting for shutdown, extension id: '{extension_id}'")
        http.request(
            "GET",
            next_url,
            headers={"Lambda-Extension-Identifier": extension_id},
        )
        logger.debug("[runtime] extension received shutdown event")
        if on_shutdown:
            on_shutdown()

    # Register the extension
    events = ["INVOKE"] if mode == ProcessorMode.ASYNC else []
    register_res = http.request(
        "POST",
        register_url,
        headers={"Lambda-Extension-Name": "internal"},
        json={"events": events},
    )
    extension_id = register_res.headers["Lambda-Extension-Identifier"]
    logger.info(f"[runtime] internal extension '{extension_id}' registered for mode: {mode.value}")

    # Start extension thread based on mode
    threading.Thread(
        target=lambda_internal_extension if mode == ProcessorMode.ASYNC else wait_for_shutdown,
        args=(extension_id,),
    ).start()

    _extension_initialized = True


@contextmanager
def debug_timing(logger: logging.Logger, operation_name: str) -> None:
    """Context manager for debug-level timing of operations."""
    if logger.isEnabledFor(logging.DEBUG):
        import time  # Only import if debug logging is enabled

        start_time = time.perf_counter()
        try:
            yield
        finally:
            end_time = time.perf_counter()
            duration = (end_time - start_time) * 1000
            logger.debug("%s: %.3fms", operation_name, duration)
    else:
        yield
