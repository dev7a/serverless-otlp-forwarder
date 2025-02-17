/**
 * Simple Hello World Lambda function using lambda-otel-lite.
 * 
 * This example demonstrates basic OpenTelemetry setup with lambda-otel-lite.
 * It creates spans for each invocation and logs the event payload using span events.
 */

const { initTelemetry, createTracedHandler, apiGatewayV2Extractor } = require('@dev7a/lambda-otel-lite');

// Initialize telemetry once at module load
const { tracer, completionHandler } = initTelemetry();

/**
 * Simple nested function that creates its own span.
 * 
 * This function demonstrates how to create a child span from the current context.
 * The span will automatically become a child of the currently active span.
 */
async function nestedFunction() {
  // Create a child span - it will automatically use the active span as parent
  return tracer.startActiveSpan('nested_function', (span) => {
    try {
      span.addEvent('Nested function called');
      // Your nested function logic here
      const result = 'success';
      if (Math.random() < 0.5) {
        throw new Error('test error');
      }
      return result;
    } finally {
      span.end();
    }
  });
}

// Create a traced handler with configuration
const handler = createTracedHandler(completionHandler, {
  name: 'simple-handler',
  attributesExtractor: apiGatewayV2Extractor
});

// Export the Lambda handler
exports.handler = handler(async (event, context, span) => {
  const requestId = context.awsRequestId;
  span.addEvent('handling request', {
    'request.id': requestId
  });

  // Add custom span attributes
  span.setAttribute('request.id', requestId);

  await nestedFunction();

  // Return a simple response
  return {
    statusCode: 200,
    body: JSON.stringify({
      message: `Hello from request ${requestId}`
    })
  };
});
