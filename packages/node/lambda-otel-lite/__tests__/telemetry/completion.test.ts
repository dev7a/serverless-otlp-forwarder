import { jest, describe, it, expect } from '@jest/globals';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { ProcessorMode } from '../../src/mode';
import { VERSION } from '../../src/version';
import { TelemetryCompletionHandler } from '../../src/internal/telemetry/completion';

describe('TelemetryCompletionHandler', () => {
  describe('getTracer', () => {
    it('should create tracer with package instrumentation scope', () => {
      const provider = new NodeTracerProvider();
      const handler = new TelemetryCompletionHandler(provider, ProcessorMode.Sync);

      // Mock provider.getTracer to capture arguments
      const getTracerSpy = jest.spyOn(provider, 'getTracer');

      handler.getTracer();

      expect(getTracerSpy).toHaveBeenCalledWith(
        VERSION.NAME,
        VERSION.VERSION
      );
    });
  });
}); 