# Lambda OTel Lite

The `lambda-otel-lite` library provides a lightweight, efficient OpenTelemetry implementation specifically designed for AWS Lambda environments. It features a custom span processor and internal extension mechanism that optimizes telemetry collection for Lambda's unique execution model.

By leveraging Lambda's execution lifecycle and providing multiple processing modes, this library enables efficient telemetry collection with minimal impact on function latency. It's designed to work seamlessly with the [otlp-stdout-adapter](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/python/adapter) for complete serverless observability.

>[!IMPORTANT]
>This package is highly experimental and should not be used in production. Contributions are welcome.

## Features

- Lambda-optimized span processor with queue-based buffering
- Three processing modes for different use cases:
  - Synchronous: Immediate span export (best for development)
  - Asynchronous: Background processing via internal extension
  - Finalize: Compatible with standard BatchSpanProcessor
- Internal extension thread for asynchronous mode
- Sigterm handler for asynchronous and finalize mode
- Automatic Lambda resource detection
- Configurable through environment variables
- Zero external dependencies beyond OpenTelemetry

## Installation

You can install the `lambda-otel-lite` package using pip:

```bash
pip install lambda-otel-lite
```

## Usage

Here's a basic example of using the library in a Lambda function:

```python
from lambda_otel_lite import init_telemetry, traced_handler

# Initialize telemetry with default configuration (do this outside the handler)
# By default, this uses:
# - LambdaSpanProcessor for efficient span processing
# - OTLPSpanExporter with StdoutAdapter for Lambda-optimized export
# - Automatic Lambda resource detection
tracer, provider = init_telemetry("my-lambda-function")

def lambda_handler(event, context):
    # Use the traced_handler context manager
    with traced_handler(tracer, provider, "lambda_handler"):
        # Your handler code here
        process_event(event)
        return {"statusCode": 200}

def process_event(event):
    with tracer.start_as_current_span("process_event") as span:
        span.set_attribute("event.type", event.get("type"))
        # Process the event
```

You can also customize the telemetry setup by providing your own processor and exporter:

```python
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter

# Initialize with custom processor and exporter
tracer, provider = init_telemetry(
    "my-lambda-function",
    processor=BatchSpanProcessor(
        OTLPSpanExporter(endpoint="https://my-collector:4318")
    )
)
```

## Processing Modes

The library supports three processing modes, controlled by the `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE` environment variable:

1. **Synchronous Mode** (`sync`, default)
   - Spans are exported immediately in the handler thread
   - Best for development and debugging
   - Highest latency but immediate span visibility
   - Does not install the internal extension thread and the sigterm handler

2. **Asynchronous Mode** (`async`)
   - Spans are queued and processed by the internal extension thread
   - Export occurs after handler completion
   - Best for production use
   - Minimal impact on handler latency
   - Install the sigterm handler to flush remaining spans on termination

3. **Finalize Mode** (`finalize`)
   - Install only the sigterm handler to flush remaining spans on termination
   - Typically used with the BatchSpanProcessor from the OpenTelemetry SDK for periodic flushes

## Environment Variables

The library can be configured using the following environment variables:

- `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Processing mode (`sync`, `async`, or `finalize`)
- `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Maximum number of spans to queue (default: 2048)
- `LAMBDA_EXTENSION_SPAN_PROCESSOR_FREQUENCY`: How often to flush spans in async mode (default: 1)

## Best Practices

1. **Initialization**
   - Initialize telemetry outside the handler
   - Use appropriate processing mode for your use case
   - Configure queue size based on span volume

2. **Handler Instrumentation**
   - Use `traced_handler` for automatic context management
   - Add relevant attributes to spans
   - Handle errors appropriately

3. **Resource Management**
   - Monitor queue size in high-volume scenarios
   - Use async mode for optimal performance
   - Consider memory constraints when configuring

4. **Error Handling**
   - Record exceptions in spans
   - Set appropriate span status
   - Use try/finally blocks for proper cleanup

## Integration with otlp-stdout-adapter

For complete serverless observability, combine with `otlp-stdout-adapter`:

```python
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from otlp_stdout_adapter import StdoutAdapter, get_lambda_resource
from lambda_otel_lite import init_telemetry, traced_handler

# Initialize with stdout adapter
tracer, provider = init_telemetry(
    "my-lambda-function",
    exporter=OTLPSpanExporter(
        session=StdoutAdapter().get_session()
    )
)

def lambda_handler(event, context):
    with traced_handler(tracer, provider, "lambda_handler"):
        # Your handler code here
        return {"statusCode": 200}
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details. 