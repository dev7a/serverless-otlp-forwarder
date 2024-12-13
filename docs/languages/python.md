---
layout: default
title: Python
parent: Language Support
nav_order: 2
---

# Python Support
{: .fs-9 }

Seamless integration with OpenTelemetry through the `otlp-stdout-adapter` package.
{: .fs-6 .fw-300 }

## Quick Links
{: .text-delta }

[![PyPI](https://img.shields.io/pypi/v/otlp-stdout-adapter.svg)](https://pypi.org/project/otlp-stdout-adapter/)
[![Python Versions](https://img.shields.io/pypi/pyversions/otlp-stdout-adapter.svg)](https://pypi.org/project/otlp-stdout-adapter/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

- [Source Code](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/python/adapter)
- [PyPI Package](https://pypi.org/project/otlp-stdout-adapter/)
- [Examples](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/python/adapter/examples)
- [Change Log](https://github.com/dev7a/lambda-otlp-forwarder/blob/main/packages/python/adapter/CHANGELOG.md)

## Installation
{: .text-delta }

```bash
pip install otlp-stdout-adapter opentelemetry-api opentelemetry-sdk opentelemetry-exporter-otlp
```

## Basic Usage
{: .text-delta }

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

def lambda_handler(event, context):
    tracer, provider = init_telemetry()
    
    with tracer.start_as_current_span(
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
        finally:
            provider.force_flush()
```

## Advanced Features
{: .text-delta }

### Async Support
{: .text-delta }

```python
import asyncio
from opentelemetry.trace import Status, StatusCode

async def async_handler(event, context):
    tracer, provider = init_telemetry()
    
    async with tracer.start_as_current_span("async-operation") as span:
        try:
            result = await process_async_request()
            span.set_status(Status(StatusCode.OK))
            return result
        except Exception as e:
            span.set_status(Status(StatusCode.ERROR, str(e)))
            span.record_exception(e)
            raise
        finally:
            await asyncio.get_event_loop().run_in_executor(
                None, provider.force_flush
            )
```

### Custom Attributes
{: .text-delta }

```python
from opentelemetry.trace import get_current_span
from opentelemetry.semconv.trace import SpanAttributes

def process_request(request_id: str):
    span = get_current_span()
    span.set_attribute(SpanAttributes.HTTP_METHOD, "POST")
    span.set_attribute("request.id", request_id)
    span.add_event("processing.start", {
        "timestamp": time.time_ns()
    })
```

### Framework Integration
{: .text-delta }

```python
from opentelemetry.instrumentation.flask import FlaskInstrumentor
from opentelemetry.instrumentation.requests import RequestsInstrumentor
from flask import Flask

app = Flask(__name__)
FlaskInstrumentor().instrument_app(app)
RequestsInstrumentor().instrument()
```

## Best Practices
{: .text-delta }

### Resource Management
{: .text-delta }

{: .info }
- Use context managers for span lifecycle
- Always call `force_flush()` in finally blocks
- Clean up resources properly
- Handle Lambda freezing gracefully

### Performance
{: .text-delta }

{: .info }
- Use batch processing when possible
- Enable compression for large payloads
- Configure appropriate timeouts
- Monitor memory usage

### Error Handling
{: .text-delta }

{: .info }
- Use try/except blocks with span recording
- Set appropriate span status on errors
- Use context managers for cleanup
- Add error details to spans

### Context Propagation
{: .text-delta }

{: .info }
- Use context managers for span lifecycle
- Implement proper async context if needed
- Maintain trace context across function calls
- Handle baggage appropriately

## Environment Variables
{: .text-delta }

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |
| `OTEL_PYTHON_LOG_CORRELATION` | Enable log correlation | `true` |
| `OTEL_PYTHON_EXCLUDED_URLS` | URLs to exclude from tracing | `health/*` |

## Troubleshooting
{: .text-delta }

{: .warning }
Common issues and solutions:

1. **Missing Spans**
   - Check if `force_flush()` is called
   - Verify context manager usage
   - Check async context handling

2. **Performance Issues**
   - Enable batch processing
   - Adjust batch configuration
   - Monitor memory usage

3. **Import Errors**
   - Verify package versions
   - Check Python version compatibility
   - Install all required dependencies

## Examples
{: .text-delta }

### API Gateway Integration
{: .text-delta }

```python
from opentelemetry.trace import SpanKind, Status, StatusCode
from opentelemetry.semconv.trace import SpanAttributes
import json

def lambda_handler(event, context):
    tracer, provider = init_telemetry("api-handler")
    
    with tracer.start_as_current_span(
        name="process-request",
        kind=SpanKind.SERVER,
        attributes={
            SpanAttributes.HTTP_METHOD: event.get("httpMethod"),
            SpanAttributes.HTTP_ROUTE: event.get("resource"),
            "aws.requestId": context.aws_request_id
        }
    ) as span:
        try:
            # Process the request
            result = process_request(event)
            span.set_status(Status(StatusCode.OK))
            
            return {
                "statusCode": 200,
                "body": json.dumps(result),
                "headers": {
                    "Content-Type": "application/json"
                }
            }
        except Exception as e:
            span.record_exception(e)
            span.set_status(Status(StatusCode.ERROR, str(e)))
            raise
        finally:
            provider.force_flush()
```

## Next Steps
{: .text-delta }

- [Configure Processors](../concepts/processors)
- [Performance Tuning](../advanced/performance)
- [Monitoring Setup](../deployment/monitoring) 