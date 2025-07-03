import { initTelemetry, createTracedHandler, TelemetryCompletionHandler } from '@dev7a/lambda-otel-lite';
import { defaultExtractor, TriggerType, SpanAttributes } from '@dev7a/lambda-otel-lite';
import { trace, SpanStatusCode, Tracer, Span } from '@opentelemetry/api';
import { registerInstrumentations } from '@opentelemetry/instrumentation';
import { HttpInstrumentation } from '@opentelemetry/instrumentation-http';
import { AwsInstrumentation } from '@opentelemetry/instrumentation-aws-sdk';
import type { Context, ScheduledEvent } from 'aws-lambda';
import type { AxiosResponse } from 'axios';
import { spanEvent, Level } from './spanEvent';

// Type Definitions

interface TodoResponse {
  id: number;
  todo: string;
  completed: boolean;
  userId: number;
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
// Get the queue URL from the environment variable
const queueUrl = process.env.TODOS_QUEUE_URL;

// Helper Functions

/**
 * Fetches random TODOs from dummyjson API within a trace span.
 */
async function getRandomTodos(count: number = 5): Promise<TodoResponse[]> {
  return tracer.startActiveSpan('producer/fetcher/fetch', async (span: Span) => {
    try {
      // We'll fetch multiple random TODOs in a single call
      const response: AxiosResponse<TodoResponse[]> = await axios.get(`https://dummyjson.com/todos/random/${count}`);
      const todos: TodoResponse[] = response.data;

      spanEvent(
        'todo-fetcher.fetched-todos-batch',
        `Fetched ${todos.length} TODOs from external API in a batch`,
        Level.INFO,
        {
          'todo-fetcher.todos.count_requested': count,
          'todo-fetcher.todos.count_received': todos.length,
          'todo-fetcher.todos.ids': todos.map(todo => todo.id).join(',')
        },
        span
      );
      
      return todos;
    } catch (error: unknown) {
      span.recordException(error as Error);
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Unknown error fetching TODOs') });
      throw error;
    } finally {
      span.end();
    }
  });
}


/**
 * Sends a TODO object to SQS within a trace span.
 */
async function sendTodo(todo: TodoResponse): Promise<void> {
  return tracer.startActiveSpan('producer/fetcher/send', async (span: Span) => {
    try {
      const command = new SendMessageCommand({
        QueueUrl: queueUrl,
        MessageBody: JSON.stringify(todo),
        MessageAttributes: {
          'todo_id': {
            DataType: 'String',
            StringValue: todo.id.toString()
          }
        }
      });
      
      // Tracing is added by the instrumentation library
      await sqs.send(command);
      
      spanEvent(
        'demo.fetcher.sent-todo',
        `TODO ${todo.id} sent to SQS queue`,
        Level.INFO,
        { 
          'demo.todo.id': todo.id,
        },
        span
      );
    } catch (error: unknown) {
      span.recordException(error as Error);
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Unknown error sending TODO') });
      throw error;
    } finally {
      span.end();
    }
  });
}

/**
 * Processes a single TODO (fetches and sends).
 */
async function processTodo(todo: TodoResponse, todoNumber: number, totalTodos: number): Promise<TodoResponse> {

  return tracer.startActiveSpan('producer/fetcher/process', async (span: Span) => {
    try {
      // Add processing metadata to the current span
      span.setAttribute('todo.id', todo.id);
      span.setAttribute('todo.number', todoNumber);
      span.setAttribute('todo.total', totalTodos);

      await sendTodo(todo);
      return todo;
    } catch (error: unknown) {
      span.recordException(error as Error);
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Unknown error processing TODO') });
      throw error;
    } finally {
      span.end();
    }
  });
}

/**
 * Processes a batch of TODOs.
 */
async function processBatch(batchSize: number): Promise<TodoResponse[]> {
  return tracer.startActiveSpan('producer/fetcher/batch', async (span: Span) => {
    try {
      const todos = await getRandomTodos(batchSize);
      const processedTodos: TodoResponse[] = [];
      
      for (let i = 0; i < todos.length; i++) {
        const processedTodo = await processTodo(todos[i], i + 1, todos.length);
        processedTodos.push(processedTodo);
      }

      return processedTodos;
    } catch (error: unknown) {
      span.recordException(error as Error);
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Unknown error processing batch') });
      throw error;
    } finally {
      span.end();
    }
  });
}

// Custom extractor function for Timer/Scheduled events
const timerExtractor = (event: ScheduledEvent, context: Context): SpanAttributes => {
  // Use the default extractor to get common attributes
  const baseAttributes = defaultExtractor(event, context);

  // Customize for Timer trigger
  return {
    ...baseAttributes,
    trigger: TriggerType.Timer,
    spanName: 'producer/fetcher/schedule',
    attributes: {
      ...baseAttributes.attributes,
      'aws.cloudwatch.event.id': event.id,
      'aws.cloudwatch.event.time': event.time,
      'aws.cloudwatch.event.resources': event.resources.join(','),
      'schedule.source': event.source,
    },
    carrier: event as any
  };
};

// Create the traced handler using the custom extractor
const traced = createTracedHandler(
  'todo-fetcher-scheduled-job',
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
    'demo.fetcher.started-processing',
    `Started processing CloudWatch scheduled event ID: ${event.id}`,
    Level.INFO,
    { 
      'invocation.start_time': new Date().toISOString(),
      'aws.cloudwatch.event.id': event.id
    }
  );

  try {
    // Determine batch size (random between 3-7)
    const batchSize = Math.floor(Math.random() * 5) + 3;
    currentSpan?.setAttribute('batch.size', batchSize);

    // Process the batch
    const todos = await processBatch(batchSize);

    spanEvent(
      'demo.fetcher.processed-batch',
      `Successfully processed and sent ${batchSize} TODOs`,
      Level.INFO,
      {
        'demo.todos.batch_size': batchSize,
        'processed.todo_ids': todos.map(t => t.id).join(',')
      }
    );

    // Return success response
    return {
      statusCode: 200,
      body: JSON.stringify({
        message: `Retrieved and sent ${batchSize} random TODOs to SQS`,
        todos: todos.map(t => ({
          id: t.id,
          title: t.todo.substring(0, 30) + (t.todo.length > 30 ? '...' : ''),
        }))
      })
    };
  } catch (error: unknown) {
    currentSpan?.recordException(error as Error);
    currentSpan?.setStatus({ code: SpanStatusCode.ERROR, message: (error instanceof Error ? error.message : 'Handler failed') });
    throw error;
  }
});
