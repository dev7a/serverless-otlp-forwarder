# Examples

This directory contains examples demonstrating how to use `lambda-otel-lite` in a Python Lambda function. The examples showcase:

1. Basic setup and initialization
2. Using the traced decorator for automatic span creation
3. Creating child spans for nested operations
4. Adding custom attributes and events
5. Error handling and propagation

## Example Overview

This example implements a simple HTTP API endpoint that demonstrates various OpenTelemetry features with Python Lambda functions.

## Template Structure

The `template.yaml` provides a Lambda function configuration:

```yaml
HelloWorld:
  Type: AWS::Serverless::Function
  Properties:
    FunctionName: !Sub '${AWS::StackName}-lambda-handler-example'
    CodeUri: ./handler
    Handler: app.handler
    Description: 'Demo Python Lambda function to showcase OpenTelemetry integration'
    FunctionUrlConfig:
      AuthType: NONE
    Environment:
      Variables:
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: async
```

This configuration:
- Uses Python 3.13 runtime (defined in Globals)
- Enables async processing mode for spans
- Includes Function URL for easy testing

## Global Configuration

The template includes global settings for all functions:

```yaml
Globals:
  Function:
    MemorySize: 128
    Timeout: 30
    Architectures:
      - arm64
    Runtime: python3.13
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
```python
# Initialize telemetry once at module load
tracer, completion_handler = init_telemetry()

# Create traced handler with API Gateway v2 extractor
traced = create_traced_handler(
    name="simple-handler",
    completion_handler=completion_handler,
    attributes_extractor=api_gateway_v2_extractor,
)
```

### Handler Implementation

1. **Automatic Context Extraction**:
   - Uses `api_gateway_v2_extractor` to automatically extract HTTP context
   - Captures HTTP method, path, headers, and other API Gateway attributes

2. **Custom Attributes and Events**:
   - Adds request ID as a custom attribute
   - Records the entire event payload as a span event
   ```python
   current_span.set_attribute("request.id", request_id)
   current_span.add_event(
       "handling request",
       {
           "event": json.dumps(event)
       },
   )
   ```

3. **Nested Operations**:
   - Creates child spans for nested function calls
   - Demonstrates proper span hierarchy and context propagation
   ```python
   with tracer.start_as_current_span("nested_function") as span:
       span.add_event("Nested function called")
       # ... operation logic ...
   ```

4. **Error Handling**:
   - Demonstrates different error scenarios:
     - Expected errors (ValueError) -> 400 response
     - Unexpected errors (RuntimeError) -> 500 response
   - Shows proper error recording in spans
   - Simulates errors when accessing `/error` path:
     - 25% chance of ValueError
     - 25% chance of RuntimeError
     - 50% chance of success

### Response Types

1. **Success Response** (Default Path):
   ```json
   {
       "statusCode": 200,
       "body": {"message": "Hello from request {request_id}"}
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
   - Uncaught RuntimeError propagates to Lambda runtime
   - Results in 500 response from API Gateway

Key files:
- `handler/app.py` - The Lambda function implementation
- `handler/requirements.txt` - Python dependencies

## Deployment

### Prerequisites
1. AWS SAM CLI installed
2. AWS credentials configured
3. Python 3.13 or later

> [!Note] The requirements.txt is referencing the local lambda-otel-lite package, which must be built first.

### Deploy with SAM

1. Build the package (in the parent directory):
```bash
python -m build
```

2. Build the function:
```bash
sam build --build-in-source
```

3. Deploy to AWS:
```bash
sam deploy --guided
```

During the guided deployment, you'll be prompted for:
- Stack name
- AWS Region
- Confirmation of IAM role creation
- Function URL authorization settings

### Testing the Deployment

After deployment, you can test the function using its URL:

```bash
curl <HelloWorldFunctionUrl>
```

The function will:
1. Generate OpenTelemetry traces
2. Output OTLP-formatted spans to CloudWatch logs
3. Be compatible with the serverless-otlp-forwarder

## Environment Variables

The example respects the following environment variables:
- `OTEL_SERVICE_NAME`: Service name for spans
- `LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE`: Processing mode (sync/async/finalize)

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