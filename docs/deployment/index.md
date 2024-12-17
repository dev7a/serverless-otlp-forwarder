---
layout: default
title: Deployment
nav_order: 5
has_children: true
---

# Deployment Guide
{: .fs-9 }

Deploy and configure Serverless OTLP Forwarder for your observability needs.
{: .fs-6 .fw-300 }

## Prerequisites
{: .text-delta }

{: .important }
Ensure that the IAM role you are using to deploy the application has permissions for the following services:
- CloudFormation
- Lambda
- Logs
- IAM

If you plan to deploy the demo application, you'll also need these permissions for:
- API Gateway
- DynamoDB

## Deployment Steps
{: .text-delta }

1. Clone the repository:
```bash
git clone https://github.com/dev7a/serverless-otlp-forwarder
cd serverless-otlp-forwarder
```

2. Configure at least one collector endpoint configuration in AWS Secrets Manager:
```bash
aws secretsmanager create-secret \
  --name "serverless-otlp-forwarder/keys/default" \
  --secret-string '{
    "name": "my-collector",
    "endpoint": "https://collector.example.com",
    "auth": "x-api-key=your-api-key"
  }'
```

3. Build and deploy:
```bash
sam build --parallel && sam deploy --guided
```

{: .note }
> The `--guided` flag initiates an interactive deployment process that helps you configure deployment parameters and creates a `samconfig.toml` file for future deployments.

## Configuration Parameters
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

## Configuration File
{: .text-delta }

The `samconfig.toml` file persists your deployment configuration:

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

## Collector Configuration
{: .text-delta }

The forwarder uses AWS Secrets Manager to store collector endpoints and authentication details. Each collector configuration requires a secret with the following structure:

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
The `exclude` parameter allows you to filter out specific log groups from being forwarded to a particular collector. This is useful when you want to send different telemetry data to different collectors.

### Multiple Collectors

To configure multiple collectors, create separate secrets under the same prefix:

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

The forwarder will:
- Load all collector configurations under the specified prefix
- Send telemetry data to all configured collectors in parallel
- Cache configurations based on `CollectorsCacheTtlSeconds`

## Troubleshooting
{: .text-delta }

Common deployment issues:
- Missing or incorrect AWS credentials
  - Check `~/.aws/credentials`
  - Verify permissions with `aws sts get-caller-identity`
- SAM CLI version incompatibility
  - Update SAM CLI: `brew upgrade aws-sam-cli` or `pip install --upgrade aws-sam-cli`
  - Clear SAM cache: `sam cache purge`
- Build errors
  - Validate template: `sam validate --lint`
  - Check CloudFormation events
  - Review IAM permissions
