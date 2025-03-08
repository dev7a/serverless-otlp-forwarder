# OTLP Stdout Kinesis Processor

AWS Lambda function and supporting resources that forward Kinesis wrapped OTLP records to OpenTelemetry collectors.

## Overview

This stack deploys:
1. A Lambda function that receives Kinesis events containing otlp-stdout format records
2. Decodes and decompresses the data
3. Converts records to TelemetryData
4. Forwards the data to collectors in parallel

## Features

- Multiple collectors with different endpoints
- Custom headers and authentication
- Base64 encoded payloads
- Gzip compressed data
- OpenTelemetry instrumentation

## Prerequisites

- Rust 1.70 or later
- AWS SAM CLI
- AWS credentials configured
- Kinesis stream set up with appropriate permissions

## Building

```bash
# Navigate to the forwarder directory
cd forwarders/experimental/otlp-stdout-kinesis-processor

# Build the stack
sam build

# Run tests
cargo test
```

## Deployment

1. First time deployment:
```bash
sam deploy --guided --stack-name otlp-stdout-kinesis-processor
```

2. Subsequent deployments:
```bash
sam build && sam deploy --stack-name otlp-stdout-kinesis-processor
```

