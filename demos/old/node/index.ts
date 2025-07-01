import { initTelemetry, createTracedHandler, TelemetryCompletionHandler } from '@dev7a/lambda-otel-lite';
import { defaultExtractor, TriggerType, SpanAttributes } from '@dev7a/lambda-otel-lite/extractors';
import { trace, SpanStatusCode, Tracer, Span } from '@opentelemetry/api';
import { registerInstrumentations } from '@opentelemetry/instrumentation';
import { HttpInstrumentation } from '@opentelemetry/instrumentation-http';
import { AwsInstrumentation } from '@opentelemetry/instrumentation-aws-sdk';
import type { Context, ScheduledEvent } from 'aws-lambda';
import type { AxiosResponse } from 'axios';
import { spanEvent, Level } from './spanEvent';

// Type Definitions

interface QuoteResponse {
  id: number;
  quote: string;
  author: string;
}

// Define the structure returned by the Lambda handler
interface HandlerResponse {
  statusCode: number;
  body: string;
}

// Telemetry Initialization

// Initialize telemetry with default configuration
const { tracer, completionHandler }: { tracer: Tracer, completionHandler: TelemetryCompletionHandler } = initTelemetry();

// Register instrumentations
registerInstrumentations({
  tracerProvider: trace.getTracerProvider(),
  instrumentations: [
    new AwsInstrumentation(),
    new HttpInstrumentation()
  ]
});

// import after registerInstrumentations
const axios = require('axios');
const { SQSClient, SendMessageCommand } = require('@aws-sdk/client-sqs');

// AWS SDK and Other Clients
const sqs = new SQSClient({});

// Helper Functions

/**
 * Fetches a random quote from dummyjson API within a trace span.
 */
async function getRandomQuote(): Promise<QuoteResponse> {
  return tracer.startActiveSpan('get-random-quote', async (span: Span) => {
    try {
      const response: AxiosResponse<QuoteResponse> = await axios.get('https://dummyjson.com/quotes/random');
      spanEvent(
        'demo.generator.fetched-quote',
        `Fetched quote ID ${response.data.id} from external API`,
        Level.INFO,
        {
          'demo.quote.text': response.data.quote,
          'demo.quote.author': response.data.author,
          'demo.quote.id': response.data.id
        },
        span
      );
      return response.data;
    } catch (error: unknown) {
      span.recordException(error as Error); // Cast to Error for recordException
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Unknown error fetching quote') });
      throw error;
    } finally {
      span.end();
    }
  });
}

/**
 * Sends a quote object to SQS within a trace span.
 */
async function sendQuote(quote: QuoteResponse): Promise<void> {
  return tracer.startActiveSpan('send-quote', async (span: Span) => {
    try {
      const queueUrl = process.env.QUOTES_QUEUE_URL;
      if (!queueUrl) {
        throw new Error('QUOTES_QUEUE_URL environment variable not set.');
      }

      const command = new SendMessageCommand({
        QueueUrl: queueUrl,
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
      spanEvent(
        'demo.generator.sent-quote',
        `Quote ${quote.id} sent to SQS queue`,
        Level.INFO,
        { 'demo.quote.id': quote.id },
        span
      );
    } catch (error: unknown) {
      span.recordException(error as Error);
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Unknown error sending quote') });
      throw error;
    } finally {
      span.end();
    }
  });
}

/**
 * Processes a single quote (fetches and sends).
 */
async function processQuote(quoteNumber: number, totalQuotes: number): Promise<QuoteResponse> {
  // You might want to use quoteNumber and totalQuotes in spans/events if needed
  const quote = await getRandomQuote();
  await sendQuote(quote);
  return quote;
}

/**
 * Processes a batch of quotes by calling processQuote multiple times.
 */
async function processBatch(batchSize: number): Promise<QuoteResponse[]> {
  const quotes: QuoteResponse[] = [];
  // Consider creating a parent span for the batch processing if desired
  // tracer.startActiveSpan('process-batch', async (batchSpan) => { ... });
  for (let i = 0; i < batchSize; i++) {
    const quote = await processQuote(i + 1, batchSize);
    quotes.push(quote);
  }
  return quotes;
}

// Traced Handler Configuration

// Custom extractor function for Timer/Scheduled events
const timerExtractor = (event: ScheduledEvent, context: Context): SpanAttributes => {
  // Use the default extractor to get common attributes
  const baseAttributes = defaultExtractor(event, context);

  // Customize for Timer trigger
  return {
    ...baseAttributes, // Spread common attributes (like service name, function name, region, etc.)
    trigger: TriggerType.Timer,
    spanName: 'generate-quotes', // More specific span name for this handler
    attributes: {
      ...baseAttributes.attributes, // Spread attributes from defaultExtractor
      // Add timer-specific attributes
      'aws.cloudwatch.event.id': event.id,
      'aws.cloudwatch.event.time': event.time,
      'aws.cloudwatch.event.resources': event.resources.join(','), // Example: Join array for attribute
      'schedule.source': event.source, // e.g., 'aws.events'
      // Assuming a fixed schedule for this example, otherwise extract dynamically if possible
      'schedule.period': '5m'
    },
    carrier: event as any

  };
};


// Create the traced handler using the custom extractor
const traced = createTracedHandler(
  'quote-generator-scheduled-job', // Operation name (can be different from spanName)
  completionHandler,
  timerExtractor
);

// Lambda Handler

export const handler = traced(async (
  event: ScheduledEvent,
  context: Context
): Promise<HandlerResponse | void> => {
  console.log(JSON.stringify(event));
  const currentSpan = trace.getActiveSpan();
  spanEvent(
    'demo.generator.started-processing',
    `Started processing CloudWatch scheduled event ID: ${event.id}`,
    Level.INFO,
    { 
      'invocation.start_time': new Date().toISOString(),
      'aws.cloudwatch.event.id': event.id
    }
  );

  try {
    // Determine batch size (example: random)
    const batchSize = Math.floor(Math.random() * 10) + 1;
    currentSpan?.setAttribute('batch.size', batchSize);

    // Process the batch
    const quotes = await processBatch(batchSize);

    spanEvent(
      'demo.generator.processed-batch',
      `Successfully processed and sent ${batchSize} quotes`,
      Level.INFO,
      {
        'demo.quotes.batch_size': batchSize,
        'processed.quote_ids': quotes.map(q => q.id).join(',')
      }
    );

    // Return success response
    return {
      statusCode: 200,
      body: JSON.stringify({
        message: `Retrieved and sent ${batchSize} random quotes to SQS`,
        quotes: quotes.map(q => q.id) // Return only IDs for brevity
      })
    };
  } catch (error: unknown) {
    currentSpan?.recordException(error as Error);
    currentSpan?.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Handler failed') });
    throw error; // Propagate error for Lambda runtime
  }
  // Note: The completionHandler ensures telemetry is flushed even if errors occur.
}); 