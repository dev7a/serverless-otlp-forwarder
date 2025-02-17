/**
 * Simple Hello World Lambda function using lambda-otel-lite.
 * 
 * This example demonstrates basic OpenTelemetry setup with lambda-otel-lite.
 * It creates spans for each invocation and logs the event payload using span events.
 */

const { initTelemetry, createTracedHandler, apiGatewayV2Extractor } = require('@dev7a/lambda-otel-lite');
const { trace } = require('@opentelemetry/api');

// Initialize telemetry once at module load
const completionHandler = initTelemetry();

/**
 * Simple nested function that creates its own span.
 * 
 * This function is used to demonstrate the nested span functionality of OpenTelemetry.
 */
async function nestedFunction() {
  return trace.getTracer('hello-world').startActiveSpan('nested_function', async (span) => {
    span.addEvent('Nested function called');
    span.end();
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
