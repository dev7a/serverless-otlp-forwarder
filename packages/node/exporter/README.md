# @dev7a/otlp-stdout-exporter

OpenTelemetry OTLP exporter that writes to stdout: the telemetry data is serialized using OTLP JSON or Protobuf encoding, and then written to stdout within a structured JSON object.
In a AWS Lambda serverless environment, the data is captured by CloudWatch and can be consumed by the Serverless OTLP Forwarder, to be forwarded to a OTEL collector or your telemetry platform.

> [!IMPORTANT] 
> This package is highly experimental and should not be used in production. Contributions are welcome.

## Features

- Implements OTLP exporter for OpenTelemetry traces
- Exports data to stdout in a structured JSON format
- Supports both JSON and Protobuf formats
- Designed for serverless environments, especially AWS Lambda
- Configurable through environment variables
- Support for GZIP compression
- Base64 encoding for binary payloads

## Installation

Install the package along with its peer dependencies:

```bash
# Using npm
npm install @dev7a/otlp-stdout-exporter @opentelemetry/api @opentelemetry/sdk-trace-node @opentelemetry/resources @opentelemetry/semantic-conventions @opentelemetry/resource-detector-aws @opentelemetry/sdk-trace-base @opentelemetry/core @opentelemetry/otlp-exporter-base

# Using yarn
yarn add @dev7a/otlp-stdout-exporter @opentelemetry/api @opentelemetry/sdk-trace-node @opentelemetry/resources @opentelemetry/semantic-conventions @opentelemetry/resource-detector-aws @opentelemetry/sdk-trace-base @opentelemetry/core @opentelemetry/otlp-exporter-base

# Using pnpm
pnpm add @dev7a/otlp-stdout-exporter @opentelemetry/api @opentelemetry/sdk-trace-node @opentelemetry/resources @opentelemetry/semantic-conventions @opentelemetry/resource-detector-aws @opentelemetry/sdk-trace-base @opentelemetry/core @opentelemetry/otlp-exporter-base
```


## AWS Lambda Usage

Here's an example of using the exporter in an AWS Lambda function with distributed tracing:

```javascript
const { NodeTracerProvider } = require('@opentelemetry/sdk-trace-node');
const { BatchSpanProcessor } = require('@opentelemetry/sdk-trace-base');
const { Resource } = require('@opentelemetry/resources');
const { ATTR_SERVICE_NAME } = require('@opentelemetry/semantic-conventions');
const { trace, SpanKind, context, propagation } = require('@opentelemetry/api');
const { StdoutOTLPExporterNode } = require('@dev7a/otlp-stdout-exporter');
const { AwsLambdaDetectorSync } = require('@opentelemetry/resource-detector-aws');
const { W3CTraceContextPropagator } = require('@opentelemetry/core');
const { CompressionAlgorithm } = require('@opentelemetry/otlp-exporter-base');

// Set up W3C Trace Context propagator
propagation.setGlobalPropagator(new W3CTraceContextPropagator());

const createProvider = () => {
  // Detect AWS Lambda resources synchronously
  const awsResource = new AwsLambdaDetectorSync().detect();
  
  // Merge AWS resource with service name
  const resource = new Resource({
    [ATTR_SERVICE_NAME]: process.env.AWS_LAMBDA_FUNCTION_NAME || 'my-lambda-function',
  }).merge(awsResource);

  const provider = new NodeTracerProvider({
    resource,
    spanProcessors: [
      // Use BatchSpanProcessor for efficient processing
      new BatchSpanProcessor(
        // Configure the stdout exporter
        new StdoutOTLPExporterNode({
          timeoutMillis: 5000,
          compression: CompressionAlgorithm.GZIP, // No compression for Lambda stdout
        }),
      ),
    ],
  });

  return provider;
};

// Initialize the provider
const provider = createProvider();
provider.register();
const tracer = trace.getTracer('lambda-function');

async function processEvent(event) {
  const span = tracer.startSpan('process_event');
  
  try {
    // Your processing logic here
    return { processed: true };
  } finally {
    span.end();
  }
}

// Example Lambda handler with tracing
exports.handler = async (event, lambdaContext) => {
  const parentSpan = tracer.startSpan('lambda_handler', {
    kind: SpanKind.SERVER
  });

  return await context.with(trace.setSpan(context.active(), parentSpan), async () => {
    try {
      // Add event information to span
      parentSpan.setAttribute('event.type', event.type);
      parentSpan.addEvent('Processing Lambda event');

      // Your Lambda logic here
      const result = await processEvent(event);

      return {
        statusCode: 200,
        body: JSON.stringify(result)
      };
    } catch (error) {
      parentSpan.recordException(error);
      parentSpan.setStatus({ code: 1 });
      throw error;
    } finally {
      parentSpan.end();
      // Ensure spans are exported before Lambda freezes
      await provider.forceFlush();
    }
  });
};
```

### Lambda Configuration Notes and Best Practices

- The exporter writes to stdout, which is automatically captured by CloudWatch Logs
- Always call `provider.forceFlush()` before the Lambda handler completes if you want to ensure all spans are exported when using a batch span processor
- The `AwsLambdaDetectorSync` automatically detects Lambda environment details
- Consider Lambda timeout when setting `timeoutMillis` for the exporter

It may be advisable to set the protocol to `http/protobuf` and enable compression to reduce the overhead (and cost)of JSON serialization in a serverless environment, by setting the environment variables:
```
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
OTEL_EXPORTER_OTLP_COMPRESSION=gzip
```

#### Best Practices

1. **Initialize Outside Handler**
   - Create the provider and tracer outside the handler to reuse across invocations

2. **Resource Detection**
   - Use `AwsLambdaDetectorSync` to automatically capture Lambda metadata
   - Merge with custom resources as needed

3. **Span Processing**
   - Use `BatchSpanProcessor` for efficient span processing
   - Always flush spans before handler completion

4. **Error Handling**
   - Record exceptions and set appropriate span status
   - Ensure spans are ended in finally blocks

5. **Context Propagation**
   - Use W3C Trace Context for distributed tracing
   - Propagate context in outgoing requests


## Configuration

### Environment Variables

| Variable                           | Description                                      | Default               |
| ---------------------------------- | ------------------------------------------------ | --------------------- |
| OTEL_EXPORTER_OTLP_PROTOCOL        | Protocol to use ('http/json' or 'http/protobuf') | 'http/protobuf'       |
| OTEL_EXPORTER_OTLP_ENDPOINT        | General endpoint URL                             | http://localhost:4318 |
| OTEL_EXPORTER_OTLP_TRACES_ENDPOINT | Traces-specific endpoint URL                     | -                     |
| OTEL_SERVICE_NAME                  | Service name                                     | -                     |
| AWS_LAMBDA_FUNCTION_NAME           | Fallback service name                            | -                     |
| OTEL_EXPORTER_OTLP_COMPRESSION     | Compression algorithm ('gzip' or 'none')         | 'none'                |

## Output Format

The exporter writes JSON objects to stdout in the following format:

```json
{
  "__otel_otlp_stdout": "@dev7a/otlp-stdout-exporter@0.1.0",
  "source": "service-name",
  "endpoint": "endpoint-url",
  "method": "POST",
  "content-type": "application/json",
  "headers": {},
  "payload": "...",
  "base64": true,
  "content-encoding": "gzip"
}
```
The `__otel_otlp_stdout` field is used to identify the data as telemetry data from this exporter, and is used to define a CloudWatch Logs subscription filter for the Serverless OTLP Forwarder.

## License

MIT

