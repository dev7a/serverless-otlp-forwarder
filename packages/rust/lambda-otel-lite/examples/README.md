# Lambda OTel Lite Examples

This directory contains example Lambda functions using the `lambda-otel-lite` crate, demonstrating different instrumentation approaches:

- `simple/`: Basic function using the `traced_handler` wrapper
- `tower/`: Function using the Tower middleware layer

## Prerequisites

- [AWS SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html)
- [Rust](https://rustup.rs/)
- AWS credentials configured

## Deployment

1. Build the functions:
```bash
sam build
```

2. Deploy to AWS (first time):
```bash
sam deploy --guided --stack-name lol-rust-example
```

This will:
- Package and upload the functions
- Create necessary IAM roles
- Deploy the CloudFormation stack
- Save the configuration for future deployments

For subsequent deployments, you can simply use:
```bash
sam deploy
```

## Testing

You can invoke the functions using the SAM CLI:

### Simple Handler Example
```bash
sam remote invoke HandlerExample --stack-name lol-rust-example
```

### Tower Middleware Example
```bash
sam remote invoke TowerExample --stack-name lol-rust-example
```

## Cleanup

To remove all deployed resources:
```bash
sam delete
```

## Understanding the Examples

### Simple Handler (`simple/main.rs`)
Demonstrates basic instrumentation using the `traced_handler` wrapper. 
### Tower Layer (`tower/main.rs`)
Shows how to use the Tower middleware layer for instrumentation. 

Both examples include:
- OpenTelemetry initialization
- Span creation and attribute extraction
- Response status tracking
- Proper span export handling

## Environment Variables

The examples respect the following environment variables:
- `OTEL_SERVICE_NAME`: Service name for spans
- `LAMBDA_TRACING_ENABLE_FMT_LAYER`: Enable console output (default: false)
- `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Processing mode (sync/async/finalize)
- `RUST_LOG`: Log level (e.g., "info", "debug") 