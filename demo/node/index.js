const { initTelemetry, createTracedHandler } = require('@dev7a/lambda-otel-lite');
const { defaultExtractor, TriggerType } = require('@dev7a/lambda-otel-lite/extractors');
const { trace, SpanStatusCode } = require('@opentelemetry/api');
const { registerInstrumentations } = require('@opentelemetry/instrumentation');
const { HttpInstrumentation } = require('@opentelemetry/instrumentation-http');
const { AwsInstrumentation } = require('@opentelemetry/instrumentation-aws-sdk');

// Initialize telemetry with default configuration
// The service name will be automatically set from OTEL_SERVICE_NAME 
// or AWS_LAMBDA_FUNCTION_NAME environment variables
const { tracer, completionHandler } = initTelemetry();

// Register instrumentations
registerInstrumentations({
  tracerProvider: trace.getTracerProvider(),
  instrumentations: [
    new AwsInstrumentation(),
    new HttpInstrumentation()
  ]
});

const axios = require('axios');
const { SQSClient, SendMessageCommand } = require('@aws-sdk/client-sqs');
const sqs = new SQSClient();

const QUOTES_URL = 'https://dummyjson.com/quotes/random';
const QUEUE_URL = process.env.QUOTES_QUEUE_URL;

// Helper function to get random quote from dummyjson
async function getRandomQuote() {
  return tracer.startActiveSpan('getRandomQuote', async (span) => {
    try {
      const response = await axios.get(QUOTES_URL);
      return response.data;
    } catch (error) {
      span.recordException(error);
      span.setStatus({ code: SpanStatusCode.ERROR });
      throw error;
    } finally {
      span.end();
    }
  });
}

// Helper function to send quote to SQS
async function sendQuote(quote) {
  return tracer.startActiveSpan('sendQuote', async (span) => {
    try {
      const command = new SendMessageCommand({
        QueueUrl: QUEUE_URL,
        MessageBody: JSON.stringify(quote),
        MessageAttributes: {
          'quote_id': {
            DataType: 'String',
            StringValue: quote.id.toString()
          },
          'author': {
            DataType: 'String',
            StringValue: quote.author
          }
        }
      });
      await sqs.send(command);
    } catch (error) {
      span.recordException(error);
      span.setStatus({ code: SpanStatusCode.ERROR });
      throw error;
    } finally {
      span.end();
    }
  });
}

// Process a single quote
async function processQuote(quoteNumber, totalQuotes) {
  const activeSpan = trace.getActiveSpan();
  const quote = await getRandomQuote();
  
  activeSpan?.addEvent('Random Quote Fetched', {
    'log.severity': 'info',
    'log.message': `Successfully fetched random quote ${quoteNumber}/${totalQuotes}`,
    'quote.text': quote.quote,
    'quote.author': quote.author
  });

  await sendQuote(quote);
  
  activeSpan?.addEvent('Quote Sent', {
    'log.severity': 'info',
    'log.message': `Quote ${quoteNumber}/${totalQuotes} sent to SQS`,
    'quote.id': quote.id
  });

  return quote;
}

// Process a batch of quotes
async function processBatch(batchSize) {
  const quotes = [];
  
  for (let i = 0; i < batchSize; i++) {
    const quote = await processQuote(i + 1, batchSize);
    quotes.push(quote);
  }
  
  return quotes;
}

// Create the traced handler with timer trigger type
const traced = createTracedHandler(
  'quote-generator',
  completionHandler,
  (event, context) => {
    const baseAttributes = defaultExtractor(event, context);
    return {
      ...baseAttributes,
      trigger: TriggerType.Timer,
      spanName: 'generate-quotes',
      attributes: {
        ...baseAttributes.attributes,
        'schedule.period': '5m'
      }
    };
  }
);

// Lambda handler
exports.handler = traced(async (event, context) => {
  // Get current span to add custom attributes
  const currentSpan = trace.getActiveSpan();
  
  currentSpan?.addEvent('Lambda Invocation Started', {
    'log.severity': 'info',
    'log.message': 'Lambda function invocation started'
  });

  try {
    const batchSize = Math.floor(Math.random() * 10) + 1;
    currentSpan?.setAttribute('batch.size', batchSize);
    
    const quotes = await processBatch(batchSize);

    currentSpan?.addEvent('Batch Processing Completed', {
      'log.severity': 'info',
      'log.message': `Successfully processed batch of ${batchSize} quotes`
    });

    return {
      statusCode: 200,
      body: JSON.stringify({
        message: `Retrieved and sent ${batchSize} random quotes to SQS`,
        input: event,
        quotes
      })
    };
  } catch (error) {
    currentSpan?.recordException(error);
    currentSpan?.setStatus({ code: SpanStatusCode.ERROR });
    throw error;
  }
});
