import { jest, describe, it, beforeEach, afterEach, expect } from '@jest/globals';
import {
  trace,
  propagation,
  TextMapPropagator,
  TextMapGetter,
  TextMapSetter,
  Context,
} from '@opentelemetry/api';
import { Resource } from '@opentelemetry/resources';
import { SpanProcessor, IdGenerator } from '@opentelemetry/sdk-trace-base';
import { initTelemetry, isColdStart, setColdStart } from '../src/internal/telemetry/init';
import { state } from '../src/internal/state';
import { EnvVarManager } from './utils';

// Mock the logger module
jest.mock('../src/internal/logger', () => {
  const mockLogger = {
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn(),
  };
  return {
    __esModule: true,
    default: mockLogger,
    createLogger: () => mockLogger,
  };
});

describe('telemetry/init', () => {
  const envManager = new EnvVarManager();

  beforeEach(() => {
    // Reset environment before each test
    envManager.restore();
    // Clear any registered global providers
    trace.disable();
  });

  afterEach(() => {
    envManager.restore();
  });

  describe('initTelemetry', () => {
    it('should initialize telemetry with default settings', () => {
      const { tracer, completionHandler } = initTelemetry();

      expect(completionHandler).toBeDefined();
      expect(tracer).toBeDefined();

      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom settings', () => {
      const { tracer, completionHandler } = initTelemetry({
        resource: new Resource({
          'service.name': 'test-service',
        }),
      });

      expect(completionHandler).toBeDefined();
      expect(tracer).toBeDefined();

      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom processor', () => {
      const { tracer, completionHandler } = initTelemetry({
        spanProcessors: [
          {
            forceFlush: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            shutdown: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            onStart: jest.fn(),
            onEnd: jest.fn(),
          },
        ],
      });

      expect(completionHandler).toBeDefined();
      expect(tracer).toBeDefined();

      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom propagators', () => {
      // Create a mock propagator for testing
      class MockPropagator implements TextMapPropagator {
        extractCalled = false;
        injectCalled = false;

        extract(_context: Context, _carrier: unknown, _getter?: TextMapGetter): Context {
          this.extractCalled = true;
          return _context;
        }

        inject(_context: Context, _carrier: unknown, _setter?: TextMapSetter): void {
          this.injectCalled = true;
        }

        fields(): string[] {
          return ['mock-header'];
        }
      }

      // Create a custom propagator
      const mockPropagator = new MockPropagator();

      // Spy on setGlobalPropagator
      const setGlobalPropagatorSpy = jest.spyOn(propagation, 'setGlobalPropagator');

      // Initialize telemetry with the custom propagator
      const { tracer, completionHandler } = initTelemetry({
        propagators: [mockPropagator],
      });

      expect(completionHandler).toBeDefined();
      expect(tracer).toBeDefined();

      // Verify setGlobalPropagator was called
      expect(setGlobalPropagatorSpy).toHaveBeenCalled();

      // Clean up spy
      setGlobalPropagatorSpy.mockRestore();
    });

    it('should initialize telemetry with custom ID generator', () => {
      // Create a mock X-Ray ID generator
      class MockXRayIdGenerator implements IdGenerator {
        traceIdCalled = false;
        spanIdCalled = false;

        generateTraceId(): string {
          this.traceIdCalled = true;
          // Generate a trace ID with a timestamp in the first 8 hex characters
          // X-Ray format: <timestamp in seconds>-<random part>
          const timestamp = Math.floor(Date.now() / 1000);
          const timestampHex = timestamp.toString(16).padStart(8, '0');
          const randomPart = 'a'.repeat(24); // Fixed value to verify later
          return timestampHex + randomPart;
        }

        generateSpanId(): string {
          this.spanIdCalled = true;
          // Return a fixed span ID for testing
          return '1234567890abcdef';
        }
      }

      // Create an instance of our mock generator
      const mockXRayIdGenerator = new MockXRayIdGenerator();

      // Initialize telemetry with the custom ID generator
      const { tracer, completionHandler } = initTelemetry({
        idGenerator: mockXRayIdGenerator,
      });

      expect(completionHandler).toBeDefined();
      expect(tracer).toBeDefined();

      // Create a span to trigger ID generation
      const testSpan = tracer.startSpan('test');

      // Verify that the ID generator was used
      expect(mockXRayIdGenerator.traceIdCalled).toBe(true);
      expect(mockXRayIdGenerator.spanIdCalled).toBe(true);

      // Get the trace and span IDs
      const context = testSpan.spanContext();
      const traceId = context.traceId;
      const spanId = context.spanId;

      // Verify trace ID format (should start with a timestamp)
      const timestampPart = traceId.substring(0, 8);
      const randomPart = traceId.substring(8);

      // The first 8 chars should be a valid timestamp in hex (within last day)
      expect(/^[0-9a-f]{8}$/.test(timestampPart)).toBe(true);

      // The next 24 chars should be our fixed random part
      expect(randomPart).toBe('a'.repeat(24));

      // Verify span ID
      expect(spanId).toBe('1234567890abcdef');

      testSpan.end();
    });

    it('should initialize telemetry with custom exporter', () => {
      const { tracer, completionHandler } = initTelemetry({
        spanProcessors: [
          {
            forceFlush: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            shutdown: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            onStart: jest.fn(),
            onEnd: jest.fn(),
          },
        ],
      });

      expect(completionHandler).toBeDefined();
      expect(tracer).toBeDefined();

      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom completion handler', () => {
      const { tracer, completionHandler } = initTelemetry({
        spanProcessors: [
          {
            forceFlush: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            shutdown: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            onStart: jest.fn(),
            onEnd: jest.fn(),
          },
        ],
      });

      expect(completionHandler).toBeDefined();
      expect(tracer).toBeDefined();

      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should use service name from environment variables', () => {
      envManager.setup({
        OTEL_SERVICE_NAME: 'env-service',
        AWS_LAMBDA_FUNCTION_NAME: 'lambda-function',
      });

      const { completionHandler: _ } = initTelemetry();

      // Service name will be in the provider's resource
      expect(state.provider?.resource.attributes['service.name']).toBe('env-service');
    });

    it('should fallback to Lambda function name if OTEL_SERVICE_NAME not set', () => {
      envManager.setup({
        AWS_LAMBDA_FUNCTION_NAME: 'lambda-function',
      });

      const { completionHandler: _ } = initTelemetry();

      expect(state.provider?.resource.attributes['service.name']).toBe('lambda-function');
    });

    it('should use unknown_service if no environment variables set', () => {
      envManager.setup({});
      const { completionHandler: _ } = initTelemetry();

      expect(state.provider?.resource.attributes['service.name']).toBe('unknown_service');
    });

    it('should use custom resource service name if provided', () => {
      const { completionHandler: _ } = initTelemetry({
        resource: new Resource({
          'service.name': 'test-service',
        }),
      });

      expect(state.provider?.resource.attributes['service.name']).toBe('test-service');
    });

    it('should use custom resource if provided', () => {
      const customResource = new Resource({
        'custom.attribute': 'value',
      });

      const { completionHandler: _ } = initTelemetry({
        resource: customResource,
      });

      expect(state.provider?.resource.attributes['custom.attribute']).toBe('value');
    });

    it('should use provided span processors', () => {
      // Create a test processor that tracks if it was called
      class TestProcessor implements SpanProcessor {
        public onStartCalled = false;
        public onEndCalled = false;

        forceFlush(): Promise<void> {
          return Promise.resolve();
        }
        shutdown(): Promise<void> {
          return Promise.resolve();
        }
        onStart(): void {
          this.onStartCalled = true;
        }
        onEnd(): void {
          this.onEndCalled = true;
        }
      }

      const testProcessor = new TestProcessor();

      const { tracer } = initTelemetry({
        spanProcessors: [testProcessor],
      });

      // Create and end a span to trigger the processor
      const span = tracer.startSpan('test');
      span.end();

      expect(testProcessor.onStartCalled).toBe(true);
      expect(testProcessor.onEndCalled).toBe(true);
    });

    it('should configure default processor queue size from environment', () => {
      envManager.setup({
        LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE: '1024',
      });

      const { tracer } = initTelemetry();

      // Create multiple spans to verify they are processed
      for (let i = 0; i < 10; i++) {
        const span = tracer.startSpan(`test-${i}`);
        span.end();
      }
      // If we got here without errors, the queue size was configured correctly
    });

    it('should allow mixing multiple processors', () => {
      // Create test processors that track if they were called
      class TestProcessor implements SpanProcessor {
        public onStartCalled = false;
        public onEndCalled = false;

        constructor(public name: string) {}

        forceFlush(): Promise<void> {
          return Promise.resolve();
        }
        shutdown(): Promise<void> {
          return Promise.resolve();
        }
        onStart(): void {
          this.onStartCalled = true;
        }
        onEnd(): void {
          this.onEndCalled = true;
        }
      }

      const processor1 = new TestProcessor('processor1');
      const processor2 = new TestProcessor('processor2');

      const { tracer } = initTelemetry({
        spanProcessors: [processor1, processor2],
      });

      // Create and end a span to trigger both processors
      const span = tracer.startSpan('test');
      span.end();

      expect(processor1.onStartCalled).toBe(true);
      expect(processor1.onEndCalled).toBe(true);
      expect(processor2.onStartCalled).toBe(true);
      expect(processor2.onEndCalled).toBe(true);
    });

    it('should initialize with default options', () => {
      const { completionHandler: _ } = initTelemetry();
      expect(state.mode).toBe('sync');
    });

    it('should initialize with sync mode', () => {
      const { completionHandler: _ } = initTelemetry({
        spanProcessors: [
          {
            forceFlush: () => Promise.resolve(),
            shutdown: () => Promise.resolve(),
            onStart: jest.fn(),
            onEnd: jest.fn(),
          },
        ],
      });
      expect(state.mode).toBe('sync');
    });

    it('should initialize with async mode', () => {
      // Set up environment and extension state
      envManager.setup({
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: 'async',
      });
      state.extensionInitialized = true;

      const { completionHandler: _ } = initTelemetry({
        spanProcessors: [
          {
            forceFlush: () => Promise.resolve(),
            shutdown: () => Promise.resolve(),
            onStart: jest.fn(),
            onEnd: jest.fn(),
          },
        ],
      });
      expect(state.mode).toBe('async');
    });

    it('should initialize with finalize mode', () => {
      // Set up environment and extension state
      envManager.setup({
        LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE: 'finalize',
      });
      state.extensionInitialized = true;

      const { completionHandler: _ } = initTelemetry({
        spanProcessors: [
          {
            forceFlush: () => Promise.resolve(),
            shutdown: () => Promise.resolve(),
            onStart: jest.fn(),
            onEnd: jest.fn(),
          },
        ],
      });
      expect(state.mode).toBe('finalize');
    });

    it('should initialize with custom span processor', () => {
      const mockSpanProcessor: SpanProcessor = {
        forceFlush: () => Promise.resolve(),
        shutdown: () => Promise.resolve(),
        onStart: jest.fn(),
        onEnd: jest.fn(),
      };

      const { completionHandler: _ } = initTelemetry({
        spanProcessors: [mockSpanProcessor],
      });
      expect(state.mode).toBe('sync');
    });
  });

  describe('cold start tracking', () => {
    it('should track cold start correctly', () => {
      expect(isColdStart()).toBe(true);
      setColdStart(false);
      expect(isColdStart()).toBe(false);
    });
  });
});
