# Node.js OTLP Stdout Span Exporter

A Node.js span exporter that writes OpenTelemetry spans to stdout, using a custom serialization format that embeds the spans serialized as OTLP protobuf in the `payload` field. The message envelope carries metadata about the spans, such as the service name, the OTLP endpoint, and the HTTP method:

```json
{
  "__otel_otlp_stdout": "0.1.0",
  "source": "my-service",
  "endpoint": "http://localhost:4318/v1/traces",
  "method": "POST",
  "content-type": "application/x-protobuf",
  "content-encoding": "gzip",
  "headers": {
    "custom-header": "value"
  },
  "payload": "<base64-encoded-gzipped-protobuf>",
  "base64": true
}
```

Outputting telemetry data in this format directly to stdout makes the library easily usable in network constrained environments, or in environments that are particularly sensitive to the overhead of HTTP connections, such as AWS Lambda.

>[!IMPORTANT]
>This package is part of the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) project and is designed for AWS Lambda environments. While it can be used in other contexts, it's primarily tested with AWS Lambda.

## Features

- Uses OTLP Protobuf serialization for efficient encoding
- Applies GZIP compression with configurable levels
- Detects service name from environment variables
- Supports custom headers via environment variables
- Consistent JSON output format
- Zero external HTTP dependencies
- Lightweight and fast

## Installation

```bash
npm install @dev7a/otlp-stdout-span-exporter
```

## Usage

The recommended way to use this exporter is with the standard OpenTelemetry `BatchSpanProcessor`, which provides better performance by buffering and exporting spans in batches, or, in conjunction with the [lambda-otel-lite](https://www.npmjs.com/package/@dev7a/lambda-otel-lite) package, with the `LambdaSpanProcessor`, which is particularly optimized for AWS Lambda.

You can create a simple tracer provider with the BatchSpanProcessor and the OTLPStdoutSpanExporter:

```typescript
import { trace } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { BatchSpanProcessor } from '@opentelemetry/sdk-trace-base';
import { OTLPStdoutSpanExporter } from '@dev7a/otlp-stdout-span-exporter';

// Initialize the exporter
const exporter = new OTLPStdoutSpanExporter({ gzipLevel: 6 });
// Use batching processor for efficiency
const processor = new BatchSpanProcessor(exporter);
// Create a tracer provider
const provider = new NodeTracerProvider({
  // Register the exporter with the provider
  spanProcessors: [processor]
});

// Set as global default tracer provider
provider.register();

// Your instrumentation code here
const tracer = trace.getTracer('example-tracer');
tracer.startActiveSpan('my-operation', span => {
  span.setAttribute('my.attribute', 'value');
  // ... do work ...
  span.end();
});
```

## Configuration

### Constructor Options

```typescript
interface OTLPStdoutSpanExporterConfig {
  // GZIP compression level (0-9, where 0 is no compression and 9 is maximum compression)
  // Defaults to 6 or value from OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL
  gzipLevel?: number;
}
```

The explicit configuration via code will override any environment variable setting.

### Environment Variables

The exporter respects the following environment variables:

- `OTEL_SERVICE_NAME`: Service name to use in output
- `AWS_LAMBDA_FUNCTION_NAME`: Fallback service name (if `OTEL_SERVICE_NAME` not set)
- `OTEL_EXPORTER_OTLP_HEADERS`: Headers for OTLP export, used in the `headers` field
- `OTEL_EXPORTER_OTLP_TRACES_HEADERS`: Trace-specific headers (which take precedence if conflicting with `OTEL_EXPORTER_OTLP_HEADERS`)
- `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: GZIP compression level (0-9). Defaults to 6.

>[!NOTE]
>For security best practices, avoid including authentication credentials or sensitive information in headers. The serverless-otlp-forwarder infrastructure is designed to handle authentication at the destination, rather than embedding credentials in your telemetry data.


## Output Format

The exporter writes JSON objects to stdout with the following structure:

```json
{
  "__otel_otlp_stdout": "0.1.0",
  "source": "my-service",
  "endpoint": "http://localhost:4318/v1/traces",
  "method": "POST",
  "content-type": "application/x-protobuf",
  "content-encoding": "gzip",
  "headers": {
    "tenant-id": "tenant-12345",
    "custom-header": "value"
  },
  "base64": true,
  "payload": "<base64-encoded-gzipped-protobuf>"
}
```

- `__otel_otlp_stdout` is a marker to identify the output of this exporter.
- `source` is the emitting service name.
- `endpoint` is the OTLP endpoint (defaults to `http://localhost:4318/v1/traces` and just indicates the signal type. The actual endpoint is determined by the process that forwards the data).
- `method` is the HTTP method (always `POST`).
- `content-type` is the content type (always `application/x-protobuf`).
- `content-encoding` is the content encoding (always `gzip`).
- `headers` is the headers defined in the `OTEL_EXPORTER_OTLP_HEADERS` and `OTEL_EXPORTER_OTLP_TRACES_HEADERS` environment variables.
- `payload` is the base64-encoded, gzipped, Protobuf-serialized span data in OTLP format.
- `base64` is a boolean flag to indicate if the payload is base64-encoded (always `true`).

## License

MIT

## See Also

- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder) - The main project repository for the Serverless OTLP Forwarder project
- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/python/otlp-stdout-span-exporter) | [PyPI](https://pypi.org/project/otlp-stdout-span-exporter/) - The Python version of this exporter
- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/otlp-stdout-span-exporter) | [crates.io](https://crates.io/crates/otlp-stdout-span-exporter) - The Rust version of this exporter 