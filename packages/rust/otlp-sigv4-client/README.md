# otlp-sigv4-client

A SigV4-compatible HTTP client wrapper for OpenTelemetry OTLP exporters, enabling AWS authentication for sending telemetry data to the [Cloudwatch OLTP endpoint](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/CloudWatch-OTLPEndpoint.html)

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
otlp-sigv4-client = "0.1.0"
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

The crate supports both reqwest and hyper HTTP clients through feature flags:
- `reqwest` (default): Use reqwest HTTP client
- `hyper`: Use hyper HTTP client

## Examples

Check out the [examples](examples/) directory for more detailed examples:
- [SigV4 Authentication](examples/sigv4_auth/): Example showing AWS SigV4 authentication configuration and usage
- More examples coming soon...

## AWS Service Compatibility

This client is compatible with AWS services that accept OTLP data and require SigV4 authentication:
- AWS X-Ray
- Amazon Managed Service for Prometheus
- Other AWS services that accept OTLP format

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

This crate is part of the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder) project. 