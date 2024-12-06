import json
import os
from contextlib import contextmanager
from opentelemetry import trace
from opentelemetry.trace import SpanKind

tracer = trace.get_tracer(__name__)

def process_level(depth: int, iterations: int) -> None:
    """
    Recursively create a tree of spans to measure OpenTelemetry overhead.
    
    Args:
        depth: Current depth level (decrements towards 0)
        iterations: Number of spans to create at each level
    """
    if depth <= 0:
        return
        
    for i in range(iterations):
        with tracer.start_as_current_span(f"operation_depth_{depth}_iter_{i}") as span:
            span.set_attributes({
                "depth": depth,
                "iteration": i
            })
            process_level(depth - 1, iterations)

def handler(event, context):
    """
    Lambda handler that creates a tree of spans based on input parameters.
    
    Args:
        event: Lambda event containing depth and iterations parameters
        context: Lambda context
    
    Returns:
        dict: Response containing the benchmark parameters
    """    
    depth = event.get('depth', 2)
    iterations = event.get('iterations', 3)
    
    with tracer.start_as_current_span(
        "benchmark-execution",
        kind=SpanKind.SERVER,
        attributes={
            "faas.trigger": "http"
        }
    ) as span:
        process_level(depth, iterations)
        
        return {
            'statusCode': 200,
            'body': json.dumps({
                'message': 'Benchmark complete',
                'depth': depth,
                'iterations': iterations
            })
        } 