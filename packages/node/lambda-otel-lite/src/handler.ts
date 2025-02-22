import { SpanKind, SpanStatusCode, ROOT_CONTEXT, propagation } from '@opentelemetry/api';
import { isColdStart, setColdStart } from './internal/telemetry/init';
import { TelemetryCompletionHandler } from './internal/telemetry/completion';
import { SpanAttributes, defaultExtractor } from './internal/telemetry/extractors';
import logger from './internal/logger';

/**
 * AWS Lambda context object
 */
export interface LambdaContext {
  awsRequestId: string;
  invokedFunctionArn: string;
  functionName: string;
  functionVersion: string;
  memoryLimitInMB: string;
  getRemainingTimeInMillis: () => number;
}

/**
 * Optional configuration for traced handler
 */
export interface TracerConfig {
  /** Optional attribute extractor function */
  attributesExtractor?: (event: unknown, context: LambdaContext) => SpanAttributes;
}

/**
 * A Lambda handler function type
 */
export type TracedFunction<TEvent, TResult> = (
  event: TEvent,
  context: LambdaContext
) => Promise<TResult>;

/**
 * Creates a traced handler for AWS Lambda functions with automatic attribute extraction
 * and context propagation.
 * 
 * @example
 * ```typescript
 * const completionHandler = initTelemetry();
 * 
 * // Create a traced handler with a name and optional attribute extractor
 * const traced = createTracedHandler(
 *   'my-handler',
 *   completionHandler,
 *   { attributesExtractor: apiGatewayV2Extractor }
 * );
 * 
 * // Use the traced handler to process Lambda events
 * export const lambdaHandler = traced(async (event, context) => {
 *   // Get current span if needed
 *   const currentSpan = trace.getActiveSpan();
 *   currentSpan?.setAttribute('custom.attribute', 'value');
 *   
 *   // Your handler logic here
 *   return {
 *     statusCode: 200,
 *     body: JSON.stringify({ message: 'Hello World' })
 *   };
 * });
 * ```
 */
export function createTracedHandler(
  name: string,
  completionHandler: TelemetryCompletionHandler,
  config?: TracerConfig
) {
  return function <TEvent, TResult>(fn: TracedFunction<TEvent, TResult>) {
    return async function (event: TEvent, context: LambdaContext): Promise<TResult> {
      const tracer = completionHandler.getTracer();

      try {
        // Extract attributes using provided extractor or default
        const extracted = (config?.attributesExtractor || defaultExtractor)(event, context);

        // Extract context from carrier if available
        let parentContext = ROOT_CONTEXT;
        if (extracted.carrier && Object.keys(extracted.carrier).length > 0) {
          try {
            parentContext = propagation.extract(ROOT_CONTEXT, extracted.carrier);
          } catch (error) {
            logger.warn('Failed to extract context:', error);
          }
        }

        // Start the span
        return await tracer.startActiveSpan(
          extracted.spanName || name,
          {
            kind: extracted.kind || SpanKind.SERVER,
            links: extracted.links,
          },
          parentContext,
          async (span) => {
            try {
              // Set common attributes
              if (isColdStart()) {
                span.setAttribute('faas.coldstart', true);
              }

              // Set Lambda context attributes
              if (context) {
                span.setAttribute('faas.invocation_id', context.awsRequestId);
                span.setAttribute('cloud.resource_id', context.invokedFunctionArn);
                            
                const arnParts = context.invokedFunctionArn.split(':');
                if (arnParts.length >= 5) {
                  span.setAttribute('cloud.account.id', arnParts[4]);
                }
              }

              // Set trigger type
              span.setAttribute('faas.trigger', extracted.trigger || 'other');

              // Set extracted attributes
              Object.entries(extracted.attributes).forEach(([key, value]) => {
                span.setAttribute(key, value);
              });

              // Execute handler
              const result = await fn(event, context);

              // Handle HTTP response attributes
              if (result && typeof result === 'object' && !Array.isArray(result) && result !== null && 'statusCode' in result) {
                const statusCode = result.statusCode as number;
                span.setAttribute('http.status_code', statusCode);
                if (statusCode >= 500) {
                  span.setStatus({
                    code: SpanStatusCode.ERROR,
                    message: `HTTP ${statusCode} response`
                  });
                } else {
                  span.setStatus({ code: SpanStatusCode.OK });
                }
              }
              
              return result;
            } catch (e) {
              // Record the error with full exception details
              span.recordException(e as Error);
              // Set explicit error attribute
              span.setAttribute('error', true);
              // Set status to ERROR
              span.setStatus({
                code: SpanStatusCode.ERROR,
                message: e instanceof Error ? e.message : String(e)
              });
              throw e;
            } finally {
              if (isColdStart()) {
                setColdStart(false);
              }
              // always end the span
              span.end();
            }
          }
        );
      } finally {
        completionHandler.complete();
      }
    };
  };
}

