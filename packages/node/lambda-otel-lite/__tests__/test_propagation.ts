// Mock the module path first
jest.mock('@opentelemetry/propagator-aws-xray');

// Now import everything
import { AWSXRayPropagator } from '@opentelemetry/propagator-aws-xray'; // Keep for type info if needed
import {
  LambdaXRayPropagator,
  createPropagator,
  setupPropagator,
} from '../src/internal/propagation'; // Import setupPropagator here
import { EnvVarManager } from './utils';
import { describe, it, beforeEach, afterEach, expect, jest } from '@jest/globals';
import { context, propagation, TextMapGetter } from '@opentelemetry/api';
import { CompositePropagator } from '@opentelemetry/core';
import { ENV_VARS } from '../src/constants';

// Mock the logger once
jest.mock('../src/internal/logger', () => ({
  createLogger: () => ({
    debug: jest.fn(),
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn(),
  }),
}));

describe('Propagation', () => {
  let envManager: EnvVarManager;

  beforeEach(() => {
    // Reset mocks before each test
    jest.clearAllMocks();
    envManager = new EnvVarManager();
    envManager.setup();
  });

  afterEach(() => {
    envManager.restore();
  });

  describe('LambdaXRayPropagator', () => {
    // Tests for LambdaXRayPropagator remain the same as the previous correct version
    it('should extract context from carrier', () => {
      const carrier = {
        'x-amzn-trace-id':
          'Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1',
      };
      const propagator = new LambdaXRayPropagator();
      const internalPropagator = (propagator as any).xrayPropagator;
      const mockExtract = jest.spyOn(internalPropagator, 'extract');
      const mockContext = context.active();
      mockExtract.mockReturnValue(mockContext);

      const defaultGetter: TextMapGetter<Record<string, string>> = {
        get: (carrier, key) => carrier?.[key],
        keys: (carrier) => Object.keys(carrier || {}),
      };

      const result = propagator.extract(context.active(), carrier, defaultGetter);

      expect(mockExtract).toHaveBeenCalled();
      expect(result).toBe(mockContext);
    });

    it('should fall back to environment variable when carrier has no valid context', () => {
      const carrier = {};
      const propagator = new LambdaXRayPropagator();
      const internalPropagator = (propagator as any).xrayPropagator;
      const mockExtract = jest.spyOn(internalPropagator, 'extract');
      const mockContext = context.active();
      mockExtract
        .mockReturnValueOnce(context.active()) // Simulate no context found in carrier
        .mockReturnValueOnce(mockContext); // Simulate context found in env var

      process.env._X_AMZN_TRACE_ID =
        'Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1';

      const defaultGetter: TextMapGetter<Record<string, string>> = {
        get: (carrier, key) => carrier?.[key],
        keys: (carrier) => Object.keys(carrier || {}),
      };

      const result = propagator.extract(context.active(), carrier, defaultGetter);
      expect(mockExtract).toHaveBeenCalledTimes(2);
      expect(result).toBe(mockContext);
    });

    it('should respect Sampled=0 flag in environment variable', () => {
      const carrier = {};
      const propagator = new LambdaXRayPropagator();
      const internalPropagator = (propagator as any).xrayPropagator;
      const mockExtract = jest.spyOn(internalPropagator, 'extract');
      mockExtract.mockReturnValue(context.active()); // Simulate no context found

      process.env._X_AMZN_TRACE_ID =
        'Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=0';

      const ctx = context.active();
      const defaultGetter: TextMapGetter<Record<string, string>> = {
        get: (carrier, key) => carrier?.[key],
        keys: (carrier) => Object.keys(carrier || {}),
      };

      const result = propagator.extract(ctx, carrier, defaultGetter);
      expect(mockExtract).toHaveBeenCalledTimes(1); // Should only call for carrier
      expect(result).toBe(ctx); // Should return original context
    });

    it('should inject context into carrier', () => {
      const carrier: Record<string, string> = {};
      const propagator = new LambdaXRayPropagator();
      const internalPropagator = (propagator as any).xrayPropagator;
      const mockInject = jest.spyOn(internalPropagator, 'inject');

      propagator.inject(context.active(), carrier, {
        set: (c, k, v) => {
          c[k] = v;
        },
      });

      expect(mockInject).toHaveBeenCalled();
    });

    it('should return fields from AWSXRayPropagator', () => {
      const propagator = new LambdaXRayPropagator();
      const internalPropagator = (propagator as any).xrayPropagator;
      const mockFields = jest.spyOn(internalPropagator, 'fields');
      mockFields.mockReturnValue(['x-amzn-trace-id']); // Ensure mock returns expected value

      const result = propagator.fields();

      expect(mockFields).toHaveBeenCalled();
      expect(result).toEqual(['x-amzn-trace-id']);
    });
  }); // End LambdaXRayPropagator describe

  describe('createPropagator', () => {
    it('should create propagator based on environment variable (tracecontext, xray)', () => {
      envManager.setup({ [ENV_VARS.OTEL_PROPAGATORS]: 'tracecontext,xray' });
      // Spy on the prototype method for this test - for the 'xray' instance
      // Note: We mock the real AWSXRayPropagator prototype here
      const xrayFieldsSpy = jest
        .spyOn(AWSXRayPropagator.prototype, 'fields')
        .mockReturnValue(['x-amzn-trace-id']);

      const result = createPropagator();

      expect(result).toBeInstanceOf(CompositePropagator);
      // Check fields from W3C and the spied AWSXRayPropagator
      expect(result.fields()).toEqual(
        expect.arrayContaining(['traceparent', 'tracestate', 'x-amzn-trace-id'])
      );
      expect(xrayFieldsSpy).toHaveBeenCalled(); // Ensure the spy on the real prototype was hit
      xrayFieldsSpy.mockRestore(); // Restore AWSXRayPropagator spy
    });

    it('should create LambdaXRayPropagator when xray-lambda is specified', () => {
      envManager.setup({ [ENV_VARS.OTEL_PROPAGATORS]: 'xray-lambda' });
      // Spy on LambdaXRayPropagator.prototype.fields for this test
      const fieldsSpy = jest
        .spyOn(LambdaXRayPropagator.prototype, 'fields')
        .mockReturnValue(['x-amzn-trace-id']);

      const result = createPropagator();

      expect(result).toBeInstanceOf(CompositePropagator);
      expect(result.fields()).toEqual(expect.arrayContaining(['x-amzn-trace-id']));
      expect(result.fields()).not.toEqual(expect.arrayContaining(['traceparent']));
      expect(fieldsSpy).toHaveBeenCalled();
      fieldsSpy.mockRestore();
    });

    it('should create no-op propagator when none is specified', () => {
      envManager.setup({ [ENV_VARS.OTEL_PROPAGATORS]: 'none' });

      const result = createPropagator();

      // No spy needed here
      expect(result.fields()).toEqual([]);
    });

    it('should use default propagators when environment variable is not set', () => {
      envManager.setup({ [ENV_VARS.OTEL_PROPAGATORS]: undefined });
      // Spy on LambdaXRayPropagator.prototype.fields for this test
      const fieldsSpy = jest
        .spyOn(LambdaXRayPropagator.prototype, 'fields')
        .mockReturnValue(['x-amzn-trace-id']);

      const result = createPropagator();

      expect(result).toBeInstanceOf(CompositePropagator);
      // Defaults are xray-lambda, tracecontext
      expect(result.fields()).toEqual(
        expect.arrayContaining(['x-amzn-trace-id', 'traceparent', 'tracestate'])
      );
      expect(fieldsSpy).toHaveBeenCalled();
      fieldsSpy.mockRestore();
    });

    it('should use default propagators when invalid propagator is specified', () => {
      envManager.setup({ [ENV_VARS.OTEL_PROPAGATORS]: 'invalid' });
      // Spy on LambdaXRayPropagator.prototype.fields for this test
      const fieldsSpy = jest
        .spyOn(LambdaXRayPropagator.prototype, 'fields')
        .mockReturnValue(['x-amzn-trace-id']);

      const result = createPropagator();

      expect(result).toBeInstanceOf(CompositePropagator);
      // Defaults are xray-lambda, tracecontext
      expect(result.fields()).toEqual(
        expect.arrayContaining(['x-amzn-trace-id', 'traceparent', 'tracestate'])
      );
      expect(fieldsSpy).toHaveBeenCalled();
      fieldsSpy.mockRestore();
    });
  }); // End createPropagator describe

  // Remove duplicate describe block
  describe('setupPropagator', () => {
    it('should set global propagator', () => {
      // Spy on LambdaXRayPropagator.prototype.fields for this test
      const fieldsSpy = jest
        .spyOn(LambdaXRayPropagator.prototype, 'fields')
        .mockReturnValue(['x-amzn-trace-id']);
      // Mock the propagation.setGlobalPropagator method
      const mockSetGlobalPropagator = jest.spyOn(propagation, 'setGlobalPropagator');

      // Use the imported setupPropagator directly
      setupPropagator();

      // Check that setGlobalPropagator was called with a CompositePropagator
      expect(mockSetGlobalPropagator).toHaveBeenCalledWith(expect.any(CompositePropagator));

      // Check the fields of the propagator that was set
      const actualPropagator = mockSetGlobalPropagator.mock.calls[0][0] as CompositePropagator;
      // Default is xray-lambda, tracecontext
      expect(actualPropagator.fields()).toEqual(
        expect.arrayContaining(['x-amzn-trace-id', 'traceparent', 'tracestate'])
      );
      expect(fieldsSpy).toHaveBeenCalled(); // Ensure fields was called during setup

      fieldsSpy.mockRestore();
    });
  }); // End setupPropagator describe
}); // End Propagation describe
