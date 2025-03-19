# Rust OTLP Stdout Span Exporter

A Rust span exporter that writes OpenTelemetry spans to stdout, using a custom serialization format that embeds the spans serialized as OTLP protobuf in the `payload` field. 
The message envelope carries some metadata about the spans, such as the service name, the OTLP endpoint, and the HTTP method:

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
Outputting the telemetry data in this format directly to stdout makes the library easily usable in network constrained environments, or in enviroments that are particularly sensitive to the overhead of HTTP connections, such as AWS Lambda.

Part of the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) project, that implements a forwarder for OTLP telemetry data from serverless environments to OTLP compliant collectors.

## Features

- Uses OTLP Protobuf serialization for efficient encoding
- Applies GZIP compression with configurable levels
- Detects service name from environment variables or AWS Lambda function name
- Supports custom headers via standard OTEL environment variables
- Consistent JSON output format
- Zero external HTTP dependencies
- Lightweight and fast

## Installation

Run `cargo add otlp-stdout-span-exporter` to add the crate to your project.

## Usage

The recommended way to use this exporter is with the standard OpenTelemetry `BatchSpanProcessor`, which provides better performance by buffering and exporting spans in batches, or, in conjunction with the [lambda-otel-lite](https://crates.io/crates/lambda-otel-lite) crate, with the `LambdaSpanProcessor` strategy, which is particularly optimized for AWS Lambda.

You can create a simple tracer provider with the default configuration:

```rust
use opentelemetry::global;
use opentelemetry::trace::Tracer;
use opentelemetry_sdk::trace::SdkTracerProvider;
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;

fn init_tracer() -> SdkTracerProvider {
    let exporter = OtlpStdoutSpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());
    provider
}

#[tokio::main]
async fn main() {
    let provider = init_tracer();
    let tracer = global::tracer("example/simple");
    tracer.in_span("parent-operation", |_cx| {
        println!("Doing work...");
        
        // Create nested spans
        tracer.in_span("child-operation", |_cx| {
            println!("Doing more work...");
        });
    });

    if let Err(err) = provider.force_flush() {
        println!("Error flushing provider: {:?}", err);
    }
}
```

This setup ensures that:
- Spans are batched together for efficient export
- Parent-child relationships are preserved
- System resources are used efficiently
- Spans are properly flushed on shutdown

## Environment Variables

The exporter respects the following environment variables:

- `OTEL_SERVICE_NAME`: Service name to use in output, used in the `source` field
- `AWS_LAMBDA_FUNCTION_NAME`: Fallback service name (if `OTEL_SERVICE_NAME` not set)
- `OTEL_EXPORTER_OTLP_HEADERS`: Headers for OTLP export, used in the `headers` field
- `OTEL_EXPORTER_OTLP_TRACES_HEADERS`: Trace-specific headers (takes precedence if conflicting with `OTEL_EXPORTER_OTLP_HEADERS`)
- `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: GZIP compression level (0-9, default: 6)

## Configuration

The exporter can be configured in multiple ways, with a strict precedence order:

1. Environment variables (highest precedence)
2. Builder methods (medium precedence)
3. Default values (lowest precedence)

### Using Environment Variables

Environment variables always take precedence over any programmatic configuration:

```bash
# Set GZIP compression level to 9 (maximum compression)
export OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL=9
```

### Using default or builder methods

The exporter provides two main ways to create and configure it:

```rust
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;

// Create with default options (compression level 6)
let default_exporter = OtlpStdoutSpanExporter::default();

// Create with specific compression level
let max_compression_exporter = OtlpStdoutSpanExporter::builder().compression_level(9).build();
```

Note that even when using these constructor parameters, environment variables will still take precedence if they are set.

## Default Values

When neither environment variables nor constructor parameters are provided, the following defaults are used:

- Compression level: 6 (good balance between speed and compression)
- Service name: "unknown-service" (unless AWS_LAMBDA_FUNCTION_NAME is available)
- Endpoint: "http://localhost:4318/v1/traces"

## Development

1. Clone the repository:
```bash
git clone https://github.com/dev7a/serverless-otlp-forwarder
cd serverless-otlp-forwarder/packages/rust/otlp-stdout-span-exporter
```

2. Run tests:
```bash
cargo test
```

3. Run the example:
```bash
cargo run --example hello
```

## License

Apache License 2.0

## See Also

- [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) - The main project repository
- [Python OTLP Stdout Span Exporter](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/python/otlp-stdout-span-exporter) - The Python version of this exporter
- [TypeScript OTLP Stdout Span Exporter](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/node/otlp-stdout-span-exporter) - The TypeScript version of this exporter 