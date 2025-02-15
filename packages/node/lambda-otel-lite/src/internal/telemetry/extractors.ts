import { SpanKind, Link } from '@opentelemetry/api';

/**
 * Common trigger types for Lambda functions.
 * 
 * These follow OpenTelemetry semantic conventions:
 * - Datasource: Database triggers
 * - Http: HTTP/API triggers
 * - PubSub: Message/event triggers
 * - Timer: Schedule/cron triggers
 * - Other: Fallback for unknown types
 */
export enum TriggerType {
    Datasource = 'datasource',
    Http = 'http',
    PubSub = 'pubsub',
    Timer = 'timer',
    Other = 'other'
}

/**
 * Data extracted from a Lambda event for span creation.
 */
export interface SpanAttributes {
    /** Optional span kind (defaults to SERVER) */
    kind?: SpanKind;
    /** Optional span name override */
    spanName?: string;
    /** Custom attributes to add to the span */
    attributes: Record<string, string | number | boolean>;
    /** Optional carrier headers for context propagation */
    carrier?: Record<string, string>;
    /** Optional span links */
    links?: Link[];
    /** 
     * The type of trigger for this Lambda invocation.
     * While common triggers are defined in TriggerType enum (http, pubsub, etc.),
     * this is intentionally a string to allow for custom trigger types beyond
     * the predefined ones.
     */
    trigger?: string;
}

/** API Gateway V2 HTTP API event interface */
export interface APIGatewayV2Event {
  requestContext?: {
    http?: {
      method?: string;
      protocol?: string;
      sourceIp?: string;
    };
    domainName?: string;
  };
  rawPath?: string;
  rawQueryString?: string;
  routeKey?: string;
  headers?: Record<string, string>;
}

/** API Gateway V1 REST API event interface */
export interface APIGatewayV1Event {
  httpMethod?: string;
  resource?: string;
  path?: string;
  multiValueQueryStringParameters?: Record<string, string[]>;
  requestContext?: {
    protocol?: string;
    identity?: {
      sourceIp?: string;
    };
    domainName?: string;
  };
  headers?: Record<string, string>;
}

/** Application Load Balancer event interface */
export interface ALBEvent {
  httpMethod?: string;
  path?: string;
  multiValueQueryStringParameters?: Record<string, string[]>;
  requestContext?: {
    elb?: {
      targetGroupArn?: string;
    };
  };
  headers?: Record<string, string>;
}

/**
 * Default attribute extractor that returns empty attributes.
 * It will attempt to extract headers for context propagation if present.
 */
export function defaultExtractor(event: unknown, _context: unknown): SpanAttributes {
  const carrier = typeof event === 'object' && event !== null && 'headers' in event
    ? (event as { headers?: Record<string, string> }).headers
    : undefined;

  return {
    kind: SpanKind.SERVER,
    attributes: {},
    carrier,
    trigger: TriggerType.Other
  };
}

/**
 * Extract attributes from API Gateway V2 HTTP API events.
 * 
 * Extracts standard HTTP attributes following OpenTelemetry semantic conventions:
 * - http.request.method: The HTTP method
 * - url.path: The request path
 * - url.query: The query string if present
 * - url.scheme: The protocol scheme (always "https" for API Gateway)
 * - network.protocol.version: The HTTP protocol version
 * - http.route: The API Gateway route key
 * - client.address: The client's IP address
 * - user_agent.original: The user agent header
 * - server.address: The domain name
 */
export function apiGatewayV2Extractor(event: unknown, _context: unknown): SpanAttributes {
  const apiEvent = event as APIGatewayV2Event;
  const attributes: Record<string, string | number | boolean> = {};
  const method = apiEvent?.requestContext?.http?.method;
  const path = apiEvent?.rawPath || '/';

  // Add HTTP attributes
  if (method) {
    attributes['http.request.method'] = method;
  }

  if (apiEvent?.rawPath) {
    attributes['url.path'] = apiEvent.rawPath;
  }

  if (apiEvent?.rawQueryString && apiEvent.rawQueryString !== '') {
    attributes['url.query'] = apiEvent.rawQueryString;
  }

  // API Gateway is always HTTPS
  attributes['url.scheme'] = 'https';

  if (apiEvent?.requestContext?.http?.protocol) {
    const protocol = apiEvent.requestContext.http.protocol.toLowerCase();
    if (protocol.startsWith('http/')) {
      attributes['network.protocol.version'] = protocol.replace('http/', '');
    }
  }

  // Add route key
  if (apiEvent?.routeKey) {
    attributes['http.route'] = apiEvent.routeKey;
  }

  // Add source IP and user agent
  if (apiEvent?.requestContext?.http?.sourceIp) {
    attributes['client.address'] = apiEvent.requestContext.http.sourceIp;
  }

  if (apiEvent?.headers?.['user-agent']) {
    attributes['user_agent.original'] = apiEvent.headers['user-agent'];
  }

  // Add domain name
  if (apiEvent?.requestContext?.domainName) {
    attributes['server.address'] = apiEvent.requestContext.domainName;
  }

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: apiEvent?.headers,
    trigger: TriggerType.Http,
    spanName: `${method} ${path}`
  };
}

