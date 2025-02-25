# otlp-stdout-span-exporter

A Rust span exporter that writes OpenTelemetry spans to stdout in OTLP format. Part of the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) project.

This exporter is particularly useful in serverless environments like AWS Lambda where writing to stdout is a common pattern for exporting telemetry data.

## Features

- Uses OTLP Protobuf serialization for efficient encoding
- Applies GZIP compression with configurable levels
- Detects service name from environment variables
- Supports custom headers via environment variables
- Consistent JSON output format
- Zero external HTTP dependencies
- Lightweight and fast

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
otlp-stdout-span-exporter = "0.1.0"
```

## Usage

The recommended way to use this exporter is with batch export, which provides better performance by buffering and exporting spans in batches:

```rust
use opentelemetry::{trace::{Tracer, TracerProvider}, KeyValue};
use opentelemetry_sdk::{trace::TracerProvider as SdkTracerProvider, Resource, runtime};
use otlp_stdout_span_exporter::OtlpStdoutSpanExporter;

#[tokio::main]
async fn main() {
    // Create a new stdout exporter
    let exporter = OtlpStdoutSpanExporter::new();

    // Create a new tracer provider with batch export
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "my-service")]))
        .build();

    // Create a tracer
    let tracer = provider.tracer("my-service");

    // Create spans
    tracer.in_span("parent-operation", |_cx| {
        println!("Doing work...");
        
        // Create nested spans
        tracer.in_span("child-operation", |_cx| {
            println!("Doing more work...");
        });
    });
    
    // Shut down the provider
    let _ = provider.shutdown();
}
```

This setup ensures that:
- Spans are batched together for efficient export
- Parent-child relationships are preserved
- System resources are used efficiently
- Spans are properly flushed on shutdown

## Environment Variables

The exporter respects the following environment variables:

- `OTEL_SERVICE_NAME`: Service name to use in output
- `AWS_LAMBDA_FUNCTION_NAME`: Fallback service name (if `OTEL_SERVICE_NAME` not set)
- `OTEL_EXPORTER_OTLP_HEADERS`: Global headers for OTLP export
- `OTEL_EXPORTER_OTLP_TRACES_HEADERS`: Trace-specific headers (takes precedence)
- `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL`: GZIP compression level (0-9, default: 6)

Header format examples:
```bash
# Single header
export OTEL_EXPORTER_OTLP_HEADERS="api-key=secret123"

# Multiple headers
export OTEL_EXPORTER_OTLP_HEADERS="api-key=secret123,custom-header=value"

# Headers with special characters
export OTEL_EXPORTER_OTLP_HEADERS="authorization=Basic dXNlcjpwYXNzd29yZA=="

# Set compression level (0 = no compression, 9 = maximum compression)
export OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL="9"
```

## Output Format

The exporter writes each batch of spans as a JSON object to stdout:

```json
{
  "__otel_otlp_stdout": "0.1.0",
  "source": "my-service",
  "endpoint": "http://localhost:4318/v1/traces",
  "method": "POST",
  "content-type": "application/x-protobuf",
  "content-encoding": "gzip",
  "headers": {
    "api-key": "secret123",
    "custom-header": "value"
  },
  "payload": "<base64-encoded-gzipped-protobuf>",
  "base64": true
}
```

## Configuration

The exporter can be configured with different GZIP compression levels:

```rust
// Create exporter with custom GZIP level (0-9)
let exporter = OtlpStdoutSpanExporter::with_gzip_level(9);
```

You can also configure the compression level using the `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL` environment variable. The explicit configuration via code will override any environment variable setting.

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
- [Python Span Exporter](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/python/otlp-stdout-span-exporter) - The Python version of this exporter
- [TypeScript Span Exporter](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/packages/node/otlp-stdout-span-exporter) - The TypeScript version of this exporter 