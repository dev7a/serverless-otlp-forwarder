# Simple Handler Example

This example demonstrates basic usage of `lambda-otel-lite` in a Python Lambda function. It shows:

1. Basic setup and initialization
2. Using the traced decorator for automatic span creation
3. Creating child spans for nested operations
4. Adding custom attributes and events
5. Error handling and propagation

## Requirements

- Python 3.13 or later
- AWS Lambda with Python 3.13 runtime

## Example Behavior

The example implements a simple HTTP API endpoint that demonstrates various OpenTelemetry features:

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
The handler demonstrates several key features:

1. **Automatic Context Extraction**:
   - Uses `api_gateway_v2_extractor` to automatically extract HTTP context
   - Captures HTTP method, path, headers, and other API Gateway attributes

2. **Custom Attributes and Events**:
   - Adds request ID as a custom attribute
   - Records the entire event payload as a span event
   ```python
   current_span.set_attribute("request.id", request_id)
   current_span.add_event("handling request", event)
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

## Structure

- `app.py` - The Lambda function implementation
- `requirements.txt` - Python dependencies

## Deployment

> [!Note] The requirements.txt is referencing the local lambda-otel-lite package, which must be built first. After building the package, you can just deploy the function as usual.

1. Build:
```bash
sam build --build-in-source
```

2. Deploy:
```bash
sam deploy --guided
``` 