import { Resource } from '@opentelemetry/resources';
import { SpanProcessor } from '@opentelemetry/sdk-trace-base';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { OTLPStdoutSpanExporter } from '@dev7a/otlp-stdout-span-exporter';
import { state } from '../state';
import { LambdaSpanProcessor } from './processor';
import { TelemetryCompletionHandler } from './completion';
import { processorModeFromEnv, ProcessorMode } from '../../mode';
import { createLogger } from '../logger';

const logger = createLogger('init');

// Track cold start
let _isColdStart = true;

export function isColdStart(): boolean {
  return _isColdStart;
}

export function setColdStart(value: boolean): void {
  _isColdStart = value;
}

/**
 * Create a Resource instance with AWS Lambda attributes and OTEL environment variables.
 * 
 * This function combines AWS Lambda environment attributes with any OTEL resource attributes
 * specified via environment variables (OTEL_RESOURCE_ATTRIBUTES and OTEL_SERVICE_NAME).
 * 
 * @returns Resource instance with AWS Lambda and OTEL environment attributes
 */
function getLambdaResource(): Resource {
  // Start with Lambda attributes
  const attributes: Record<string, string> = {
    'cloud.provider': 'aws'
  };

  // Map environment variables to attribute names
  const envMappings: Record<string, string> = {
    AWS_REGION: 'cloud.region',
    AWS_LAMBDA_FUNCTION_NAME: 'faas.name',
    AWS_LAMBDA_FUNCTION_VERSION: 'faas.version',
    AWS_LAMBDA_LOG_STREAM_NAME: 'faas.instance',
    AWS_LAMBDA_FUNCTION_MEMORY_SIZE: 'faas.max_memory'
  };

  // Add attributes only if they exist in environment
  for (const [envVar, attrName] of Object.entries(envMappings)) {
    const value = process.env[envVar];
    if (value) {
      attributes[attrName] = value;
    }
  }

  // Add service name (guaranteed to have a value)
  const serviceName = process.env.OTEL_SERVICE_NAME || process.env.AWS_LAMBDA_FUNCTION_NAME || 'unknown_service';
  attributes['service.name'] = serviceName;

  // Add OTEL environment resource attributes if present
  const envResourcesItems = process.env.OTEL_RESOURCE_ATTRIBUTES;
  if (envResourcesItems) {
    for (const item of envResourcesItems.split(',')) {
      try {
        const [key, value] = item.split('=', 2);
        if (value?.trim()) {
          const valueUrlDecoded = decodeURIComponent(value.trim());
          attributes[key.trim()] = valueUrlDecoded;
        }
      } catch {
        // Skip malformed items
        continue;
      }
    }
  }

  // Create resource and merge with default resource
  const resource = new Resource(attributes);
  return Resource.default().merge(resource);
}

/**
 * Initializes OpenTelemetry telemetry for a Lambda function.
 * 
 * This is the main entry point for setting up telemetry in your Lambda function.
 * It configures the OpenTelemetry SDK with Lambda-optimized defaults and returns
 * a completion handler that manages the telemetry lifecycle.
 * 
 * Features:
 * - Automatic Lambda resource detection (function name, version, memory, etc.)
 * - Environment-based configuration
 * - Custom span processor support
 * - Extension integration for async processing
 * 
 * @param name - Name of the tracer to create. This should be your service name
 *               or a descriptive identifier (e.g., 'payment-service', 'user-api').
 * 
 * @param options - Optional configuration options
 * @param options.resource - Custom Resource to use instead of auto-detected resources.
 *                          Useful for adding custom attributes or overriding defaults.
 * @param options.spanProcessors - Array of SpanProcessor implementations to use.
 *                                If not provided, defaults to LambdaSpanProcessor
 *                                with OTLPStdoutSpanExporter.
 * 
 * @returns TelemetryCompletionHandler instance for managing telemetry lifecycle
 * 
 * @throws Error if name is not provided
 * 
 * @example
 * Basic usage:
 * ```typescript
 * const completionHandler = initTelemetry('my-service');
 * 
 * export const handler = async (event, context) => {
 *   return tracedHandler({
 *     completionHandler,
 *     name: 'my-handler'
 *   }, event, context, async (span) => {
 *     // Your handler code
 *     return response;
 *   });
 * };
 * ```
 * 
 * Custom configuration:
 * ```typescript
 * const resource = new Resource({
 *   'service.version': '1.0.0',
 *   'deployment.environment': 'production'
 * });
 * 
 * const processor = new BatchSpanProcessor(
 *   new OTLPTraceExporter({
 *     url: 'https://your-collector:4318/v1/traces'
 *   })
 * );
 * 
 * const completionHandler = initTelemetry('my-service', {
 *   resource,
 *   spanProcessors: [processor]
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
export function initTelemetry(
  name: string,
  options?: {
    resource?: Resource,
    spanProcessors?: SpanProcessor[]
  }
): TelemetryCompletionHandler {
  if (!name) {
    throw new Error('Tracer name must be provided to initTelemetry');
  }

  // Setup resource
  const baseResource = options?.resource || getLambdaResource();

  // Setup processors with compression level support
  const processors = options?.spanProcessors || [
    new LambdaSpanProcessor(
      new OTLPStdoutSpanExporter({
        gzipLevel: parseInt(process.env.OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL || '6', 10)
      }),
      { maxQueueSize: parseInt(process.env.LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE || '2048', 10), maxExportBatchSize: parseInt(process.env.LAMBDA_SPAN_PROCESSOR_BATCH_SIZE || '512', 10) }
    )
  ];

  // Create provider with resources
  const provider = new NodeTracerProvider({
    resource: baseResource,
    spanProcessors: processors
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

  // Return completion handler
  return new TelemetryCompletionHandler(provider, mode);
} 