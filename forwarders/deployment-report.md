# Deployment Report - rearset-pipeline

## Pipeline Description
Rearset Pipeline
This pipeline deploys the Rearset infrastructure and related resources.
It includes a collector configuration layer, a OTLP Relay for the CloudWatch Logs transport,
and a Kinesis Relay for the Kinesis Data Streams transport.
It also includes an experimental Kinesis extension to send data to the Kinesis Relay.

## Stack Deployment Results

## collector
- **stack name**: `rearset-collector`
- **CloudFormation Status**: `UPDATE_COMPLETE`
#### Parameters
  _None_
#### Outputs

| Key        | Value                |
|------------|----------------------|
| CollectorConfigMapArn | arn:aws:lambda:us-east-1:************:layer:rearset-collector-configmap:17 |
| CollectorConfigSecretsArn | arn:aws:secretsmanager:us-east-1:************:secret:rearset-collector/configmap/secrets-RgR8wl |

---

## logs-relay
- **stack name**: `rearset-logs-relay`
- **CloudFormation Status**: `UPDATE_COMPLETE`
#### Parameters

| Key        | Value                |
|------------|----------------------|
| CollectorExtensionArn | arn:aws:lambda:us-east-1:************:layer:ocel-arm64-minimal-forwarder-0_128_0-beta:2 |
| CollectorConfigMapArn | arn:aws:lambda:us-east-1:************:layer:rearset-collector-configmap:17 |
| RouteAllLogs | true |
| VpcId |  |
| SubnetIds |  |
#### Outputs

| Key        | Value                |
|------------|----------------------|
| ProcessorFunctionArn | arn:aws:lambda:us-east-1:************:function:rearset-logs-relay |

---

## kinesis-relay
- **stack name**: `rearset-kinesis-relay`
- **CloudFormation Status**: `UPDATE_COMPLETE`
#### Parameters

| Key        | Value                |
|------------|----------------------|
| CollectorExtensionArn | arn:aws:lambda:us-east-1:************:layer:ocel-arm64-minimal-forwarder-0_128_0-beta:2 |
| CollectorConfigMapArn | arn:aws:lambda:us-east-1:************:layer:rearset-collector-configmap:17 |
| VpcId |  |
| SubnetIds |  |
| RouteAllLogs | true |
| KinesisStreamMode | PROVISIONED |
| ShardCount | 1 |
#### Outputs

| Key        | Value                |
|------------|----------------------|
| ProcessorFunctionArn | arn:aws:lambda:us-east-1:************:function:rearset-kinesis-relay |
| KinesisStreamName | rearset-kinesis-relay-otlp-stream |

---

## kinesis-extension
- **stack name**: `rearset-kinesis-extension`
- **CloudFormation Status**: `UPDATE_COMPLETE`
#### Parameters

| Key        | Value                |
|------------|----------------------|
| KinesisStreamName | rearset-kinesis-relay-otlp-stream |
#### Outputs

| Key        | Value                |
|------------|----------------------|
| ExampleFunctionUrl | https://************.lambda-url.us-east-1.on.aws/ |
| ExtensionLayerARM64Arn | arn:aws:lambda:us-east-1:************:layer:rearset-kinesis-extension-layer-arm64:4 |

---

## signals-relay
- **stack name**: `rearset-signals-relay`
- **CloudFormation Status**: `UPDATE_COMPLETE`
#### Parameters

| Key        | Value                |
|------------|----------------------|
| CollectorExtensionArn | arn:aws:lambda:us-east-1:************:layer:ocel-arm64-minimal-forwarder-0_128_0-beta:2 |
| CollectorConfigMapArn | arn:aws:lambda:us-east-1:************:layer:rearset-collector-configmap:17 |
| VpcId |  |
| SubnetIds |  |
#### Outputs

| Key        | Value                |
|------------|----------------------|
| ProcessorFunctionArn | arn:aws:lambda:us-east-1:************:function:rearset-signals-relay |

---

## Pipeline Summary
# Rearset Pipeline - Deployment Complete

Your OpenTelemetry infrastructure has been successfully deployed with the following components:


### 1. Configure Collector Secrets
**Critical:** Update the collector configuration secret in AWS Secrets Manager:

```bash
aws secretsmanager update-secret \
    --secret-id arn:aws:secretsmanager:us-east-1:************:secret:rearset-collector/configmap/secrets-RgR8wl \
    --secret-string '{
      "name": "Production OTel Collector",
      "endpoint": "https://your-otel-collector.example.com",
      "auth": "api-key=your-api-key,x-custom-header=your-value"
    }'
```

**Alternative:** Use the [AWS Secrets Manager Console](https://console.aws.amazon.com/secretsmanager/) to update the secret manually.

### 2. Verify Network Configuration
- **VPC ID**: `No VPC`
- **Subnets**: `No subnets`
- **Extension Layer**: `arn:aws:lambda:us-east-1:************:layer:ocel-arm64-minimal-forwarder-0_128_0-beta:2`

## Testing Your Deployment

### CloudWatch Logs Relay
1. Check CloudWatch Logs for any log groups being processed
2. Verify logs are being forwarded to your OTel Collector endpoint


## Important Notes

- **Secrets Configuration**: The pipeline will not function until you configure the collector secrets
- **Network Access**: Ensure your Lambda functions can reach the OTel Collector endpoint
- **Cost Monitoring**: Monitor Kinesis and Lambda costs, especially in high-throughput scenarios
- **Security**: Review IAM roles and VPC configurations for your security requirements
