# otlp-stdout-client

The `otlp-stdout-client` library is designed to export OpenTelemetry data to stdout in a formatted JSON structure, suitable for serverless environments like AWS Lambda. It implements the `opentelemetry_http::HttpClient` interface and can be used in an OpenTelemetry OTLP pipeline to send OTLP data (both JSON and Protobuf formats) to stdout.

By outputting telemetry data to stdout, this library enables seamless integration with log management systems in serverless environments. For instance, in AWS Lambda, CloudWatch Logs can capture this output, allowing tools like the [lambda-otlp-forwarder](https://github.com/dev7a/lambda-otlp-forwarder) to efficiently collect and forward the data to centralized OpenTelemetry collectors. This approach facilitates a robust observability pipeline in serverless architectures without necessitating direct network access to collectors.

>[!NOTE] This library is experimental, not part of the OpenTelemetry project, and subject to change.

## Features

- Implements `opentelemetry_http::HttpClient` for use in OTLP pipelines
- Exports OpenTelemetry data to stdout in a structured format
- Supports both HTTP/JSON and HTTP/Protobuf OTLP records
- Designed for serverless environments, especially AWS Lambda
- Configurable through environment variables
- Optional GZIP compression of payloads

## Usage

Add this to your `Cargo.toml` or run `cargo add otlp-stdout-client`:

```toml
[dependencies]
otlp-stdout-client = "0.2.1"
```

## Example

```rust
use otlp_stdout_client::StdoutClient;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry::trace::Tracer;
use opentelemetry::global;

fn init_tracer_provider() -> Result<TracerProvider, Box<dyn std::error::Error>> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_http_client(StdoutClient::default())
        .build()?;
    
    let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();

    Ok(tracer_provider)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tracer_provider = init_tracer_provider()?;
    global::set_tracer_provider(tracer_provider);
    
    let tracer = global::tracer("my_tracer");
    
    // Use the tracer for instrumenting your code
    tracer.in_span("example_span", |_cx| {
        // Your code here
    });

    Ok(())
}
```

## Record Structure

The `otlp-stdout-client` writes OTLP data to stdout in a JSON format. Each log record includes:

- `__otel_otlp_stdout`: A version identifier for the otlp-stdout-client
- `source`: The service name or identifier for the log record
- `content-type`: The content type of the payload (JSON or Protobuf)
- `endpoint`: The configured OTLP endpoint
- `method`: The HTTP method
- `payload`: The OTLP data (may be base64 encoded if compressed or binary)
- `headers`: A map of http headers sent with the request
- `content-encoding`: The compression method (if used, only gzip is supported)
- `base64`: A boolean indicating if the payload is base64 encoded

## Configuration

The exporter can be configured using standard OpenTelemetry environment variables:

- `OTEL_EXPORTER_OTLP_PROTOCOL`: Specifies the protocol (only http/protobuf or http/json are supported)
- `OTEL_EXPORTER_OTLP_ENDPOINT`: Sets the endpoint for the OTLP collector, i.e `http://collector.example.com:4318`
- `OTEL_EXPORTER_OTLP_HEADERS`: Sets additional headers for the OTLP exporter, i.e `x-api-key=<key>`
- `OTEL_EXPORTER_OTLP_COMPRESSION`: Specifies the compression algorithm (only gzip is supported)

For detailed information on these variables and their effects, please refer to the OpenTelemetry documentation.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
