from opentelemetry import trace
import json
import urllib3

tracer = trace.get_tracer("benchmark-test")

example_url = "https://example.com"


def test_http_operations() -> None:
    """
    Makes a HEAD request to example.com and traces it with OpenTelemetry spans.
    """
    with tracer.start_as_current_span("http-operations"):
        # Test HTTP HEAD request
        with tracer.start_as_current_span("http-head"):
            urllib3.request("HEAD", example_url)


def handler(event, context):
    """
    Lambda handler that exercises HTTP operations with OpenTelemetry instrumentation.

    This handler executes HTTP operations to generate telemetry spans. It uses
    the URLLib3Instrumentor to automatically instrument HTTP calls.

    Args:
        event: Lambda event (not used)
        context: Lambda context (not used)

    Returns:
        dict: Response with status code 200 and message indicating benchmark completion
    """
    test_http_operations()
    return {
        "statusCode": 200,
        "body": json.dumps({"message": "Benchmark complete"}),
    }
