---
layout: default
title: Configuration
parent: Getting Started
nav_order: 2
---

# Configuration Guide
{: .fs-9 }

Configure Lambda OTLP Forwarder for your observability needs.
{: .fs-6 .fw-300 }

## Quick Start
{: .text-delta }

```yaml
# samconfig.toml
version = 0.1
[default.deploy.parameters]
stack_name = "lambda-otlp-forwarder"
resolve_s3 = true
s3_prefix = "lambda-otlp-forwarder"
region = "us-west-2"
confirm_changeset = true
capabilities = "CAPABILITY_IAM"
parameter_overrides = [
  "ProcessorType=otlp-stdout",
  "CollectorEndpoint=https://collector.example.com:4318",
  "CollectorAuthType=basic",
  "CollectorAuthSecret=otlp-forwarder/collector/auth"
]
```

## Core Settings
{: .text-delta }

### Processor Configuration
{: .text-delta }

{: .highlight }
Choose your processor type:

```yaml
Parameters:
  ProcessorType:
    Type: String
    Default: otlp-stdout
    AllowedValues:
      - otlp-stdout
      - aws-appsignals
```

### Collector Settings
{: .text-delta }

{: .highlight }
Configure your collector endpoint:

```yaml
Parameters:
  CollectorEndpoint:
    Type: String
    Default: https://collector.example.com:4318
  
  CollectorProtocol:
    Type: String
    Default: http/protobuf
    AllowedValues:
      - http/protobuf
      - http/json
  
  CollectorCompression:
    Type: String
    Default: gzip
    AllowedValues:
      - gzip
      - none
```

## Authentication
{: .text-delta }

### Basic Auth
{: .text-delta }

1. Create secret in AWS Secrets Manager:
```bash
aws secretsmanager create-secret \
  --name otlp-forwarder/collector/basic-auth \
  --secret-string '{"username":"your-username","password":"your-password"}'
```

2. Configure in template:
```yaml
Parameters:
  CollectorAuthType:
    Type: String
    Default: basic
  
  CollectorAuthSecret:
    Type: String
    Default: otlp-forwarder/collector/basic-auth
```

### Bearer Token
{: .text-delta }

1. Create secret:
```bash
aws secretsmanager create-secret \
  --name otlp-forwarder/collector/bearer \
  --secret-string '{"token":"your-bearer-token"}'
```

2. Configure in template:
```yaml
Parameters:
  CollectorAuthType:
    Type: String
    Default: bearer
  
  CollectorAuthSecret:
    Type: String
    Default: otlp-forwarder/collector/bearer
```

### AWS IAM
{: .text-delta }

```yaml
Parameters:
  CollectorAuthType:
    Type: String
    Default: aws-iam
  
  CollectorRegion:
    Type: String
    Default: us-west-2
```

## Network Configuration
{: .text-delta }

### VPC Settings
{: .text-delta }

```yaml
Parameters:
  VpcId:
    Type: AWS::EC2::VPC::Id
  
  SubnetIds:
    Type: List<AWS::EC2::Subnet::Id>
  
  SecurityGroupIds:
    Type: List<AWS::EC2::SecurityGroup::Id>
```

### Private Link
{: .text-delta }

```yaml
Parameters:
  EnablePrivateLink:
    Type: String
    Default: false
    AllowedValues: [true, false]
  
  VpcEndpointId:
    Type: String
    Default: ""
```

## Performance Tuning
{: .text-delta }

### Memory and Timeout
{: .text-delta }

```yaml
Parameters:
  MemorySize:
    Type: Number
    Default: 256
    MinValue: 128
    MaxValue: 10240
  
  Timeout:
    Type: Number
    Default: 30
    MinValue: 1
    MaxValue: 900
```

### Batch Processing
{: .text-delta }

```yaml
Parameters:
  BatchSize:
    Type: Number
    Default: 100
    MinValue: 1
    MaxValue: 1000
  
  BatchTimeoutSeconds:
    Type: Number
    Default: 5
    MinValue: 1
    MaxValue: 30
```

