import json
import os
from contextlib import contextmanager
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource
from opentelemetry.trace import SpanKind
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource

def init_telemetry() -> tuple[trace.Tracer, TracerProvider]:
    """Initialize OpenTelemetry with manual OTLP stdout configuration."""
    # Configure resource with Lambda-specific attributes
    resource = get_lambda_resource()

    provider = TracerProvider(resource=resource)

    # Configure exporter with stdout adapter
    otlp_exporter = OTLPSpanExporter(
        session=StdoutAdapter().get_session(),
        timeout=5,  # 5 seconds timeout for Lambda environment
    )

    # Use BatchSpanProcessor with Lambda-optimized settings
    provider.add_span_processor(
        BatchSpanProcessor(
            otlp_exporter,
            schedule_delay_millis=1000,
            max_export_batch_size=512,
            max_queue_size=2048,
        )
    )

    trace.set_tracer_provider(provider)
    return trace.get_tracer("benchmark-test"), provider

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

@contextmanager
def force_flush(tracer_provider):
    """Ensure spans are exported before Lambda freezes."""
    try:
        yield
    finally:
        tracer_provider.force_flush()

tracer, provider = init_telemetry()

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
    iterations = event.get('iterations', 2)
    
    with force_flush(provider):
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