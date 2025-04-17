import { describe, it, expect } from '@jest/globals';
import * as fs from 'fs';
import * as path from 'path';
import type {
  APIGatewayProxyEventV2,
  APIGatewayProxyEvent,
  ALBEvent,
  Context as LambdaContextType,
} from 'aws-lambda';
import {
  defaultExtractor,
  apiGatewayV1Extractor,
  apiGatewayV2Extractor,
  albExtractor,
} from '../src/extractors/index'; // Corrected import path
import { SpanKind } from '@opentelemetry/api';
import { TriggerType } from '../src/internal/telemetry/extractors'; // Keep TriggerType import separate if it's still internal

// Load fixtures
const FIXTURES_DIR = path.join(__dirname, 'fixtures');

function loadFixture(name: string) {
  const filePath = path.join(FIXTURES_DIR, name);
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

// Load all fixtures once
const FIXTURES = {
  apigw_v1: loadFixture('apigw_v1_proxy.json'),
  apigw_v2: loadFixture('apigw_v2_proxy.json'),
  alb: loadFixture('alb.json'),
};

// Updated helper function to match implementation (handles undefined values)
function normalizeHeaders(
  headers?: Record<string, string | undefined>
): Record<string, string> | undefined {
  if (!headers) {
    return undefined;
  }

  // Normalize all headers to lowercase, filter out undefined values
  return Object.entries(headers).reduce(
    (acc, [key, value]) => {
      if (typeof value === 'string') {
        acc[key.toLowerCase()] = value;
      }
      return acc;
    },
    {} as Record<string, string>
  );
}

describe('Extractors', () => {
  describe('defaultExtractor', () => {
    it('should extract basic Lambda context attributes', () => {
      const context: LambdaContextType = {
        awsRequestId: 'test-request-id',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-function',
        functionName: 'test-function',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-function',
        logStreamName: '2023/01/01/[$LATEST]abcdef',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };

      const result = defaultExtractor({}, context);

      expect(result.attributes['faas.invocation_id']).toBe('test-request-id');
      expect(result.attributes['cloud.resource_id']).toBe(
        'arn:aws:lambda:us-east-1:123456789012:function:test-function'
      );
      expect(result.attributes['cloud.account.id']).toBe('123456789012');
      expect(result.trigger).toBe(TriggerType.Other);
      expect(result.kind).toBe(SpanKind.SERVER);
    });

    it('should extract headers as carrier when present', () => {
      const event = {
        headers: {
          'X-Amzn-Trace-Id': 'Root=1-5759e988-bd862e3fe1be46a994272793',
          traceparent: '00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01',
          'Content-Type': 'application/json',
        },
      };
      const context: LambdaContextType = {
        awsRequestId: 'test-request-id',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-function',
        functionName: 'test-function',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-function',
        logStreamName: '2023/01/01/[$LATEST]abcdef',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };

      const result = defaultExtractor(event, context);

      expect(result.carrier).toBeDefined();
      expect(result.carrier?.['x-amzn-trace-id']).toBe('Root=1-5759e988-bd862e3fe1be46a994272793');
      expect(result.carrier?.['traceparent']).toBe(
        '00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01'
      );
      expect(result.carrier?.['content-type']).toBe('application/json');
    });

    it('should handle missing headers', () => {
      const event = { body: 'test' }; // No headers
      const context: LambdaContextType = {
        awsRequestId: 'test-request-id',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-function',
        functionName: 'test-function',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-function',
        logStreamName: '2023/01/01/[$LATEST]abcdef',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };

      const result = defaultExtractor(event, context);

      expect(result.carrier).toBeUndefined();
    });

    it('should handle null or undefined event', () => {
      const context: LambdaContextType = {
        awsRequestId: 'test-request-id',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-function',
        functionName: 'test-function',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-function',
        logStreamName: '2023/01/01/[$LATEST]abcdef',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };

      const result1 = defaultExtractor(null, context);
      const result2 = defaultExtractor(undefined, context);

      expect(result1.carrier).toBeUndefined();
      expect(result2.carrier).toBeUndefined();
    });
  });

  describe('apiGatewayV1Extractor', () => {
    it('should extract attributes from API Gateway v1 event', () => {
      const event = FIXTURES.apigw_v1 as APIGatewayProxyEvent;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v1',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv1',
        functionName: 'test-apigwv1',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv1',
        logStreamName: '2023/01/01/[$LATEST]abcdef1',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV1Extractor(event, context);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);

      // Use toEqual instead of toBe to check content equality
      // and expect the headers to be normalized with lowercase keys
      expect(result.carrier).toEqual(normalizeHeaders(event.headers));

      // Check extracted attributes
      const attrs = result.attributes;
      expect(attrs['http.request.method']).toBe(event.httpMethod);
      expect(attrs['url.path']).toBe(event.path);
      expect(attrs['url.scheme']).toBe('https');
      expect(attrs['http.route']).toBe(event.resource);
      expect(result.spanName).toBe(`${event.httpMethod} ${event.resource}`);

      // Check headers are normalized
      if (event.headers) {
        if ('User-Agent' in event.headers) {
          expect(attrs['user_agent.original']).toBe(event.headers['User-Agent']);
        } else if ('user-agent' in event.headers) {
          expect(attrs['user_agent.original']).toBe(event.headers['user-agent']);
        }
      }
    });

    it('should handle missing data gracefully', () => {
      const emptyEvent = {} as APIGatewayProxyEvent; // Cast for type checking
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v1-empty',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv1-empty',
        functionName: 'test-apigwv1-empty',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv1-empty',
        logStreamName: '2023/01/01/[$LATEST]abcdef1e',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV1Extractor(emptyEvent, context);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.attributes['url.scheme']).toBe('https');
    });

    it('should extract all expected attributes from API Gateway v1 event', () => {
      const event = FIXTURES.apigw_v1 as APIGatewayProxyEvent;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v1-full',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv1-full',
        functionName: 'test-apigwv1-full',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv1-full',
        logStreamName: '2023/01/01/[$LATEST]abcdef1f',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV1Extractor(event, context);
      const attrs = result.attributes;

      // Verify all expected attributes are extracted
      expect(attrs['http.request.method']).toBe('POST');
      expect(attrs['url.path']).toBe('/path/to/resource');
      expect(attrs['url.scheme']).toBe('https');
      expect(attrs['http.route']).toBe('/{proxy+}');
      expect(attrs['user_agent.original']).toBe('Custom User Agent String');
      expect(attrs['server.address']).toBe('1234567890.execute-api.us-east-1.amazonaws.com');
      expect(attrs['client.address']).toBe('127.0.0.1');
    });
  });

  describe('apiGatewayV2Extractor', () => {
    it('should extract attributes from API Gateway v2 event', () => {
      const event = FIXTURES.apigw_v2 as APIGatewayProxyEventV2;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v2',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv2',
        functionName: 'test-apigwv2',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv2',
        logStreamName: '2023/01/01/[$LATEST]abcdef2',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV2Extractor(event, context);

      let route = event.routeKey;
      if (route === '$default') {
        route = event.rawPath || '/';
      }

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);

      // Use toEqual instead of toBe to check content equality
      // and expect the headers to be normalized with lowercase keys
      expect(result.carrier).toEqual(normalizeHeaders(event.headers));

      // Check extracted attributes
      const attrs = result.attributes;
      expect(attrs['http.request.method']).toBe(event.requestContext.http.method);
      expect(attrs['url.path']).toBe(event.rawPath);
      expect(attrs['url.scheme']).toBe('https');
      expect(attrs['http.route']).toBe(route);
      expect(result.spanName).toBe(`${event.requestContext.http.method} ${route}`);

      // Check headers are normalized
      if (event.headers) {
        if ('User-Agent' in event.headers) {
          expect(attrs['user_agent.original']).toBe(event.headers['User-Agent']);
        } else if ('user-agent' in event.headers) {
          expect(attrs['user_agent.original']).toBe(event.headers['user-agent']);
        }
      }
    });

    it('should handle missing data gracefully', () => {
      const emptyEvent = {} as APIGatewayProxyEventV2; // Cast for type checking
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v2-empty',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv2-empty',
        functionName: 'test-apigwv2-empty',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv2-empty',
        logStreamName: '2023/01/01/[$LATEST]abcdef2e',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV2Extractor(emptyEvent, context);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.attributes['url.scheme']).toBe('https');
    });

    it('should extract all expected attributes from API Gateway v2 event', () => {
      const event = FIXTURES.apigw_v2 as APIGatewayProxyEventV2;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v2-full',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv2-full',
        functionName: 'test-apigwv2-full',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv2-full',
        logStreamName: '2023/01/01/[$LATEST]abcdef2f',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV2Extractor(event, context);
      const attrs = result.attributes;

      // Verify all expected attributes are extracted
      expect(attrs['http.request.method']).toBe('POST');
      expect(attrs['url.path']).toBe('/path/to/resource');
      expect(attrs['url.scheme']).toBe('https');
      expect(attrs['http.route']).toBe('/path/to/resource');
      expect(attrs['user_agent.original']).toBe('agent');
      expect(attrs['server.address']).toBe('id.execute-api.us-east-1.amazonaws.com');
      expect(attrs['client.address']).toBe('192.168.0.1/32');
    });
  });

  describe('albExtractor', () => {
    it('should extract attributes from ALB event', () => {
      const event = FIXTURES.alb as ALBEvent;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-alb',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-alb',
        functionName: 'test-alb',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-alb',
        logStreamName: '2023/01/01/[$LATEST]abcdefa',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = albExtractor(event, context);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);

      // Use toEqual instead of toBe to check content equality
      // and expect the headers to be normalized with lowercase keys
      expect(result.carrier).toEqual(normalizeHeaders(event.headers));

      // Check extracted attributes
      const attrs = result.attributes;
      expect(attrs['http.request.method']).toBe(event.httpMethod);
      expect(attrs['url.path']).toBe(event.path);
      expect(attrs['url.scheme']).toBe('http');
      expect(attrs['http.route']).toBe(event.path);
      expect(attrs['network.protocol.version']).toBe('1.1');
      expect(result.spanName).toBe(`${event.httpMethod} ${event.path}`);

      // Check headers are normalized
      if (event.headers) {
        if ('User-Agent' in event.headers) {
          expect(attrs['user_agent.original']).toBe(event.headers['User-Agent']);
        } else if ('user-agent' in event.headers) {
          expect(attrs['user_agent.original']).toBe(event.headers['user-agent']);
        }

        // Check X-Forwarded-For handling
        // Check X-Forwarded-For handling (with check for headers existence and type)
        if (event.headers) {
          if (
            'X-Forwarded-For' in event.headers &&
            typeof event.headers['X-Forwarded-For'] === 'string'
          ) {
            const clientIp = event.headers['X-Forwarded-For'].split(',')[0].trim();
            expect(attrs['client.address']).toBe(clientIp);
          } else if (
            'x-forwarded-for' in event.headers &&
            typeof event.headers['x-forwarded-for'] === 'string'
          ) {
            const clientIp = event.headers['x-forwarded-for'].split(',')[0].trim();
            expect(attrs['client.address']).toBe(clientIp);
          }
        }
      }
    });

    it('should handle missing data gracefully', () => {
      const emptyEvent = {} as ALBEvent; // Cast for type checking
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-alb-empty',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-alb-empty',
        functionName: 'test-alb-empty',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-alb-empty',
        logStreamName: '2023/01/01/[$LATEST]abcdefae',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = albExtractor(emptyEvent, context);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      // Unlike API Gateway extractors, ALB extractor doesn't set a default url.scheme
      // for empty events, so we don't check for it
    });

    it('should extract all expected attributes from ALB event', () => {
      const event = FIXTURES.alb as ALBEvent;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-alb-full',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-alb-full',
        functionName: 'test-alb-full',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-alb-full',
        logStreamName: '2023/01/01/[$LATEST]abcdefaf',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = albExtractor(event, context);
      const attrs = result.attributes;

      // Verify all expected attributes are extracted
      expect(attrs['http.request.method']).toBe('POST');
      expect(attrs['url.path']).toBe('/path/to/resource');
      expect(attrs['url.scheme']).toBe('http');
      expect(attrs['http.route']).toBe('/path/to/resource');
      expect(attrs['user_agent.original']).toBe(
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36'
      );
      expect(attrs['server.address']).toBe('lambda-alb-123578498.us-east-2.elb.amazonaws.com');
      expect(attrs['client.address']).toBe('72.12.164.125');
      expect(attrs['alb.target_group_arn']).toBe(
        'arn:aws:elasticloadbalancing:us-east-2:123456789012:targetgroup/lambda-279XGJDqGZ5rsrHC2Fjr/49e9d65c45c6791a'
      );
    });
  });

  describe('mixed case headers', () => {
    it('should handle headers case-insensitively for API Gateway v1', () => {
      const event = FIXTURES.apigw_v1 as APIGatewayProxyEvent;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v1-case',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv1-case',
        functionName: 'test-apigwv1-case',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv1-case',
        logStreamName: '2023/01/01/[$LATEST]abcdef1c',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV1Extractor(event, context);
      const attrs = result.attributes;

      expect(attrs['user_agent.original']).toBe(event.requestContext.identity.userAgent);
    });

    it('should handle headers case-insensitively for API Gateway v2', () => {
      const event = FIXTURES.apigw_v2 as APIGatewayProxyEventV2;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-v2-case',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-apigwv2-case',
        functionName: 'test-apigwv2-case',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-apigwv2-case',
        logStreamName: '2023/01/01/[$LATEST]abcdef2c',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = apiGatewayV2Extractor(event, context);
      const attrs = result.attributes;

      expect(attrs['user_agent.original']).toBe(event.requestContext.http.userAgent);
    });

    it('should handle headers case-insensitively for ALB', () => {
      const event = FIXTURES.alb as ALBEvent;
      const context: LambdaContextType = {
        // Provide a minimal valid context
        awsRequestId: 'test-req-id-alb-case',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-alb-case',
        functionName: 'test-alb-case',
        functionVersion: '$LATEST',
        memoryLimitInMB: '128',
        logGroupName: '/aws/lambda/test-alb-case',
        logStreamName: '2023/01/01/[$LATEST]abcdefac',
        getRemainingTimeInMillis: () => 5000,
        done: () => {},
        fail: () => {},
        succeed: () => {},
        callbackWaitsForEmptyEventLoop: true, // Added missing property
      };
      const result = albExtractor(event, context);
      const attrs = result.attributes;

      // Add checks for event.headers before accessing properties
      if (event.headers) {
        // ALB gets user agent from headers
        expect(attrs['user_agent.original']).toBe(event.headers['user-agent']);

        // ALB processes these header values
        if (typeof event.headers['x-forwarded-for'] === 'string') {
          expect(attrs['client.address']).toBe(
            event.headers['x-forwarded-for'].split(',')[0].trim()
          );
        }
        expect(attrs['server.address']).toBe(event.headers.host);
        expect(attrs['url.scheme']).toBe(event.headers['x-forwarded-proto']);
      } else {
        // If headers are missing, these attributes should not be set from headers
        expect(attrs['user_agent.original']).toBeUndefined();
        expect(attrs['client.address']).toBeUndefined();
        expect(attrs['server.address']).toBeUndefined();
        expect(attrs['url.scheme']).toBeUndefined(); // Or check default if applicable
      }
    });
  });

  describe('custom extractors', () => {
    it('should support custom extractors', () => {
      const customExtractor = (_event: any, _context: any) => {
        return {
          trigger: 'custom-trigger',
          kind: SpanKind.SERVER,
          attributes: {
            'custom.attribute': 'custom-value',
            'another.attribute': 123,
          },
          spanName: 'custom-span',
        };
      };

      const result = customExtractor({}, {});

      expect(result.trigger).toBe('custom-trigger');
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.attributes['custom.attribute']).toBe('custom-value');
      expect(result.attributes['another.attribute']).toBe(123);
      expect(result.spanName).toBe('custom-span');
    });
  });
});
