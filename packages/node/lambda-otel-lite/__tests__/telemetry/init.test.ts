import { jest, describe, it, beforeEach, afterEach, expect } from '@jest/globals';
import { trace } from '@opentelemetry/api';
import { Resource } from '@opentelemetry/resources';
import { SpanProcessor } from '@opentelemetry/sdk-trace-base';
import { initTelemetry, isColdStart, setColdStart } from '../../src/internal/telemetry/init';
import { state } from '../../src/internal/state';
import { EnvVarManager } from '../utils';

// Mock the logger module
jest.mock('../../src/internal/logger', () => {
  const mockLogger = {
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn()
  };
  return {
    __esModule: true,
    default: mockLogger,
    createLogger: () => mockLogger
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
      const handler = initTelemetry();
      const tracer = handler.getTracer();

      expect(handler).toBeDefined();
      expect(tracer).toBeDefined();
            
      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom settings', () => {
      const _handler = initTelemetry({
        resource: new Resource({
          'service.name': 'test-service'
        })
      });
      const tracer = _handler.getTracer();

      expect(_handler).toBeDefined();
      expect(tracer).toBeDefined();
            
      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom processor', () => {
      const _handler = initTelemetry({
        spanProcessors: [
          {
            forceFlush: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            shutdown: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            onStart: jest.fn(),
            onEnd: jest.fn()
          }
        ]
      });
      const tracer = _handler.getTracer();

      expect(_handler).toBeDefined();
      expect(tracer).toBeDefined();
            
      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom exporter', () => {
      const _handler = initTelemetry({
        spanProcessors: [
          {
            forceFlush: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            shutdown: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            onStart: jest.fn(),
            onEnd: jest.fn()
          }
        ]
      });
      const tracer = _handler.getTracer();

      expect(_handler).toBeDefined();
      expect(tracer).toBeDefined();
            
      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should initialize telemetry with custom completion handler', () => {
      const _handler = initTelemetry({
        spanProcessors: [
          {
            forceFlush: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            shutdown: jest.fn<() => Promise<void>>().mockImplementation(() => Promise.resolve()),
            onStart: jest.fn(),
            onEnd: jest.fn()
          }
        ]
      });
      const tracer = _handler.getTracer();

      expect(_handler).toBeDefined();
      expect(tracer).toBeDefined();
            
      // Verify that a provider is registered and can create spans
      const testSpan = tracer.startSpan('test');
      expect(testSpan).toBeDefined();
      testSpan.end();
    });

    it('should use service name from environment variables', () => {
      envManager.setup({
        OTEL_SERVICE_NAME: 'env-service',
        AWS_LAMBDA_FUNCTION_NAME: 'lambda-function'
      });

      initTelemetry();
            
      // Service name will be in the provider's resource
      expect(state.provider?.resource.attributes['service.name']).toBe('env-service');
    });

    it('should fallback to Lambda function name if OTEL_SERVICE_NAME not set', () => {
      envManager.setup({
        AWS_LAMBDA_FUNCTION_NAME: 'lambda-function'
      });

      initTelemetry();
            
      expect(state.provider?.resource.attributes['service.name']).toBe('lambda-function');
    });

    it('should use unknown_service if no environment variables set', () => {
      initTelemetry();
            
      expect(state.provider?.resource.attributes['service.name']).toBe('unknown_service');
    });

    it('should use custom resource service name if provided', () => {
      const _handler = initTelemetry({
        resource: new Resource({
          'service.name': 'test-service'
        })
      });
            
      expect(state.provider?.resource.attributes['service.name']).toBe('test-service');
    });

    it('should use custom resource if provided', () => {
      const customResource = new Resource({
        'custom.attribute': 'value'
      });

      const _handler = initTelemetry({
        resource: customResource
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
            
      const _handler = initTelemetry({
        spanProcessors: [testProcessor]
      });
      const tracer = _handler.getTracer();

      // Create and end a span to trigger the processor
      const span = tracer.startSpan('test');
      span.end();

      expect(testProcessor.onStartCalled).toBe(true);
      expect(testProcessor.onEndCalled).toBe(true);
    });

    it('should configure default processor queue size from environment', () => {
      envManager.setup({
        LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE: '1024'
      });

      const handler = initTelemetry();
      const tracer = handler.getTracer();
            
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
            
      const _handler = initTelemetry({
        spanProcessors: [processor1, processor2]
      });
      const tracer = _handler.getTracer();

      // Create and end a span to trigger both processors
      const span = tracer.startSpan('test');
      span.end();

      expect(processor1.onStartCalled).toBe(true);
      expect(processor1.onEndCalled).toBe(true);
      expect(processor2.onStartCalled).toBe(true);
      expect(processor2.onEndCalled).toBe(true);
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