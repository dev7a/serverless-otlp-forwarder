// Mock OpenTelemetry API
const mockApi = {
  SpanKind: {
    SERVER: 1,
  },
  SpanStatusCode: {
    OK: 'OK',
    ERROR: 'ERROR',
  },
  ROOT_CONTEXT: {},
  trace: {
    getSpan: jest.fn(),
    getTracer: jest.fn(),
  },
  propagation: {
    extract: jest.fn().mockReturnValue({}),
  },
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
    extensionInitialized: false,
  },
}));

// Mock cold start functions
jest.doMock('../src/internal/telemetry/init', () => ({
  isColdStart: jest.fn(),
  setColdStart: jest.fn(),
}));

// Import after mocks
import { SpanStatusCode } from '@opentelemetry/api';
import { jest } from '@jest/globals';
import { createTracedHandler } from '../src/handler';
import * as init from '../src/internal/telemetry/init';
import { TelemetryCompletionHandler } from '../src/internal/telemetry/completion';
import { defaultExtractor } from '../src/internal/telemetry/extractors'; // Import defaultExtractor
import { describe, it, beforeEach, expect } from '@jest/globals';

describe('createTracedHandler', () => {
  let tracer: any;
  let mockSpan: any;
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
      addEvent: jest.fn(),
    };

    // Create mock tracer
    tracer = {
      startActiveSpan: jest.fn(
        (name: string, options: any, context: any, fn: (span: any) => Promise<any>) => {
          return fn(mockSpan);
        }
      ),
    };

    // Create mock span processor
    const _mockSpanProcessor = {
      onStart: jest.fn(),
      onEnd: jest.fn(),
    };

    // Create mock completion handler
    completionHandler = {
      complete: jest.fn(),
      shutdown: jest.fn(),
      getTracer: jest.fn().mockReturnValue(tracer),
    } as any;

    // Set up default event and context
    defaultEvent = {};
    // Update defaultContext to match the full aws-lambda Context type
    defaultContext = {
      awsRequestId: 'test-id',
      invokedFunctionArn: 'arn:aws:lambda:region:account:function:name',
      functionName: 'test-function',
      functionVersion: '$LATEST',
      memoryLimitInMB: '128',
      getRemainingTimeInMillis: () => 1000,
      callbackWaitsForEmptyEventLoop: true, // Added
      logGroupName: '/aws/lambda/test-function', // Added
      logStreamName: '2023/01/01/[$LATEST]abcdef', // Added
      done: jest.fn(), // Added
      fail: jest.fn(), // Added
      succeed: jest.fn(), // Added
    };

    // Mock cold start as true initially
    (init.isColdStart as jest.Mock).mockReturnValue(true);

    // Mock propagation.extract
    mockApi.propagation.extract.mockImplementation(() => mockApi.ROOT_CONTEXT);
  });

  describe('basic functionality', () => {
    it('should work with basic options', async () => {
      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => 'success');
      const result = await handler(defaultEvent, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.coldstart', true);
      expect(mockSpan.setStatus).not.toHaveBeenCalled();
      expect(completionHandler.complete).toHaveBeenCalled();
    });

    it('should set default faas.trigger for non-HTTP events', async () => {
      const event = { type: 'custom' };
      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => 'success');
      const result = await handler(event, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.trigger', 'other');
    });
  });

  describe('Lambda context handling', () => {
    it('should extract attributes from Lambda context', async () => {
      // Update lambdaContext to match the full aws-lambda Context type
      const lambdaContext = {
        awsRequestId: '123',
        invokedFunctionArn: 'arn:aws:lambda:region:account:function:name',
        functionName: 'test-function',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        getRemainingTimeInMillis: () => 1000,
        callbackWaitsForEmptyEventLoop: false, // Added
        logGroupName: '/aws/lambda/test-function', // Added
        logStreamName: '2023/01/01/[$LATEST]ghijkl', // Added
        done: jest.fn(), // Added
        fail: jest.fn(), // Added
        succeed: jest.fn(), // Added
      };

      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => 'success');
      await handler(defaultEvent, lambdaContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.invocation_id', '123');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('cloud.account.id', 'account');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith(
        'cloud.resource_id',
        'arn:aws:lambda:region:account:function:name'
      );
    });
  });

  describe('custom extractor', () => {
    it('should use custom extractor', async () => {
      const customExtractor = (_event: any, _context: any) => {
        return {
          trigger: 'custom-trigger',
          kind: mockApi.SpanKind.SERVER,
          attributes: {
            'custom.attribute': 'custom-value',
          },
          spanName: 'custom-span',
        };
      };

      const traced = createTracedHandler('test-handler', completionHandler, customExtractor);

      const handler = traced(async (_event, _context) => 'success');
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('custom.attribute', 'custom-value');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.trigger', 'custom-trigger');
    });
  });

  describe('HTTP response handling', () => {
    it('should handle successful HTTP responses', async () => {
      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => ({
        statusCode: 200,
        body: 'success',
      }));
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.status_code', 200);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
    });

    it('should handle error HTTP responses', async () => {
      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => ({
        statusCode: 500,
        body: 'error',
      }));
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.status_code', 500);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({
        code: SpanStatusCode.ERROR,
        message: 'HTTP 500 response',
      });
    });
  });

  describe('error handling', () => {
    it('should handle errors in handler', async () => {
      const error = new Error('test error');
      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => {
        throw error;
      });

      await expect(handler(defaultEvent, defaultContext)).rejects.toThrow(error);
      expect(mockSpan.recordException).toHaveBeenCalledWith(error);
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('error', true);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({
        code: SpanStatusCode.ERROR,
        message: 'test error',
      });
    });

    it('should handle errors in context extraction', async () => {
      mockApi.propagation.extract.mockImplementation(() => {
        throw new Error('extraction error');
      });

      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => 'success');
      const result = await handler(defaultEvent, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setStatus).not.toHaveBeenCalled();
    });

    it('should complete telemetry even if handler throws', async () => {
      const error = new Error('test error');
      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => {
        throw error;
      });

      await expect(handler(defaultEvent, defaultContext)).rejects.toThrow(error);
      expect(completionHandler.complete).toHaveBeenCalled();
    });
  });

  describe('cold start handling', () => {
    it('should handle cold start correctly', async () => {
      const traced = createTracedHandler('test-handler', completionHandler, defaultExtractor); // Added defaultExtractor

      const handler = traced(async (_event, _context) => 'success');
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.coldstart', true);
      expect(init.setColdStart).toHaveBeenCalledWith(false);
    });
  });
});
