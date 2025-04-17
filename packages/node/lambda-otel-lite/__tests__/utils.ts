import { SpanContext, SpanKind, TraceFlags } from '@opentelemetry/api';
import { ExportResultCode } from '@opentelemetry/core';
import { ReadableSpan, SpanExporter, Span } from '@opentelemetry/sdk-trace-base';
import { Resource } from '@opentelemetry/resources';

/**
 * Test implementation of SpanExporter for testing purposes
 */
export class TestSpanExporter implements SpanExporter {
  spans: ReadableSpan[] = [];
  exportCalledTimes = 0;
  shutdownOnce = false;
  exportResult: { code: ExportResultCode; error?: Error } = { code: ExportResultCode.SUCCESS };

  export(
    spans: ReadableSpan[],
    resultCallback: (result: { code: ExportResultCode; error?: Error }) => void
  ): void {
    this.exportCalledTimes++;
    if (this.exportResult.code === ExportResultCode.SUCCESS) {
      this.spans.push(...spans);
    }
    setTimeout(() => resultCallback(this.exportResult), 0);
  }

  shutdown(): Promise<void> {
    this.shutdownOnce = true;
    return Promise.resolve();
  }

  reset(): void {
    this.spans = [];
    this.exportCalledTimes = 0;
    this.shutdownOnce = false;
    this.exportResult = { code: ExportResultCode.SUCCESS };
  }
}

/**
 * Creates a test span for onStart method
 */
export const createSpan = (spanName = 'default'): Span => {
  return {
    name: spanName,
    kind: SpanKind.INTERNAL,
    spanContext: () => ({
      traceId: 'd4cda95b652f4a1592b449d5929fda1b',
      spanId: '6e0c63257de34c92',
      traceFlags: TraceFlags.SAMPLED,
    }),
    parentSpanId: undefined,
    startTime: [1566156729, 709],
    endTime: [1566156731, 709],
    ended: true,
    status: { code: 0 },
    attributes: {},
    links: [],
    events: [],
    duration: [32, 800000000],
    resource: new Resource({}),
    instrumentationLibrary: { name: 'default', version: '0.0.1' },
    _spanContext: {
      traceId: 'd4cda95b652f4a1592b449d5929fda1b',
      spanId: '6e0c63257de34c92',
      traceFlags: TraceFlags.SAMPLED,
    },
    _droppedAttributesCount: 0,
    _droppedEventsCount: 0,
    _droppedLinksCount: 0,
    setAttribute: () => ({}),
    setAttributes: () => ({}),
    addEvent: () => ({}),
    setStatus: () => ({}),
    updateName: () => ({}),
    end: () => ({}),
    isRecording: () => true,
    recordException: () => ({}),
  } as unknown as Span;
};

/**
 * Creates a test span with the given name and optional configuration
 */
export const createTestSpan = (spanName = 'default'): ReadableSpan => {
  return {
    name: spanName,
    kind: SpanKind.INTERNAL,
    spanContext(): SpanContext {
      return {
        traceId: 'd4cda95b652f4a1592b449d5929fda1b',
        spanId: '6e0c63257de34c92',
        traceFlags: TraceFlags.SAMPLED,
      };
    },
    parentSpanId: undefined,
    startTime: [1566156729, 709],
    endTime: [1566156731, 709],
    ended: true,
    status: { code: 0 },
    attributes: {},
    links: [],
    events: [],
    duration: [32, 800000000],
    resource: new Resource({}),
    instrumentationLibrary: { name: 'default', version: '0.0.1' },
    droppedAttributesCount: 0,
    droppedEventsCount: 0,
    droppedLinksCount: 0,
  };
};

/**
 * Helper to manage environment variables in tests
 */
export class EnvVarManager {
  private originalEnv: NodeJS.ProcessEnv;

  constructor() {
    // Create a deep copy of the environment, not just a reference
    this.originalEnv = { ...process.env };
  }

  setup(vars: Record<string, string | undefined> = {}) {
    // Start with a clean environment
    process.env = {};

    // Only add the variables we specifically want for this test
    Object.keys(vars).forEach((key) => {
      if (vars[key] !== undefined) {
        process.env[key] = vars[key];
      }
    });
  }

  restore() {
    // Restore to our saved copy
    process.env = { ...this.originalEnv };
  }
}
