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
  Other = 'other',
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

import type {
  APIGatewayProxyEventV2,
  APIGatewayProxyEvent,
  ALBEvent,
  Context as LambdaContextType,
} from 'aws-lambda';

// Remove the internal LambdaContext alias
// We will use Context from 'aws-lambda' directly
/**
 * Normalize headers by converting all keys to lowercase.
 *
 * HTTP header names are case-insensitive, so we normalize them to lowercase
 * for consistent processing. The X-Ray propagator does its own case-insensitive
 * lookup for X-Amzn-Trace-Id, so no special handling is needed.
 *
 * @param headers - The original headers object (may be undefined)
 * @returns A normalized copy of the headers, or undefined if no headers
 */
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

/**
 * Default attribute extractor that returns Lambda context attributes.
 * Extracts standard OpenTelemetry FaaS attributes from the Lambda context.
 */
export function defaultExtractor(event: unknown, context: unknown): SpanAttributes {
  const attributes: Record<string, string | number | boolean> = {};

  // Add invocation ID if available
  if (context && typeof context === 'object' && 'awsRequestId' in context) {
    const typedContext = context as LambdaContextType; // Use imported type
    attributes['faas.invocation_id'] = typedContext.awsRequestId;
  }

  // Add function ARN and account ID if available
  if (context && typeof context === 'object' && 'invokedFunctionArn' in context) {
    const typedContext = context as LambdaContextType; // Use imported type
    const arn = typedContext.invokedFunctionArn;
    attributes['cloud.resource_id'] = arn;
    // Extract account ID from ARN (arn:aws:lambda:region:account-id:...)
    const arnParts = arn.split(':');
    if (arnParts.length >= 5) {
      attributes['cloud.account.id'] = arnParts[4];
    }
  }

  // Extract carrier headers if present
  let carrier: Record<string, string> | undefined;
  if (
    event &&
    typeof event === 'object' &&
    'headers' in event &&
    event.headers &&
    typeof event.headers === 'object' &&
    !Array.isArray(event.headers)
  ) {
    // Only include if all values are strings
    const headersObj = event.headers as Record<string, unknown>;
    const allStringValues = Object.values(headersObj).every((v) => typeof v === 'string');
    if (allStringValues) {
      carrier = normalizeHeaders(headersObj as Record<string, string>);
    }
  }

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier,
    trigger: TriggerType.Other,
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
export function apiGatewayV2Extractor(
  event: APIGatewayProxyEventV2,
  context: LambdaContextType // Use imported type
): SpanAttributes {
  // Start with default attributes
  const base = defaultExtractor(event, context);
  const attributes = { ...base.attributes };

  const method = event.requestContext?.http?.method;

  // Add HTTP attributes
  if (method) {
    attributes['http.request.method'] = method;
  }

  if (event.rawPath) {
    attributes['url.path'] = event.rawPath;
  }

  if (event.rawQueryString && event.rawQueryString !== '') {
    attributes['url.query'] = event.rawQueryString;
  }

  attributes['url.scheme'] = 'https';

  if (event.requestContext?.http?.protocol) {
    const protocol = event.requestContext.http.protocol.toLowerCase();
    if (protocol.startsWith('http/')) {
      attributes['network.protocol.version'] = protocol.replace('http/', '');
    }
  }

  // Add route with special handling for $default
  if (event.routeKey) {
    if (event.routeKey === '$default') {
      attributes['http.route'] = event.rawPath || '/';
    } else {
      attributes['http.route'] = event.routeKey;
    }
  } else {
    attributes['http.route'] = '/';
  }

  if (event.requestContext?.http?.sourceIp) {
    attributes['client.address'] = event.requestContext.http.sourceIp;
  }

  if (event.requestContext?.http?.userAgent) {
    attributes['user_agent.original'] = event.requestContext.http.userAgent;
  }

  if (event.requestContext?.domainName) {
    attributes['server.address'] = event.requestContext.domainName;
  }

  // Normalize headers once and reuse
  const normalizedHeaders = event.headers ? normalizeHeaders(event.headers) : undefined;

  // Get method and route for span name
  const spanMethod = attributes['http.request.method'] || 'HTTP';
  const spanRoute = attributes['http.route'];

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: normalizedHeaders,
    trigger: TriggerType.Http,
    spanName: `${spanMethod} ${spanRoute}`,
  };
}

