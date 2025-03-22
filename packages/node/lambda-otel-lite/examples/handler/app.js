/**
 * Simple Hello World Lambda function using lambda-otel-lite.
 * 
 * This example demonstrates basic OpenTelemetry setup with lambda-otel-lite.
 * It creates spans for each invocation and logs the event payload using span events.
 */

const { trace, SpanStatusCode } = require('@opentelemetry/api');
// Import initTelemetry and createTracedHandler from the main package
const { initTelemetry, createTracedHandler } = require('@dev7a/lambda-otel-lite');
// Import extractor from the dedicated subpath
const { apiGatewayV2Extractor } = require('@dev7a/lambda-otel-lite/extractors');

// Initialize telemetry once at module load
const { tracer, completionHandler } = initTelemetry();

/**
 * Simple nested function that creates its own span.
 * 
 * This function demonstrates how to create a child span from the current context.
 * The span will automatically become a child of the currently active span.
 */
async function nestedFunction(event) {
  // Create a child span - it will automatically use the active span as parent
  return tracer.startActiveSpan('nested_function', async (span) => {
    try {
      span.addEvent('Nested function called');
      
      if (event?.rawPath === '/error') {
        // simulate a random error
        const r = Math.random();
        if (r < 0.25) {
          throw new Error('expected error');
        } else if (r < 0.5) {
          throw new Error('unexpected error');
        }
      }
      return 'success';
    } finally {
      span.end();
    }
  });
}

// Create a traced handler with configuration
const traced = createTracedHandler(
  'simple-handler',
  completionHandler,
  apiGatewayV2Extractor 
);

/**
 * Lambda handler function.
 * 
 * This example shows how to:
 * 1. Use the traced decorator for automatic span creation
 * 2. Access the current span via OpenTelemetry API
 * 3. Create child spans for nested operations
 * 4. Add custom attributes and events
 */
exports.handler = traced(async (event, context) => {
  const currentSpan = trace.getActiveSpan();
  const requestId = context.awsRequestId;
  
  currentSpan?.setAttribute('request.id', requestId);
  currentSpan?.addEvent('handling request', {
    event: JSON.stringify(event)
  });

  try {
    await nestedFunction(event);
    // Return a simple response
    return {
      statusCode: 200,
      body: JSON.stringify({
        message: `Hello from request ${requestId}`
      })
    };
  } catch (error) {
    if (error.message === 'expected error') {
      currentSpan?.recordException(error);
      currentSpan?.setStatus({
        code: SpanStatusCode.ERROR,
        message: error.message
      });
      return {
        statusCode: 400,
        body: JSON.stringify({
          message: error.message
        })
      };
    }
    // Re-throw unexpected errors
    throw error;
  }
});
