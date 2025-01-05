"""
Telemetry initialization for lambda-otel-lite.

This module provides the initialization function for OpenTelemetry in AWS Lambda.
"""

import os

from opentelemetry import trace
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import SpanExporter, SpanProcessor
from otlp_stdout_adapter import StdoutAdapter

from . import ProcessorMode
from .extension import init_extension
from .processor import LambdaSpanProcessor

# Global state
_tracer_provider: TracerProvider | None = None
_processor_mode: ProcessorMode = ProcessorMode.from_env(
    "LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE", ProcessorMode.ASYNC
)


def get_lambda_resource() -> Resource:
    """Create a Resource instance with AWS Lambda attributes.
    
    Returns:
        Resource instance with AWS Lambda environment attributes
    """
    attributes = {
        "cloud.provider": "aws",
        "cloud.region": os.environ.get("AWS_REGION", ""),
        "faas.name": os.environ.get("AWS_LAMBDA_FUNCTION_NAME", ""),
        "faas.version": os.environ.get("AWS_LAMBDA_FUNCTION_VERSION", ""),
        "faas.instance": os.environ.get("AWS_LAMBDA_LOG_STREAM_NAME", ""),
        "faas.max_memory": os.environ.get("AWS_LAMBDA_FUNCTION_MEMORY_SIZE", ""),
    }

    return Resource.create(attributes)


def init_telemetry(
    name: str,
    resource: Resource | None = None,
    span_processor: SpanProcessor | None = None,
    exporter: SpanExporter | None = None,
) -> tuple[trace.Tracer, TracerProvider]:
    """Initialize OpenTelemetry with manual OTLP stdout configuration.

    This function provides a flexible way to initialize OpenTelemetry for AWS Lambda,
    with sensible defaults that work well in most cases but allowing customization
    where needed.

    Args:
        name: Name for the tracer (e.g., 'my-service', 'payment-processor')
        resource: Optional custom Resource. Defaults to Lambda resource detection
        span_processor: Optional custom SpanProcessor. Defaults to LambdaSpanProcessor
        exporter: Optional custom SpanExporter. Defaults to OTLPSpanExporter with stdout

    Returns:
        tuple: (tracer, provider) instances
    """
    global _tracer_provider

    # Setup resource
    resource = resource or get_lambda_resource()
    _tracer_provider = TracerProvider(resource=resource)

    # Setup exporter and processor
    if span_processor is None:
        exporter = exporter or OTLPSpanExporter(session=StdoutAdapter().get_session())
        span_processor = LambdaSpanProcessor(
            exporter, max_queue_size=int(os.getenv("LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE", "2048"))
        )

    _tracer_provider.add_span_processor(span_processor)
    trace.set_tracer_provider(_tracer_provider)

    # Initialize extension for async and finalize modes
    if _processor_mode in [ProcessorMode.ASYNC, ProcessorMode.FINALIZE]:
        init_extension(_processor_mode, _tracer_provider)

    return trace.get_tracer(name), _tracer_provider 