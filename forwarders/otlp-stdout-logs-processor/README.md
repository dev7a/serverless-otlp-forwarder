# OTLP Stdout Logs Processor

AWS Lambda function and supporting resources that process CloudWatch logs containing OpenTelemetry data and forward them to configured collectors.

## Overview

This stack deploys:
1. A Lambda function that receives CloudWatch logs containing OpenTelemetry data
2. Necessary permissions to access CloudWatch logs
3. A subscription filter to route logs to the Lambda function
4. Security group for VPC deployment (if configured)

## Features

- Processes CloudWatch logs containing OpenTelemetry data
- Forwards data to multiple collectors in parallel
- Supports custom headers and authentication
- Handles base64 encoded and gzip compressed data
- Includes OpenTelemetry instrumentation

## Prerequisites

- Rust 1.70 or later
- AWS SAM CLI
- AWS credentials configured

## Building

```bash
# Navigate to the forwarder directory
cd forwarders/otlp-stdout-logs-processor

# Build the stack
sam build
```

## Deployment

1. First time deployment:
```bash
sam deploy --guided --stack-name otlp-stdout-logs-processor
```

2. Subsequent deployments:
```bash
sam build && sam deploy --stack-name otlp-stdout-logs-processor
```

## Configuration

The forwarder can be configured using environment variables and AWS Secrets Manager. See the template.yaml file for details on the available configuration options. 