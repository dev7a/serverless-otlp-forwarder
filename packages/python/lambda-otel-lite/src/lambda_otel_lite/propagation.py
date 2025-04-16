"""
Propagation utilities for lambda-otel-lite.

This module provides propagators for extracting and injecting context in AWS Lambda
environments, with special handling for X-Ray trace headers.
"""

import os
from typing import Optional, Set, Any

from opentelemetry.context import Context
from opentelemetry.propagate import set_global_textmap
from opentelemetry.propagators.aws.aws_xray_propagator import (
    AwsXRayPropagator,
    TRACE_HEADER_KEY,
)
from opentelemetry.propagators.composite import CompositePropagator
from opentelemetry.propagators.textmap import (
    TextMapPropagator,
    Getter,
    Setter,
    default_getter,
    default_setter,
)

from opentelemetry.trace import get_current_span

from .config import get_str_env
from .constants import EnvVars
from .logger import create_logger

# Setup logging
logger = create_logger("propagation")


def has_valid_span(context: Context) -> bool:
    """Check if the context has a valid span.

    Args:
        context: The context to check

    Returns:
        True if the context has a valid span, False otherwise
    """
    span = get_current_span(context)
    if not span:
        return False

    span_context = span.get_span_context()
    if not span_context:
        return False

    # Check if the span context is valid (has valid trace ID and span ID)
    return bool(span_context.trace_id and span_context.span_id)


class LambdaXRayPropagator(AwsXRayPropagator):
    """
    AWS X-Ray Lambda propagator with special handling for Sampled=0 in the environment variable.

    Why this is needed:
    - When X-Ray is not enabled for the Lambda, AWS still sets the _X_AMZN_TRACE_ID environment variable, but always with Sampled=0.
    - The stock AwsXRayLambdaPropagator will extract this and create a non-sampled context, which disables tracing for the execution—even if other propagators (like W3C tracecontext) are enabled and would otherwise create a root span.
    - By skipping extraction if Sampled=0 is present, this propagator allows the next propagator or the default sampler to create a root span, ensuring traces are captured even when X-Ray is not enabled or not sampled.
    - This is essential for environments where you want to use W3C or other propagation mechanisms alongside (or instead of) X-Ray.
    """

    def extract(
        self,
        carrier: Any,
        context: Optional[Context] = None,
        getter: Getter[Any] = default_getter,
    ) -> Context:
        xray_context = super().extract(carrier, context=context, getter=getter)

        # Check for a valid span in the extracted context
        if get_current_span(context=xray_context).get_span_context().is_valid:
            return xray_context

        trace_header = os.environ.get("_X_AMZN_TRACE_ID")

        # If no env var or Sampled=0, do not extract further
        if trace_header is None or "Sampled=0" in trace_header:
            return xray_context

        # Fallback: extract from the environment variable
        return super().extract(
            {TRACE_HEADER_KEY: trace_header},
            context=xray_context,
            getter=getter,
        )


class NoopPropagator(TextMapPropagator):
    """No-op propagator that does nothing."""

    def extract(
        self,
        carrier: Any,
        context: Optional[Context] = None,
        getter: Getter[Any] = default_getter,  # type: ignore
    ) -> Context:
        """Extract from carrier (no-op)."""
        return context or Context()

    def inject(
        self,
        carrier: Any,
        context: Optional[Context] = None,
        setter: Setter[Any] = default_setter,  # type: ignore
    ) -> None:
        """Inject into carrier (no-op)."""
        pass

    @property
    def fields(self) -> Set[str]:
        """Get fields (no-op)."""
        return set()


def create_propagator() -> TextMapPropagator:
    """Create a composite propagator based on the OTEL_PROPAGATORS environment variable.

    The environment variable should be a comma-separated list of propagator names.
    Supported propagators:
    - "tracecontext" - W3C Trace Context propagator
    - "xray" - AWS X-Ray propagator
    - "xray-lambda" - AWS X-Ray propagator with Lambda support
    - "none" - No propagation

    If the environment variable is not set, defaults to ["xray-lambda,tracecontext"]
    with tracecontext taking precedence.

    Returns:
        A composite propagator with the specified propagators
    """
    from opentelemetry.trace.propagation.tracecontext import (
        TraceContextTextMapPropagator,
    )

    propagator_names = set(
        name.strip().lower()
        for name in get_str_env(
            EnvVars.OTEL_PROPAGATORS, None, "xray-lambda,tracecontext"
        ).split(",")
        if name.strip()
    )

    if "none" in propagator_names:
        logger.debug("Using no propagator as requested by OTEL_PROPAGATORS=none")
        return NoopPropagator()

    propagators: list[TextMapPropagator] = []
    if "tracecontext" in propagator_names:
        propagators.append(TraceContextTextMapPropagator())
    if "xray" in propagator_names or "xray-lambda" in propagator_names:
        propagators.append(LambdaXRayPropagator())

    if not propagators:
        logger.info("No valid propagators specified, using defaults")
        propagators = [LambdaXRayPropagator(), TraceContextTextMapPropagator()]

    logger.debug(f"Using propagators: {[type(p).__name__ for p in propagators]}")
    return CompositePropagator(propagators)


def setup_propagator() -> None:
    """Set up the global propagator based on environment variables.

    This should be called during initialization.
    """
    propagator = create_propagator()
    set_global_textmap(propagator)