/**
 * Extract attributes from API Gateway V1 REST API events.
 */
export function apiGatewayV1Extractor(
  event: APIGatewayProxyEvent,
  context: LambdaContextType // Use imported type
): SpanAttributes {
  // Start with default attributes
  const base = defaultExtractor(event, context);
  const attributes = { ...base.attributes };

  const method = event.httpMethod;
  const route = event.resource || '/';

  // Add HTTP attributes
  if (method) {
    attributes['http.request.method'] = method;
  }

  if (event.path) {
    attributes['url.path'] = event.path;
  }

  // Handle query string parameters
  if (
    event.multiValueQueryStringParameters &&
    Object.keys(event.multiValueQueryStringParameters).length > 0
  ) {
    const queryParts: string[] = [];
    for (const [key, values] of Object.entries(event.multiValueQueryStringParameters)) {
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

  if (event.requestContext?.protocol) {
    const protocol = event.requestContext.protocol.toLowerCase();
    if (protocol.startsWith('http/')) {
      attributes['network.protocol.version'] = protocol.replace('http/', '');
    }
  }

  // Add route
  attributes['http.route'] = route;

  // Add source IP from identity
  if (event.requestContext?.identity?.sourceIp) {
    attributes['client.address'] = event.requestContext.identity.sourceIp;
  }

  // Add user agent from identity
  if (event.requestContext?.identity?.userAgent) {
    attributes['user_agent.original'] = event.requestContext.identity.userAgent;
  }

  // Normalize headers once and reuse
  const normalizedHeaders = event.headers ? normalizeHeaders(event.headers) : undefined;

  // Add server address from Host header
  if (normalizedHeaders?.['host']) {
    attributes['server.address'] = normalizedHeaders['host'];
  }

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: normalizedHeaders,
    trigger: TriggerType.Http,
    spanName: `${method} ${route}`,
  };
}

/**
 * Extract attributes from Application Load Balancer target group events.
 */
export function albExtractor(
  event: ALBEvent,
  context: LambdaContextType // Use imported type
): SpanAttributes {
  // Start with default attributes
  const base = defaultExtractor(event, context);
  const attributes = { ...base.attributes };

  const method = event.httpMethod;
  const path = event.path || '/';

  // Add HTTP attributes
  if (method) {
    attributes['http.request.method'] = method;
  }

  if (path) {
    attributes['url.path'] = path;
    attributes['http.route'] = path;
  }

  // Handle query string parameters
  if (event.queryStringParameters && Object.keys(event.queryStringParameters).length > 0) {
    const queryParts: string[] = [];
    for (const [key, value] of Object.entries(event.queryStringParameters)) {
      if (typeof value === 'string') {
        queryParts.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`);
      }
    }
    if (queryParts.length > 0) {
      attributes['url.query'] = queryParts.join('&');
    }
  }

  // Add ALB specific attributes
  if (event.requestContext?.elb?.targetGroupArn) {
    attributes['alb.target_group_arn'] = event.requestContext.elb.targetGroupArn;
  }

  // Normalize headers once and reuse
  const normalizedHeaders = event.headers ? normalizeHeaders(event.headers) : undefined;

  // Extract attributes from headers
  if (normalizedHeaders) {
    // Set URL scheme based on x-forwarded-proto
    if (normalizedHeaders['x-forwarded-proto']) {
      attributes['url.scheme'] = normalizedHeaders['x-forwarded-proto'];
    } else {
      attributes['url.scheme'] = 'http';
    }

    // Extract user agent
    if (typeof normalizedHeaders['user-agent'] === 'string') {
      attributes['user_agent.original'] = normalizedHeaders['user-agent'];
    }

    // Extract server address from host
    if (normalizedHeaders['host']) {
      attributes['server.address'] = normalizedHeaders['host'];
    }

    // Extract client IP from x-forwarded-for
    if (normalizedHeaders['x-forwarded-for']) {
      const clientIp = normalizedHeaders['x-forwarded-for'].split(',')[0]?.trim();
      if (clientIp) {
        attributes['client.address'] = clientIp;
      }
    }
  }

  // ALB uses HTTP/1.1
  attributes['network.protocol.version'] = '1.1';

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: normalizedHeaders,
    trigger: TriggerType.Http,
    spanName: `${method} ${path}`,
  };
}
