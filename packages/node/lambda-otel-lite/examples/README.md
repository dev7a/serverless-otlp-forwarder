# Examples

This directory contains examples demonstrating how to use `lambda-otel-lite` in AWS Lambda functions. The examples showcase:

1. Basic setup and initialization
2. Using the traced handler wrapper for automatic span creation
3. Creating child spans for nested operations
4. Adding custom attributes and events
5. Error handling and propagation
6. Different deployment methods (standard and bundled)

## Example Overview

The examples demonstrate:
- Standard Node.js function with direct module loading
- ESBuild-bundled function for optimized deployment

## Template Structure

The `template.yaml` provides two example Lambda functions with different build configurations:

### 1. Standard Node.js Function (`HelloWorld`)
```yaml
HelloWorld:
  Type: AWS::Serverless::Function
  Properties:
    FunctionName: !Sub '${AWS::StackName}-lambda-handler-example'
    CodeUri: ./handler
    Handler: app.handler
    Description: 'Demo Node Lambda function to showcase OpenTelemetry integration'
    FunctionUrlConfig:
      AuthType: NONE
    Environment:
      Variables:
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: async
        NODE_OPTIONS: --require @dev7a/lambda-otel-lite/extension
```
This configuration:
- Uses standard Node.js module resolution
- Loads the extension directly from `node_modules`
- Suitable for simple deployments without bundling
- Includes Function URL for easy testing

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
      Platform: "node"
      EntryPoints: 
        - app.js
        - init.js
  Properties:
    FunctionName: !Sub '${AWS::StackName}-lambda-handler-example-esbuild'
    CodeUri: ./handler
    Handler: app.handler
    Description: 'Demo Node Lambda function to showcase OpenTelemetry integration'
    FunctionUrlConfig:
      AuthType: NONE
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
      ApplicationLogLevel: INFO
      SystemLogLevel: INFO
```

These settings:
- Use ARM64 architecture for better cost/performance
- Configure JSON logging for better integration with log processors
- Set appropriate memory and timeout values

## Function Structure

The example demonstrates several key features:

### Basic Setup
```javascript
// Initialize telemetry once at module load
const { tracer, completionHandler } = initTelemetry();

// Create a traced handler with configuration
const traced = createTracedHandler(
  'simple-handler',
  completionHandler,
  apiGatewayV2Extractor 
);
```

### Handler Implementation

1. **Automatic Context Extraction**:
   - Uses `apiGatewayV2Extractor` to automatically extract HTTP context
   - Captures HTTP method, path, headers, and other API Gateway attributes

2. **Custom Attributes and Events**:
   - Adds request ID as a custom attribute
   - Records the entire event payload as a span event
   ```javascript
  currentSpan?.setAttribute('request.id', requestId);
  currentSpan?.addEvent('handling request', {
    event: JSON.stringify(event)
  });
   ```

3. **Nested Operations**:
   - Creates child spans for nested function calls
   - Demonstrates proper span hierarchy and context propagation
   ```javascript
   return tracer.startActiveSpan('nested_function', async (span) => {
     try {
       span.addEvent('Nested function called');
       // ... operation logic ...
       return 'success';
     } finally {
       span.end();
     }
   });
   ```

4. **Error Handling**:
   - Demonstrates different error scenarios:
     - Expected errors (Error with message 'expected error') -> 400 response
     - Unexpected errors (Error with message 'unexpected error') -> 500 response
   - Shows proper error recording in spans
   - Simulates errors when accessing `/error` path:
     - 25% chance of expected error
     - 25% chance of unexpected error
     - 50% chance of success

### Response Types

1. **Success Response** (Default Path):
   ```json
   {
       "statusCode": 200,
       "body": {"message": "Hello from request {requestId}"}
   }
   ```

2. **Client Error** (`/error` path, 25% chance):
   ```json
   {
       "statusCode": 400,
       "body": {"message": "expected error"}
   }
   ```

3. **Server Error** (`/error` path, 25% chance):
   - Uncaught Error propagates to Lambda runtime
   - Results in 500 response from API Gateway

Key files:
- `handler/app.js` - Main handler implementation
- `handler/init.js` - Extension initialization for bundled deployment
- `handler/package.json` - Dependencies and project configuration

## Environment Variables

The examples respect the following environment variables:
- `OTEL_SERVICE_NAME`: Service name for spans
- `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Processing mode (sync/async/finalize)
- `NODE_OPTIONS`: Used to load the extension
- `LAMBDA_TRACING_ENABLE_FMT_LAYER`: Enable console output (default: false)

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

After deployment, you can test the functions using their URLs:

```bash
# Standard function
curl <HelloWorldFunctionUrl>

# ESBuild bundled function
curl <HelloWorldESBuildFunctionUrl>
```

Both functions will:
1. Generate OpenTelemetry traces
2. Output OTLP-formatted spans to CloudWatch logs
3. Be compatible with the serverless-otlp-forwarder

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
