"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.LambdaSpanProcessor = void 0;
const api_1 = require("@opentelemetry/api");
const api_2 = require("@opentelemetry/api");
/**
 * A fixed-size circular buffer implementation that provides efficient FIFO operations.
 * The buffer maintains head and tail pointers that wrap around when reaching the end,
 * allowing for constant time insertions and batch retrievals while efficiently managing memory.
 */
class CircularBuffer {
    /**
     * Creates a new CircularBuffer with the specified capacity.
     * @param capacity The maximum number of items the buffer can hold
     * @throws {Error} If capacity is less than or equal to 0
     */
    constructor(capacity) {
        this.capacity = capacity;
        // Points to the next item to be removed (oldest item)
        this.head = 0;
        // Points to the next free slot for insertion
        this.tail = 0;
        // Current number of items in the buffer
        this._size = 0;
        if (capacity <= 0) {
            throw new Error('Buffer capacity must be greater than 0');
        }
        this.buffer = new Array(capacity);
    }
    /**
     * Returns the current number of items in the buffer.
     */
    get size() {
        return this._size;
    }
    /**
     * Attempts to add an item to the buffer.
     * @param item The item to add
     * @returns true if the item was added, false if the buffer is full
     */
    push(item) {
        if (this._size === this.capacity) {
            return false;
        }
        this.buffer[this.tail] = item;
        this.tail = (this.tail + 1) % this.capacity;
        this._size++;
        return true;
    }
    /**
     * Removes and returns up to maxSize items from the buffer.
     * This operation helps with batch processing while maintaining memory efficiency
     * by clearing references to processed items.
     *
     * @param maxSize Maximum number of items to remove
     * @returns Array of removed items
     */
    drainBatch(maxSize) {
        const batchSize = Math.min(maxSize, this._size);
        if (batchSize === 0)
            return [];
        const items = new Array(batchSize);
        for (let i = 0; i < batchSize; i++) {
            items[i] = this.buffer[this.head];
            this.buffer[this.head] = undefined; // Clear reference for GC
            this.head = (this.head + 1) % this.capacity;
            this._size--;
        }
        return items;
    }
    /**
     * Removes and returns all items currently in the buffer.
     * @returns Array of all items
     */
    drain() {
        return this.drainBatch(this._size);
    }
}
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
class LambdaSpanProcessor {
    /**
     * Creates a new LambdaSpanProcessor.
     *
     * @param exporter - The span exporter to use
     * @param config - Configuration options
     * @param config.maxQueueSize - Maximum number of spans that can be buffered (default: 2048)
     * @param config.maxExportBatchSize - Maximum number of spans to export in each batch (default: 512).
     *                                    Lower values reduce event loop blocking but may decrease throughput.
     */
    constructor(exporter, config) {
        this.exporter = exporter;
        this.isShutdown = false;
        this.droppedSpansCount = 0;
        const maxQueueSize = config?.maxQueueSize || 2048;
        this.maxExportBatchSize = config?.maxExportBatchSize || 64;
        this.buffer = new CircularBuffer(maxQueueSize);
    }
    /**
     * Forces a flush of all buffered spans.
     * This should be called before the Lambda function ends to ensure all spans are exported.
     *
     * @returns Promise that resolves when all spans have been exported
     */
    forceFlush() {
        if (this.isShutdown) {
            api_2.diag.warn('Cannot force flush - span processor is shutdown');
            return Promise.resolve();
        }
        return this.flush();
    }
    /**
     * Called when a span starts. Currently a no-op as we only process spans on end.
     */
    onStart(_span, _context) { }
    /**
     * Called when a span ends. The span is added to the buffer if it is sampled.
     * If the buffer is full, the span will be dropped and counted in droppedSpansCount.
     *
     * @param span - The span that has ended
     */
    onEnd(span) {
        if (this.isShutdown) {
            api_2.diag.warn('span processor is shutdown, dropping span');
            return;
        }
        // Skip unsampled spans
        if ((span.spanContext().traceFlags & api_1.TraceFlags.SAMPLED) === 0) {
            return;
        }
        try {
            this.addToBuffer(span);
        }
        catch (error) {
            api_2.diag.error('failed to queue span:', error);
        }
    }
    /**
     * Attempts to add a span to the buffer.
     * Tracks and logs dropped spans if the buffer is full.
     *
     * @param span - The span to add to the buffer
     */
    addToBuffer(span) {
        const added = this.buffer.push(span);
        if (!added) {
            this.droppedSpansCount++;
            if (this.droppedSpansCount === 1 || this.droppedSpansCount % 100 === 0) {
                api_2.diag.warn(`Dropping spans: ${this.droppedSpansCount} spans dropped because buffer is full`);
            }
            return;
        }
        if (this.droppedSpansCount > 0) {
            api_2.diag.warn(`Recovered from dropping spans: ${this.droppedSpansCount} spans were dropped`);
            this.droppedSpansCount = 0;
        }
    }
    async flush() {
        if (this.buffer.size === 0) {
            api_2.diag.debug('no spans to flush');
            return;
        }
        // Use a recursive pattern with setImmediate to ensure each batch processing
        // occurs in its own event loop tick. This prevents event loop blocking by:
        // 1. Processing one batch at a time
        // 2. Yielding to the event loop between batches via setImmediate
        // 3. Allowing other operations to interleave between batch processing
        return new Promise((resolve, reject) => {
            const processNextBatch = () => {
                // Get next batch using configured batch size
                const spansToExport = this.buffer.drainBatch(this.maxExportBatchSize);
                if (spansToExport.length === 0) {
                    resolve();
                    return;
                }
                // Export without additional Promise wrapping
                this.exporter.export(spansToExport, (result) => {
                    if (result.code === 0) {
                        // Schedule next batch in a new event loop tick
                        setImmediate(processNextBatch);
                    }
                    else {
                        reject(result.error || new Error('Failed to export spans'));
                    }
                });
            };
            // Start processing
            processNextBatch();
        });
    }
    /**
     * Shuts down the processor and flushes any remaining spans.
     * After shutdown, no new spans will be accepted.
     *
     * @returns Promise that resolves when shutdown is complete
     */
    async shutdown() {
        this.isShutdown = true;
        await this.flush();
        await this.exporter.shutdown();
    }
}
exports.LambdaSpanProcessor = LambdaSpanProcessor;
//# sourceMappingURL=processor.js.map