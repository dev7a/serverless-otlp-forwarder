# Serverless OTLP Forwarder

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![OpenTelemetry](https://img.shields.io/badge/OpenTelemetry-enabled-blue.svg)](https://opentelemetry.io)
![AWS Lambda](https://img.shields.io/badge/AWS-Lambda-orange?logo=amazon-aws)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Python](https://img.shields.io/badge/Python-3.12%2B-blue.svg)](https://www.python.org)
[![Node.js](https://img.shields.io/badge/Node.js-18.x-green.svg)](https://nodejs.org)

![diagram](https://github.com/user-attachments/assets/aa9c2b02-5e66-4829-af08-8ceb509472ff)

The Serverless OTLP Forwarder enables serverless applications to send OpenTelemetry data to collectors without the overhead of direct connections or sidecars.

## Key Features

- 🚀 **Reduced Latency**: Minimal impact on Lambda execution and cold start times
- 🔒 **Enhanced Security**: Keeps telemetry data within AWS infrastructure
- 💰 **Cost Optimization**: Supports compression and efficient protocols
- 🔄 **Multiple Languages**: Support for Rust, Python, and Node.js
- 📊 **AWS Application Signals**: Experimental support for AWS Application Signals

## Quick Start

1. Install prerequisites:
   ```bash
   # Install AWS SAM CLI
   brew install aws-sam-cli  # or your preferred package manager

   # For Rust development (optional)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   cargo install cargo-lambda
   ```

2. Deploy the forwarder:
   ```bash
   git clone https://github.com/dev7a/serverless-otlp-forwarder
   cd serverless-otlp-forwarder
   sam build && sam deploy --guided
   ```

3. Instrument your application using our language-specific libraries:
   - [Rust Guide](https://dev7a.github.io/serverless-otlp-forwarder/languages/rust)
   - [Python Guide](https://dev7a.github.io/serverless-otlp-forwarder/languages/python)
   - [Node.js Guide](https://dev7a.github.io/serverless-otlp-forwarder/languages/nodejs)

## Documentation

Visit our [documentation site](https://dev7a.github.io/serverless-otlp-forwarder) for:
- [Getting Started Guide](https://dev7a.github.io/serverless-otlp-forwarder/getting-started)
- [Architecture Overview](https://dev7a.github.io/serverless-otlp-forwarder/concepts/architecture)
- [Configuration Guide](https://dev7a.github.io/serverless-otlp-forwarder/deployment)
- [Troubleshooting](https://dev7a.github.io/serverless-otlp-forwarder/troubleshooting)

## Contributing

- [Contributing Guidelines](CONTRIBUTING.md)
- [Security Policy](SECURITY.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
