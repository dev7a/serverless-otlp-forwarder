---
layout: default
title: Configuration
parent: Getting Started
nav_order: 2
---

# Configuration Guide

This guide covers configuring both the Lambda OTLP Forwarder and your instrumented applications.

## Environment Variables

All language implementations support these standard OpenTelemetry environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Collector endpoint URL | `http://localhost:4318` |

> [!TIP]
> For Lambda functions, you can set `OTEL_EXPORTER_OTLP_ENDPOINT` to any value (e.g., `http://localhost:4318`) as the actual endpoint is determined by the forwarder.

## Collector Configuration

The forwarder requires a secret in AWS Secrets Manager to define collector endpoints and authentication:

```bash
aws secretsmanager create-secret \
  --name "lambda-otlp-forwarder/keys/default" \
  --secret-string '{
    "name": "my-collector",
    "endpoint": "https://collector.example.com",
    "auth": "x-api-key=your-api-key"
  }'
```

### Secret Fields

| Field | Description | Example |
|-------|-------------|---------|
| `name` | Collector identifier | `honeycomb`, `datadog` |
| `endpoint` | Collector endpoint URL | `https://api.honeycomb.io/v1/traces` |
| `auth` | Authentication header | `x-api-key=key`, `sigv4`, `iam` |

### Multiple Collectors

You can configure multiple collectors by creating additional secrets:

```bash
# Honeycomb configuration
aws secretsmanager create-secret \
  --name "lambda-otlp-forwarder/keys/honeycomb" \
  --secret-string '{
    "name": "honeycomb",
    "endpoint": "https://api.honeycomb.io/v1/traces",
    "auth": "x-honeycomb-team=your-key"
  }'

# AWS Application Signals configuration
aws secretsmanager create-secret \
  --name "lambda-otlp-forwarder/keys/appsignals" \
  --secret-string '{
    "name": "appsignals",
    "endpoint": "https://xray.us-east-1.amazonaws.com",
    "auth": "sigv4"
  }'
```

## SAM Template Configuration

Configure your Lambda functions in the SAM template:

```yaml
Resources:
  MyFunction:
    Type: AWS::Serverless::Function
    Properties:
      FunctionName: !Sub '${AWS::StackName}-example'
      Runtime: python3.12
      Environment:
        Variables:
          OTEL_EXPORTER_OTLP_PROTOCOL: http/protobuf
          OTEL_EXPORTER_OTLP_COMPRESSION: gzip
          OTEL_SERVICE_NAME: !Sub '${AWS::StackName}-example'
```

## Forwarder Configuration

The forwarder's behavior is controlled through SAM parameters:

### Core Parameters

- `ProcessorType` (Default: "otlp-stdout")
  - Options: `otlp-stdout` or `aws-appsignals`
  - Controls which processor is active

- `RouteAllLogs` (Default: "true")
  - Controls automatic log routing
  - Only applies to OTLP stdout processor

### Optional Features

- `DeployDemo` (Default: "true")
  - Deploys example applications
  - Useful for testing

- `CollectorsSecretsKeyPrefix` (Default: "lambda-otlp-forwarder/keys")
  - Prefix for collector configuration secrets

### Example Configurations

1. Standard setup:
   ```bash
   sam deploy --parameter-overrides ProcessorType=otlp-stdout RouteAllLogs=true
   ```

2. Application Signals:
   ```bash
   sam deploy --parameter-overrides ProcessorType=aws-appsignals
   ```

3. Development setup:
   ```bash
   sam deploy --parameter-overrides \
     ProcessorType=otlp-stdout \
     DeployDemo=true \
     DemoExporterProtocol=http/json
   ```

## Best Practices

1. **Protocol Selection**
   - Use `http/protobuf` for better performance
   - Enable GZIP compression to reduce costs

2. **Security**
   - Store collector credentials in Secrets Manager
   - Use IAM roles appropriately
   - Enable encryption in transit

3. **Cost Optimization**
   - Configure appropriate log retention
   - Monitor CloudWatch Logs usage
   - Use compression when possible

4. **Multi-Account Setup**
   - Deploy one forwarder per account
   - Use AWS Organizations for management

## Next Steps

- [Create your first application](first-application)
- [Learn about processors](../concepts/processors)
- [Advanced configuration](../advanced/configuration) 