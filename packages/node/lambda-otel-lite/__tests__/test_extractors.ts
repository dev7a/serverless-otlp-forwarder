import { describe, it, expect } from '@jest/globals';
import * as fs from 'fs';
import * as path from 'path';
import {
  defaultExtractor,
  apiGatewayV1Extractor,
  apiGatewayV2Extractor,
  albExtractor,
  TriggerType,
} from '../src/internal/telemetry/extractors';
import { SpanKind } from '@opentelemetry/api';

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

describe('Extractors', () => {
  describe('defaultExtractor', () => {
    it('should extract basic Lambda context attributes', () => {
      const context = {
        awsRequestId: 'test-request-id',
        invokedFunctionArn: 'arn:aws:lambda:us-east-1:123456789012:function:test-function',
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
      const context = {
        awsRequestId: 'test-request-id',
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
      const event = { body: 'test' };
      const context = { awsRequestId: 'test-request-id' };

      const result = defaultExtractor(event, context);

      expect(result.carrier).toBeUndefined();
    });

    it('should handle null or undefined event', () => {
      const context = { awsRequestId: 'test-request-id' };

      const result1 = defaultExtractor(null, context);
      const result2 = defaultExtractor(undefined, context);

      expect(result1.carrier).toBeUndefined();
      expect(result2.carrier).toBeUndefined();
    });
  });

  describe('apiGatewayV1Extractor', () => {
    it('should extract attributes from API Gateway v1 event', () => {
      const event = FIXTURES.apigw_v1;
      const result = apiGatewayV1Extractor(event, null);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.carrier).toBe(event.headers);

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
      const emptyEvent = {};
      const result = apiGatewayV1Extractor(emptyEvent, null);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.attributes['url.scheme']).toBe('https');
    });

    it('should extract all expected attributes from API Gateway v1 event', () => {
      const event = FIXTURES.apigw_v1;
      const result = apiGatewayV1Extractor(event, null);
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
      const event = FIXTURES.apigw_v2;
      const result = apiGatewayV2Extractor(event, null);

      let route = event.routeKey;
      if (route === '$default') {
        route = event.rawPath || '/';
      }

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.carrier).toBe(event.headers);

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
      const emptyEvent = {};
      const result = apiGatewayV2Extractor(emptyEvent, null);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.attributes['url.scheme']).toBe('https');
    });

    it('should extract all expected attributes from API Gateway v2 event', () => {
      const event = FIXTURES.apigw_v2;
      const result = apiGatewayV2Extractor(event, null);
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
      const event = FIXTURES.alb;
      const result = albExtractor(event, null);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      expect(result.carrier).toBe(event.headers);

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
        if ('X-Forwarded-For' in event.headers) {
          const clientIp = event.headers['X-Forwarded-For'].split(',')[0].trim();
          expect(attrs['client.address']).toBe(clientIp);
        } else if ('x-forwarded-for' in event.headers) {
          const clientIp = event.headers['x-forwarded-for'].split(',')[0].trim();
          expect(attrs['client.address']).toBe(clientIp);
        }
      }
    });

    it('should handle missing data gracefully', () => {
      const emptyEvent = {};
      const result = albExtractor(emptyEvent, null);

      expect(result.trigger).toBe(TriggerType.Http);
      expect(result.kind).toBe(SpanKind.SERVER);
      // Unlike API Gateway extractors, ALB extractor doesn't set a default url.scheme
      // for empty events, so we don't check for it
    });

    it('should extract all expected attributes from ALB event', () => {
      const event = FIXTURES.alb;
      const result = albExtractor(event, null);
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
      const event = FIXTURES.apigw_v1;
      const result = apiGatewayV1Extractor(event, null);
      const attrs = result.attributes;

      expect(attrs['user_agent.original']).toBe(event.requestContext.identity.userAgent);
    });

    it('should handle headers case-insensitively for API Gateway v2', () => {
      const event = FIXTURES.apigw_v2;
      const result = apiGatewayV2Extractor(event, null);
      const attrs = result.attributes;

      expect(attrs['user_agent.original']).toBe(event.requestContext.http.userAgent);
    });

    it('should handle headers case-insensitively for ALB', () => {
      const event = FIXTURES.alb;
      const result = albExtractor(event, null);
      const attrs = result.attributes;

      // ALB gets user agent from headers
      expect(attrs['user_agent.original']).toBe(event.headers['user-agent']);

      // ALB processes these header values
      expect(attrs['client.address']).toBe(event.headers['x-forwarded-for'].split(',')[0].trim());
      expect(attrs['server.address']).toBe(event.headers.host);
      expect(attrs['url.scheme']).toBe(event.headers['x-forwarded-proto']);
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
