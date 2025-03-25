# Examples

This directory contains examples demonstrating how to use `lambda-otel-lite` in AWS Lambda functions. The examples showcase:

1. Basic setup and initialization
2. Using the traced handler wrapper for automatic span creation
3. Creating child spans for nested operations
4. Adding custom attributes and events
5. Error handling and propagation
6. Different instrumentation approaches (handler wrapper and Tower middleware)

## Example Overview

The examples demonstrate:
- `handler/`: Basic function using the `create_traced_handler` wrapper
- `tower/`: Function using the Tower middleware layer

## Template Structure

The `template.yaml` provides two example Lambda functions with different integration patterns:

```yaml
HandlerExample:
  Type: AWS::Serverless::Function
  Metadata:
    BuildMethod: rust-cargolambda
    BuildProperties:
      Binary: handler-example
  Properties:
    FunctionName: !Sub '${AWS::StackName}-lambda-handler-example'
    CodeUri: ./
    Handler: bootstrap
    Description: 'Demo Handler Example Lambda function to showcase OpenTelemetry integration'
    FunctionUrlConfig:
      AuthType: NONE
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
    BuildProperties:
      Binary: tower-example
  Properties:
    FunctionName: !Sub '${AWS::StackName}-lambda-tower-example'
    CodeUri: ./
    Handler: bootstrap
    Runtime: provided.al2023
    Description: 'Demo Tower Example Lambda function to showcase OpenTelemetry integration'
    FunctionUrlConfig:
      AuthType: NONE
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
      ApplicationLogLevel: INFO
      SystemLogLevel: INFO
```

These settings:
- Use ARM64 architecture for better cost/performance
- Configure JSON logging for better integration with log processors
- Set appropriate memory and timeout values
- Use the provided.al2023 runtime for Rust functions

## Function Structure

The examples demonstrate different integration approaches:

### Basic Setup

#### Handler Approach
```rust
// Initialize telemetry with default config
let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;

// Create the traced handler
let handler = create_traced_handler("simple-handler", completion_handler, handler);

// Use it directly with the runtime
Runtime::new(service_fn(handler)).run().await
```

#### Tower Approach
```rust
// Initialize telemetry with default configuration
let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;

// Build service with OpenTelemetry tracing middleware
let service = ServiceBuilder::new()
    .layer(OtelTracingLayer::new(completion_handler).with_name("tower-handler"))
    .service_fn(handler);

// Create and run the Lambda runtime
let runtime = Runtime::new(service);
runtime.run().await
```

### Handler Implementation

1. **Automatic Context Extraction**:
   - Automatically extracts HTTP context from API Gateway events
   - Captures HTTP method, path, headers, and other API Gateway attributes

2. **Custom Attributes and Events**:
   - Adds request ID as a custom attribute
   - Records the entire event payload as a span event
   ```rust
   // Set request ID as span attribute
   current_span.set_attribute("request.id", request_id.to_string());

   // Log the full event payload
   info!(
       event = serde_json::to_string(&event.payload).unwrap_or_default(),
       "handling request"
   );
   ```

3. **Nested Operations**:
   - Creates child spans for nested function calls using the `#[instrument]` attribute
   - Demonstrates proper span hierarchy and context propagation
   ```rust
   #[instrument(skip(event), level = "info", err)]
   async fn nested_function(event: &ApiGatewayV2httpRequest) -> Result<String, ErrorType> {
       info!("Nested function called");
       // ... operation logic ...
   }
   ```

4. **Error Handling**:
   - Demonstrates different error scenarios:
     - Expected errors (ErrorType::Expected) -> 400 response
     - Unexpected errors (ErrorType::Unexpected) -> 500 response
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
       "body": "Hello from request {request_id}"
   }
   ```

2. **Client Error** (`/error` path, 25% chance):
   ```json
   {
       "statusCode": 400,
       "body": {"message": "This is an expected error"}
   }
   ```

3. **Server Error** (`/error` path, 25% chance):
   - Uncaught Error propagates to Lambda runtime
   - Results in 500 response from API Gateway

Key files:
- `handler/main.rs` - Handler wrapper implementation
- `tower/main.rs` - Tower middleware implementation
- `Cargo.toml` - Dependencies and binary configurations

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

Both functions will:
1. Generate OpenTelemetry traces
2. Output OTLP-formatted spans to CloudWatch logs
3. Be compatible with the serverless-otlp-forwarder

## Environment Variables

The examples respect the following environment variables:
- `OTEL_SERVICE_NAME`: Service name for spans
- `LAMBDA_TRACING_ENABLE_FMT_LAYER`: Control console output formatting
  - Setting to "true" enables console output even if disabled in code
  - Setting to "false" disables console output even if enabled in code
  - Only accepts exact string values "true" or "false" (case-insensitive)
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