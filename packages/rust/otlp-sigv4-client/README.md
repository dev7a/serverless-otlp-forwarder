# otlp-sigv4-client

A SigV4-compatible HTTP client wrapper for OpenTelemetry OTLP exporters, enabling AWS authentication for sending telemetry data to the [CloudWatch OTLP endpoint](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/CloudWatch-OTLPEndpoint.html)

[![Crates.io](https://img.shields.io/crates/v/otlp-sigv4-client.svg)](https://crates.io/crates/otlp-sigv4-client)
[![Documentation](https://docs.rs/otlp-sigv4-client/badge.svg)](https://docs.rs/otlp-sigv4-client)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- AWS SigV4 authentication for OpenTelemetry OTLP exporters
- Support for both reqwest and hyper HTTP clients
- Automatic AWS region detection from environment
- Configurable AWS service name
- Compatible with AWS credentials provider chain
- Implements OpenTelemetry's `HttpClient` trait

## Installation

Add this to your `Cargo.toml` or run `cargo add otlp-sigv4-client`:

```toml
[dependencies]
otlp-sigv4-client = "0.10.0"
```

## Usage

Here's a basic example using the reqwest HTTP client:

```rust
use aws_config;
use aws_credential_types::provider::ProvideCredentials;
use opentelemetry_otlp::{HttpExporterBuilder, WithHttpConfig};
use otlp_sigv4_client::SigV4ClientBuilder;
use reqwest::Client as ReqwestClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load AWS configuration from the environment
    let config = aws_config::load_from_env().await;
    let credentials = config
        .credentials_provider()
        .expect("No credentials provider found")
        .provide_credentials()
        .await?;

    // Create the SigV4 client
    let sigv4_client = SigV4ClientBuilder::new()
        .with_client(ReqwestClient::new())
        .with_credentials(credentials)
        .with_service("xray") // AWS X-Ray service name
        .build()?;

    // Configure and build the OTLP exporter
    let exporter = HttpExporterBuilder::default()
        .with_http_client(sigv4_client)
        .with_endpoint("https://xray.us-east-1.amazonaws.com")
        .build_span_exporter()?;

    // Use the exporter with your OpenTelemetry pipeline...
    Ok(())
}
```

## Configuration

### AWS Region

The region is determined in the following order:
1. Explicitly set via `with_region()`
2. Environment variable `AWS_REGION`
3. Default value "us-east-1"

### AWS Credentials

The client supports any valid AWS credentials source:
1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
2. AWS credentials file (`~/.aws/credentials`)
3. IAM roles (EC2 instance role, ECS task role, Lambda function role)
4. Other AWS credential sources (SSO, web identity, etc.)

### Required IAM Permissions

The client requires the following IAM permissions:
- `xray:PutTraceSegments`
- `xray:PutSpans`
- `xray:PutSpansForIndexing`

### HTTP Client

The crate works with any HTTP client that implements the `opentelemetry_http::HttpClient` trait. The client is generic, allowing you to wrap your preferred HTTP client implementation.

#### Feature Flags

The package includes the following feature flags:

- `reqwest` (default feature): Includes reqwest as a dependency for convenience. Disable this feature if you want to use a different HTTP client or if you already have reqwest in your dependencies.

To use without reqwest, disable default features in your Cargo.toml:

```toml
[dependencies]
otlp-sigv4-client = { version = "0.10.0", default-features = false }
```

An example implementation using the reqwest client is provided in the examples directory.

## Examples

Check out the [examples](examples/) directory for more detailed examples:
- [SigV4 Authentication](examples/sigv4_auth/): Example showing AWS SigV4 authentication configuration and usage
- More examples coming soon...

## Important Usage Note for OpenTelemetry SDK 0.28.0+

Starting with OpenTelemetry SDK 0.28.0, there's a known issue when using custom HTTP clients with the batch processor. According to the [changelog](https://github.com/open-telemetry/opentelemetry-rust/blob/opentelemetry_sdk-0.28.0/opentelemetry-sdk/CHANGELOG.md), the batch processor now uses a blocking client even when the `rt-tokio` feature is enabled, which can cause panics like:

```
thread 'OpenTelemetry.Traces.BatchProcessor' panicked at [...]:
there is no reactor running, must be called from the context of a Tokio 1.x runtime
```

### Recommended Solution

If you're experiencing this issue, create your HTTP client in a separate thread to isolate runtime initialization:

```rust
use aws_config;
use aws_credential_types::provider::ProvideCredentials;
use opentelemetry_otlp::{HttpExporterBuilder, WithHttpConfig};
use otlp_sigv4_client::SigV4ClientBuilder;
use reqwest::blocking::Client as BlockingReqwestClient;

// Load AWS configuration first (in the async context)
let config = aws_config::load_from_env().await;
let credentials = config
    .credentials_provider()
    .expect("No credentials provider found")
    .provide_credentials()
    .await?;

// Create the blocking SigV4 client in a separate thread to avoid runtime conflicts
let sigv4_client = std::thread::spawn(move || {
    let client = BlockingReqwestClient::new();
    SigV4ClientBuilder::new()
        .with_client(client)
        .with_credentials(credentials)
        .with_region("us-east-1")
        .with_service("xray")
        .build()
        .expect("Failed to build SigV4 client")
}).join().unwrap();

// Use the isolated client with your exporter
let exporter = HttpExporterBuilder::default()
    .with_http_client(sigv4_client)
    .with_endpoint("https://xray.us-east-1.amazonaws.com")
    .build_span_exporter()?;
```

This pattern works by isolating the client creation in a separate thread, which prevents runtime conflicts between the batch processor and the main application.

## AWS Service Compatibility

This client is designed primarily for use with AWS X-Ray OTLP endpoints. By default, it uses "xray" as the service name for AWS SigV4 authentication.

### Service Configuration

The default service name is "xray" if not specified. This can be set explicitly using the `with_service()` method:

```rust
// For AWS X-Ray (default)
.with_service("xray")
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

This crate is part of the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) project. 