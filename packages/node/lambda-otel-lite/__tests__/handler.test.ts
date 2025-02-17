// Mock OpenTelemetry API
const mockApi = {
  SpanKind: {
    SERVER: 'SERVER'
  },
  SpanStatusCode: {
    OK: 'OK',
    ERROR: 'ERROR'
  },
  ROOT_CONTEXT: {},
  trace: {
    getSpan: jest.fn(),
    getTracer: jest.fn()
  },
  propagation: {
    extract: jest.fn().mockReturnValue({})
  }
};

jest.doMock('@opentelemetry/api', () => mockApi);

// Mock the logger
jest.doMock('../src/internal/logger', () => ({
  debug: jest.fn(),
  info: jest.fn(),
  warn: jest.fn(),
  error: jest.fn(),
}));

// Mock the state module
jest.doMock('../src/internal/state', () => ({
  getState: jest.fn(),
  setState: jest.fn(),
  clearState: jest.fn(),
  state: {
    mode: null,
    extensionInitialized: false
  }
}));

// Mock cold start functions
jest.doMock('../src/internal/telemetry/init', () => ({
  isColdStart: jest.fn(),
  setColdStart: jest.fn()
}));

// Import after mocks
import { SpanStatusCode } from '@opentelemetry/api';
import { jest } from '@jest/globals';
import { ProcessorMode } from '../src/mode';
import { createTracedHandler } from '../src/handler';
import { state } from '../src/internal/state';
import * as init from '../src/internal/telemetry/init';
import { TelemetryCompletionHandler } from '../src/internal/telemetry/completion';
import { apiGatewayV1Extractor, apiGatewayV2Extractor } from '../src/internal/telemetry/extractors';
import { describe, it, beforeEach, expect } from '@jest/globals';

