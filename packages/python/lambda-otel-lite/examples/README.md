# Examples

This directory contains examples demonstrating how to use `lambda-otel-lite` in AWS Lambda functions. The examples showcase integration patterns and observability best practices.

## Template Structure

The `template.yaml` provides a Lambda function with Python 3.13 runtime:

```yaml
Handler:
  Type: AWS::Serverless::Function
  Properties:
    CodeUri: handler/
    Handler: app.handler
    Runtime: python3.13
    Environment:
      Variables:
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: async
```

This configuration:
- Uses the standard Python Lambda runtime
- Enables async processing mode for optimal performance
- Includes a Function URL for easy testing

## Global Configuration

The template includes global settings for all functions:

```yaml
Globals:
  Function:
    MemorySize: 128
    Timeout: 30
    Architectures:
      - arm64
    LoggingConfig:
      LogFormat: JSON
```

These settings:
- Use ARM64 architecture for better cost/performance
- Configure JSON logging for better integration with log processors
- Set appropriate memory and timeout values

## Deployment

### Prerequisites
1. AWS SAM CLI installed
2. AWS credentials configured
3. Python 3.13 or later

### Deploy with SAM

1. Build the functions:
```bash
sam build
```

2. Deploy to AWS:
```bash
sam deploy --guided
```

During the guided deployment, you'll be prompted for:
- Stack name
- AWS Region
- Confirmation of IAM role creation
- Function URL authorization settings

### Testing the Deployment

After deployment, you can test the function using its URL and curl:

```bash
curl <HandlerFunctionUrl>
```

The function will:
1. Generate OpenTelemetry traces
2. Output OTLP-formatted spans to CloudWatch logs
3. Be compatible with the serverless-otlp-forwarder

## Function Structure

The example function demonstrates:
1. Initialization of telemetry
2. Usage of the traced handler decorator
3. Event attribute extraction
4. Proper error handling and span status setting

Key files:
- `handler/app.py` - Main handler implementation
- `handler/requirements.txt` - Dependencies

## Viewing Traces

The traces will be available in your configured OpenTelemetry backend when using the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder)

For setup instructions and configuration options, see the [serverless-otlp-forwarder documentation](https://github.com/dev7a/serverless-otlp-forwarder).

## Additional Resources

- [Main lambda-otel-lite Documentation](../README.md)
- [AWS SAM Documentation](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/what-is-sam.html)
- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)