"""
Simple Hello World Lambda function using lambda-otel-lite.

This example demonstrates basic OpenTelemetry setup with lambda-otel-lite.
It creates spans for each invocation and logs the event payload using span events.
"""

import json
import random
from typing import Any

from opentelemetry import trace
from opentelemetry.trace import StatusCode

from lambda_otel_lite import (
    api_gateway_v2_extractor,
    create_traced_handler,
    init_telemetry,
)

# Initialize telemetry once at module load
tracer, completion_handler = init_telemetry()

# Create a traced handler with configuration
traced = create_traced_handler(
    name="simple-handler",
    completion_handler=completion_handler,
    attributes_extractor=api_gateway_v2_extractor,
)


def nested_function(event: dict[str, Any]) -> str:
    """Simple nested function that creates its own span.

    This function demonstrates how to create a child span from the current context.
    The span will automatically become a child of the currently active span.
    """
    # Create a child span - it will automatically use the active span as parent
    with tracer.start_as_current_span("nested_function") as span:
        span.add_event("Nested function called")
        if event.get("rawPath") == "/error":
            # simulate a random error
            r = random.random()
            if r < 0.25:
                raise ValueError("expected error")
            elif r < 0.5:
                raise RuntimeError("unexpected error")
        return "success"


@traced
def handler(event: dict[str, Any], context: Any) -> dict[str, Any]:
    """Lambda handler function.

    This example shows how to:
    1. Use the traced decorator for automatic span creation
    2. Access the current span via OpenTelemetry API
    3. Create child spans for nested operations
    4. Add custom attributes and events
    """
    current_span = trace.get_current_span()
    request_id = context.aws_request_id
    current_span.add_event(
        "handling request",
        event,
    )
    current_span.set_attribute("request.id", request_id)

    try:
        nested_function(event)
        # Return a simple response
        return {
            "statusCode": 200,
            "body": json.dumps({"message": f"Hello from request {request_id}"}),
        }
    except ValueError as e:
        current_span.record_exception(e)
        current_span.set_status(StatusCode.ERROR, str(e))
        return {
            "statusCode": 400,
            "body": json.dumps({"message": str(e)}),
        }
    except RuntimeError:
        raise
