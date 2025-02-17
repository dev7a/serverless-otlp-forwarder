import { SpanKind, SpanStatusCode, Span, ROOT_CONTEXT, propagation } from '@opentelemetry/api';
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
 * Configuration for creating a traced Lambda handler
 */
export interface TracerConfig {
  /** Name of the span (used if extractor does not provide a name) */
  name: string;
  /** Optional attribute extractor function */
  attributesExtractor?: (event: unknown, context: LambdaContext) => SpanAttributes;
}

/**
 * A Lambda handler function that receives a span for manual instrumentation
 */
export type TracedFunction<TEvent, TResult> = (
  event: TEvent,
  context: LambdaContext,
  span: Span
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
 * const handler = createTracedHandler(completionHandler, {
 *   name: 'my-handler',
 *   attributesExtractor: apiGatewayV2Extractor
 * });
 * 
 * // Use the traced handler to process Lambda events
 * export const lambdaHandler = handler(async (event, context, span) => {
 *   // Add custom attributes or create child spans
 *   span.setAttribute('custom.attribute', 'value');
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
  completionHandler: TelemetryCompletionHandler,
  config: TracerConfig
) {
  return function <TEvent, TResult>(fn: TracedFunction<TEvent, TResult>) {
    return async function (event: TEvent, context: LambdaContext): Promise<TResult> {
      const tracer = completionHandler.getTracer();
      let error: Error | undefined;
      let result: TResult;

      try {
        const wrapCallback = (fn: (span: Span) => Promise<TResult>) => {
          return async (span: Span) => {
            try {
              // Extract attributes using provided extractor or default
              const extracted = (config.attributesExtractor || defaultExtractor)(event, context);

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
              result = await fn(span);

              // Handle HTTP response attributes
              if (result && typeof result === 'object' && 'statusCode' in result) {
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
              } else {
                span.setStatus({ code: SpanStatusCode.OK });
              }
              
              return result;
            } catch (e) {
              error = e as Error;
              // Record the error with full exception details
              span.recordException(error);
              // Set explicit error attribute
              span.setAttribute('error', true);
              // Set status to ERROR
              span.setStatus({
                code: SpanStatusCode.ERROR,
                message: error.message
              });
              throw error;
            } finally {
              span.end();
              if (isColdStart()) {
                setColdStart(false);
              }
            }
          };
        };

        // Extract attributes and context
        const extracted = (config.attributesExtractor || defaultExtractor)(event, context);
            
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
        result = await tracer.startActiveSpan(
          extracted.spanName || config.name,
          {
            kind: extracted.kind || SpanKind.SERVER,
            links: extracted.links,
          },
          parentContext,
          wrapCallback((span) => fn(event, context, span))
        );
        logger.debug('returning result');
        return result;
      } catch (e) {
        error = e as Error;
        throw error;
      } finally {
        completionHandler.complete();
      }
    };
  };
}

