import { SpanKind, SpanStatusCode, ROOT_CONTEXT, propagation } from '@opentelemetry/api';
import { isColdStart, setColdStart } from './internal/telemetry/init';
import { TelemetryCompletionHandler } from './internal/telemetry/completion';
import { SpanAttributes, defaultExtractor } from './internal/telemetry/extractors';
import logger from './internal/logger';
import type { Context as AwsLambdaContext } from 'aws-lambda'; // Import standard Context

/**
 * A function that extracts attributes from a Lambda event and context.
 *
 * @template TEvent The event type for the Lambda function
 * @param event The Lambda event
 * @param context The Lambda context
 * @returns The extracted span attributes
 */
export type AttributesExtractor<TEvent = any> = (
  event: TEvent,
  context: AwsLambdaContext
) => SpanAttributes;

/**
 * A Lambda handler function type using the standard Context
 */
export type TracedFunction<TEvent, TResult> = (
  event: TEvent,
  context: AwsLambdaContext
) => Promise<TResult>;
/**
 * Creates a traced handler for AWS Lambda functions with automatic attribute extraction
 * and context propagation.
 *
 * @template TEvent The event type for the Lambda function
 * @template TResult The result type for the Lambda function
 * @param name The name of the handler
 * @param completionHandler The telemetry completion handler
 * @param attributesExtractor Optional extractor for attributes from the event
 * @returns A function that wraps a Lambda handler function
 */
export function createTracedHandler<TEvent = any, TResult = any>(
  name: string,
  completionHandler: TelemetryCompletionHandler,
  attributesExtractor?: AttributesExtractor<TEvent>
) {
  // Return a function that wraps the user's handler
  return function (fn: TracedFunction<TEvent, TResult>) {
    return async function (event: TEvent, context: AwsLambdaContext): Promise<TResult> {
      const tracer = completionHandler.getTracer();

      try {
        // Use the provided extractor or the default one
        const extracted = attributesExtractor
          ? attributesExtractor(event, context)
          : defaultExtractor(event as unknown, context);

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
              if (
                result &&
                typeof result === 'object' &&
                !Array.isArray(result) &&
                result !== null &&
                'statusCode' in result
              ) {
                const statusCode = result.statusCode as number;
                span.setAttribute('http.status_code', statusCode);
                if (statusCode >= 500) {
                  span.setStatus({
                    code: SpanStatusCode.ERROR,
                    message: `HTTP ${statusCode} response`,
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
                message: e instanceof Error ? e.message : String(e),
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
