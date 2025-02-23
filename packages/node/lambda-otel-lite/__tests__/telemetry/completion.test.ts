import { jest, describe, it, expect } from '@jest/globals';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { ProcessorMode } from '../../src/mode';
import { VERSION } from '../../src/version';
import { TelemetryCompletionHandler } from '../../src/internal/telemetry/completion';

describe('TelemetryCompletionHandler', () => {
  describe('constructor', () => {
    it('should create tracer with package instrumentation scope', () => {
      const provider = new NodeTracerProvider();
      const getTracerSpy = jest.spyOn(provider, 'getTracer');

      new TelemetryCompletionHandler(provider, ProcessorMode.Sync);

      expect(getTracerSpy).toHaveBeenCalledWith(VERSION.NAME, VERSION.VERSION);
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