/**
 * Extract attributes from API Gateway V1 REST API events.
 * 
 * Extracts standard HTTP attributes following OpenTelemetry semantic conventions:
 * - http.request.method: The HTTP method
 * - url.path: The request path
 * - url.query: The query string
 * - url.scheme: The protocol scheme (always "https" for API Gateway)
 * - network.protocol.version: The HTTP protocol version
 * - http.route: The API Gateway resource path
 * - client.address: The client's IP address
 * - user_agent.original: The user agent header
 * - server.address: The domain name
 */
export function apiGatewayV1Extractor(event: unknown, _context: unknown): SpanAttributes {
  const apiEvent = event as APIGatewayV1Event;
  const attributes: Record<string, string | number | boolean> = {};
  const method = apiEvent?.httpMethod;
  const route = apiEvent?.resource || '/';

  // Add HTTP attributes
  if (method) {
    attributes['http.request.method'] = method;
  }

  if (apiEvent?.path) {
    attributes['url.path'] = apiEvent.path;
  }

  // Handle query string parameters
  if (apiEvent?.multiValueQueryStringParameters && Object.keys(apiEvent.multiValueQueryStringParameters).length > 0) {
    const queryParts: string[] = [];
    for (const [key, values] of Object.entries(apiEvent.multiValueQueryStringParameters)) {
      if (Array.isArray(values)) {
        for (const value of values) {
          queryParts.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`);
        }
      }
    }
    if (queryParts.length > 0) {
      attributes['url.query'] = queryParts.join('&');
    }
  }

  // API Gateway is always HTTPS
  attributes['url.scheme'] = 'https';

  if (apiEvent?.requestContext?.protocol) {
    const protocol = apiEvent.requestContext.protocol.toLowerCase();
    if (protocol.startsWith('http/')) {
      attributes['network.protocol.version'] = protocol.replace('http/', '');
    }
  }

  // Add route
  attributes['http.route'] = route;

  // Add source IP and user agent
  if (apiEvent?.requestContext?.identity?.sourceIp) {
    attributes['client.address'] = apiEvent.requestContext.identity.sourceIp;
  }

  if (apiEvent?.headers?.['User-Agent']) {
    attributes['user_agent.original'] = apiEvent.headers['User-Agent'];
  }

  // Add domain name
  if (apiEvent?.requestContext?.domainName) {
    attributes['server.address'] = apiEvent.requestContext.domainName;
  }

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: apiEvent?.headers,
    trigger: TriggerType.Http,
    spanName: `${method} ${route}`
  };
}

/**
 * Extract attributes from Application Load Balancer target group events.
 * 
 * Extracts standard HTTP attributes following OpenTelemetry semantic conventions:
 * - http.request.method: The HTTP method
 * - url.path: The request path
 * - url.query: The query string
 * - url.scheme: The protocol scheme (defaults to "http")
 * - network.protocol.version: The HTTP protocol version (always "1.1" for ALB)
 * - http.route: The request path
 * - client.address: The client's IP address (from x-forwarded-for)
 * - user_agent.original: The user agent header
 * - server.address: The host header
 * - alb.target_group_arn: The ARN of the target group
 */
export function albExtractor(event: unknown, _context: unknown): SpanAttributes {
  const albEvent = event as ALBEvent;
  const attributes: Record<string, string | number | boolean> = {};
  const method = albEvent?.httpMethod;
  const path = albEvent?.path || '/';

  // Add HTTP attributes
  if (method) {
    attributes['http.request.method'] = method;
  }

  if (path) {
    attributes['url.path'] = path;
    attributes['http.route'] = path;
  }

  // Handle query string parameters
  if (albEvent?.multiValueQueryStringParameters && Object.keys(albEvent.multiValueQueryStringParameters).length > 0) {
    const queryParts: string[] = [];
    for (const [key, values] of Object.entries(albEvent.multiValueQueryStringParameters)) {
      if (Array.isArray(values)) {
        for (const value of values) {
          queryParts.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`);
        }
      }
    }
    if (queryParts.length > 0) {
      attributes['url.query'] = queryParts.join('&');
    }
  }

  // ALB can be HTTP or HTTPS, default to HTTP
  attributes['url.scheme'] = 'http';
  // ALB uses HTTP/1.1
  attributes['network.protocol.version'] = '1.1';

  // Add ALB specific attributes
  if (albEvent?.requestContext?.elb?.targetGroupArn) {
    attributes['alb.target_group_arn'] = albEvent.requestContext.elb.targetGroupArn;
  }

  // Add source IP from X-Forwarded-For
  const forwardedFor = albEvent?.headers?.['x-forwarded-for'];
  if (forwardedFor) {
    const clientIp = forwardedFor.split(',')[0]?.trim();
    if (clientIp) {
      attributes['client.address'] = clientIp;
    }
  }

  // Add user agent
  if (albEvent?.headers?.['user-agent']) {
    attributes['user_agent.original'] = albEvent.headers['user-agent'];
  }

  // Add host as server address
  if (albEvent?.headers?.host) {
    attributes['server.address'] = albEvent.headers.host;
  }

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: albEvent?.headers,
    trigger: TriggerType.Http,
    spanName: `${method} ${path}`
  };
} 