## Monitoring
{: .text-delta }

### CloudWatch Logs
{: .text-delta }

```yaml
Parameters:
  LogRetentionDays:
    Type: Number
    Default: 7
    AllowedValues: [1, 3, 5, 7, 14, 30, 60, 90, 120, 150, 180, 365, 400, 545, 731, 1827, 3653]
  
  EnableDetailedMetrics:
    Type: String
    Default: true
    AllowedValues: [true, false]
```

### X-Ray Tracing
{: .text-delta }

```yaml
Parameters:
  EnableXRayTracing:
    Type: String
    Default: true
    AllowedValues: [true, false]
  
  XRaySamplingRate:
    Type: Number
    Default: 0.1
    MinValue: 0
    MaxValue: 1
```

## Advanced Configuration
{: .text-delta }

### Error Handling
{: .text-delta }

```yaml
Parameters:
  MaxRetries:
    Type: Number
    Default: 3
    MinValue: 0
    MaxValue: 10
  
  EnableDLQ:
    Type: String
    Default: true
    AllowedValues: [true, false]
```

### Custom Headers
{: .text-delta }

1. Create secret:
```bash
aws secretsmanager create-secret \
  --name otlp-forwarder/collector/headers \
  --secret-string '{"X-Custom-Header":"value"}'
```

2. Configure in template:
```yaml
Parameters:
  CustomHeadersSecret:
    Type: String
    Default: otlp-forwarder/collector/headers
```

## Environment Variables
{: .text-delta }

### Common Settings
{: .text-delta }

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Collector endpoint | `http://localhost:4318` |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | Protocol to use | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | Compression type | `gzip` |
| `OTEL_SERVICE_NAME` | Service name | Function name |

### Advanced Settings
{: .text-delta }

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_BATCH_MAX_QUEUE_SIZE` | Maximum queue size | `2048` |
| `OTEL_BATCH_SCHEDULE_DELAY` | Batch delay in ms | `5000` |
| `OTEL_BATCH_MAX_EXPORT_SIZE` | Maximum batch size | `512` |
| `OTEL_EXPERIMENTAL_BATCH_SPANS` | Enable batching | `true` |

## Best Practices
{: .text-delta }

### Security
{: .text-delta }

{: .warning }
- Use AWS Secrets Manager for credentials
- Enable encryption in transit
- Implement least privilege IAM roles
- Use VPC endpoints for private access
- Regular credential rotation

### Performance
{: .text-delta }

{: .info }
- Configure appropriate memory size
- Enable batch processing
- Use compression
- Set reasonable timeouts
- Monitor and adjust settings

### Reliability
{: .text-delta }

{: .info }
- Enable retries with backoff
- Configure DLQ for failed events
- Set up monitoring and alerts
- Regular health checks
- Implement circuit breakers

## Validation
{: .text-delta }

### Configuration Check
{: .text-delta }

```bash
# Validate template
sam validate

# Test configuration
sam local invoke \
  --event events/test.json \
  --env-vars env.json

# Check deployment
aws cloudformation describe-stacks \
  --stack-name lambda-otlp-forwarder
```

### Health Check
{: .text-delta }

```bash
# Test collector connection
aws lambda invoke \
  --function-name otlp-forwarder \
  --payload '{"action":"health"}' \
  response.json

# Check metrics
aws cloudwatch get-metric-statistics \
  --namespace AWS/Lambda \
  --metric-name Errors \
  --dimensions Name=FunctionName,Value=otlp-forwarder \
  --start-time $(date -u -v-1H +%Y-%m-%dT%H:%M:%SZ) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%SZ) \
  --period 300 \
  --statistics Sum
```

## Next Steps
{: .text-delta }

- [Set up Language SDKs](../languages)
- [Understand Architecture](../concepts/architecture)
- [Configure Processors](../concepts/processors)
- [View Advanced Features](../advanced) 