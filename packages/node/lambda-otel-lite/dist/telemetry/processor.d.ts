import { Span } from '@opentelemetry/sdk-trace-base';
import { Context } from '@opentelemetry/api';
import { SpanProcessor, ReadableSpan, SpanExporter } from '@opentelemetry/sdk-trace-base';
/**
 * Implementation of the SpanProcessor that batches spans exported by the SDK.
 * This processor is specifically designed for AWS Lambda environments, optimizing for:
 * 1. Memory efficiency using a circular buffer
 * 2. Non-blocking batch exports that yield to the event loop
 * 3. Configurable batch sizes to balance throughput and latency
 *
 * @example
 * ```typescript
 * const exporter = new OTLPExporter();
 * const processor = new LambdaSpanProcessor(exporter, {
 *   maxQueueSize: 2048,      // Maximum number of spans that can be buffered
 *   maxExportBatchSize: 512  // Maximum number of spans to export in each batch
 * });
 * ```
 */
export declare class LambdaSpanProcessor implements SpanProcessor {
    private readonly exporter;
    private readonly buffer;
    private isShutdown;
    private droppedSpansCount;
    private readonly maxExportBatchSize;
    /**
     * Creates a new LambdaSpanProcessor.
     *
     * @param exporter - The span exporter to use
     * @param config - Configuration options
     * @param config.maxQueueSize - Maximum number of spans that can be buffered (default: 2048)
     * @param config.maxExportBatchSize - Maximum number of spans to export in each batch (default: 512).
     *                                    Lower values reduce event loop blocking but may decrease throughput.
     */
    constructor(exporter: SpanExporter, config?: {
        maxQueueSize?: number;
        maxExportBatchSize?: number;
    });
    /**
     * Forces a flush of all buffered spans.
     * This should be called before the Lambda function ends to ensure all spans are exported.
     *
     * @returns Promise that resolves when all spans have been exported
     */
    forceFlush(): Promise<void>;
    /**
     * Called when a span starts. Currently a no-op as we only process spans on end.
     */
    onStart(_span: Span, _context: Context): void;
    /**
     * Called when a span ends. The span is added to the buffer if it is sampled.
     * If the buffer is full, the span will be dropped and counted in droppedSpansCount.
     *
     * @param span - The span that has ended
     */
    onEnd(span: ReadableSpan): void;
    /**
     * Attempts to add a span to the buffer.
     * Tracks and logs dropped spans if the buffer is full.
     *
     * @param span - The span to add to the buffer
     */
    private addToBuffer;
    private flush;
    /**
     * Shuts down the processor and flushes any remaining spans.
     * After shutdown, no new spans will be accepted.
     *
     * @returns Promise that resolves when shutdown is complete
     */
    shutdown(): Promise<void>;
}
//# sourceMappingURL=processor.d.ts.map