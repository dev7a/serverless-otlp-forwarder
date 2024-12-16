---
layout: default
title: Architecture
parent: Concepts
nav_order: 1
---

# Architecture Details
{: .fs-9 }

Technical implementation details of the Serverless OTLP Forwarder components.
{: .fs-6 .fw-300 }

## Component Implementation
{: .text-delta }

### Application Instrumentation
{: .text-delta }

The Serverless OTLP Forwarder implements language-specific packages that adapt the OpenTelemetry SDK for serverless environments. Instead of sending telemetry data directly to collectors over HTTP, these packages serialize the data to stdout in a structured format:

- **Node.js**: [`@aws-lambda/otlp-stdout-exporter`](../languages/nodejs)
- **Python**: [`stdout-adapter`](../languages/python) 
- **Rust**: [`otlp-stdout-client`](../languages/rust)

The packages integrate with the standard OpenTelemetry SDK interfaces while avoiding network connections during function execution. This approach reduces cold start overhead since the telemetry data is processed asynchronously by the forwarder after the function completes.

Key design principles:
- Lightweight initialization to minimize cold start impact
- Efficient serialization to stdout with compression support
- Simple integration with OpenTelemetry SDKs
- Zero network overhead during function execution

{: .note }
In serverless environments, there's an important tradeoff between instrumentation and cold start time. While the OpenTelemetry SDK components are required, additional instrumentation libraries should be loaded selectively based on your needs. This helps maintain a balance between observability and function startup time.

### CloudWatch Transport
{: .text-delta }

CloudWatch Logs serves as a durable message queue with several advantages:

- **Durability**: AWS-managed storage with configurable retention (can be as low as 1 day, to save on storage costs)
- **Scalability**: Automatic scaling with Lambda concurrency
- **Cost-efficiency**: Pay only for ingestion and storage, minimize costs with short retention periods
- **Security**: Integrated with IAM and KMS encryption

The forwarder can subscribe to all logs in an AWS account while efficiently filtering only relevant telemetry data:

```json
{ $.__otel_otlp_stdout = * }
```

{: .note }
By combining account-wide log subscription with precise filtering, the forwarder processes only the logs containing OTLP data, making it both comprehensive and efficient.

### Forwarder Implementation
{: .text-delta }

The forwarder Lambda function is optimized for performance:

- **Runtime**: Rust for minimal cold start and memory usage
- **Architecture**: arm64 for cost optimization
- **Memory**: Configurable based on workload (default: 128MB)
- **Concurrency**: Automatic scaling with CloudWatch Logs

Processing pipeline:
1. Log event validation and parsing
2. OTLP data extraction and decompression
3. Protocol transformation (if needed)
4. Collector authentication and forwarding

{: .note }
Batching and buffering should be configured in your instrumented applications using the OpenTelemetry SDK's BatchSpanProcessor. This reduces the number of log entries and collector requests, improving efficiency and reducing costs.

### Processing Options
{: .text-delta }

The forwarder supports two processor types:

#### OTLP Stdout Processor
- Forwards OTLP data directly to collectors
- Supports two authentication methods:
  - API key via custom headers (OTEL_EXPORTER_OTLP_HEADERS format)
  - AWS SigV4 for AWS Application Signals
- Authentication credentials stored securely in AWS Secrets Manager
- Respects compression configuration from the application

{: .warning }
> The AWS AppSignals Processor is experimental and should not be used in production environments.

#### AWS AppSignals Processor (Experimental)
- Parses trace data from the `aws/span` log group
- Converts AWS Application Signals format to OTLP JSON
- Provides compatibility with standard OTLP collectors
- May have limitations and known issues

### Performance Characteristics
{: .text-delta }

The system scales automatically based on:
- Lambda concurrency limits
- CloudWatch Logs subscription throughput
- Collector endpoint capacity
- Network bandwidth

## Security Implementation
{: .text-delta }

### Authentication Flow
{: .text-delta }

The forwarder supports two authentication mechanisms:

1. **Custom Headers Authentication**:
   - Headers are stored in AWS Secrets Manager
   - Retrieved and cached in memory for a configurable TTL
   - Follows OTEL_EXPORTER_OTLP_HEADERS format
   - Supports any collector-specific authentication scheme

2. **AWS SigV4 Authentication**:
   - Uses the forwarder's IAM role to sign requests
   - No additional credentials needed
   - Compatible with AWS Application Signals
   - Automatic credential rotation

{: .note }
Credential caching helps minimize Secrets Manager API calls while ensuring credentials are regularly refreshed for security.

### Access Control
{: .text-delta }

Required IAM permissions for the forwarder:
```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:BatchGetSecretValue",
        "secretsmanager:ListSecrets",
        "xray:PutTraceSegments",
        "xray:PutSpans",
        "xray:PutSpansForIndexing"
      ],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:GetSecretValue"
      ],
      "Resource": "arn:${Partition}:secretsmanager:${Region}:${AccountId}:secret:${CollectorsSecretsKeyPrefix}/*"
    }
  ]
}
```

{: .note }
The policy includes permissions for Secrets Manager operations (for collector credentials) and X-Ray API operations (required for sending telemetry to AWS Application Signals).

## Monitoring and Observability
{: .text-delta }

The forwarder provides observability through two channels:

1. **OpenTelemetry Traces**:
   - Rich span attributes for operational insights
   - Detailed processing information
   - Can be used to derive operational metrics

2. **Lambda CloudWatch Metrics**:
   - Standard Lambda execution metrics
   - Invocation counts
   - Error rates
   - Duration