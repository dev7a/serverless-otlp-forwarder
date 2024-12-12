---
layout: default
title: Node.js
parent: Language Support
nav_order: 3
---

# Node.js Support

The Node.js implementation provides a modern, Promise-based integration with OpenTelemetry through the `@dev7a/otlp-stdout-exporter` package.

## Quick Links
- [Source Code](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/node/exporter)
- [Documentation](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/node/exporter/README.md)
- [NPM Package](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
- [Examples](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/node/exporter/examples)

## Features
- Compatible with OpenTelemetry JS
- Promise-based API
- TypeScript support
- AWS Lambda context integration
- Batch processing support

## Example Usage

```javascript
const { NodeTracerProvider } = require('@opentelemetry/sdk-trace-node');
const { BatchSpanProcessor } = require('@opentelemetry/sdk-trace-base');
const { Resource } = require('@opentelemetry/resources');
const { trace, SpanKind, context, propagation } = require('@opentelemetry/api');
const { StdoutOTLPExporterNode } = require('@dev7a/otlp-stdout-exporter');
const { AwsLambdaDetectorSync } = require('@opentelemetry/resource-detector-aws');
const { W3CTraceContextPropagator } = require('@opentelemetry/core');

// Set up W3C Trace Context propagator
propagation.setGlobalPropagator(new W3CTraceContextPropagator());

const createProvider = () => {
  const awsResource = new AwsLambdaDetectorSync().detect();
  const resource = new Resource({
    ["service.name"]: process.env.AWS_LAMBDA_FUNCTION_NAME || 'demo-function',
  }).merge(awsResource);

  const provider = new NodeTracerProvider({ resource });
  provider.addSpanProcessor(new BatchSpanProcessor(new StdoutOTLPExporterNode()));
  return provider;
};

const provider = createProvider();
provider.register();
const tracer = trace.getTracer('demo-function');

exports.handler = async (event, context) => {
  const parentSpan = tracer.startSpan('lambda-invocation', {
    kind: SpanKind.SERVER
  });

  return await context.with(trace.setSpan(context.active(), parentSpan), async () => {
    try {
      const result = { message: 'Hello from Lambda!' };
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
      await provider.forceFlush();
    }
  });
};
```

## Configuration

The Node.js implementation can be configured through environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |

## Best Practices

1. **Error Handling**
   - Use try/catch blocks with span recording
   - Set appropriate span status on errors
   - Implement proper async error handling

2. **Resource Management**
   - Always call `forceFlush()` in finally blocks
   - Clean up resources properly
   - Handle Lambda freezing gracefully

3. **Performance**
   - Use batch processing when possible
   - Enable compression for large payloads
   - Configure appropriate timeouts

4. **Context Propagation**
   - Use the W3C Trace Context propagator
   - Maintain context across async boundaries
   - Properly manage active spans 