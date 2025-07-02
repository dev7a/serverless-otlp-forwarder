# Serverless TODO Application with OpenTelemetry

This project demonstrates a serverless TODO application built with AWS SAM, using the lambda-otel-lite library for OpenTelemetry instrumentation. The application showcases distributed tracing across multiple Lambda functions written in different languages (Node.js, Python, and Rust).

## Architecture

The application consists of the following components:

1. **TODO Fetcher (Node.js)**: A Lambda function that periodically fetches random TODOs and sends them to an SQS queue.
2. **TODO Processor (Python)**: A Lambda function that processes TODOs from the SQS queue and forwards them to the backend API.
3. **TODO Storage API (Rust)**: A Lambda function that handles CRUD operations for TODOs in DynamoDB.
4. **TODO UI (Rust)**: A Lambda function that serves a web UI for viewing and interacting with TODOs.
5. **Lambda@Edge Functions (Python)**: Functions that handle CloudFront request and response processing.

## Infrastructure

The application uses the following AWS services:

- **AWS Lambda**: For serverless compute
- **Amazon API Gateway**: For the REST API
- **Amazon DynamoDB**: For TODO storage
- **Amazon SQS**: For message queuing
- **Amazon CloudFront**: For content delivery
- **AWS CloudWatch**: For logging and monitoring

## OpenTelemetry Instrumentation

Each component is instrumented with OpenTelemetry using the lambda-otel-lite library:

- **Node.js**: Uses `@dev7a/lambda-otel-lite` with AWS SDK and HTTP instrumentations
- **Python**: Uses `lambda_otel_lite` with requests instrumentation
- **Rust**: Uses `lambda_otel_lite` with tracing integration

## Deployment

The application is deployed using AWS SAM (Serverless Application Model). The `template.yaml` file defines all the resources needed for the application.

### Prerequisites

- AWS CLI
- AWS SAM CLI
- Node.js 22.x
- Python 3.13
- Rust (with cargo-lambda)

### Deployment Steps

1. Build the application:

```bash
sam build
```

2. Deploy the application:

```bash
sam deploy --guided
```

## Local Development

### Node.js Component

```bash
cd node
npm install
npm run build
```

### Python Component

```bash
cd python/processor
pip install -r requirements.txt
```

### Rust Component

```bash
cd rust
cargo build
```

## Features

- Create, read, update, and delete TODOs
- Filter TODOs by completion status
- Categorize TODOs
- Assign priority levels to TODOs
- Track creation and completion dates
- Responsive web UI

## Observability

The application generates the following telemetry data:

- **Traces**: Distributed traces across all components
- **Events**: Span events for important operations

## License

This project is licensed under the MIT License - see the LICENSE file for details.
