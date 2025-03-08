import { jest, describe, it, expect, beforeEach } from '@jest/globals';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { VERSION } from '../../src/version';
import { ProcessorMode } from '../../src/mode';
import { TelemetryCompletionHandler } from '../../src/internal/telemetry/completion';

describe('TelemetryCompletionHandler', () => {
  let provider: NodeTracerProvider;
  let _handler: TelemetryCompletionHandler;

  beforeEach(() => {
    provider = new NodeTracerProvider();
    const getTracerSpy = jest.spyOn(provider, 'getTracer');
    _handler = new TelemetryCompletionHandler(provider, ProcessorMode.Sync);

    // Verify tracer is initialized with correct package info
    expect(getTracerSpy).toHaveBeenCalledWith('@dev7a/lambda-otel-lite', VERSION);
  });

  describe('constructor', () => {
    it('should create tracer with package instrumentation scope', () => {
      const provider = new NodeTracerProvider();
      const getTracerSpy = jest.spyOn(provider, 'getTracer');

      new TelemetryCompletionHandler(provider, ProcessorMode.Sync);

      expect(getTracerSpy).toHaveBeenCalledWith('@dev7a/lambda-otel-lite', VERSION);
    });
  });

  describe('getTracer', () => {
    it('should return cached tracer instance', () => {
      const provider = new NodeTracerProvider();
      const getTracerSpy = jest.spyOn(provider, 'getTracer');

      const handler = new TelemetryCompletionHandler(provider, ProcessorMode.Sync);
      const tracer1 = handler.getTracer();
      const tracer2 = handler.getTracer();

      expect(tracer1).toBe(tracer2);
      expect(getTracerSpy).toHaveBeenCalledTimes(1);
    });
  });
});
