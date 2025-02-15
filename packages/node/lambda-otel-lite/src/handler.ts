import { SpanKind, SpanStatusCode, Span, ROOT_CONTEXT, propagation } from '@opentelemetry/api';
import { isColdStart, setColdStart } from './internal/telemetry/init';
import { TelemetryCompletionHandler } from './internal/telemetry/completion';
import { SpanAttributes, defaultExtractor } from './internal/telemetry/extractors';
import logger from './internal/logger';

/** Lambda Context interface */
export interface LambdaContext {
  awsRequestId: string;
  invokedFunctionArn: string;
  [key: string]: unknown;
}

/**
 * Options for the traced handler function
 */
export interface TracedHandlerOptions {
    /** Completion handler from initTelemetry */
    completionHandler: TelemetryCompletionHandler;
    /** Default name of the span (used if extractor does not provide a name) */
    name: string;
    /** Optional attribute extractor function */
    attributesExtractor?: (event: unknown, context: LambdaContext) => SpanAttributes;
}

/**
 * Creates a traced handler for AWS Lambda functions with automatic attribute extraction
 * and context propagation.
 * 
 * Features:
 * - Automatic cold start detection
 * - Lambda context attribute extraction (invocation ID, cloud resource ID, account ID)
 * - Event-specific attribute extraction via extractors
 * - Automatic context propagation from headers
 * - HTTP response status code handling
 * - Error handling and recording
 * 
 * @example
 * Basic usage:
 * ```typescript
 * import { initTelemetry, tracedHandler } from '@dev7a/lambda-otel-lite';
 * import { apiGatewayV2Extractor } from  '@dev7a/lambda-otel-lite/telemetry/extractors';
 * 
 * // Initialize telemetry (do this once, outside the handler)
 * const completionHandler = initTelemetry('my-service');
 *
 * export const handler = async (event: any, context: any) => {
 *   return tracedHandler({
 *     completionHandler: completionHandler,
 *     name: 'my-handler',
 *     attributesExtractor: apiGatewayV2Extractor
 *   }, event, context, async (span) => {
 *     // Your handler code here
 *     return {
 *       statusCode: 200,
 *       body: 'Success'
 *     };
 *   });
 * };
 * ```
 * 
 * @template T The return type of the handler function
 * @param options Configuration options for the traced handler
 * @param event Lambda event object
 * @param context Lambda context object
 * @param fn Handler function that receives the span instance
 * @returns The result of the handler function
 */
export async function tracedHandler<T>(
  options: TracedHandlerOptions,
  event: unknown,
  context: LambdaContext,
  fn: (span: Span) => Promise<T>
): Promise<T> {
  const tracer = options.completionHandler.getTracer(options.name);
  let error: Error | undefined;
  let result: T;

  try {
    const wrapCallback = (fn: (span: Span) => Promise<T>) => {
      return async (span: Span) => {
        try {
          // Extract attributes using provided extractor or default
          const extracted = (options.attributesExtractor || defaultExtractor)(event, context);

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
          span.recordException(error);
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
    const extracted = (options.attributesExtractor || defaultExtractor)(event, context);
        
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
      extracted.spanName || options.name,
      {
        kind: extracted.kind || SpanKind.SERVER,
        links: extracted.links,
      },
      parentContext,
      wrapCallback(fn)
    );
    logger.debug('returning result');
    return result;
  } catch (e) {
    error = e as Error;
    throw error;
  } finally {
    options.completionHandler.complete();
  }
}

