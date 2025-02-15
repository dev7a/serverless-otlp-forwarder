import { TraceFlags, context } from '@opentelemetry/api';
import { LambdaSpanProcessor } from '../src/internal/telemetry/processor';
import { TestSpanExporter, createTestSpan, createSpan, EnvVarManager } from './utils';
import { describe, it, beforeEach, afterEach, expect } from '@jest/globals';
import { ExportResultCode } from '@opentelemetry/core';

describe('LambdaSpanProcessor', () => {
  let processor: LambdaSpanProcessor;
  let exporter: TestSpanExporter;
  let envManager: EnvVarManager;

  beforeEach(() => {
    envManager = new EnvVarManager();
    envManager.setup({ AWS_LAMBDA_FUNCTION_NAME: 'test-function' });
    exporter = new TestSpanExporter();
    processor = new LambdaSpanProcessor(exporter);
  });

  afterEach(() => {
    envManager.restore();
    exporter.reset();
  });

  it('should create processor with default config', () => {
    expect(processor).toBeDefined();
  });

  it('should not export unsampled spans', () => {
    const span = {
      ...createTestSpan(),
      spanContext: () => ({
        traceId: 'd4cda95b652f4a1592b449d5929fda1b',
        spanId: '6e0c63257de34c92',
        traceFlags: TraceFlags.NONE
      })
    };
    
    processor.onStart(createSpan(), context.active());
    processor.onEnd(span);
    expect(exporter.exportCalledTimes).toBe(0);
    expect(exporter.spans.length).toBe(0);
  });

  it('should export single span when in Lambda context', async () => {
    const span = createTestSpan();
    processor.onStart(createSpan(), context.active());
    processor.onEnd(span);

    // Wait for export
    await processor.forceFlush();
    expect(exporter.exportCalledTimes).toBe(1);
    expect(exporter.spans.length).toBe(1);
    expect(exporter.spans[0].name).toBe(span.name);
  });

  it('should force flush all spans', async () => {
    const numSpans = 10;
    for (let i = 0; i < numSpans; i++) {
      const span = createTestSpan(`span-${i}`);
      processor.onStart(createSpan(), context.active());
      processor.onEnd(span);
    }

    await processor.forceFlush();
    expect(exporter.exportCalledTimes).toBeGreaterThan(0);
    expect(exporter.spans.length).toBe(numSpans);
  });

  it('should shutdown and flush remaining spans', async () => {
    const span = createTestSpan();
    processor.onStart(createSpan(), context.active());
    processor.onEnd(span);

    await processor.shutdown();
    expect(exporter.shutdownOnce).toBe(true);
    expect(exporter.spans.length).toBe(1);
  });

  it('should drop spans when maxQueueSize is reached', async () => {
    const processor = new LambdaSpanProcessor(exporter, { maxQueueSize: 1 });
    
    // Add two spans - second one should be dropped
    const span1 = createTestSpan('span-1');
    const span2 = createTestSpan('span-2');
    
    processor.onStart(createSpan(), context.active());
    processor.onEnd(span1);
    processor.onEnd(span2);

    // Wait for export
    await processor.forceFlush();
    expect(exporter.spans.length).toBe(1);
    expect(exporter.spans[0].name).toBe('span-1');
  });

  it('should handle multiple shutdown calls gracefully', async () => {
    const span = createTestSpan();
    processor.onStart(createSpan(), context.active());
    processor.onEnd(span);

    await processor.shutdown();
    expect(exporter.shutdownOnce).toBe(true);
    expect(exporter.spans.length).toBe(1);

    // Second shutdown should be no-op
    await processor.shutdown();
    expect(exporter.shutdownOnce).toBe(true); // Should not change
    expect(exporter.spans.length).toBe(1); // Should not change
  });

  it('should not accept spans after shutdown', async () => {
    await processor.shutdown();
    
    const span = createTestSpan();
    processor.onStart(createSpan(), context.active());
    processor.onEnd(span);

    expect(exporter.spans.length).toBe(0);
  });

  it('should handle export failures gracefully', async () => {
    // Mock export failure
    const failingExporter = new TestSpanExporter();
    failingExporter.exportResult = { code: ExportResultCode.FAILED };
    
    const processor = new LambdaSpanProcessor(failingExporter);
    const span = createTestSpan();
    
    processor.onStart(createSpan(), context.active());
    processor.onEnd(span);

    // Export should fail but not throw
    await expect(processor.forceFlush()).rejects.toThrow('Failed to export spans');
    expect(failingExporter.exportCalledTimes).toBe(1);
    expect(failingExporter.spans.length).toBe(0); // No spans should be stored on failure
  });

  it('should handle concurrent span additions', async () => {
    const processor = new LambdaSpanProcessor(exporter, { maxQueueSize: 100 });
    const numSpans = 50;
    
    // Simulate concurrent span additions
    await Promise.all(Array.from({ length: numSpans }).map(async (_, i) => {
      const span = createTestSpan(`span-${i}`);
      processor.onStart(createSpan(), context.active());
      processor.onEnd(span);
    }));

    await processor.forceFlush();
    expect(exporter.spans.length).toBe(numSpans);
    
    // Verify span order is maintained
    const spanNames = exporter.spans.map(s => s.name);
    expect(spanNames).toEqual(Array.from({ length: numSpans }).map((_, i) => `span-${i}`));
  });

  it('should maintain span attributes during export', async () => {
    const span = createTestSpan();
    const testAttributes = {
      'test.key': 'test-value',
      'test.number': 123,
      'test.bool': true
    };
    
    Object.entries(testAttributes).forEach(([key, value]) => {
      span.attributes[key] = value;
    });

    processor.onStart(createSpan(), context.active());
    processor.onEnd(span);

    await processor.forceFlush();
    expect(exporter.spans.length).toBe(1);
    expect(exporter.spans[0].attributes).toEqual(testAttributes);
  });
}); 