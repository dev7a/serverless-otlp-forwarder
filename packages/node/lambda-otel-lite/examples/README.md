# Examples

This directory contains examples demonstrating how to use `lambda-otel-lite` in AWS Lambda functions. The examples showcase different deployment methods and configurations using AWS SAM.

## Template Structure

The `template.yaml` provides two example Lambda functions with different build configurations:

### 1. Standard Node.js Function (`HelloWorld`)
```yaml
HelloWorld:
  Type: AWS::Serverless::Function
  Properties:
    Handler: app.handler
    Environment:
      Variables:
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: async
        NODE_OPTIONS: --require @dev7a/lambda-otel-lite/extension
```
This configuration:
- Uses standard Node.js module resolution
- Loads the extension directly from `node_modules`
- Suitable for simple deployments without bundling

### 2. ESBuild-bundled Function (`HelloWorldESBuild`)
```yaml
HelloWorldESBuild:
  Type: AWS::Serverless::Function
  Metadata:
    BuildMethod: esbuild
    BuildProperties:
      Minify: true
      Target: "es2022"
      Format: "cjs"
      EntryPoints: 
        - app.js
        - init.js
  Properties:
    Handler: app.handler
    Environment:
      Variables:
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: async
        NODE_OPTIONS: --require /var/task/init.js
```
This configuration:
- Uses esbuild to bundle the function and dependencies
- Includes the extension in the bundle
- Optimized for production deployments
- Reduces cold start time by bundling dependencies

## Global Configuration

The template includes global settings for all functions:

```yaml
Globals:
  Function:
    MemorySize: 128
    Timeout: 30
    Architectures:
      - arm64
    Runtime: nodejs22.x
    LoggingConfig:
      LogFormat: JSON
      ApplicationLogLevel: DEBUG
      SystemLogLevel: INFO
```

These settings:
- Use ARM64 architecture for better cost/performance
- Configure JSON logging for better integration with log processors
- Set appropriate memory and timeout values
- Enable detailed logging for debugging

## Deployment

### Prerequisites
1. AWS SAM CLI installed
2. AWS credentials configured
3. Node.js 18 or later

### Deploy with SAM

1. Build the functions:
```bash
sam build --build-in-source
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

After deployment, you can test the functions using their URLs and curl

```bash
curl <HelloWorldFunctionUrl>
```

Both functions will:
1. Generate OpenTelemetry traces
2. Output OTLP-formatted spans to CloudWatch logs
3. Be compatible with the serverless-otlp-forwarder

## Function Structure

The example functions demonstrate:
1. Initialization of telemetry
2. Usage of the traced handler wrapper
3. Event attribute extraction
4. Proper error handling

Key files:
- `handler/app.js` - Main handler implementation
- `handler/init.js` - Extension initialization

## Viewing Traces

The traces will be available in your configured OpenTelemetry backend when using the [serverless-otlp-forwarder](https://github.com/dev7a/serverless-otlp-forwarder)

For setup instructions and configuration options, see the [serverless-otlp-forwarder documentation](https://github.com/dev7a/serverless-otlp-forwarder).

## Additional Resources

- [Main lambda-otel-lite Documentation](../README.md)
- [AWS SAM Documentation](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/what-is-sam.html)
- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)