describe('createTracedHandler', () => {
  let tracer: any;
  let mockSpan: any;
  let mockSpanProcessor: any;
  let completionHandler: TelemetryCompletionHandler;
  let defaultEvent: any;
  let defaultContext: any;

  beforeEach(() => {
    // Reset all mocks
    jest.clearAllMocks();
        
    // Create mock span
    mockSpan = {
      setAttribute: jest.fn(),
      setStatus: jest.fn(),
      end: jest.fn(),
      recordException: jest.fn(),
      addEvent: jest.fn()
    };

    // Create mock tracer
    tracer = {
      startActiveSpan: jest.fn((name: string, options: any, context: any, fn: (span: any) => Promise<any>) => {
        return fn(mockSpan);
      })
    };

    // Create mock span processor
    const _mockSpanProcessor = {
      onStart: jest.fn(),
      onEnd: jest.fn()
    };

    // Create mock completion handler
    completionHandler = {
      complete: jest.fn(),
      shutdown: jest.fn(),
      getTracer: jest.fn().mockReturnValue(tracer)
    } as any;

    // Set up default event and context
    defaultEvent = {};
    defaultContext = {
      awsRequestId: 'test-id',
      invokedFunctionArn: 'arn:aws:lambda:region:account:function:name',
      functionName: 'test-function',
      functionVersion: '$LATEST',
      memoryLimitInMB: '128',
      getRemainingTimeInMillis: () => 1000
    };

    // Mock cold start as true initially
    (init.isColdStart as jest.Mock).mockReturnValue(true);

    // Mock propagation.extract
    mockApi.propagation.extract.mockImplementation(() => mockApi.ROOT_CONTEXT);
  });

  describe('basic functionality', () => {
    it('should work with basic options', async () => {
      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      const result = await handler(defaultEvent, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.coldstart', true);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
      expect(mockSpan.end).toHaveBeenCalled();
      expect(completionHandler.complete).toHaveBeenCalled();
    });

    it('should set default faas.trigger for non-HTTP events', async () => {
      const event = { type: 'custom' };
      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      const result = await handler(event, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.trigger', 'other');
    });
  });

  describe('Lambda context handling', () => {
    it('should extract attributes from Lambda context', async () => {
      const lambdaContext = {
        awsRequestId: '123',
        invokedFunctionArn: 'arn:aws:lambda:region:account:function:name',
        functionName: 'test-function',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        getRemainingTimeInMillis: () => 1000
      };

      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      await handler(defaultEvent, lambdaContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.invocation_id', '123');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('cloud.account.id', 'account');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith(
        'cloud.resource_id',
        'arn:aws:lambda:region:account:function:name'
      );
    });
  });

  describe('API Gateway event handling', () => {
    it('should handle API Gateway v2 events', async () => {
      const event = {
        version: '2.0',
        routeKey: '/test',
        rawPath: '/test',
        requestContext: {
          http: {
            method: 'GET',
            path: '/test',
            protocol: 'HTTP/1.1'
          }
        }
      };

      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler',
        attributesExtractor: apiGatewayV2Extractor
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      await handler(event, defaultContext);

      // Get all calls to setAttribute
      const calls = mockSpan.setAttribute.mock.calls;
            
      // Create a map of all attributes that were set
      const attributesSet = new Map<string, string | number | boolean>(
        calls.map(([k, v]: [string, string | number | boolean]) => [k, v])
      );

      // Verify the expected attributes
      expect(attributesSet.get('faas.trigger')).toBe('http');
      expect(attributesSet.get('http.route')).toBe('/test');
      expect(attributesSet.get('http.request.method')).toBe('GET');
      expect(attributesSet.get('url.path')).toBe('/test');
      expect(attributesSet.get('url.scheme')).toBe('https');
    });

    it('should handle API Gateway v1 events', async () => {
      const event = {
        httpMethod: 'POST',
        resource: '/test',
        path: '/test',
        requestContext: {
          protocol: 'HTTPS'
        }
      };

      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler',
        attributesExtractor: apiGatewayV1Extractor
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      await handler(event, defaultContext);

      // Get all calls to setAttribute
      const calls = mockSpan.setAttribute.mock.calls;
            
      // Create a map of all attributes that were set
      const attributesSet = new Map<string, string | number | boolean>(
        calls.map(([k, v]: [string, string | number | boolean]) => [k, v])
      );

      // Verify the expected attributes
      expect(attributesSet.get('faas.trigger')).toBe('http');
      expect(attributesSet.get('http.route')).toBe('/test');
      expect(attributesSet.get('http.request.method')).toBe('POST');
      expect(attributesSet.get('url.path')).toBe('/test');
      expect(attributesSet.get('url.scheme')).toBe('https');
    });
  });

  describe('HTTP response handling', () => {
    it('should handle successful HTTP responses', async () => {
      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => ({
        statusCode: 200,
        body: 'success'
      }));
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.status_code', 200);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
    });

    it('should handle error HTTP responses', async () => {
      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => ({
        statusCode: 500,
        body: 'error'
      }));
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.status_code', 500);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({
        code: SpanStatusCode.ERROR,
        message: 'HTTP 500 response'
      });
    });
  });

  describe('context propagation', () => {
    it('should extract context from headers', async () => {
      const event = {
        headers: {
          traceparent: 'test-trace-id'
        }
      };

      // Mock propagation.extract
      const mockContext = {} as any;
      mockApi.propagation.extract.mockReturnValue(mockContext);

      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      await handler(event, defaultContext);

      expect(mockApi.propagation.extract).toHaveBeenCalledWith(mockApi.ROOT_CONTEXT, event.headers);
      expect(tracer.startActiveSpan).toHaveBeenCalledWith(
        'test-handler',
        expect.any(Object),
        mockContext,
        expect.any(Function)
      );
    });
  });

  describe('error handling', () => {
    it('should handle and record exceptions', async () => {
      const error = new Error('test error');
      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => {
        throw error;
      });
      await expect(handler(defaultEvent, defaultContext)).rejects.toThrow(error);

      expect(mockSpan.recordException).toHaveBeenCalledWith(error);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({
        code: SpanStatusCode.ERROR,
        message: 'test error'
      });
    });
  });

  describe('processor mode handling', () => {
    it('should call complete in sync mode', async () => {
      state.mode = ProcessorMode.Sync;
      state.extensionInitialized = false;

      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      await handler(defaultEvent, defaultContext);

      expect(completionHandler.complete).toHaveBeenCalled();
    });

    it('should call complete in async mode', async () => {
      state.mode = ProcessorMode.Async;
      state.extensionInitialized = true;

      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => 'success');
      await handler(defaultEvent, defaultContext);

      expect(completionHandler.complete).toHaveBeenCalled();
    });
  });

  describe('handler interface', () => {
    it('should handle custom span attributes', async () => {
      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => {
        _span.setAttribute('custom', 'value');
        return 'success';
      });
      const result = await handler(defaultEvent, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('custom', 'value');
      expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
      expect(mockSpan.end).toHaveBeenCalled();
    });

    it('should handle errors', async () => {
      const testError = new Error('test error');
      const traced = createTracedHandler(completionHandler, {
        name: 'test-handler'
      });
      
      const handler = traced(async (_event, _context, _span) => {
        throw testError;
      });
      await expect(handler(defaultEvent, defaultContext)).rejects.toThrow(testError);

      expect(mockSpan.recordException).toHaveBeenCalledWith(testError);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({
        code: SpanStatusCode.ERROR,
        message: 'test error'
      });
    });
  });
}); 