# Node.js OTLP Stdout Span Exporter

[![npm version](https://img.shields.io/npm/v/@dev7a/otlp-stdout-span-exporter.svg)](https://www.npmjs.com/package/@dev7a/otlp-stdout-span-exporter)

A Node.js span exporter that writes OpenTelemetry spans to stdout, using a custom serialization format that embeds the spans serialized as OTLP protobuf in the `payload` field. The message envelope carries metadata about the spans, such as the service name, the OTLP endpoint, and the HTTP method:

```json
{
  "__otel_otlp_stdout": "0.16.0",
  "source": "my-service",
  "endpoint": "http://localhost:4318/v1/traces",
  "method": "POST",
  "content-type": "application/x-protobuf",
  "content-encoding": "gzip",
  "headers": {
    "custom-header": "value"
  },
  "payload": "<base64-encoded-gzipped-protobuf>",
  "base64": true,
  "level": "DEBUG"
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
- Supports log level for filtering in log aggregation systems
- Supports writing to stdout or named pipe
- Consistent JSON output format
- Zero external HTTP dependencies
- Lightweight and fast

## Installation

```bash
npm install @dev7a/otlp-stdout-span-exporter
```

### Peer Dependencies

This package requires the following OpenTelemetry packages to be installed:

```bash
npm install @opentelemetry/api @opentelemetry/core @opentelemetry/otlp-transformer @opentelemetry/sdk-trace-base
```

Or install everything in one command:

```bash
npm install @dev7a/otlp-stdout-span-exporter @opentelemetry/api @opentelemetry/core @opentelemetry/otlp-transformer @opentelemetry/sdk-trace-base
```

>[!NOTE]
>This package requires OpenTelemetry SDK 2.x (with API 1.3.0+). If you're using OpenTelemetry 1.x, please use version 0.15.0 of this package.

## Usage

The exporter works with CommonJS:

```javascript
const { OTLPStdoutSpanExporter } = require('@dev7a/otlp-stdout-span-exporter');
```

>[!NOTE]
>ESM support has been temporarily removed due to bundler compatibility issues. This package currently only supports CommonJS imports.

### ESM Support

This package provides experimental ESM support via a subpath export. Due to bundler compatibility issues, ESM is not available via the main export.

>[!NOTE]
>ESM support via the `/esm` subpath was broken in v0.17.2 but has been fixed in v0.17.3.

To use ESM in native Node.js environments:

```javascript
// Use the /esm subpath
import { OTLPStdoutSpanExporter } from '@dev7a/otlp-stdout-span-exporter/esm';
```

>[!WARNING]
>The ESM export is not compatible with webpack bundling. If you're using webpack, please use the CommonJS syntax instead.

### Webpack Configuration

If you're using webpack and encountering module resolution issues, add this package to your externals:

```javascript
module.exports = {
  // ... your config
  externals: [
    '@dev7a/otlp-stdout-span-exporter',
    // ... other externals
  ],
};
```

This ensures webpack doesn't try to bundle the package and uses Node.js's native module resolution at runtime.

The recommended way to use this exporter is with the standard OpenTelemetry `BatchSpanProcessor`, which provides better performance by buffering and exporting spans in batches, or, in conjunction with the [lambda-otel-lite](https://www.npmjs.com/package/@dev7a/lambda-otel-lite) package, with the `LambdaSpanProcessor`, which is particularly optimized for AWS Lambda.

You can create a simple tracer provider with the BatchSpanProcessor and the OTLPStdoutSpanExporter:

```typescript
import { trace } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { BatchSpanProcessor } from '@opentelemetry/sdk-trace-base';
import { OTLPStdoutSpanExporter, LogLevel, OutputType } from '@dev7a/otlp-stdout-span-exporter';

// Initialize the exporter with default options (stdout output)
const exporter = new OTLPStdoutSpanExporter({ gzipLevel: 6 });

// Or with log level for filtering
const debugExporter = new OTLPStdoutSpanExporter({ 
  gzipLevel: 6,
  logLevel: LogLevel.Debug
});

// Or with named pipe output
const pipeExporter = new OTLPStdoutSpanExporter({
  outputType: OutputType.Pipe  // Will write to /tmp/otlp-stdout-span-exporter.pipe
});

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
  // Defaults to 6 if not specified
  gzipLevel?: number;
  
  // Log level for filtering in log aggregation systems
  // If not specified, no level field will be included in the output
  logLevel?: LogLevel;
  
  // Output type (stdout or pipe)
  // Defaults to OutputType.Stdout if not specified
  outputType?: OutputType;
}

// Available log levels
enum LogLevel {
  Debug = "DEBUG",
  Info = "INFO",
  Warn = "WARN",
  Error = "ERROR"
}

// Available output types
enum OutputType {
  Stdout = "stdout",
  Pipe = "pipe"
}
```

The exporter follows a strict configuration precedence:
1. Environment variables (highest precedence)
2. Constructor parameters in config object
3. Default values (lowest precedence)

This means that if any of the environment variables are set, they will always take precedence over the configuration in the constructor.

### Environment Variables

The exporter respects the following environment variables:

- `OTEL_SERVICE_NAME`: Service name to use in output
- `AWS_LAMBDA_FUNCTION_NAME`: Fallback service name (if `OTEL_SERVICE_NAME` not set)
- `OTEL_EXPORTER_OTLP_HEADERS`: Headers for OTLP export, used in the `headers` field
- `OTEL_EXPORTER_OTLP_TRACES_HEADERS`: Trace-specific headers (which take precedence if conflicting with `OTEL_EXPORTER_OTLP_HEADERS`)
- `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: GZIP compression level (0-9). Defaults to 6. Takes precedence over the constructor parameter if set.
- `OTLP_STDOUT_SPAN_EXPORTER_LOG_LEVEL`: Log level for filtering (debug, info, warn, error). If set, adds a `level` field to the output.
- `OTLP_STDOUT_SPAN_EXPORTER_OUTPUT_TYPE`: Output type ("pipe" or "stdout"). Defaults to "stdout". If set to "pipe", writes to `/tmp/otlp-stdout-span-exporter.pipe`.

>[!NOTE]
>For security best practices, avoid including authentication credentials or sensitive information in headers. The serverless-otlp-forwarder infrastructure is designed to handle authentication at the destination, rather than embedding credentials in your telemetry data.

## Output Format

The exporter writes JSON objects to stdout with the following structure:

```json
{
  "__otel_otlp_stdout": "0.16.0",
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
  "payload": "<base64-encoded-gzipped-protobuf>",
  "level": "INFO"
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
- `level` is the log level (only present if configured via constructor or environment variable).

## Named Pipe Output

When configured to use named pipe output (either via constructor or environment variable), the exporter will write to `/tmp/otlp-stdout-span-exporter.pipe` instead of stdout. This can be useful in environments where you want to process the telemetry data with a separate process.

If the pipe doesn't exist or can't be written to, the exporter will automatically fall back to stdout with a warning.

## License

MIT

## See Also

- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder) - The main project repository for the Serverless OTLP Forwarder project
- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/python/otlp-stdout-span-exporter) | [PyPI](https://pypi.org/project/otlp-stdout-span-exporter/) - The Python version of this exporter
- [GitHub](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/rust/otlp-stdout-span-exporter) | [crates.io](https://crates.io/crates/otlp-stdout-span-exporter) - The Rust version of this exporter