# Serverless OTLP Forwarder

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![OpenTelemetry](https://img.shields.io/badge/OpenTelemetry-enabled-blue.svg)](https://opentelemetry.io)
![AWS Lambda](https://img.shields.io/badge/AWS-Lambda-orange?logo=amazon-aws)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Python](https://img.shields.io/badge/Python-3.12%2B-blue.svg)](https://www.python.org)
[![Node.js](https://img.shields.io/badge/Node.js-18.x-green.svg)](https://nodejs.org)
![Stability: Experimental](https://img.shields.io/badge/stability-Experimental-important.svg)

![diagram](https://github.com/user-attachments/assets/aa9c2b02-5e66-4829-af08-8ceb509472ff)

The Serverless OTLP Forwarder enables serverless applications to send OpenTelemetry data to collectors without the overhead of direct connections or sidecars.

## Key Features

- 🚀 **Minimal Performance Impact**: Optimized for Lambda execution and cold start times
- 🔒 **Secure by Design**: Uses CloudWatch Logs for data transport, no direct collector exposure
- 💰 **Cost Optimization**: Supports compression and efficient protocols
- 🔄 **Language Support**: Native implementations for Rust, Python, and Node.js
- 📊 **AWS Application Signals**: Experimental integration support

## Documentation

Visit the [documentation site](https://dev7a.github.io/serverless-otlp-forwarder) for:
- [Getting Started Guide](https://dev7a.github.io/serverless-otlp-forwarder/getting-started)
- [Configuration Guide](https://dev7a.github.io/serverless-otlp-forwarder/getting-started/configuration)
- [Architecture Overview](https://dev7a.github.io/serverless-otlp-forwarder/concepts/architecture)
- [Technical Concepts](https://dev7a.github.io/serverless-otlp-forwarder/concepts)

## Quick Start

1. Install prerequisites:
   ```bash
   # Install AWS SAM CLI
   brew install aws-sam-cli  # or your preferred package manager

   # Install rust and cargo lambda
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   cargo install cargo-lambda
   ```
2. Configure the default collector:
   ```bash
   # Create a configuration in AWS Secrets Manager
   aws secretsmanager create-secret \
      --name "serverless-otlp-forwarder/keys/default" \
      --secret-string '{
        "name": "my-collector",
        "endpoint": "https://collector.example.com",
        "auth": "x-api-key=your-api-key"
      }'
   ```
3. Deploy the forwarder:
   ```bash
   # Clone the repository
   git clone https://github.com/dev7a/serverless-otlp-forwarder && cd serverless-otlp-forwarder
   # Deploy
   sam build --parallel && sam deploy --guided
   ```

4. Instrument your application using our language-specific libraries:
   - [Rust Guide](https://dev7a.github.io/serverless-otlp-forwarder/languages/rust)
   - [Python Guide](https://dev7a.github.io/serverless-otlp-forwarder/languages/python)
   - [Node.js Guide](https://dev7a.github.io/serverless-otlp-forwarder/languages/nodejs)

## Background

This project addresses specific challenges in serverless observability, particularly the performance impact of traditional OpenTelemetry collection methods. The standard approach using OTEL/ADOT Lambda Layer extensions introduces significant overhead through sidecar agents, affecting both cold start times and runtime performance.

This becomes especially relevant in scenarios requiring memory-optimized Lambda functions, where the resource overhead of traditional collectors can offset the benefits of memory optimization. The forwarder approach provides an alternative that maintains telemetry capabilities while minimizing resource utilization.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
