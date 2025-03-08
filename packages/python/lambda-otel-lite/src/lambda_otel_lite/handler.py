"""
Handler implementation for lambda-otel-lite.

This module provides the traced_handler decorator for instrumenting Lambda handlers
with OpenTelemetry tracing.
"""

from collections.abc import Callable
from functools import wraps
from typing import Any, TypeVar, cast

from opentelemetry.propagate import extract
from opentelemetry.trace import Status, StatusCode

from .extractors import SpanAttributes, default_extractor
from .logger import create_logger
from .telemetry import TelemetryCompletionHandler

# Setup logging
logger = create_logger("handler")

# Global state
_is_cold_start: bool = True

# Type variables for handler return type
TResult = TypeVar("TResult")
TEvent = TypeVar("TEvent")
TContext = TypeVar("TContext")


class TracedHandler:
    """A decorator for instrumenting Lambda handlers with OpenTelemetry tracing."""

    def __init__(
        self,
        name: str,
        completion_handler: TelemetryCompletionHandler,
        attributes_extractor: Callable[[Any, Any], SpanAttributes] | None = None,
    ):
        self.name = name
        self.completion_handler = completion_handler
        self.attributes_extractor = attributes_extractor or default_extractor

    def __call__(
        self,
        handler_func: Callable[[TEvent, TContext], TResult],
    ) -> Callable[[TEvent, TContext], TResult]:
        """Decorate the handler function with tracing."""

        @wraps(handler_func)
        def wrapper(event: TEvent, context: TContext) -> TResult:
            global _is_cold_start
            try:
                # Extract attributes using provided extractor
                logger.debug(
                    "Using attributes extractor: %s", self.attributes_extractor.__name__
                )
                extracted = self.attributes_extractor(event, context)

                # Get tracer from completion handler
                tracer = self.completion_handler.get_tracer()

                # Extract context from carrier if available
                parent_context = None
                if extracted.carrier:
                    try:
                        logger.debug(
                            "Attempting to extract context from carrier: %s",
                            extracted.carrier,
                        )
                        parent_context = extract(extracted.carrier)
                        if parent_context:
                            logger.debug("Successfully extracted parent context")
                    except Exception as ex:
                        logger.warn("Failed to extract context from carrier:", ex)

                # Start the span
                with tracer.start_as_current_span(
                    name=extracted.span_name or self.name,
                    context=parent_context,
                    kind=extracted.kind,
                    attributes=extracted.attributes,
                    links=extracted.links,
                ) as span:
                    if _is_cold_start:
                        span.set_attribute("faas.cold_start", True)
                        _is_cold_start = False

                    try:
                        result = handler_func(event, context)
                        # If it looks like an HTTP response, set appropriate span attributes
                        if isinstance(result, dict) and "statusCode" in result:
                            status_code = result["statusCode"]
                            span.set_attribute("http.status_code", status_code)
                            if status_code >= 500:
                                span.set_status(
                                    Status(
                                        StatusCode.ERROR, f"HTTP {status_code} response"
                                    )
                                )
                            else:
                                span.set_status(Status(StatusCode.OK))
                        return result
                    except Exception as error:
                        # Record error in the span before re-raising
                        span.record_exception(error)
                        span.set_status(Status(StatusCode.ERROR, str(error)))
                        raise

            except Exception as error:
                # This block handles infrastructure errors
                logger.error("Unhandled error in traced handler:", error)
                raise
            finally:
                # Always complete telemetry, even if there were errors
                self.completion_handler.complete()

        return cast(Callable[[TEvent, TContext], TResult], wrapper)


def create_traced_handler(
    name: str,
    completion_handler: TelemetryCompletionHandler,
    *,
    attributes_extractor: Callable[[Any, Any], SpanAttributes] | None = None,
) -> TracedHandler:
    """Create a decorator for tracing Lambda handlers.

    This function creates a decorator that wraps a Lambda handler with OpenTelemetry
    instrumentation, automatically extracting attributes and propagating context based on
    the event type.

    Features:
    - Automatic cold start detection
    - Event-specific attribute extraction
    - Context propagation from headers
    - HTTP response status code handling
    - Error recording in spans

    Args:
        name: Name for the handler span
        completion_handler: Handler for coordinating span lifecycle
        attributes_extractor: Optional function to extract span attributes from events.
                            If not provided, the default_extractor will be used.

    Returns:
        A decorator that can be used to trace Lambda handlers

    Example:
        ```python
        # Initialize telemetry once, outside the handler
        tracer, completion_handler = init_telemetry()

        # Create traced handler with configuration
        traced = create_traced_handler(
            name="my-handler",
            completion_handler=completion_handler,
            attributes_extractor=api_gateway_v2_extractor,
        )

        @traced
        def handler(event, context):
            try:
                # Your code here
                raise ValueError("something went wrong")
            except ValueError as e:
                # Handle the error and return an appropriate response
                return {"statusCode": 400, "body": str(e)}
        ```
    """
    return TracedHandler(name, completion_handler, attributes_extractor)
