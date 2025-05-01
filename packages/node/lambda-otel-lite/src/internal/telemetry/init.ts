import { Resource } from '@opentelemetry/resources';
import { SpanProcessor, IdGenerator } from '@opentelemetry/sdk-trace-base';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { OTLPStdoutSpanExporter } from '@dev7a/otlp-stdout-span-exporter';
import { propagation, TextMapPropagator } from '@opentelemetry/api';
import { CompositePropagator } from '@opentelemetry/core';
import { state } from '../state';
import { LambdaSpanProcessor } from './processor';
import { TelemetryCompletionHandler } from './completion';
import { resolveProcessorMode, ProcessorMode } from '../../mode';
import { createLogger } from '../logger';
import { Tracer } from '@opentelemetry/api';
import { getLambdaResource } from './resource';
import { setupPropagator } from '../propagation';

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
 * @param options.idGenerator - Optional ID generator to use for creating trace and span IDs.
 *                            If not provided, the default random ID generator will be used.
 *                            Use an AWS X-Ray compatible ID generator for X-Ray integration.
 * @param options.processorMode - Optional processor mode to control how spans are processed and exported.
 *                              Environment variable LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE takes precedence if set.
 *                              If neither environment variable nor this option is set, defaults to 'sync'.
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
 * Custom configuration with X-Ray ID generator:
 * ```typescript
 * import { AWSXRayIdGenerator } from '@opentelemetry/id-generator-aws-xray';
 *
 * const { tracer, completionHandler } = initTelemetry({
 *   idGenerator: new AWSXRayIdGenerator(),
 *   processorMode: ProcessorMode.Async
 * });
 * ```
 *
 * Environment Variables:
 * - LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: Processing mode (sync, async, finalize)
 * - LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE: Maximum spans to queue (default: 2048)
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
  idGenerator?: IdGenerator;
  processorMode?: ProcessorMode;
}): { tracer: Tracer; completionHandler: TelemetryCompletionHandler } {
  // Setup resource
  const baseResource = options?.resource || getLambdaResource();

  // Setup propagators
  if (options?.propagators) {
    // If custom propagators are provided, use them
    const compositePropagator = new CompositePropagator({
      propagators: options.propagators,
    });
    propagation.setGlobalPropagator(compositePropagator);
    logger.debug(
      `Set custom propagators: ${options.propagators.map((p) => p.constructor.name).join(', ')}`
    );
  } else {
    // Otherwise, use the default propagator setup based on environment variables
    setupPropagator();
  }

  // Setup processors with environment variables taking precedence
  const processors = options?.spanProcessors || [
    new LambdaSpanProcessor(new OTLPStdoutSpanExporter(), {}),
  ];

  // Create provider with resources
  const provider = new NodeTracerProvider({
    resource: baseResource,
    spanProcessors: processors,
    idGenerator: options?.idGenerator,
  });

  // Store in shared state for extension
  state.provider = provider;

  // Register as global tracer
  provider.register();
  // Resolve processor mode with proper precedence: env var > config > default
  let mode = resolveProcessorMode(options?.processorMode);

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
