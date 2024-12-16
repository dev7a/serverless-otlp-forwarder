---
layout: default
title: Node.js
parent: Language Support
nav_order: 3
---

# Node.js Support
{: .fs-9 }

OpenTelemetry exporter for AWS Lambda that writes telemetry data to stdout in OTLP format.
{: .fs-6 .fw-300 }

## Quick Links
{: .text-delta }

[![npm](https://img.shields.io/npm/v/@dev7a/otlp-stdout-exporter.svg)](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
[![Node Version](https://img.shields.io/node/v/@dev7a/otlp-stdout-exporter.svg)](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

- [Source Code](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/node/exporter)
- [NPM Package](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
- [Examples](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/node/exporter/examples)
- [Change Log](https://github.com/dev7a/serverless-otlp-forwarder/blob/main/packages/node/exporter/CHANGELOG.md)

## Installation
{: .text-delta }

Install the exporter and its dependencies:

```bash
npm install @dev7a/otlp-stdout-exporter \
            @opentelemetry/api \
            @opentelemetry/sdk-trace-node \
            @opentelemetry/sdk-trace-base \
            @opentelemetry/resources \
            @opentelemetry/resource-detector-aws
```

{: .note }
The package requires Node.js 18 or later.

## Basic Usage
{: .text-delta }

The following example shows an AWS Lambda function that:
- Handles API Gateway requests
- Creates a tracer provider with AWS Lambda resource detection
- Creates a span for each invocation
- Ensures telemetry is flushed before the function exits

{: .note }
The provider is created for each invocation to handle Lambda cold starts and ensure proper resource cleanup.

```javascript
const { NodeTracerProvider } = require('@opentelemetry/sdk-trace-node');
const { BatchSpanProcessor } = require('@opentelemetry/sdk-trace-base');
const { Resource } = require('@opentelemetry/resources');
const { trace, SpanKind, context } = require('@opentelemetry/api');
const { StdoutOTLPExporterNode } = require('@dev7a/otlp-stdout-exporter');
const { AwsLambdaDetectorSync } = require('@opentelemetry/resource-detector-aws');

const createProvider = () => {
  // Detect AWS Lambda resources (function name, version, etc.)
  const awsResource = new AwsLambdaDetectorSync().detect();
  
  // Create a resource merging Lambda attributes with service name
  const resource = new Resource({
    ["service.name"]: process.env.AWS_LAMBDA_FUNCTION_NAME || 'demo-function',
  }).merge(awsResource);

  // Initialize provider with resource attributes
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
    // Ensure all spans are flushed before the Lambda ends
    await provider.forceFlush();
  }
};
```

## Environment Variables
{: .text-delta }

Configuration is handled through environment variables:

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | Protocol for OTLP data (`http/protobuf` or `http/json`) | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | Compression type (`gzip` or `none`) | `none` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |

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
