# Lambda OTel Lite

The `lambda-otel-lite` library provides a lightweight, efficient OpenTelemetry implementation specifically designed for AWS Lambda environments. It features a custom span processor and internal extension mechanism that optimizes telemetry collection for Lambda's unique execution model.

By leveraging Lambda's execution lifecycle and providing multiple processing modes, this library enables efficient telemetry collection with minimal impact on function latency. By default, it uses the [@dev7a/otlp-stdout-exporter](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter) to export spans to stdout for the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) project.

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
- Automatic FAAS attributes from Lambda context and events
- Cold start detection and tracking
- Configurable through environment variables
- Zero external dependencies beyond OpenTelemetry
- Optimized for cold start performance

## Installation

You can install the `lambda-otel-lite` package using npm:

```bash
npm install @dev7a/lambda-otel-lite
```

## Usage

### Basic Usage

```typescript
import { initTelemetry, tracedHandler } from '@dev7a/lambda-otel-lite';
import { SpanKind } from '@opentelemetry/api';

const { tracer, provider } = initTelemetry('my-service');

export const handler = async (event: any, context: any) => {
    return tracedHandler({
        tracer,
        provider,
        name: 'my-handler',
        kind: SpanKind.SERVER,
        event,  // Optional: Enables automatic FAAS attributes from event
        context,  // Optional: Enables automatic FAAS attributes from context
        fn: async (span) => {
            // Your handler code here
            await processEvent(event);
            return { statusCode: 200 };
        }
    });
};

async function processEvent(event: any) {
    return tracer.startActiveSpan('process_event', async (span) => {
        span.setAttribute('event.type', event.type);
        // Process the event
        span.end();
    });
}
```

### Automatic FAAS Attributes

The library automatically sets relevant FAAS attributes based on the Lambda context and event. Both `event` and `context` parameters must be passed to `tracedHandler` to enable all automatic attributes:

- Resource Attributes (set at initialization):
  - `cloud.provider`: "aws"
  - `cloud.region`: from AWS_REGION
  - `faas.name`: from AWS_LAMBDA_FUNCTION_NAME
  - `faas.version`: from AWS_LAMBDA_FUNCTION_VERSION
  - `faas.instance`: from AWS_LAMBDA_LOG_STREAM_NAME
  - `faas.max_memory`: from AWS_LAMBDA_FUNCTION_MEMORY_SIZE

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

Example with API Gateway:
```typescript
export const handler = async (event: any, context: any) => {
    return tracedHandler({
        tracer,
        provider,
        name: 'api-handler',
        event,
        context,
        fn: async (span) => {
            // HTTP attributes are automatically set based on event
            await processRequest(event);
            return {
                statusCode: 200,  // Will be set as http.status_code
                body: 'Success'
            };
        }
    });
};

export const errorHandler = async (event: any, context: any) => {
    return tracedHandler({
        tracer,
        provider,
        name: 'api-handler',
        event,
        context,
        fn: async (span) => {
            return {
                statusCode: 500,  // Will set http.status_code and span status ERROR
                body: 'Internal error'
            };
        }
    });
};
```

### Distributed Tracing

The library supports distributed tracing across service boundaries. Context propagation is handled automatically when you pass the `event` parameter and it contains a `headers` property. You can also provide a custom carrier extraction function for more complex scenarios:

```typescript
import { SpanKind } from '@opentelemetry/api';

export const handler = async (event: any, context: any) => {
    // Context propagation is handled automatically if event has 'headers'
    return tracedHandler({
        tracer,
        provider,
        name: 'my-handler',
        kind: SpanKind.SERVER,
        event,  // Will automatically extract context from event.headers if present
        context,
        attributes: { 'custom.attribute': 'value' },
        fn: async (span) => {
            // Your handler code here
            return { statusCode: 200 };
        }
    });
};

// For custom carrier extraction:
function extractFromSQS(event: any): Record<string, any> {
    // Extract tracing headers from the first record's message attributes
    if (event.Records?.[0]?.messageAttributes) {
        return event.Records[0].messageAttributes;
    }
    return {};
}

export const handlerWithCustomExtraction = async (event: any, context: any) => {
    return tracedHandler({
        tracer,
        provider,
        name: 'my-handler',
        kind: SpanKind.SERVER,
        event,
        context,
        getCarrier: extractFromSQS,  // Custom function to extract carrier from event
        fn: async (span) => {
            // Your handler code here
            return { statusCode: 200 };
        }
    });
};
```

The library provides:
- Automatic context extraction from `event.headers` for HTTP/API Gateway events
- Custom carrier extraction through the `getCarrier` parameter
- Support for any event type through custom extraction functions
- Seamless integration with OpenTelemetry context propagation

This allows you to:
- Maintain trace context across Lambda invocations
- Track requests as they flow through your distributed system
- Connect traces across different services and functions
- Support custom event sources and propagation mechanisms
- Visualize complete request flows in your observability platform

### Custom Configuration

You can customize the telemetry setup by providing your own processor and exporter:

```typescript
import { BatchSpanProcessor } from '@opentelemetry/sdk-trace-base';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http';

// Initialize with custom processor and exporter
const { tracer, provider } = initTelemetry('my-lambda-function', {
    spanProcessor: new BatchSpanProcessor(
        new OTLPTraceExporter({
            url: 'https://my-collector:4318/v1/traces'
        })
    )
});
```

## Processing Modes

The library supports three processing modes, controlled by the `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE` environment variable:

1. **Synchronous Mode** (`sync`, default)
   - Spans are exported immediately in the handler thread
   - Best for development and debugging
   - Highest latency but immediate span visibility
   - Does not install the internal extension thread and the sigterm handler
   - Recommended for lower memory configurations (< 512MB)

2. **Asynchronous Mode** (`async`)
   - Spans are queued and processed by the internal extension thread
   - Export occurs after handler completion
   - Best for production use with higher memory configurations (>= 512MB)
   - Minimal impact on handler latency at higher memory configurations
   - Install the sigterm handler to flush remaining spans on termination
   - Note: At lower memory configurations, the overhead of the extension loop can actually increase latency compared to sync mode

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
   - Use `tracedHandler` for automatic context management
   - Pass both event and context to enable all automatic FAAS attributes
   - Leverage automatic context propagation or provide custom extraction
   - Add relevant custom attributes to spans
   - Handle errors appropriately

3. **Resource Management**
   - Monitor queue size in high-volume scenarios
   - Use async mode for optimal performance
   - Consider memory constraints when configuring

4. **Error Handling**
   - Record exceptions in spans
   - Set appropriate span status
   - Use try/catch blocks for proper cleanup

5. **Cold Start Optimization**
   - Keep imports minimal
   - Initialize telemetry outside handler
   - Use async mode to minimize handler latency

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details. 