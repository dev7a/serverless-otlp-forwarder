import { Resource } from '@opentelemetry/resources';
import { SpanProcessor } from '@opentelemetry/sdk-trace-base';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { OTLPStdoutSpanExporter } from '@dev7a/otlp-stdout-span-exporter';
import { propagation, TextMapPropagator } from '@opentelemetry/api';
import { CompositePropagator } from '@opentelemetry/core';
import { state } from '../state';
import { LambdaSpanProcessor } from './processor';
import { TelemetryCompletionHandler } from './completion';
import { processorModeFromEnv, ProcessorMode } from '../../mode';
import { createLogger } from '../logger';
import { Tracer } from '@opentelemetry/api';
import { getLambdaResource } from './resource';

const logger = createLogger('init');

// Track cold start
let _isColdStart = true;

export function isColdStart(): boolean {
  return _isColdStart;
}

export function setColdStart(value: boolean): void {
  _isColdStart = value;
}

// Re-export getLambdaResource for backward compatibility
export { getLambdaResource } from './resource';

/**
 * Initializes OpenTelemetry telemetry for a Lambda function.
 *
 * This is the main entry point for setting up telemetry in your Lambda function.
 * It configures the OpenTelemetry SDK with Lambda-optimized defaults and returns
 * both a tracer for manual instrumentation and a completion handler that manages
 * the telemetry lifecycle.
 *
 * Features:
 * - Automatic Lambda resource detection (function name, version, memory, etc.)
 * - Environment-based configuration
 * - Custom span processor support
 * - Extension integration for async processing
 *
 * @param options - Optional configuration options
 * @param options.resource - Custom Resource to use instead of auto-detected resources.
 *                          Useful for adding custom attributes or overriding defaults.
 * @param options.spanProcessors - Array of SpanProcessor implementations to use.
 *                                If not provided, defaults to LambdaSpanProcessor
 *                                with OTLPStdoutSpanExporter.
 * @param options.propagators - Array of TextMapPropagator implementations to use.
 *                             If not provided, the default propagators from the SDK will be used
 *                             (W3C Trace Context and W3C Baggage).
 *
 * @returns Object containing:
 *   - tracer: Tracer instance for manual instrumentation
 *   - completionHandler: Handler for managing telemetry lifecycle
 *
 * @example
 * Basic usage:
 * ```typescript
 * const { tracer, completionHandler } = initTelemetry();
 *
 * // Use completionHandler with traced handler
 * export const handler = createTracedHandler(completionHandler, {
 *   name: 'my-handler'
 * }, async (event, context, span) => {
 *   // Add attributes to the handler's span
 *   span.setAttribute('request.id', context.awsRequestId);
 *
 *   // Create a nested span for a sub-operation
 *   return tracer.startActiveSpan('process_request', span => {
 *     span.setAttribute('some.attribute', 'some value');
 *     // ... do some work ...
 *     span.end();
 *     return { statusCode: 200, body: 'success' };
 *   });
 * });
 * ```
 *
 * Custom configuration with propagators:
 * ```typescript
 * import { W3CTraceContextPropagator } from '@opentelemetry/core';
 * import { B3Propagator } from '@opentelemetry/propagator-b3';
 *
 * const { tracer, completionHandler } = initTelemetry({
 *   propagators: [
 *     new W3CTraceContextPropagator(),
 *     new B3Propagator(),
 *   ]
 * });
 * ```
 *
 * Environment Variables:
 * - LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: Processing mode (sync, async, finalize)
 * - LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE: Maximum spans to queue (default: 2048)
 * - LAMBDA_SPAN_PROCESSOR_BATCH_SIZE: Maximum batch size (default: 512)
 * - OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL: GZIP level (default: 6)
 * - OTEL_SERVICE_NAME: Service name (defaults to function name)
 * - OTEL_RESOURCE_ATTRIBUTES: Additional resource attributes (key=value,...)
 *
 * @note
 * - Initialize once outside the handler for better performance
 * - Use with tracedHandler for automatic completion handling
 * - For async mode, ensure extension is loaded via NODE_OPTIONS
 * - Cold start is automatically tracked
 */
export function initTelemetry(options?: {
  resource?: Resource;
  spanProcessors?: SpanProcessor[];
  propagators?: TextMapPropagator[];
}): { tracer: Tracer; completionHandler: TelemetryCompletionHandler } {
  // Setup resource
  const baseResource = options?.resource || getLambdaResource();

  // Setup propagators if provided
  if (options?.propagators) {
    // Create a composite propagator and set it as the global propagator
    const compositePropagator = new CompositePropagator({
      propagators: options.propagators,
    });
    propagation.setGlobalPropagator(compositePropagator);
    logger.debug(
      `Set custom propagators: ${options.propagators.map((p) => p.constructor.name).join(', ')}`
    );
  }

  // Setup processors with environment variables taking precedence
  const processors = options?.spanProcessors || [
    new LambdaSpanProcessor(new OTLPStdoutSpanExporter(), {}),
  ];

  // Create provider with resources
  const provider = new NodeTracerProvider({
    resource: baseResource,
    spanProcessors: processors,
  });

  // Store in shared state for extension
  state.provider = provider;

  // Register as global tracer
  provider.register();

  // Get processor mode from environment
  let mode = processorModeFromEnv();

  // Check if we're in async mode but extension isn't loaded
  if (mode === ProcessorMode.Async && !state.extensionInitialized) {
    logger.warn(
      'Async processor mode requested but extension not loaded. ' +
        'Falling back to sync mode. To use async mode, set NODE_OPTIONS=--require @dev7a/lambda-otel-lite/extension'
    );
    mode = ProcessorMode.Sync;
  }

  // Store mode in shared state
  state.mode = mode;

  // Create completion handler and return both handler and tracer
  const completionHandler = new TelemetryCompletionHandler(provider, mode);
  return {
    tracer: completionHandler.getTracer(),
    completionHandler,
  };
}
