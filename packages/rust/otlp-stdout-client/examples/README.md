# Examples

This directory contains examples demonstrating how to use the `otlp-stdout-client` library.

## Basic Example

The basic example shows how to set up a simple tracer that outputs spans to stdout. You can run it with different configurations using environment variables.

### Running the Example

```bash
# Run with default settings (JSON output)
cargo run --example basic

# Run with Protobuf output
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf cargo run --example basic

# Run with JSON output and GZIP compression
OTEL_EXPORTER_OTLP_PROTOCOL=http/json OTEL_EXPORTER_OTLP_COMPRESSION=gzip cargo run --example basic

# Run with Protobuf output and GZIP compression
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf OTEL_EXPORTER_OTLP_COMPRESSION=gzip cargo run --example basic
```

### Example Output

With default settings (JSON):
```json
{
  "__otel_otlp_stdout": "otlp-stdout-client@0.2.1",
  "source": "unknown-service",
  "endpoint": "http://localhost:4318/v1/traces",
  "method": "POST",
  "payload": { ... },
  "headers": {
    "content-type": "application/json"
  },
  "content-type": "application/json"
}
```

With Protobuf:
```json
{
  "__otel_otlp_stdout": "otlp-stdout-client@0.2.1",
  "source": "unknown-service",
  "endpoint": "http://localhost:4318/v1/traces",
  "method": "POST",
  "payload": "base64-encoded-protobuf-data",
  "headers": {
    "content-type": "application/x-protobuf"
  },
  "content-type": "application/x-protobuf",
  "base64": true
}
```

With GZIP compression:
```json
{
  "__otel_otlp_stdout": "otlp-stdout-client@0.2.1",
  "source": "unknown-service",
  "endpoint": "http://localhost:4318/v1/traces",
  "method": "POST",
  "payload": "base64-encoded-gzipped-data",
  "headers": {
    "content-type": "application/json",
    "content-encoding": "gzip"
  },
  "content-type": "application/json",
  "content-encoding": "gzip",
  "base64": true
}
```

### Environment Variables

The following environment variables can be used to configure the exporter:

- `OTEL_EXPORTER_OTLP_PROTOCOL`: Sets the protocol format
  - `http/json`: Uses JSON format (default)
  - `http/protobuf`: Uses Protobuf format

- `OTEL_EXPORTER_OTLP_COMPRESSION`: Enables compression
  - `gzip`: Enables GZIP compression
  - If not set or any other value: No compression

- `OTEL_SERVICE_NAME`: Sets the service name (defaults to "unknown-service") 