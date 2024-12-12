---
layout: default
title: Python
parent: Language Support
nav_order: 2
---

# Python Support

The Python implementation provides a seamless integration with OpenTelemetry through the `otlp-stdout-adapter` package.

## Quick Links
- [Source Code](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/python/adapter)
- [Documentation](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/python/adapter/README.md)
- [PyPI Package](https://pypi.org/project/otlp-stdout-adapter/)
- [Examples](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/python/adapter/examples)

## Features
- Easy integration with existing OTEL SDK
- Support for async and sync code
- Automatic context propagation
- AWS Lambda context integration
- Batch processing support

## Example Usage

```python
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource
from opentelemetry.trace import SpanKind
from contextlib import contextmanager

def init_telemetry(service_name: str = __name__) -> tuple[trace.Tracer, TracerProvider]:
    """Initialize OpenTelemetry with AWS Lambda-specific configuration"""
    provider = TracerProvider(resource=get_lambda_resource())
    
    provider.add_span_processor(BatchSpanProcessor(
        OTLPSpanExporter(
            session=StdoutAdapter().get_session(),
            timeout=5
        )
    ))

    trace.set_tracer_provider(provider)
    return trace.get_tracer(service_name), provider

# Initialize tracer
tracer, tracer_provider = init_telemetry()

@contextmanager
def force_flush(tracer_provider):
    """Ensure traces are flushed even if Lambda freezes"""
    try:
        yield
    finally:
        tracer_provider.force_flush()

def lambda_handler(event, context):
    with force_flush(tracer_provider), tracer.start_as_current_span(
        "lambda-invocation",
        kind=SpanKind.SERVER
    ) as span:
        try:
            result = {"message": "Hello from Lambda!"}
            return {
                "statusCode": 200,
                "body": json.dumps(result)
            }
        except Exception as e:
            span.record_exception(e)
            span.set_status(trace.StatusCode.ERROR, str(e))
            raise
```

## Configuration

The Python implementation can be configured through environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |

## Best Practices

1. **Error Handling**
   - Use try/except blocks with span recording
   - Set appropriate span status on errors
   - Use context managers for cleanup

2. **Resource Management**
   - Always use the `force_flush` context manager
   - Clean up resources properly
   - Handle Lambda freezing gracefully

3. **Performance**
   - Use batch processing when possible
   - Enable compression for large payloads
   - Configure appropriate timeouts

4. **Context Propagation**
   - Use context managers for span lifecycle
   - Implement proper async context if needed
   - Maintain trace context across function calls
``` 