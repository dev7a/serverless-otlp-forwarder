const { NodeTracerProvider } = require('@opentelemetry/sdk-trace-node');
const { BatchSpanProcessor } = require('@opentelemetry/sdk-trace-base');
const { Resource } = require('@opentelemetry/resources');
const { trace, SpanKind, SpanStatusCode, context, propagation } = require('@opentelemetry/api');
const { StdoutOTLPExporterNode } = require('@dev7a/otlp-stdout-exporter');
const { AwsLambdaDetectorSync } = require('@opentelemetry/resource-detector-aws');
const { W3CTraceContextPropagator } = require('@opentelemetry/core');

// Configure axios with OpenTelemetry
const { registerInstrumentations } = require('@opentelemetry/instrumentation');
const { HttpInstrumentation } = require('@opentelemetry/instrumentation-http');

registerInstrumentations({
  instrumentations: [
    new HttpInstrumentation(),
  ],
});

// finally import axios
const axios = require('axios');

const QUOTES_URL = 'https://dummyjson.com/quotes/random';
const TARGET_URL = process.env.TARGET_URL;

// Set up W3C Trace Context propagator
propagation.setGlobalPropagator(new W3CTraceContextPropagator());

const createProvider = () => {
  const awsResource = new AwsLambdaDetectorSync().detect();
  
  const resource = new Resource({
    ["service.name"]: process.env.OTEL_SERVICE_NAME || process.env.AWS_LAMBDA_FUNCTION_NAME || 'demo-function',
    ["faas.name"]: process.env.AWS_LAMBDA_FUNCTION_NAME,
  }).merge(awsResource);

  const provider = new NodeTracerProvider({
    resource
  });

  // Configure the stdout exporter with env based configuration
  const exporter = new StdoutOTLPExporterNode();

  // Add the exporter to the provider with batch processing
  provider.addSpanProcessor(new BatchSpanProcessor(exporter));

  return provider;
};

const provider = createProvider();
provider.register();
const tracer = trace.getTracer('demo-function');

// Helper function to get random quote
async function getRandomQuote() {
  // Get the parent context
  const parentContext = context.active();
  
  const span = tracer.startSpan('getRandomQuote', parentContext);  // Pass the parent context

  // Set the context with the new span
  return await context.with(trace.setSpan(parentContext, span), async () => {
    try {
      const response = await axios.get(QUOTES_URL);
      return response.data;
    } catch (e) {
      span.recordException(e);
      span.setStatus({
        code: SpanStatusCode.ERROR,
        message: e.message
      });
      throw e;
    } finally {
      span.end();
    }
  });
}

// Helper function to save quote
async function saveQuote(quote) {
  const parentContext = context.active();
  
  const span = tracer.startSpan('saveQuote', parentContext);

  return await context.with(trace.setSpan(parentContext, span), async () => {
    try {
      const headers = {
        'content-type': 'application/json',
      };
      
      // Inject trace context into headers
      propagation.inject(context.active(), headers);
      
      const response = await axios.post(TARGET_URL, quote, { headers });
      return response.data;
    } catch (error) {
      span.recordException(error);
      span.setStatus({ code: 1 });
      throw error;
    } finally {
      span.end();
    }
  });
}

exports.handler = async (event, lambdaContext) => {
  const parentSpan = tracer.startSpan('lambda-invocation', {
    kind: SpanKind.SERVER,
    attributes: {
      'faas.trigger': 'timer'
    }
  });

  // Set the root span as active
  return await context.with(trace.setSpan(context.active(), parentSpan), async () => {
    try {
      parentSpan.addEvent('Lambda Invocation Started');

      // Get random quote
      const quote = await getRandomQuote();
      parentSpan.addEvent('Random Quote Fetched', {
        attributes: {
          severity: 'info',
          quote: quote.quote
        }
      });

      // Save the quote
      const response = await saveQuote(quote);
      parentSpan.addEvent('Quote Saved', {
        attributes: {
          severity: 'info',
          quote: quote.quote
        }
      });

      parentSpan.addEvent('Lambda Execution Completed');

      return {
        statusCode: 200,
        body: JSON.stringify({
          message: 'Hello from demo Lambda!',
          input: event,
          quote: quote,
          response: response
        })
      };
    } catch (error) {
      parentSpan.recordException(error);
      parentSpan.setStatus({ code: 1 });
      throw error;
    } finally {
      parentSpan.end();
      await provider.forceFlush();
    }
  });
};
