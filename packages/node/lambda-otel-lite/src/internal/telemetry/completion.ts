import { Tracer } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { ProcessorMode } from '../../mode';
import { state } from '../state';
import logger from '../logger';

/**
 * Manages the lifecycle of span export based on the processing mode.
 * 
 * This handler is responsible for ensuring that spans are properly exported before
 * the Lambda function completes. It MUST be used to signal when spans should be exported.
 * 
 * The behavior varies by processing mode:
 * - Sync: Forces immediate export in the handler thread
 * - Async: Signals the extension to export after the response is sent
 * - Finalize: Defers to span processor (used with BatchSpanProcessor)
 * 
 * @example
 * ```typescript
 * // Initialize once, outside the handler
 * const completionHandler = initTelemetry('my-service');
 * 
 * // Basic usage with try/finally to ensure completion
 * export const handler = async (event, context) => {
 *   try {
 *     // Your handler code
 *     return response;
 *   } finally {
 *     // Always call complete() to ensure spans are exported
 *     completionHandler.complete();
 *   }
 * };
 * 
 * // Recommended: Use tracedHandler which handles completion automatically
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
 * @important
 * - Failing to call `complete()` may result in lost spans
 * - In sync mode, `complete()` blocks until spans are exported
 * - In async mode, spans are exported after the response is sent
 * - Multiple calls to `complete()` are safe but unnecessary
 * - The handler is designed to be reused across invocations
 */
export class TelemetryCompletionHandler {
  constructor(
    private readonly provider: NodeTracerProvider,
    private readonly mode: ProcessorMode
  ) { }

  /**
   * Complete telemetry processing for the current invocation.
   * 
   * This method must be called to ensure spans are exported. The behavior depends
   * on the processing mode:
   * 
   * - Sync mode: Blocks until spans are flushed. Any errors during flush are logged
   *   but do not affect the handler response.
   * 
   * - Async mode: Schedules span export via the extension after the response is sent.
   *   This is non-blocking and optimizes perceived latency.
   * 
   * - Finalize mode: No-op as export is handled by the span processor configuration
   *   (e.g., BatchSpanProcessor with custom export triggers).
   * 
   * Multiple calls to this method are safe but have no additional effect.
   * 
   * @example
   * ```typescript
   * // Always use try/finally when calling complete() directly
   * try {
   *   // Create and populate spans
   *   const span = tracer.startSpan('operation');
   *   // ... span operations ...
   *   span.end();
   * } finally {
   *   // Ensure spans are exported
   *   completionHandler.complete();
   * }
   * ```
   */
  complete(): void {
    switch (this.mode) {
      case ProcessorMode.Sync:
        if (this.provider.forceFlush) {
          this.provider.forceFlush()
            .catch(e => logger.warn('[completion] Error flushing telemetry:', e));
        }
        break;
      case ProcessorMode.Async:
      // In async mode, we want to ensure the Lambda runtime has a chance to send the response
      // before we signal completion and start flushing spans
        process.nextTick(() => {
          state.handlerComplete.signal();
        });
        break;
      case ProcessorMode.Finalize:
      // Do nothing, handled by drop
        break;
    }
  }

  /**
   * Get a tracer instance for creating spans.
   * 
   * Returns a tracer instance that can be used to create spans. The tracer is configured
   * with the provider's settings and will automatically use the correct span processor
   * based on the processing mode.
   * 
   * @param name - Name to identify the tracer. This should be descriptive and unique
   *               within your service (e.g., 'payment-processor', 'user-api').
   * @returns A tracer instance for creating spans
   * 
   * @example
   * ```typescript
   * const tracer = completionHandler.getTracer('payment-service');
   * const span = tracer.startSpan('process-payment');
   * try {
   *   // Process payment
   *   span.setStatus({ code: SpanStatusCode.OK });
   * } catch (error) {
   *   span.recordException(error);
   *   span.setStatus({ code: SpanStatusCode.ERROR });
   *   throw error;
   * } finally {
   *   span.end();
   * }
   * ```
   */
  getTracer(name: string): Tracer {
    return this.provider.getTracer(name);
  }
} 