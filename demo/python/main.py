import json
import os
from contextlib import contextmanager
from requests import Session
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource
from opentelemetry.instrumentation.requests import RequestsInstrumentor
from opentelemetry.trace import SpanKind
from opentelemetry.sdk.resources import Resource
from opentelemetry.semconv.trace import SpanAttributes

def init_telemetry(
    service_name: str = os.getenv("OTEL_SERVICE_NAME") or os.getenv("AWS_LAMBDA_FUNCTION_NAME") or __name__,
    lambda_context=None,
) -> tuple[trace.Tracer, TracerProvider]:
    """
    Initialize OpenTelemetry with AWS Lambda-specific configuration.

    Args:
        service_name: Name of the service for tracing identification. 
                     Defaults to OTEL_SERVICE_NAME, then AWS_LAMBDA_FUNCTION_NAME, then __name__
        lambda_context: Optional AWS Lambda context object for enhanced attributes

    Returns:
        tuple[trace.Tracer, TracerProvider]: Configured tracer and provider instances
    """
    # Configure tracer provider with resource
    resource = get_lambda_resource()

    # Add AWS Lambda specific attributes if context is provided
    if lambda_context:
        aws_attributes = {
            "faas.name": lambda_context.function_name,
            "faas.version": lambda_context.function_version,
            "aws.region": os.environ.get("AWS_REGION"),
            "cloud.account.id": lambda_context.invoked_function_arn.split(":")[4],
            "faas.max_memory": os.environ.get("AWS_LAMBDA_FUNCTION_MEMORY_SIZE"),
            "faas.runtime": "python",
        }
        resource = resource.merge(Resource.create(aws_attributes))

    provider = TracerProvider(resource=resource)

    # Configure exporter with timeout suitable for Lambda
    otlp_exporter = OTLPSpanExporter(
        session=StdoutAdapter().get_session(),
        timeout=5,  # 5 seconds timeout for Lambda environment
    )

    # Use BatchSpanProcessor with Lambda-optimized settings
    provider.add_span_processor(
        BatchSpanProcessor(
            otlp_exporter,
            schedule_delay_millis=1000,  # More frequent exports for Lambda
            max_export_batch_size=512,
            max_queue_size=2048,
        )
    )

    trace.set_tracer_provider(provider)
    return trace.get_tracer(service_name), provider


# Initialize tracer
tracer, tracer_provider = init_telemetry()

# instrument requests library
RequestsInstrumentor().instrument()

http_session = Session()
target_url = os.environ.get("TARGET_URL")
quotes_url = "https://dummyjson.com/quotes/random"


@tracer.start_as_current_span("get_random_quote")
def get_random_quote():
    response = http_session.get(quotes_url)
    response.raise_for_status()
    return response.json()


@tracer.start_as_current_span("save_quote")
def save_quote(quote: dict):
    response = http_session.post(
        f"{target_url}",
        json=quote,
        headers={
            "content-type": "application/json",
        },
    )
    response.raise_for_status()
    return response.json()


@contextmanager
def force_flush(tracer_provider):
    """
    Context manager that ensures force_flush is called on the tracer provider.

    Args:
        tracer_provider: The OpenTelemetry TracerProvider instance to force flush
    """
    try:
        yield
    finally:
        tracer_provider.force_flush()


def lambda_handler(event, context):
    with force_flush(tracer_provider), tracer.start_as_current_span(
        "lambda-invocation",
        kind=SpanKind.SERVER,
        attributes={
            SpanAttributes.FAAS_TRIGGER: "timer",
        },
    ) as span:
        span.add_event("Lambda Invocation Started")
        try:
            quote = get_random_quote()
            response = save_quote(quote)
            span.add_event(
                "Quote Saved",
                attributes={
                    "quote": quote["quote"],
                },
            )

            span.add_event("Lambda Execution Completed")
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "message": "Hello from demo Lambda!",
                        "input": event,
                        "quote": quote,
                        "response": response,
                    }
                ),
            }
        except Exception as e:
            span.record_exception(e)
            span.set_status(trace.StatusCode.ERROR, str(e))
            raise
