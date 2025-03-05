#!/usr/bin/env python3
"""
Simple example showing basic usage of OTLPStdoutSpanExporter.

This example demonstrates:
1. Basic tracer setup
2. Creating a parent span
3. Creating a nested child span
4. Proper cleanup with force flush

Run with:
    python -m otlp_stdout_span_exporter.examples.simple_stdout_hello
"""

from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor

from otlp_stdout_span_exporter import OTLPStdoutSpanExporter


def init_tracer() -> TracerProvider:
    """Initialize the tracer with OTLPStdoutSpanExporter."""
    exporter = OTLPStdoutSpanExporter()
    provider = TracerProvider()
    processor = BatchSpanProcessor(exporter)
    provider.add_span_processor(processor)
    
    # Set as global default tracer provider
    trace.set_tracer_provider(provider)
    return provider


def main() -> None:
    """Run the example."""
    provider = init_tracer()
    tracer = trace.get_tracer("example/simple")

    with tracer.start_as_current_span("parent-operation"):
        print("Doing work...")

        # Create nested spans
        with tracer.start_as_current_span("child-operation"):
            print("Doing more work...")

    # Force flush before exit
    provider.force_flush()


if __name__ == "__main__":
    main() 