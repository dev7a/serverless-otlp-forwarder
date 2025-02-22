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
import { createTracedHandler } from '../src/handler';
import * as init from '../src/internal/telemetry/init';
import { TelemetryCompletionHandler } from '../src/internal/telemetry/completion';
import { apiGatewayV1Extractor, apiGatewayV2Extractor, albExtractor } from '../src/internal/telemetry/extractors';
import { describe, it, beforeEach, expect } from '@jest/globals';

// Import fixtures
import apigwV1Event from './fixtures/apigw_v1_proxy.json';
import apigwV2Event from './fixtures/apigw_v2_proxy.json';
import albEvent from './fixtures/alb.json';

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
      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => 'success');
      const result = await handler(defaultEvent, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.coldstart', true);
      expect(mockSpan.setStatus).not.toHaveBeenCalled();
      expect(completionHandler.complete).toHaveBeenCalled();
    });

    it('should set default faas.trigger for non-HTTP events', async () => {
      const event = { type: 'custom' };
      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => 'success');
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

      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
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

  describe('API Gateway event handling', () => {
    it('should handle API Gateway v2 events', async () => {
      const traced = createTracedHandler(
        'test-handler',
        completionHandler,
        { attributesExtractor: apiGatewayV2Extractor }
      );
      
      const handler = traced(async (_event, _context) => 'success');
      await handler(apigwV2Event, defaultContext);

      // Get all calls to setAttribute
      const calls = mockSpan.setAttribute.mock.calls;
            
      // Create a map of all attributes that were set
      const attributesSet = new Map<string, string | number | boolean>(
        calls.map(([k, v]: [string, string | number | boolean]) => [k, v])
      );

      // Verify the expected attributes
      expect(attributesSet.get('faas.trigger')).toBe('http');
      expect(attributesSet.get('http.route')).toBe('/path/to/resource');
      expect(attributesSet.get('http.request.method')).toBe('POST');
      expect(attributesSet.get('url.path')).toBe('/path/to/resource');
      expect(attributesSet.get('url.scheme')).toBe('https');
      expect(attributesSet.get('user_agent.original')).toBe('agent');
      expect(attributesSet.get('server.address')).toBe('id.execute-api.us-east-1.amazonaws.com');
      expect(attributesSet.get('client.address')).toBe('192.168.0.1/32');
    });

    it('should handle API Gateway v1 events', async () => {
      const traced = createTracedHandler(
        'test-handler',
        completionHandler,
        { attributesExtractor: apiGatewayV1Extractor }
      );
      
      const handler = traced(async (_event, _context) => 'success');
      await handler(apigwV1Event, defaultContext);

      // Get all calls to setAttribute
      const calls = mockSpan.setAttribute.mock.calls;
            
      // Create a map of all attributes that were set
      const attributesSet = new Map<string, string | number | boolean>(
        calls.map(([k, v]: [string, string | number | boolean]) => [k, v])
      );

      // Verify the expected attributes
      expect(attributesSet.get('faas.trigger')).toBe('http');
      expect(attributesSet.get('http.route')).toBe('/{proxy+}');
      expect(attributesSet.get('http.request.method')).toBe('POST');
      expect(attributesSet.get('url.path')).toBe('/path/to/resource');
      expect(attributesSet.get('url.scheme')).toBe('https');
      expect(attributesSet.get('user_agent.original')).toBe('Custom User Agent String');
      expect(attributesSet.get('server.address')).toBe('1234567890.execute-api.us-east-1.amazonaws.com');
      expect(attributesSet.get('client.address')).toBe('127.0.0.1');
    });

    it('should handle ALB events', async () => {
      const traced = createTracedHandler(
        'test-handler',
        completionHandler,
        { attributesExtractor: albExtractor }
      );
      
      const handler = traced(async (_event, _context) => 'success');
      await handler(albEvent, defaultContext);

      // Get all calls to setAttribute
      const calls = mockSpan.setAttribute.mock.calls;
            
      // Create a map of all attributes that were set
      const attributesSet = new Map<string, string | number | boolean>(
        calls.map(([k, v]: [string, string | number | boolean]) => [k, v])
      );

      // Verify the expected attributes
      expect(attributesSet.get('faas.trigger')).toBe('http');
      expect(attributesSet.get('http.request.method')).toBe('POST');
      expect(attributesSet.get('url.path')).toBe('/path/to/resource');
      expect(attributesSet.get('url.scheme')).toBe('http');
      expect(attributesSet.get('user_agent.original')).toBe('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36');
      expect(attributesSet.get('server.address')).toBe('lambda-alb-123578498.us-east-2.elb.amazonaws.com');
      expect(attributesSet.get('client.address')).toBe('72.12.164.125');
      expect(attributesSet.get('alb.target_group_arn')).toBe('arn:aws:elasticloadbalancing:us-east-2:123456789012:targetgroup/lambda-279XGJDqGZ5rsrHC2Fjr/49e9d65c45c6791a');
    });
  });

  describe('HTTP response handling', () => {
    it('should handle successful HTTP responses', async () => {
      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => ({
        statusCode: 200,
        body: 'success'
      }));
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('http.status_code', 200);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({ code: SpanStatusCode.OK });
    });

    it('should handle error HTTP responses', async () => {
      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => ({
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

  describe('error handling', () => {
    it('should handle errors in handler', async () => {
      const error = new Error('test error');
      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => {
        throw error;
      });

      await expect(handler(defaultEvent, defaultContext)).rejects.toThrow(error);
      expect(mockSpan.recordException).toHaveBeenCalledWith(error);
      expect(mockSpan.setAttribute).toHaveBeenCalledWith('error', true);
      expect(mockSpan.setStatus).toHaveBeenCalledWith({
        code: SpanStatusCode.ERROR,
        message: 'test error'
      });
    });

    it('should handle errors in context extraction', async () => {
      mockApi.propagation.extract.mockImplementation(() => {
        throw new Error('extraction error');
      });

      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => 'success');
      const result = await handler(defaultEvent, defaultContext);

      expect(result).toBe('success');
      expect(mockSpan.setStatus).not.toHaveBeenCalled();
    });

    it('should complete telemetry even if handler throws', async () => {
      const error = new Error('test error');
      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => {
        throw error;
      });

      await expect(handler(defaultEvent, defaultContext)).rejects.toThrow(error);
      expect(completionHandler.complete).toHaveBeenCalled();
    });
  });

  describe('cold start handling', () => {
    it('should handle cold start correctly', async () => {
      const traced = createTracedHandler(
        'test-handler',
        completionHandler
      );
      
      const handler = traced(async (_event, _context) => 'success');
      await handler(defaultEvent, defaultContext);

      expect(mockSpan.setAttribute).toHaveBeenCalledWith('faas.coldstart', true);
      expect(init.setColdStart).toHaveBeenCalledWith(false);
    });
  });
}); 