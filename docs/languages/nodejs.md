---
layout: default
title: Node.js
parent: Language Support
nav_order: 3
---

# Node.js Support
{: .fs-9 }

Modern, Promise-based integration with OpenTelemetry through the `@dev7a/otlp-stdout-exporter` package.
{: .fs-6 .fw-300 }

## Quick Links
{: .text-delta }

[![npm](https://img.shields.io/npm/v/@dev7a/otlp-stdout-exporter.svg)](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
[![Node Version](https://img.shields.io/node/v/@dev7a/otlp-stdout-exporter.svg)](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

- [Source Code](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/node/exporter)
- [NPM Package](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
- [Examples](https://github.com/dev7a/lambda-otlp-forwarder/tree/main/packages/node/exporter/examples)
- [Change Log](https://github.com/dev7a/lambda-otlp-forwarder/blob/main/packages/node/exporter/CHANGELOG.md)

## Installation
{: .text-delta }

```bash
npm install @dev7a/otlp-stdout-exporter @opentelemetry/api @opentelemetry/sdk-trace-node @opentelemetry/sdk-trace-base
```

## Basic Usage
{: .text-delta }

```javascript
const { NodeTracerProvider } = require('@opentelemetry/sdk-trace-node');
const { BatchSpanProcessor } = require('@opentelemetry/sdk-trace-base');
const { Resource } = require('@opentelemetry/resources');
const { trace, SpanKind, context } = require('@opentelemetry/api');
const { StdoutOTLPExporterNode } = require('@dev7a/otlp-stdout-exporter');
const { AwsLambdaDetectorSync } = require('@opentelemetry/resource-detector-aws');

const createProvider = () => {
  const awsResource = new AwsLambdaDetectorSync().detect();
  const resource = new Resource({
    ["service.name"]: process.env.AWS_LAMBDA_FUNCTION_NAME || 'demo-function',
  }).merge(awsResource);

  const provider = new NodeTracerProvider({ resource });
  provider.addSpanProcessor(new BatchSpanProcessor(new StdoutOTLPExporterNode()));
  return provider;
};

exports.handler = async (event, context) => {
  const provider = createProvider();
  provider.register();
  const tracer = trace.getTracer('lambda-handler');

  try {
    const span = tracer.startSpan('process-request', {
      kind: SpanKind.SERVER
    });

    return await context.with(trace.setSpan(context.active(), span), async () => {
      const result = { message: 'Hello from Lambda!' };
      span.end();
      return {
        statusCode: 200,
        body: JSON.stringify(result)
      };
    });
  } finally {
    await provider.forceFlush();
  }
};
```

## Advanced Features
{: .text-delta }

### TypeScript Support
{: .text-delta }

```typescript
import { StdoutOTLPExporterNode, ExporterConfig } from '@dev7a/otlp-stdout-exporter';
import { SpanKind, SpanStatusCode } from '@opentelemetry/api';
import { APIGatewayProxyEvent, APIGatewayProxyResult } from 'aws-lambda';

const config: ExporterConfig = {
  compression: 'gzip',
  format: 'protobuf',
  timeout: 5000
};

const exporter = new StdoutOTLPExporterNode(config);
```

### Express/Fastify Integration
{: .text-delta }

```javascript
const { trace } = require('@opentelemetry/api');
const express = require('express');
const app = express();

app.use((req, res, next) => {
  const tracer = trace.getTracer('express-app');
  const span = tracer.startSpan('http-request', {
    attributes: {
      'http.method': req.method,
      'http.url': req.url
    }
  });

  res.on('finish', () => {
    span.setStatus({ code: res.statusCode < 400 ? SpanStatusCode.OK : SpanStatusCode.ERROR });
    span.end();
  });

  next();
});
```

### Custom Attribute Processors
{: .text-delta }

```javascript
const { diag, DiagLogLevel } = require('@opentelemetry/api');
const { SemanticAttributes } = require('@opentelemetry/semantic-conventions');

class CustomAttributeProcessor {
  onStart(span, context) {
    span.setAttribute('custom.startTime', Date.now());
    span.setAttribute(SemanticAttributes.HTTP_METHOD, 'POST');
  }

  onEnd(span) {
    span.setAttribute('custom.endTime', Date.now());
  }
}

provider.addSpanProcessor(new CustomAttributeProcessor());
```

## Best Practices
{: .text-delta }

### Resource Management
{: .text-delta }

{: .info }
- Always call `forceFlush()` in finally blocks
- Use proper Promise error handling
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
- Use try/catch blocks with span recording
- Set appropriate span status on errors
- Implement proper async error handling
- Add error details to spans

### Context Propagation
{: .text-delta }

{: .info }
- Use the context API properly
- Maintain context across async boundaries
- Handle baggage appropriately
- Use proper span lifecycle management

## Environment Variables
{: .text-delta }

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |
| `OTEL_NODE_RESOURCE_DETECTORS` | Enabled resource detectors | `aws` |
| `OTEL_NODEJS_LOG_LEVEL` | Logging level | `info` |

## Troubleshooting
{: .text-delta }

{: .warning }
Common issues and solutions:

1. **Missing Spans**
   - Check if `forceFlush()` is called
   - Verify span lifecycle management
   - Check context propagation

2. **Performance Issues**
   - Enable batch processing
   - Adjust batch configuration
   - Monitor memory usage

3. **Build Errors**
   - Check TypeScript configuration
   - Verify package versions
   - Enable appropriate features

## Examples
{: .text-delta }

### API Gateway Integration
{: .text-delta }

```javascript
const { trace, SpanStatusCode } = require('@opentelemetry/api');
const { SemanticAttributes } = require('@opentelemetry/semantic-conventions');

exports.handler = async (event, context) => {
  const provider = createProvider();
  provider.register();
  const tracer = trace.getTracer('api-handler');

  try {
    const span = tracer.startSpan('process-request', {
      kind: SpanKind.SERVER,
      attributes: {
        [SemanticAttributes.HTTP_METHOD]: event.httpMethod,
        [SemanticAttributes.HTTP_ROUTE]: event.resource,
        'aws.requestId': context.awsRequestId
      }
    });

    return await context.with(trace.setSpan(context.active(), span), async () => {
      try {
        const result = await processRequest(event);
        span.setStatus({ code: SpanStatusCode.OK });
        
        return {
          statusCode: 200,
          body: JSON.stringify(result),
          headers: {
            'Content-Type': 'application/json'
          }
        };
      } catch (error) {
        span.recordException(error);
        span.setStatus({
          code: SpanStatusCode.ERROR,
          message: error.message
        });
        throw error;
      } finally {
        span.end();
      }
    });
  } finally {
    await provider.forceFlush();
  }
};
```

## Next Steps
{: .text-delta }

- [Configure Processors](../concepts/processors)
- [Performance Tuning](../advanced/performance)
- [Monitoring Setup](../deployment/monitoring)