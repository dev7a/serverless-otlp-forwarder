# Lambda OTel Lite

[![PyPI](https://img.shields.io/pypi/v/lambda-otel-lite.svg)](https://pypi.org/project/lambda-otel-lite/)

The `lambda-otel-lite` library provides a lightweight, efficient OpenTelemetry implementation specifically designed for AWS Lambda environments. It features a custom span processor and internal extension mechanism that optimizes telemetry collection for Lambda's unique execution model.

By leveraging Lambda's execution lifecycle and providing multiple processing modes, this library enables efficient telemetry collection with minimal impact on function latency. By default, it uses the [otlp-stdout-span-exporter](https://pypi.org/project/otlp-stdout-span-exporter) to export spans to stdout for the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) project.

>[!IMPORTANT]
>This package is highly experimental and should not be used in production. Contributions are welcome.

## Table of Contents

- [Requirements](#requirements)
- [Features](#features)
- [Architecture and Modules](#architecture-and-modules)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Processing Modes](#processing-modes)
  - [Async Processing Mode Architecture](#async-processing-mode-architecture)
- [Telemetry Configuration](#telemetry-configuration)
  - [Custom configuration with custom resource attributes](#custom-configuration-with-custom-resource-attributes)
  - [Custom configuration with custom span processors](#custom-configuration-with-custom-span-processors)
  - [Custom configuration with context propagators](#custom-configuration-with-context-propagators)
  - [Custom configuration with ID generator](#custom-configuration-with-id-generator)
  - [Library specific Resource Attributes](#library-specific-resource-attributes)
- [Event Extractors](#event-extractors)
  - [Automatic Attributes extraction](#automatic-attributes-extraction)
  - [Built-in Extractors](#built-in-extractors)
  - [Custom Extractors](#custom-extractors)
- [Environment Variables](#environment-variables)
  - [Processing Configuration](#processing-configuration)
  - [Resource Configuration](#resource-configuration)
  - [Export Configuration](#export-configuration)
  - [Logging](#logging)
  - [AWS Lambda Environment](#aws-lambda-environment)
- [License](#license)
- [See Also](#see-also)

## Requirements

- Python 3.12+
- OpenTelemetry packages (automatically installed as dependencies)
- For OTLP HTTP export: Additional dependencies available via `pip install "lambda_otel_lite[otlp-http]"`

## Features

- **Flexible Processing Modes**: Support for synchronous, asynchronous, and custom export strategies
- **Automatic Resource Detection**: Automatic extraction of Lambda environment attributes
- **Lambda Extension Integration**: Built-in extension for efficient telemetry export
- **Efficient Memory Usage**: Fixed-size queue to prevent memory growth
- **AWS Event Support**: Automatic extraction of attributes from common AWS event types
- **Flexible Context Propagation**: Support for W3C Trace Context, AWS X-Ray, and custom propagators. Now supports configuration via the `OTEL_PROPAGATORS` environment variable (comma-separated list: `tracecontext`, `xray`, `xray-lambda`, `none`).

## Architecture and Modules

- `telemetry`: Core initialization and configuration
  - Main entry point via `init_telemetry`
  - Configures global tracer and span processors
  - Returns a `TelemetryCompletionHandler` for span lifecycle management

- `processor`: Lambda-optimized span processor
  - Fixed-size queue implementation
  - Multiple processing modes
  - Coordinates with extension for async export

- `extension`: Lambda Extension implementation
  - Manages extension lifecycle and registration
  - Handles span export coordination
  - Implements graceful shutdown

- `extractors`: Event processing
  - Built-in support for API Gateway and ALB events
  - Extensible interface for custom events
  - W3C Trace Context propagation

- `handler`: Handler decorator
  - Provides `create_traced_handler` function to create tracing decorators
  - Automatically tracks cold starts using the `faas.cold_start` attribute
  - Extracts and propagates context from request headers
  - Manages span lifecycle with automatic status handling for HTTP responses
  - Records exceptions in spans with appropriate status codes
  - Properly completes telemetry processing on handler completion

## Installation

```bash
# Requires Python 3.12+
pip install lambda_otel_lite

# Optional: For OTLP HTTP export support
pip install "lambda_otel_lite[otlp-http]"
```

## Quick Start

```python
from opentelemetry import trace
from lambda_otel_lite import init_telemetry, create_traced_handler
from lambda_otel_lite.extractors import api_gateway_v2_extractor
import json

# Initialize telemetry once, outside the handler
tracer, completion_handler = init_telemetry()

# Define business logic separately
def process_user(user_id):
    # Your business logic here
    return {"name": "User Name", "id": user_id}

# Create traced handler with specific extractor
traced = create_traced_handler(
    name="my-api-handler",
    completion_handler=completion_handler,
    attributes_extractor=api_gateway_v2_extractor
)

@traced
def handler(event, context):
    try:
        # Get current span to add custom attributes
        current_span = trace.get_current_span()
        current_span.set_attribute("handler.version", "1.0")
        
        # Extract userId from event path parameters
        path_parameters = event.get("pathParameters", {}) or {}
        user_id = path_parameters.get("userId", "unknown")
        current_span.set_attribute("user.id", user_id)
        
        # Process business logic
        user = process_user(user_id)
        
        # Return formatted HTTP response
        return {
            "statusCode": 200,
            "headers": {"Content-Type": "application/json"},
            "body": json.dumps({"success": True, "data": user})
        }
    except Exception as error:
        # Simple error handling (see Error Handling section for more details)
        return {
            "statusCode": 500,
            "headers": {"Content-Type": "application/json"},
            "body": json.dumps({
                "success": False,
                "error": "Internal server error"
            })
        }
```

## Processing Modes

The library supports three processing modes for span export:

1. **Sync Mode** (default):
   - Direct, synchronous export in handler thread
   - Recommended for:
     - low-volume telemetry
     - limited resources (memory, cpu)
     - when latency is not critical
   - Set via `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE=sync`

2. **Async Mode**:
   - Export via Lambda extension using AWS Lambda Extensions API
   - Spans are queued and exported after handler completion
   - Uses event-based communication between handler and extension
   - Registers specifically for Lambda INVOKE events
   - Implements graceful shutdown with SIGTERM handling
   - Error handling for:
     - Event communication failures
     - Export failures
     - Extension registration issues
   - Best for production use with high telemetry volume
   - Set via `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE=async`

3. **Finalize Mode**:
   - Registers extension with no events
   - Maintains SIGTERM handler for graceful shutdown
   - Ensures all spans are flushed during shutdown
   - Compatible with BatchSpanProcessor for custom export strategies
   - Best for specialized export requirements where you need full control
   - Set via `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE=finalize`

### Async Processing Mode Architecture

```mermaid
sequenceDiagram
    participant Lambda Runtime
    participant Extension Thread
    participant Handler
    participant LambdaSpanProcessor
    participant OTLPStdoutSpanExporter

    Note over Extension Thread: Initialization
    Extension Thread->>Lambda Runtime: Register extension (POST /register)
    Lambda Runtime-->>Extension Thread: Extension ID
    Extension Thread->>Lambda Runtime: Get next event (GET /next)

    Note over Handler: Function Invocation
    Handler->>LambdaSpanProcessor: Create & queue spans
    Note over LambdaSpanProcessor: Spans stored in fixed-size queue

    Handler->>Extension Thread: Set handler_complete_event
    Note over Handler: Handler returns response

    Extension Thread->>LambdaSpanProcessor: process_spans()
    LambdaSpanProcessor->>OTLPStdoutSpanExporter: export() batched spans
    Extension Thread->>Lambda Runtime: Get next event (GET /next)

    Note over Extension Thread: On SIGTERM
    Lambda Runtime->>Extension Thread: SHUTDOWN event
    Extension Thread->>LambdaSpanProcessor: force_flush()
    LambdaSpanProcessor->>OTLPStdoutSpanExporter: export() remaining spans
```

The async mode leverages Lambda's extension API to optimize perceived latency by deferring span export until after the response is sent to the user. The diagram above shows the core coordination between components:

1. Extension thread registers and waits for events from Runtime
2. Handler queues spans during execution via LambdaSpanProcessor
3. Handler signals completion via event before returning
4. Extension processes and exports queued spans after handler completes
5. Extension returns to waiting for next event
6. On shutdown, remaining spans are flushed and exported

## Telemetry Configuration

The library provides several ways to configure the OpenTelemetry tracing pipeline, which is a required first step to instrument your Lambda function:

### Custom configuration with custom resource attributes

```python
from opentelemetry.sdk.resources import Resource
from lambda_otel_lite import init_telemetry

# Create a custom resource with additional attributes
resource = Resource.create({
    "service.version": "1.0.0",
    "deployment.environment": "production",
    "custom.attribute": "value"
})

# Initialize with custom resource
tracer, completion_handler = init_telemetry(resource=resource)

# Use the tracer and completion handler as usual
```

### Custom configuration with custom span processors

```python
from opentelemetry.sdk.trace import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from lambda_otel_lite import init_telemetry

# First install the optional dependency:
# pip install "lambda_otel_lite[otlp-http]"

# Create a custom processor with OTLP HTTP exporter
processor = BatchSpanProcessor(
    OTLPSpanExporter(
        endpoint="https://your-otlp-endpoint/v1/traces"
    )
)

# Initialize with custom processor
tracer, completion_handler = init_telemetry(span_processors=[processor])
```

You can provide multiple span processors, and they will all be used to process spans. This allows you to send telemetry to multiple destinations or use different processing strategies for different types of spans.

### Custom configuration with context propagators

You can now also configure context propagation using the `OTEL_PROPAGATORS` environment variable, which takes precedence over the `propagators` parameter. Supported values: `tracecontext`, `xray`, `xray-lambda`, `none` (comma-separated for multiple). For example:

```bash
export OTEL_PROPAGATORS="xray,tracecontext"
```

If neither the environment variable nor the parameter is set, the default is `[LambdaXrayPropagator(), TraceContextTextMapPropagator()]`.

```python
from opentelemetry.propagate import set_global_textmap
from opentelemetry.propagators.b3 import B3Format
from opentelemetry.trace.propagation.tracecontext import TraceContextTextMapPropagator
from lambda_otel_lite import init_telemetry

# Create a custom configuration with specific propagators
tracer, completion_handler = init_telemetry(
    propagators=[
        TraceContextTextMapPropagator(),  # W3C Trace Context
        B3Format(),  # B3 format for Zipkin compatibility
    ]
)
```

By default, OpenTelemetry Python uses W3C Trace Context and W3C Baggage propagators. The `propagators` parameter allows you to customize which propagators are used for context extraction and injection. This is useful when you need to integrate with systems that use different context propagation formats.

You can provide multiple propagators, and they will be combined into a composite propagator. The order matters - propagators are applied in the order they are provided.

> **Note:** The OpenTelemetry SDK also supports configuring propagators via the `OTEL_PROPAGATORS` environment variable. If set, this environment variable takes precedence over programmatic configuration. See the [OpenTelemetry Python documentation](https://opentelemetry.io/docs/languages/python/instrumentation/) for more details.

### Custom configuration with ID generator

```python
from opentelemetry.sdk.extension.aws.trace import AwsXRayIdGenerator
from lambda_otel_lite import init_telemetry

# Initialize with X-Ray compatible ID generator
tracer, completion_handler = init_telemetry(
    id_generator=AwsXRayIdGenerator()
)
```

By default, OpenTelemetry uses a random ID generator that creates W3C-compatible trace and span IDs. The `id_generator` parameter allows you to customize the ID generation strategy. This is particularly useful when you need to integrate with AWS X-Ray, which requires a specific ID format.

To use the X-Ray ID generator, you'll need to install the AWS X-Ray SDK for OpenTelemetry:

```bash
pip install opentelemetry-sdk-extension-aws
```

### Library specific Resource Attributes

The library adds several resource attributes under the `lambda_otel_lite` namespace to provide configuration visibility:

- `lambda_otel_lite.extension.span_processor_mode`: Current processing mode (`sync`, `async`, or `finalize`)
- `lambda_otel_lite.lambda_span_processor.queue_size`: Maximum number of spans that can be queued
- `lambda_otel_lite.lambda_span_processor.batch_size`: Maximum batch size for span export
- `lambda_otel_lite.otlp_stdout_span_exporter.compression_level`: GZIP compression level used for span export

These attributes are automatically added to the resource and can be used to understand the telemetry configuration in your observability backend.

## Event Extractors

Event extractors are responsible for extracting span attributes and context from Lambda event and context objects. The library provides built-in extractors for common Lambda triggers.

### Automatic Attributes extraction

The library automatically sets relevant FAAS attributes based on the Lambda context and event. Both `event` and `context` parameters must be passed to `tracedHandler` to enable all automatic attributes:

- Resource Attributes (set at initialization):
  - `cloud.provider`: "aws"
  - `cloud.region`: from AWS_REGION
  - `faas.name`: from AWS_LAMBDA_FUNCTION_NAME
  - `faas.version`: from AWS_LAMBDA_FUNCTION_VERSION
  - `faas.instance`: from AWS_LAMBDA_LOG_STREAM_NAME
  - `faas.max_memory`: from AWS_LAMBDA_FUNCTION_MEMORY_SIZE
  - `service.name`: from OTEL_SERVICE_NAME (defaults to function name)
  - Additional attributes from OTEL_RESOURCE_ATTRIBUTES (URL-decoded)

- Span Attributes (set per invocation when passing context):
  - `faas.cold_start`: true on first invocation
  - `cloud.account.id`: extracted from context's invokedFunctionArn
  - `faas.invocation_id`: from awsRequestId
  - `cloud.resource_id`: from context's invokedFunctionArn

- HTTP Attributes (set for API Gateway events):
  - `faas.trigger`: "http"
  - `http.status_code`: from handler response
  - `http.route`: from routeKey (v2) or resource (v1)
  - `http.method`: from requestContext (v2) or httpMethod (v1)
  - `http.target`: from path
  - `http.scheme`: from protocol

The library automatically detects API Gateway v1 and v2 events and sets the appropriate HTTP attributes. For HTTP responses, the status code is automatically extracted from the handler's response and set as `http.status_code`. For 5xx responses, the span status is set to ERROR.

### Built-in Extractors

```python
from lambda_otel_lite.extractors import (
    api_gateway_v1_extractor,  # API Gateway REST API
    api_gateway_v2_extractor,  # API Gateway HTTP API
    alb_extractor,            # Application Load Balancer
    default_extractor,        # Basic Lambda attributes
)
```

Each extractor is designed to handle a specific event type and extract relevant attributes:

- `api_gateway_v1_extractor`: Extracts HTTP attributes from API Gateway REST API events
- `api_gateway_v2_extractor`: Extracts HTTP attributes from API Gateway HTTP API events
- `alb_extractor`: Extracts HTTP attributes from Application Load Balancer events
- `default_extractor`: Extracts basic Lambda attributes from any event type

```python
from lambda_otel_lite.extractors import api_gateway_v1_extractor
import json
# Initialize telemetry with default configuration
tracer, completion_handler = init_telemetry()

traced = create_traced_handler(
    name="api-v1-handler",
    completion_handler=completion_handler,
    attributes_extractor=api_gateway_v1_extractor
)

@traced
def handler(event, context):
    # Your handler code
    return {
        "statusCode": 200,
        "body": json.dumps({"message": "Hello, world!"})
    }
```

### Custom Extractors

You can create custom extractors for event types not directly supported by the library by implementing the extractor interface:

```python
from lambda_otel_lite.extractors import SpanAttributes, TriggerType
from lambda_otel_lite import create_traced_handler, init_telemetry

def custom_extractor(event, context) -> SpanAttributes:
    return SpanAttributes(
        trigger=TriggerType.OTHER,  # Or any custom string
        attributes={
            'custom.attribute': 'value',
            # ... other attributes
        },
        span_name='custom-operation',  # Optional
        carrier=event.get('headers')   # Optional: For context propagation
    )

# Initialize telemetry
tracer, completion_handler = init_telemetry()

# Create traced handler with custom extractor
traced = create_traced_handler(
    name="custom-handler",
    completion_handler=completion_handler,
    attributes_extractor=custom_extractor
)

@traced
def handler(event, context):
    # Your handler code
    return {"statusCode": 200}
```

The `SpanAttributes` object returned by the extractor contains:

- `trigger`: The type of trigger (HTTP, SQS, etc.) - affects how spans are named
- `attributes`: A dictionary of attributes to add to the span
- `span_name`: Optional custom name for the span (defaults to handler name)
- `carrier`: Optional dictionary containing trace context headers for propagation

## Environment Variables

The library can be configured using the following environment variables:

### Processing Configuration
- `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Controls span processing strategy
  - `sync`: Direct export in handler thread (default)
  - `async`: Deferred export via extension
  - `finalize`: Custom export strategy
- `LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE`: Maximum number of spans to queue (default: 2048)
- `LAMBDA_SPAN_PROCESSOR_BATCH_SIZE`: Maximum number of spans to export in each batch (default: 512)

### Resource Configuration
- `OTEL_SERVICE_NAME`: Override the service name (defaults to function name)
- `OTEL_RESOURCE_ATTRIBUTES`: Additional resource attributes in key=value,key2=value2 format

### Export Configuration
- `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: Gzip compression level for stdout exporter
  - 0: No compression
  - 1: Best speed
  - 6: Good balance between size and speed (default)
  - 9: Best compression

### Logging
- `AWS_LAMBDA_LOG_LEVEL` or `LOG_LEVEL`: Configure log level (debug, info, warn, error, none)

### AWS Lambda Environment
The following AWS Lambda environment variables are automatically used for resource attributes:
- `AWS_REGION`: Region where function runs
- `AWS_LAMBDA_FUNCTION_NAME`: Function name
- `AWS_LAMBDA_FUNCTION_VERSION`: Function version
- `AWS_LAMBDA_LOG_STREAM_NAME`: Log stream name
- `AWS_LAMBDA_FUNCTION_MEMORY_SIZE`: Function memory size

- `OTEL_PROPAGATORS`: Comma-separated list of propagators to use for context propagation. Supported: `tracecontext`, `xray`, `xray-lambda`, `none`. Takes precedence over programmatic configuration.

## License

MIT 

## See Also

- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder) - The main project repository for the Serverless OTLP Forwarder project
- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/node/lambda-otel-lite) | [npm](https://www.npmjs.com/package/@dev7a/lambda-otel-lite) - The Node.js version of this library
- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/lambda-otel-lite) | [crates.io](https://crates.io/crates/lambda-otel-lite) - The Rust version of this library