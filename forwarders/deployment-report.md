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
