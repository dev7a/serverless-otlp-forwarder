import { Span } from '@opentelemetry/sdk-trace-base';
import { Context, TraceFlags } from '@opentelemetry/api';
import { SpanProcessor, ReadableSpan, SpanExporter } from '@opentelemetry/sdk-trace-base';
import { ExportResult } from '@opentelemetry/core';
import { createLogger } from '../logger';
import { ENV_VARS, DEFAULTS } from '../../constants';

const logger = createLogger('processor');

/**
 * A fixed-size circular buffer implementation that provides efficient FIFO operations.
 * The buffer maintains head and tail pointers that wrap around when reaching the end,
 * allowing for constant time insertions and batch retrievals while efficiently managing memory.
 */
class CircularBuffer<T> {
  // The underlying array storage
  private buffer: Array<T | undefined>;
  // Points to the next item to be removed (oldest item)
  private head = 0;
  // Points to the next free slot for insertion
  private tail = 0;
  // Current number of items in the buffer
  private _size = 0;

  /**
   * Creates a new CircularBuffer with the specified capacity.
   * @param capacity The maximum number of items the buffer can hold
   * @throws {Error} If capacity is less than or equal to 0
   */
  constructor(private readonly capacity: number) {
    if (capacity <= 0) {
      throw new Error('Buffer capacity must be greater than 0');
    }
    this.buffer = new Array(capacity);
  }

  /**
   * Returns the current number of items in the buffer.
   */
  get size(): number {
    return this._size;
  }

  /**
   * Attempts to add an item to the buffer.
   * @param item The item to add
   * @returns true if the item was added, false if the buffer is full
   */
  push(item: T): boolean {
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
  drainBatch(maxSize: number): T[] {
    const batchSize = Math.min(maxSize, this._size);
    if (batchSize === 0) {
      return [];
    }

    const items: T[] = new Array(batchSize);
    for (let i = 0; i < batchSize; i++) {
      items[i] = this.buffer[this.head]!;
      this.buffer[this.head] = undefined;
      this.head = (this.head + 1) % this.capacity;
      this._size--;
    }
    return items;
  }

  /**
   * Removes and returns all items currently in the buffer.
   * @returns Array of all items
   */
  drain(): T[] {
    return this.drainBatch(this._size);
  }
}

/**
 * Configuration options for the LambdaSpanProcessor.
 */
export interface LambdaSpanProcessorConfig {
  /**
   * Maximum number of spans that can be buffered (default: 2048).
   * Environment variable LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE takes precedence if set.
   */
  maxQueueSize?: number;

  /**
   * Maximum number of spans to export in each batch (default: 512).
   * Lower values reduce event loop blocking but may decrease throughput.
   * Environment variable LAMBDA_SPAN_PROCESSOR_BATCH_SIZE takes precedence if set.
   */
  maxExportBatchSize?: number;
}

/**
 * Implementation of the SpanProcessor that batches spans exported by the SDK.
 * This processor is specifically designed for AWS Lambda environments, optimizing for:
 * 1. Memory efficiency using a circular buffer
 * 2. Non-blocking batch exports that yield to the event loop
 * 3. Configurable batch sizes to balance throughput and latency
 *
 * Configuration Precedence:
 * 1. Environment variables (highest precedence)
 * 2. Constructor parameters in config object
 * 3. Default values (lowest precedence)
 *
 * Environment Variables:
 * - LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE: Maximum spans to queue (default: 2048)
 * - LAMBDA_SPAN_PROCESSOR_BATCH_SIZE: Maximum batch size (default: 512)
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
export class LambdaSpanProcessor implements SpanProcessor {
  private readonly buffer: CircularBuffer<ReadableSpan>;
  private isShutdown = false;
  private droppedSpansCount = 0;
  private readonly maxExportBatchSize: number;

  /**
   * Creates a new LambdaSpanProcessor.
   *
   * @param exporter - The span exporter to use
   * @param config - Configuration options
   * @param config.maxQueueSize - Maximum number of spans that can be buffered.
   *                              Environment variable LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE takes precedence if set.
   *                              Defaults to 2048 if neither environment variable nor parameter is provided.
   * @param config.maxExportBatchSize - Maximum number of spans to export in each batch.
   *                                    Environment variable LAMBDA_SPAN_PROCESSOR_BATCH_SIZE takes precedence if set.
   *                                    Defaults to 512 if neither environment variable nor parameter is provided.
   */
  constructor(
    private readonly exporter: SpanExporter,
    config?: LambdaSpanProcessorConfig
  ) {
    // Set queue size with proper precedence
    const configQueueSize = config?.maxQueueSize;
    const envQueueSize = process.env[ENV_VARS.QUEUE_SIZE];
    let maxQueueSize: number;

    if (envQueueSize !== undefined) {
      try {
        const parsedQueueSize = parseInt(envQueueSize, 10);
        if (!isNaN(parsedQueueSize) && parsedQueueSize > 0) {
          maxQueueSize = parsedQueueSize;
        } else {
          logger.warn(`Invalid value in ${ENV_VARS.QUEUE_SIZE}: ${envQueueSize}, using fallback`);
          maxQueueSize = configQueueSize !== undefined ? configQueueSize : DEFAULTS.QUEUE_SIZE;
        }
      } catch {
        // Empty catch block - no need to use the error variable
        logger.warn(`Failed to parse ${ENV_VARS.QUEUE_SIZE}: ${envQueueSize}, using fallback`);
        maxQueueSize = configQueueSize !== undefined ? configQueueSize : DEFAULTS.QUEUE_SIZE;
      }
    } else {
      // No environment variable, use parameter from config or default
      maxQueueSize = configQueueSize !== undefined ? configQueueSize : DEFAULTS.QUEUE_SIZE;
    }

    // Set batch size with proper precedence
    const configBatchSize = config?.maxExportBatchSize;
    const envBatchSize = process.env[ENV_VARS.BATCH_SIZE];

    if (envBatchSize !== undefined) {
      try {
        const parsedBatchSize = parseInt(envBatchSize, 10);
        if (!isNaN(parsedBatchSize) && parsedBatchSize > 0) {
          this.maxExportBatchSize = parsedBatchSize;
        } else {
          logger.warn(`Invalid value in ${ENV_VARS.BATCH_SIZE}: ${envBatchSize}, using fallback`);
          this.maxExportBatchSize =
            configBatchSize !== undefined ? configBatchSize : DEFAULTS.BATCH_SIZE;
        }
      } catch {
        // Empty catch block - no need to use the error variable
        logger.warn(`Failed to parse ${ENV_VARS.BATCH_SIZE}: ${envBatchSize}, using fallback`);
        this.maxExportBatchSize =
          configBatchSize !== undefined ? configBatchSize : DEFAULTS.BATCH_SIZE;
      }
    } else {
      // No environment variable, use parameter from config or default
      this.maxExportBatchSize =
        configBatchSize !== undefined ? configBatchSize : DEFAULTS.BATCH_SIZE;
    }

    // Initialize the buffer with the determined queue size
    this.buffer = new CircularBuffer<ReadableSpan>(maxQueueSize);
  }

