"""
Handler implementation for lambda-otel-lite.

This module provides the traced_handler context manager for instrumenting Lambda handlers.
"""

import logging
import os
from contextlib import contextmanager
from typing import Any, Callable

from opentelemetry import trace
from opentelemetry.context import Context
from opentelemetry.propagate import extract
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.trace import SpanKind, Status, StatusCode

from . import ProcessorMode
from .extension import _handler_complete, _handler_ready, debug_timing

# Setup logging
logger = logging.getLogger(__name__)
logger.setLevel(os.getenv("AWS_LAMBDA_LOG_LEVEL", os.getenv("LOG_LEVEL", "INFO")).upper())

# Global state
_is_cold_start: bool = True
_processor_mode: ProcessorMode = ProcessorMode.from_env(
    "LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", ProcessorMode.ASYNC
)


def _extract_span_attributes(event: dict[str, Any] | None = None, context: Any | None = None) -> dict[str, Any]:
    """Extract span attributes from Lambda event.
    
    Only extracts attributes that are specific to the span and not already
    set at the resource level by the init telemetry.

    Args:
        event: Optional Lambda event dictionary
        context: Optional Lambda context object

    Returns:
        Dictionary of span attributes
    """
    attributes: dict[str, Any] = {'faas.trigger': 'other'}

    # Add invocation ID, cloud resource ID and account ID if context is available
    if context:
        if hasattr(context, "aws_request_id"):
            attributes["faas.invocation_id"] = context.aws_request_id
        if hasattr(context, "invoked_function_arn") and context.invoked_function_arn:
            arn_parts = context.invoked_function_arn.split(":")
            if len(arn_parts) >= 5:  # arn:aws:lambda:region:account-id:...
                attributes["cloud.resource_id"] = context.invoked_function_arn
                attributes["cloud.account.id"] = arn_parts[4]

    # Extract trigger metadata
    if event and isinstance(event, dict):
        # HTTP triggers (API Gateway v1 and v2)
        if "requestContext" in event or "httpMethod" in event:
            attributes["faas.trigger"] = "http"
            if event.get("version") == "2.0":
                attributes["http.route"] = event.get("routeKey", "")
                if "requestContext" in event:
                    ctx = event["requestContext"]
                    attributes.update({
                        "http.method": ctx.get("http", {}).get("method", ""),
                        "http.target": ctx.get("http", {}).get("path", ""),
                        "http.scheme": ctx.get("http", {}).get("protocol", "").lower(),
                    })
            else:
                attributes["http.route"] = event.get("resource", "")
                attributes["http.method"] = event.get("httpMethod", "")
                attributes["http.target"] = event.get("path", "")
                if "requestContext" in event:
                    ctx = event["requestContext"]
                    attributes["http.scheme"] = ctx.get("protocol", "").lower()
    return attributes


def _extract_context(
    event: dict[str, Any] | None = None,
    get_carrier: Callable[[dict[str, Any]], dict[str, Any]] | None = None,
) -> Context | None:
    """Extract trace context from event.
    
    Args:
        event: Lambda event dictionary
        get_carrier: Optional function to extract carrier from event

    Returns:
        Context if found, None otherwise
    """
    if not event or not isinstance(event, dict):
        return None

    # Use custom carrier extraction if provided
    if get_carrier:
        try:
            carrier = get_carrier(event)
            if carrier:
                return extract(carrier)
        except Exception as ex:
            logger.warning("Failed to extract carrier: %s", ex)
            return None

    # Default extraction from headers
    if "headers" in event:
        try:
            return extract(event["headers"])
        except Exception as ex:
            logger.warning("Failed to extract context from headers: %s", ex)
            return None

    return None


@contextmanager
def traced_handler(
    tracer: trace.Tracer,
    tracer_provider: TracerProvider,
    name: str,
    event: dict[str, Any] | None = None,
    context: Any | None = None,
    kind: SpanKind = SpanKind.SERVER,
    attributes: dict[str, Any] | None = None,
    links: list[Any] | None = None,
    start_time: int | None = None,
    record_exception: bool = True,
    set_status_on_exception: bool = True,
    end_on_exit: bool = True,
    parent_context: Context | None = None,
    get_carrier: Callable[[dict[str, Any]], dict[str, Any]] | None = None,
):
    """Context manager for tracing Lambda handlers.

    Example:
        ```python
        def handler(event, context):
            with traced_handler(tracer, provider, "my-handler", event, context,
                              attributes={"custom.attr": "value"}):
                # The span is available as the current span in the context
                # No need to access it directly
                result = process_event(event)
                return result  # Status code will be extracted if present
        ```

    Args:
        tracer: OpenTelemetry tracer instance
        tracer_provider: OpenTelemetry tracer provider instance
        name: Name of the span
        event: Optional Lambda event dictionary
        context: Optional Lambda context object
        kind: Kind of span (default: SERVER). Use CONSUMER for message processing (e.g., SQS)
        attributes: Optional additional span attributes
        links: Optional span links
        start_time: Optional span start time
        record_exception: Whether to record exceptions (default: True)
        set_status_on_exception: Whether to set status on exceptions (default: True)
        end_on_exit: Whether to end span on exit (default: True)
        parent_context: Optional parent context for trace propagation
        get_carrier: Optional function to extract carrier from event for context propagation
    """
    global _is_cold_start
    result = None
    try:
        # Only wait for handler ready in async mode
        if _processor_mode == ProcessorMode.ASYNC:
            _handler_ready.wait()
            _handler_ready.clear()

        # Extract span attributes and merge with custom attributes
        span_attributes = _extract_span_attributes(event, context)
        if attributes:
            span_attributes.update(attributes)

        # Extract context from event if no parent_context provided
        if parent_context is None:
            parent_context = _extract_context(event, get_carrier)

        with tracer.start_as_current_span(
            name,
            context=parent_context,
            kind=kind,
            attributes=span_attributes,
            links=links,
            start_time=start_time,
            record_exception=record_exception,
            set_status_on_exception=set_status_on_exception,
            end_on_exit=end_on_exit,
        ) as span:
            if _is_cold_start:
                span.set_attribute("faas.cold_start", True)
                _is_cold_start = False
            result = yield
            # Set HTTP status code if available
            if isinstance(result, dict) and "statusCode" in result:
                status_code = result["statusCode"]
                span.set_attribute("http.status_code", status_code)
                # Only set error status for 5xx responses
                if status_code >= 500:
                    span.set_status(Status(StatusCode.ERROR))
    finally:
        if _processor_mode == ProcessorMode.SYNC:
            # In sync mode, force flush before returning
            with debug_timing(logger, "force_flush call"):
                tracer_provider.force_flush()
        elif _processor_mode == ProcessorMode.ASYNC:
            # In async mode, signal completion to extension
            _handler_complete.set()
        # In finalize mode, do nothing - let the processor handle flushing 