# Examples

This directory contains examples demonstrating how to use `lambda-otel-lite` in AWS Lambda functions. The examples showcase different instrumentation approaches:

- `handler/`: Basic function using the `create_traced_handler` wrapper
- `tower/`: Function using the Tower middleware layer

## Template Structure

The `template.yaml` provides two example Lambda functions with different integration patterns:

```yaml
HandlerExample:
  Type: AWS::Serverless::Function
  Metadata:
    BuildMethod: rust-cargolambda
  Properties:
    CodeUri: .
    Handler: handler
    Environment:
      Variables:
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: async
```

This configuration:
- Uses the Rust Cargo Lambda builder
- Provides ARM64 architecture for optimal performance
- Enables async processing mode
- Includes Function URLs for easy testing

```yaml
TowerExample:
  Type: AWS::Serverless::Function
  Metadata:
    BuildMethod: rust-cargolambda
  Properties:
    CodeUri: .
    Handler: tower
    Environment:
      Variables:
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: async
```

## Global Configuration

The template includes global settings for all functions:

```yaml
Globals:
  Function:
    MemorySize: 128
    Timeout: 3
    Architectures:
      - arm64
    Runtime: provided.al2023
    LoggingConfig:
      LogFormat: JSON
```

These settings:
- Use ARM64 architecture for better cost/performance
- Configure JSON logging for better integration with log processors
- Set appropriate memory and timeout values
- Use the provided.al2023 runtime for Rust functions

## Deployment

### Prerequisites
1. AWS SAM CLI installed
2. AWS credentials configured
3. Rust toolchain
4. Cargo Lambda (`cargo install cargo-lambda`)

### Deploy with SAM

1. Build the functions:
```bash
sam build
```

2. Deploy to AWS:
```bash
sam deploy --guided --stack-name lol-rust-example
```

During the guided deployment, you'll be prompted for:
- Stack name
- AWS Region
- Confirmation of IAM role creation
- Function URL authorization settings

For subsequent deployments, you can simply use:
```bash
sam deploy
```

### Testing the Deployment

After deployment, you can test the functions using their URLs:

```bash
# Handler example
curl <HandlerExampleFunctionUrl>

# Tower example
curl <TowerExampleFunctionUrl>
```

Or via the SAM CLI:

```bash
# Handler example
sam remote invoke HandlerExample --stack-name lol-rust-example

# Tower example
sam remote invoke TowerExample --stack-name lol-rust-example
```

Both functions will:
1. Generate OpenTelemetry traces
2. Output OTLP-formatted spans to CloudWatch logs
3. Be compatible with the serverless-otlp-forwarder

## Function Structure

The examples demonstrate different integration approaches:

1. **Handler Wrapper** (`handler/main.rs`):
   - Uses the `create_traced_handler` function to wrap a Lambda handler
   - Simpler approach for basic Lambda functions
   - Automatically extracts context and creates spans

2. **Tower Middleware** (`tower/main.rs`):
   - Uses the Tower service model with `OtelTracingLayer`
   - Suitable for complex Lambda applications with middleware chains
   - More flexible for advanced use cases
   - Integrates with other Tower middleware

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

## Viewing Traces

The traces will be available in your configured OpenTelemetry backend when using the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder)

For setup instructions and configuration options, see the [serverless-otlp-forwarder documentation](https://github.com/dev7a/serverless-otlp-forwarder).

## Cleanup

To remove all deployed resources:
```bash
sam delete
```

## Additional Resources

- [Main lambda-otel-lite Documentation](../README.md)
- [AWS SAM Documentation](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/what-is-sam.html)
- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)