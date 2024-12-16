---
layout: default
title: Python
parent: Language Support
nav_order: 2
---

# Python Support
{: .fs-9 }

OpenTelemetry exporter for AWS Lambda that writes telemetry data to stdout in OTLP format.
{: .fs-6 .fw-300 }

## Quick Links
{: .text-delta }

[![PyPI](https://img.shields.io/pypi/v/otlp-stdout-adapter.svg)](https://pypi.org/project/otlp-stdout-adapter/)
[![Python Versions](https://img.shields.io/pypi/pyversions/otlp-stdout-adapter.svg)](https://pypi.org/project/otlp-stdout-adapter/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

- [Source Code](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/python/adapter)
- [PyPI Package](https://pypi.org/project/otlp-stdout-adapter/)
- [Examples](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/python/adapter/examples)
- [Change Log](https://github.com/dev7a/serverless-otlp-forwarder/blob/main/packages/python/adapter/CHANGELOG.md)

## Installation
{: .text-delta }

Install the adapter and its dependencies:

```bash
pip install otlp-stdout-adapter \
          opentelemetry-api \
          opentelemetry-sdk \
          opentelemetry-exporter-otlp
```

{: .note }
The package supports Python 3.11 and later.

## Basic Usage
{: .text-delta }

The following example shows an AWS Lambda function that:
- Handles API Gateway requests
- Creates a tracer provider with AWS Lambda resource detection
- Creates a span for each invocation
- Ensures telemetry is flushed before the function exits

{: .note }
The key integration is using `StdoutAdapter` with the OTLP exporter, which redirects all telemetry data to stdout instead of making HTTP calls.

```python
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource
from opentelemetry.trace import SpanKind
import json

def init_telemetry(service_name: str = __name__) -> tuple[trace.Tracer, TracerProvider]:
    """Initialize OpenTelemetry with AWS Lambda-specific configuration"""
    # Create a provider with Lambda resource attributes
    provider = TracerProvider(resource=get_lambda_resource())
    
    # Configure the OTLP exporter with StdoutAdapter
    exporter = OTLPSpanExporter(
        http_adapter=StdoutAdapter()
    )
    
    # Add the exporter to the provider
    provider.add_span_processor(BatchSpanProcessor(exporter))
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
            # Ensure all spans are flushed before the Lambda ends
            provider.force_flush()
```

## Configuration
{: .text-delta }

Configuration is handled through environment variables:

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | Protocol for OTLP data (`http/protobuf` or `http/json`) | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | Compression type (`gzip` or `none`) | `none` |
| `OTEL_SERVICE_NAME` | Name of your service | Falls back to `AWS_LAMBDA_FUNCTION_NAME` or "unknown-service" |

## Troubleshooting
{: .text-delta }

{: .warning }
Common issues and solutions:

1. **No Data in Logs**
   - Verify `force_flush()` is called before the function exits
   - Check that the `StdoutAdapter` is properly configured in the exporter
   - Ensure spans are being created and closed properly

2. **JSON Parsing Errors**
   - Verify the correct protocol is set in `OTEL_EXPORTER_OTLP_PROTOCOL`
   - Check for valid JSON in your attributes and events