  /**
   * Forces a flush of all buffered spans.
   * This should be called before the Lambda function ends to ensure all spans are exported.
   *
   * @returns Promise that resolves when all spans have been exported
   */
  forceFlush(): Promise<void> {
    if (this.isShutdown) {
      logger.warn('Cannot force flush - span processor is shutdown');
      return Promise.resolve();
    }
    return this.flush();
  }

  /**
   * Called when a span starts. Currently a no-op as we only process spans on end.
   */
  onStart(_span: Span, _context: Context): void {}

  /**
   * Called when a span ends. The span is added to the buffer if it is sampled.
   * If the buffer is full, the span will be dropped and counted in droppedSpansCount.
   *
   * @param span - The span that has ended
   */
  onEnd(span: ReadableSpan): void {
    if (this.isShutdown) {
      logger.warn('span processor is shutdown, dropping span');
      return;
    }

    // Skip unsampled spans
    if ((span.spanContext().traceFlags & TraceFlags.SAMPLED) === 0) {
      return;
    }

    try {
      this.addToBuffer(span);
    } catch (error) {
      logger.error('failed to queue span:', error);
    }
  }

  /**
   * Attempts to add a span to the buffer.
   * Tracks and logs dropped spans if the buffer is full.
   *
   * @param span - The span to add to the buffer
   */
  private addToBuffer(span: ReadableSpan): void {
    const added = this.buffer.push(span);
    if (!added) {
      this.droppedSpansCount++;
      if (this.droppedSpansCount === 1 || this.droppedSpansCount % 100 === 0) {
        logger.warn(
          `Dropping spans: ${this.droppedSpansCount} spans dropped because buffer is full`
        );
      }
      return;
    }

    if (this.droppedSpansCount > 0) {
      logger.warn(`Recovered from dropping spans: ${this.droppedSpansCount} spans were dropped`);
      this.droppedSpansCount = 0;
    }
  }

  private async flush(): Promise<void> {
    if (this.buffer.size === 0) {
      logger.debug('no spans to flush');
      return;
    }

    // Use a recursive pattern with setImmediate to ensure each batch processing
    // occurs in its own event loop tick. This prevents event loop blocking by:
    // 1. Processing one batch at a time
    // 2. Yielding to the event loop between batches via setImmediate
    // 3. Allowing other operations to interleave between batch processing
    logger.debug(`flushing ${this.buffer.size} spans`);
    return new Promise<void>((resolve, reject) => {
      const processNextBatch = () => {
        // Get next batch using configured batch size
        const spansToExport = this.buffer.drainBatch(this.maxExportBatchSize);

        if (spansToExport.length === 0) {
          resolve();
          return;
        }

        // Export without additional Promise wrapping
        this.exporter.export(spansToExport, (result: ExportResult) => {
          if (result.code === 0) {
            // Schedule next batch in a new event loop tick
            setImmediate(processNextBatch);
          } else {
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
  async shutdown(): Promise<void> {
    this.isShutdown = true;
    await this.flush();
    await this.exporter.shutdown();
  }
}
