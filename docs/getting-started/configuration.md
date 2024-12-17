---
layout: default
title: Configuration
parent: Getting Started
nav_order: 2
---

# Configuration Guide
{: .fs-9 }

Configure Serverless OTLP Forwarder for your observability needs.
{: .fs-6 .fw-300 }

## Overview
{: .text-delta }

The Serverless OTLP Forwarder can be configured through:
- AWS SAM CLI parameters during deployment
- AWS CloudFormation template parameters
- Persisted configuration in `samconfig.toml`

{: .note }
> While you can use `sam deploy --guided` for interactive configuration, we recommend using `samconfig.toml` for reproducible deployments.

## Configuration File
{: .text-delta }

The `samconfig.toml` file allows you to persist your configuration:

```toml
version = 0.1
[default.deploy.parameters]
stack_name = "serverless-otlp-forwarder"
resolve_s3 = true
s3_prefix = "serverless-otlp-forwarder"
region = "us-west-2"
confirm_changeset = true
capabilities = "CAPABILITY_IAM"
parameter_overrides = [
    "ProcessorType=otlp-stdout",
    "CollectorsSecretsKeyPrefix=serverless-otlp-forwarder/keys",
    "CollectorsCacheTtlSeconds=300",
    "RouteAllLogs=true",
    "DeployDemo=false"
]
```

{: .important }
> The configuration file can be created automatically using `sam deploy --guided` and then modified as needed.

## Parameter Reference
{: .text-delta }

### Core Parameters

| Parameter | Type | Default | Description |
|:----------|:-----|:--------|:------------|
| `ProcessorType` | String | `otlp-stdout` | Processor type to use (`otlp-stdout` or `aws-spans`) |
| `CollectorsSecretsKeyPrefix` | String | `serverless-otlp-forwarder/keys` | Prefix for AWS Secrets Manager keys |
| `CollectorsCacheTtlSeconds` | String | `300` | TTL for the collector cache in seconds |
| `RouteAllLogs` | String | `true` | Route all AWS logs to the Lambda function |

### Optional Features

| Parameter | Type | Default | Description |
|:----------|:-----|:--------|:------------|
| `DeployDemo` | String | `true` | Deploy the demo application |
| `DemoExporterProtocol` | String | `http/protobuf` | Protocol for demo exporter |
| `DemoExporterCompression` | String | `gzip` | Compression for demo exporter |
| `DeployBenchmark` | String | `false` | Deploy the benchmark stack |

{: .note }
> The Lambda function uses arm64 architecture and is configured with 128MB memory by default. These settings are optimized for cost and performance.

## Application Configuration
{: .text-delta }

To use the Serverless OTLP Forwarder with your applications, you need to configure two components:

1. **OpenTelemetry SDK Integration**: Integrate the OpenTelemetry SDK for your programming language and configure it to write telemetry data to stdout. 
To do so, you should use our language-specific packages and follow the corresponding guide to instrument your application:

- <i class="devicon-rust-plain colored"></i> [Rust Development Guide](../languages/rust)
- <i class="devicon-python-plain colored"></i> [Python Development Guide](../languages/python)
- <i class="devicon-nodejs-plain colored"></i> [Node.js Development Guide](../languages/nodejs)

2. **Environment Variables**: Configure your Lambda function with the following environment variables:

```yaml
Environment:
  Variables:
    OTEL_EXPORTER_OTLP_PROTOCOL: http/protobuf
    OTEL_EXPORTER_OTLP_COMPRESSION: gzip
    OTEL_SERVICE_NAME: my-service-name
```

{: .note }
> The `OTEL_EXPORTER_OTLP_ENDPOINT` configuration is not required in your instrumented Lambda functions. The forwarder will automatically determine the endpoint based on its own configuration.

### Environment Variables Reference

Most of the OTEL environment variables are supported as usual in the OpenTelemetry SDK, but to the very least you should explicitly set the following:

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | Protocol for OTLP data (`http/protobuf` or `http/json`) | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | Compression type (`gzip` or `none`) | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service for telemetry identification | - |

{: .warning }
> Do not set `OTEL_EXPORTER_OTLP_HEADERS` in your instrumented Lambda functions. Authentication headers should be configured in the forwarder itself as secrets on AWS Secrets Manager.

## Collector Configuration
{: .text-delta }

The forwarder uses AWS Secrets Manager to store collector endpoints and authentication details. By default, it looks for secrets under the prefix specified by `CollectorsSecretsKeyPrefix` (default: `serverless-otlp-forwarder/keys`).

### Secret Structure

Each collector configuration requires a secret with the following structure:

```json
{
  "name": "my-collector",
  "endpoint": "https://collector.example.com",
  "auth": "x-api-key=your-api-key",
  "exclude": "^/aws/lambda/excluded-function-.*$"
}
```

Configuration fields:
- `name`: A friendly name for the collector (e.g., `selfhosted`, `honeycomb`, `datadog`)
- `endpoint`: The OTLP collector endpoint URL
- `auth`: Authentication header or method (`x-api-key=your-key`, `sigv4`, `iam`, `none`)
- `exclude`: Optional regex pattern to exclude specific log groups from being forwarded to this collector

{: .note }
The `exclude` parameter allows you to filter out specific log groups from being forwarded to a particular collector. This is useful when you want to send different telemetry data to different collectors. The pattern is matched against the full log group name.

Example using AWS CLI with exclusion pattern:
```bash
aws secretsmanager create-secret \
  --name "serverless-otlp-forwarder/keys/default" \
  --secret-string '{
    "name": "honeycomb",
    "endpoint": "https://api.honeycomb.io/v1/traces",
    "auth": "x-api-key=your-honeycomb-key",
    "exclude": "^/aws/lambda/internal-.*$"
  }'
```

### Multiple Collectors

The forwarder can send telemetry data to multiple collectors simultaneously. To configure multiple collectors:

1. Create separate secrets under the same prefix:
```bash
# Additional collector
aws secretsmanager create-secret \
  --name "serverless-otlp-forwarder/keys/appsignals" \
  --secret-string '{
    "name": "appsignals",
    "endpoint": "https://xray.us-east-1.amazonaws.com",
    "auth": "sigv4"
  }'
```

2. The forwarder will:
   - Load all collector configurations under the specified prefix
   - Send telemetry data to all configured collectors in parallel
   - Cache configurations based on `CollectorsCacheTtlSeconds`

{: .note }
The forwarder itself is instrumented and sends its telemetry to the collector defined in the `default` secret (e.g., `serverless-otlp-forwarder/keys/default`).

### AWS Application Signals

To use AWS Application Signals OTLP endpoint as a destination:

1. Create a secret with:
   - `endpoint`: The Application Signals OTLPendpoint for your region (e.g., `https://xray.us-east-1.amazonaws.com`)
   - `auth`: Set to `sigv4` or `iam`

2. The forwarder will:
   - Automatically append `/v1/traces` to the endpoint
   - Use the Lambda function's IAM role for authentication
   - Require `xray:PutTraceSegments` and `xray:PutSpansForIndexing` permissions

Example configuration:
```bash
aws secretsmanager create-secret \
  --name "serverless-otlp-forwarder/keys/appsignals" \
  --secret-string '{
    "name": "appsignals",
    "endpoint": "https://xray.us-east-1.amazonaws.com",
    "auth": "sigv4"
  }'
```

{: .important }
To make Application Signals your default destination, create this configuration as `serverless-otlp-forwarder/keys/default`.

