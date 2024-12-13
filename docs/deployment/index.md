---
layout: default
title: Deployment
nav_order: 5
has_children: true
---

# Deployment Guide
{: .fs-9 }

Deploy and configure Lambda OTLP Forwarder for your observability needs.
{: .fs-6 .fw-300 }

## Quick Deploy
{: .text-delta }

```bash
git clone https://github.com/dev7a/lambda-otlp-forwarder
cd lambda-otlp-forwarder
sam build && sam deploy --guided
```

## Deployment Options
{: .text-delta }

### Basic Deployment
{: .text-delta }

{: .highlight }
Standard setup with OTLP stdout processor:
- Single AWS account
- One collector endpoint
- Basic authentication
- Default configuration

```yaml
ProcessorType: otlp-stdout
RouteAllLogs: true
CollectorEndpoint: https://collector.example.com:4318
```

### Application Signals
{: .text-delta }

{: .highlight }
Setup with AWS Application Signals processor:
- Native AWS X-Ray integration
- CloudWatch integration
- Simplified configuration
- Automatic context propagation

```yaml
ProcessorType: aws-appsignals
EnableXRayTracing: true
EnableCloudWatchMetrics: true
```

### Multi-Account
{: .text-delta }

{: .highlight }
Deploying across multiple AWS accounts:
- Central collector account
- Multiple source accounts
- Cross-account IAM roles
- Consolidated monitoring

```yaml
ProcessorType: otlp-stdout
CrossAccountRoleArn: arn:aws:iam::ACCOUNT_ID:role/OTLPForwarderRole
EnableCrossAccountAccess: true
```

## SAM Template Parameters
{: .text-delta }

| Parameter | Description | Default | Required |
|:----------|:------------|:---------|:---------|
| `ProcessorType` | Type of processor to deploy | `otlp-stdout` | Yes |
| `RouteAllLogs` | Automatically route all logs | `true` | No |
| `CollectorEndpoint` | OTLP collector endpoint | - | Yes* |
| `CollectorAuthType` | Authentication type | `none` | No |
| `RetentionInDays` | Log retention period | `7` | No |
| `MemorySize` | Lambda function memory | `256` | No |
| `Timeout` | Lambda function timeout | `30` | No |

{: .info }
\* Required only for `otlp-stdout` processor type

## Authentication
{: .text-delta }

### Basic Auth
{: .text-delta }

```yaml
CollectorAuthType: basic
CollectorAuthSecret: otlp-forwarder/collector/basic-auth
```

Store credentials in AWS Secrets Manager:
```json
{
  "username": "your-username",
  "password": "your-password"
}
```

### Bearer Token
{: .text-delta }

```yaml
CollectorAuthType: bearer
CollectorAuthSecret: otlp-forwarder/collector/bearer-token
```

Store token in AWS Secrets Manager:
```json
{
  "token": "your-bearer-token"
}
```

### AWS IAM
{: .text-delta }

```yaml
CollectorAuthType: aws-iam
CollectorRegion: us-west-2
```

## Network Configuration
{: .text-delta }

### VPC Access
{: .text-delta }

```yaml
VpcConfig:
  SecurityGroupIds:
    - sg-xxxxxxxxxxxxxxxxx
  SubnetIds:
    - subnet-xxxxxxxxxxxxxxxxx
    - subnet-yyyyyyyyyyyyyyyyy
```

### Private Link
{: .text-delta }

```yaml
EnablePrivateLink: true
VpcEndpointId: vpce-xxxxxxxxxxxxxxxxx
```

## Monitoring
{: .text-delta }

### CloudWatch Metrics
{: .text-delta }

{: .info }
Key metrics to monitor:
- `ProcessedLogEvents`: Number of log events processed
- `ForwardedSpans`: Number of spans forwarded
- `ProcessingErrors`: Number of processing errors
- `ForwardingLatency`: Time taken to forward data

### Alarms
{: .text-delta }

```yaml
Alarms:
  ProcessingErrorsAlarm:
    Type: AWS::CloudWatch::Alarm
    Properties:
      MetricName: ProcessingErrors
      Threshold: 10
      Period: 300
      EvaluationPeriods: 2
```

## Cost Optimization
{: .text-delta }

### Log Filtering
{: .text-delta }

{: .info }
Optimize costs by:
- Using precise subscription filters
- Setting appropriate retention periods
- Enabling compression
- Configuring sampling

Example subscription filter:
```json
{
  "filterPattern": "{ $.source = \"otlp-forwarder\" }",
  "filterName": "OTLPFilter"
}
```

### Batch Configuration
{: .text-delta }

```yaml
BatchingConfig:
  MaxItems: 100
  MaxBytes: 1048576
  TimeoutSeconds: 30
```

## Best Practices
{: .text-delta }

### Security
{: .text-delta }

{: .warning }
Security recommendations:
- Use AWS Secrets Manager for credentials
- Enable VPC endpoints for private access
- Implement least privilege IAM roles
- Enable encryption in transit
- Regular security audits

### Performance
{: .text-delta }

{: .info }
Performance optimization:
- Configure appropriate memory size
- Set optimal batch sizes
- Enable compression
- Use efficient protocols
- Monitor and adjust timeouts

### Reliability
{: .text-delta }

{: .info }
Reliability measures:
- Implement proper error handling
- Set up monitoring and alerts
- Configure DLQ for failed events
- Regular backup and recovery testing
- Multi-region deployment if needed

### Scaling
{: .text-delta }

{: .info }
Scaling considerations:
- Monitor Lambda concurrency
- Adjust batch sizes based on load
- Configure appropriate timeouts
- Use provisioned concurrency if needed
- Set up throttling alerts

## Troubleshooting
{: .text-delta }

{: .warning }
Common deployment issues:

1. **IAM Permissions**
   - Check role permissions
   - Verify cross-account access
   - Review resource policies

2. **Network Issues**
   - Verify VPC configuration
   - Check security groups
   - Test collector connectivity

3. **Authentication Problems**
   - Validate secrets configuration
   - Check token expiration
   - Verify endpoint URLs

## Next Steps
{: .text-delta }

- [Configure Processors](../concepts/processors)
- [Set up Monitoring](monitoring)
- [Advanced Features](../advanced) 