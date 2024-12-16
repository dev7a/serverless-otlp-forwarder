# OTLP Stdout Adapter

The `otlp-stdout-adapter` library is designed to export OpenTelemetry data to stdout in a formatted JSON structure, suitable for serverless environments like AWS Lambda. It implements a custom HTTP adapter that can be used with OpenTelemetry OTLP exporters to send telemetry data to stdout.

By outputting telemetry data to stdout, this library enables seamless integration with log management systems in serverless environments. For instance, in AWS Lambda, CloudWatch Logs can capture this output, allowing tools like the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) to efficiently collect and forward the data to centralized OpenTelemetry collectors.

>[!IMPORTANT]
>This package is highly experimental and should not be used in production. Contributions are welcome.

## Features

- Implements a custom HTTP adapter for use in OpenTelemetry OTLP pipelines
- Exports OpenTelemetry data to stdout in a structured JSON format
- Designed for serverless environments, especially AWS Lambda
- Configurable through environment variables
- Support for GZIP compression
- Base64 encoding for binary payloads

## Installation

You can install the `otlp-stdout-adapter` using pip:

```bash
pip install otlp-stdout-adapter
```

## Usage

Here's a basic example of how to use the library with OpenTelemetry:

```python
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource

# Initialize the StdoutAdapter
adapter = StdoutAdapter()
session = adapter.get_session()

# Create OTLP exporter with custom session
exporter = OTLPSpanExporter(
    endpoint="http://your-collector:4318/v1/traces",
    session=session
)

# Set up the trace provider
provider = TracerProvider(resource=get_lambda_resource())
processor = BatchSpanProcessor(exporter)
provider.add_span_processor(processor)
trace.set_tracer_provider(provider)

# Get a tracer
tracer = trace.get_tracer(__name__)

# Create spans as usual
with tracer.start_as_current_span("example_span") as span:
    span.set_attribute("example.attribute", "value")
```

## Environment Variables

The adapter can be configured using standard OpenTelemetry environment variables:

- `OTEL_SERVICE_NAME`: Sets the service name for the logs
- `AWS_LAMBDA_FUNCTION_NAME`: Used as fallback for service name if OTEL_SERVICE_NAME is not set
- `OTEL_EXPORTER_OTLP_ENDPOINT`: Sets the endpoint for the OTLP collector
- `OTEL_EXPORTER_OTLP_HEADERS`: Sets additional headers for the OTLP exporter
- `OTEL_EXPORTER_OTLP_COMPRESSION`: Specifies the compression algorithm (only gzip is supported)
- `OTEL_EXPORTER_OTLP_TRACES_PROTOCOL`: Specifies the protocol (http/protobuf or http/json)*

[!IMPORTANT]: While the OpenTelemetry specification supports both JSON and Protobuf over HTTP,
the Python SDK currently only supports Protobuf (see [opentelemetry-python#1003](https://github.com/open-telemetry/opentelemetry-python/issues/1003)). 
The environment variable is recognized but JSON format is not yet implemented. 
All exports will use application/x-protobuf content-type.

## Output Format

Each log record includes:
- `__otel_otlp_stdout`: Record marker and version identifier for the adapter (format: package-name@version)
- `source`: The service name from configuration
- `endpoint`: The configured OTLP endpoint
- `method`: The HTTP method (always "POST")
- `content-type`: The content type (currently always "application/x-protobuf")
- `payload`: The OTLP data (base64 encoded if compressed)
- `base64`: Boolean indicating if the payload is base64 encoded

Example output:
```json
{
    "__otel_otlp_stdout": "otlp-stdout-adapter@0.1.3",
    "source": "my-lambda-function",
    "headers": {
        "content-type": "application/x-protobuf"
    },
    "endpoint": "http://collector:4318/v1/traces",
    "method": "POST",
    "content-type": "application/x-protobuf",
    "payload": "base64-encoded-content",
    "base64": true
}
```

## AWS Lambda Usage

Here's a complete example of using the adapter in an AWS Lambda function with distributed tracing:

```python
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from opentelemetry.trace.propagation.tracecontext import TraceContextTextMapPropagator
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource

# Initialize once per Lambda container
adapter = StdoutAdapter()
session = adapter.get_session()

# Set up the trace provider with Lambda resource detection
provider = TracerProvider(resource=get_lambda_resource())
exporter = OTLPSpanExporter(
    endpoint="http://your-collector:4318/v1/traces",
    session=session
)
processor = BatchSpanProcessor(exporter)
provider.add_span_processor(processor)
trace.set_tracer_provider(provider)

# Get a tracer
tracer = trace.get_tracer(__name__)

def process_event(event):
    with tracer.start_as_current_span("process_event") as span:
        # Your processing logic here
        span.set_attribute("event.type", event.get("type"))
        return {"processed": True}

def lambda_handler(event, context):
    with tracer.start_as_current_span(
        "lambda_handler",
        kind=trace.SpanKind.SERVER
    ) as span:
        try:
            # Add event information to span
            span.set_attribute("event.type", event.get("type"))
            span.add_event("Processing Lambda event")
            
            result = process_event(event)
            
            return {
                "statusCode": 200,
                "body": json.dumps(result)
            }
        except Exception as e:
            span.record_exception(e)
            span.set_status(trace.Status(trace.StatusCode.ERROR))
            raise
        finally:
            # Ensure spans are exported before Lambda freezes
            provider.force_flush()
```

### Lambda Configuration Notes

- Initialize tracing components outside the handler for container reuse
- Always call `provider.force_flush()` before the handler completes
- Use environment variables for configuration:
  ```bash
  OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
  OTEL_EXPORTER_OTLP_COMPRESSION=gzip
  ```

## Best Practices

1. **Initialize Outside Handler**
   - Create providers, exporters, and tracers outside the handler
   - Reuse these components across invocations

2. **Resource Detection**
   - Use `get_lambda_resource()` to automatically capture Lambda metadata
   - Merge with custom resources as needed

3. **Span Processing**
   - Use `BatchSpanProcessor` for efficient span processing
   - Always flush spans before handler completion
   - Configure appropriate batch size and timeout for Lambda

4. **Error Handling**
   - Record exceptions using `span.record_exception()`
   - Set appropriate span status
   - Ensure spans are ended in finally blocks

5. **Context Propagation**
   - Use TraceContext propagator for distributed tracing
   - Propagate context in outgoing requests

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
