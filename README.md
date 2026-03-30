# Serverless OTLP Forwarder

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![OpenTelemetry](https://img.shields.io/badge/OpenTelemetry-enabled-blue.svg)](https://opentelemetry.io)
![AWS Lambda](https://img.shields.io/badge/AWS-Lambda-orange?logo=amazon-aws)
[![Rust](https://img.shields.io/badge/Rust-1.84%2B-orange.svg)](https://www.rust-lang.org)
[![Python](https://img.shields.io/badge/Python-3.13%2B-blue.svg)](https://www.python.org)
[![Node.js](https://img.shields.io/badge/Node.js-22.x-green.svg)](https://nodejs.org)
![Stability: Experimental](https://img.shields.io/badge/stability-Experimental-important.svg)

**A different path for serverless observability.**

---

The Serverless OTLP Forwarder offers a novel approach to collecting OpenTelemetry data from serverless applications, particularly AWS Lambda functions. It's designed to minimize performance overhead and integrate seamlessly with the serverless paradigm by leveraging existing AWS infrastructure like CloudWatch Logs.

![Serverless OTLP Forwarder Architecture](https://github.com/user-attachments/assets/aa9c2b02-5e66-4829-af08-8ceb509472ff)
*A serverless-native approach to observability*

> This project is under active development. Your feedback and contributions are welcome.

## The Challenge of Serverless Observability

Traditional OpenTelemetry collection methods, often involving sidecar agents or direct synchronous exports, can introduce significant latency and resource overhead in ephemeral, short-lived Lambda environments. This can negate some of the core benefits of serverless, such as rapid scaling and pay-per-use cost models.

The Serverless OTLP Forwarder addresses this by:
-   **Writing telemetry to `stdout`**: Lambda functions use lightweight SDKs to serialize OTLP data (protobuf, gzipped, base64-encoded) directly to standard output.
-   **Leveraging CloudWatch Logs**: CloudWatch Logs acts as a durable and scalable transport layer, capturing this `stdout` data.
-   **Asynchronous Processing**: A central forwarder Lambda function subscribes to these logs, processes the OTLP data, and forwards it to your configured OpenTelemetry collector(s) asynchronously, outside the execution path of your primary application Lambdas.

This approach minimizes the performance impact on your application functions, avoids the need for direct network connections from your Lambdas to collectors, and works harmoniously with Lambda's execution model.

## Key Features & Components

The project consists of several key components designed to provide a comprehensive serverless observability solution:

*   **Core Forwarder (`otlp-stdout-logs-processor`)**:
    *   An AWS Lambda function that subscribes to CloudWatch Log Groups.
    *   Decodes, decompresses, and validates OTLP data from log events.
    *   Batches and forwards telemetry to one or more configured OTLP collectors.
    *   Highly configurable for aspects like target collectors, authentication, and log group filtering.
    *   [Learn more](./website/docs/forwarders/otlp-stdout-logs-processor.md) <!-- Placeholder -->

*   **Lightweight SDKs (`lambda-otel-lite`)**:
    *   Minimalist OpenTelemetry wrappers tailored for AWS Lambda.
    *   Available for **Node.js**, **Python**, and **Rust**.
    *   Provide helpers for easy instrumentation, context propagation, and automatic attribute extraction.
    *   Designed for low overhead and efficient integration with the `otlp-stdout-span-exporter`.
    *   [Node.js SDK](./website/docs/instrumentation/nodejs/lambda-otel-lite.md) <!-- Placeholder -->
    *   [Python SDK](./website/docs/instrumentation/python/lambda-otel-lite.md) <!-- Placeholder -->
    *   [Rust SDK](./website/docs/instrumentation/rust/lambda-otel-lite.md) <!-- Placeholder -->

*   **Stdout Span Exporters (`otlp-stdout-span-exporter`)**:
    *   Language-specific OpenTelemetry span exporters that serialize spans to `stdout` in the required JSON-wrapped, gzipped, base64-encoded OTLP protobuf format.
    *   Available for **Node.js**, **Python**, and **Rust**.
    *   Work in tandem with `lambda-otel-lite` or can be used with standard OpenTelemetry SDKs.
    *   [Node.js Exporter](./website/docs/instrumentation/nodejs/otlp-stdout-span-exporter.md) <!-- Placeholder -->
    *   [Python Exporter](./website/docs/instrumentation/python/otlp-stdout-span-exporter.md) <!-- Placeholder -->
    *   [Rust Exporter](./website/docs/instrumentation/rust/otlp-stdout-span-exporter.md) <!-- Placeholder -->

*   **CLI Tools**:
    *   **`startled`**: A powerful CLI for detailed performance analysis and benchmarking of AWS Lambda functions, especially useful for comparing telemetry solutions and understanding the impact of extensions.
        *   [Learn more](./website/docs/cli-tools/startled.md) <!-- Placeholder -->
    *   **`livetrace`**: A CLI tool for real-time tailing and visualization of traces from CloudWatch Logs in your terminal, designed for local development with the OTLP-stdout architecture.
        *   [Learn more](./website/docs/cli-tools/livetrace.md) <!-- Placeholder -->

*   **Experimental Forwarders**:
    *   **`otlp-stdout-kinesis-processor`**: An alternative forwarder that processes OTLP-stdout records from a Kinesis Data Stream instead of CloudWatch Logs.
        *   [Learn more](./website/docs/forwarders/otlp-stdout-kinesis-processor.md) <!-- Placeholder -->
    *   **`aws-span-processor`**: A forwarder designed to ingest spans from AWS Application Signals (via the `/aws/spans` log group) and convert/forward them to OTLP collectors.
        *   [Learn more](./website/docs/forwarders/aws-span-processor.md) <!-- Placeholder -->

*   **Utility Packages**:
    *   **`otlp-sigv4-client` (Rust)**: A SigV4-compatible HTTP client wrapper for OpenTelemetry OTLP exporters, enabling authenticated telemetry export to AWS services like X-Ray or the CloudWatch OTLP endpoint.
        *   [Learn more](./website/docs/instrumentation/rust/otlp-sigv4-client.md) <!-- Placeholder -->

## Demo Application

A multi-language demo application is included in the [`./demo`](./demo/) directory. It showcases a distributed microservices system (Node.js, Python, Rust) instrumented using the `lambda-otel-lite` libraries and this forwarding mechanism.
*   [Explore the Demo](./demo/README.md)

## Quick Start

Get the Serverless OTLP Forwarder up and running in your AWS account:

1.  **Prerequisites**: Ensure you have AWS CLI, AWS SAM CLI, and language-specific build tools (Rust/Cargo, Node/npm, Python/pip) installed.
2.  **Configure Collector Secret**: Store your OTLP collector endpoint and authentication details in AWS Secrets Manager.
    ```bash
    aws secretsmanager create-secret \
      --name "serverless-otlp-forwarder/keys/default" \
      --secret-string '{
        "name": "my-collector",
        "endpoint": "https://your-collector.example.com/v1/traces",
        "auth": "x-api-key=YOUR_API_KEY"
      }'
    ```
    (Replace placeholders with your actual collector details.)
3.  **Deploy the Forwarder**:
    ```bash
    git clone https://github.com/dev7a/serverless-otlp-forwarder.git
    cd serverless-otlp-forwarder
    sam build --parallel
    sam deploy --guided
    ```
4.  **Instrument Your Lambdas**: Use the `lambda-otel-lite` and `otlp-stdout-span-exporter` packages for your respective languages.


## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.
