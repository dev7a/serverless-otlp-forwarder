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

/** API Gateway V2 HTTP API event interface */
export interface APIGatewayV2Event {
  requestContext?: {
    http?: {
      method?: string;
      protocol?: string;
      sourceIp?: string;
      userAgent?: string;
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
      userAgent?: string;
    };
    domainName?: string;
  };
  headers?: Record<string, string>;
}

/** Application Load Balancer event interface */
export interface ALBEvent {
  httpMethod?: string;
  path?: string;
  queryStringParameters?: Record<string, string>;
  requestContext?: {
    elb?: {
      targetGroupArn?: string;
    };
  };
  headers?: Record<string, string>;
}

/** Lambda context interface for type safety */
interface LambdaContext {
  awsRequestId: string;
  invokedFunctionArn: string;
}

/**
 * Normalize headers by converting all keys to lowercase.
 * This makes header lookup case-insensitive.
 */
function normalizeHeaders(headers?: Record<string, string>): Record<string, string> | undefined {
  if (!headers) {
    return undefined;
  }
  return Object.entries(headers).reduce(
    (acc, [key, value]) => {
      acc[key.toLowerCase()] = value;
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
    const typedContext = context as LambdaContext;
    attributes['faas.invocation_id'] = typedContext.awsRequestId;
  }

  // Add function ARN and account ID if available
  if (context && typeof context === 'object' && 'invokedFunctionArn' in context) {
    const typedContext = context as LambdaContext;
    const arn = typedContext.invokedFunctionArn;
    attributes['cloud.resource_id'] = arn;
    // Extract account ID from ARN (arn:aws:lambda:region:account-id:...)
    const arnParts = arn.split(':');
    if (arnParts.length >= 5) {
      attributes['cloud.account.id'] = arnParts[4];
    }
  }

  return {
    kind: SpanKind.SERVER,
    attributes,
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
export function apiGatewayV2Extractor(event: unknown, context: unknown): SpanAttributes {
  // Start with default attributes
  const base = defaultExtractor(event, context);
  const attributes = { ...base.attributes };

  const apiEvent = event as APIGatewayV2Event;
  const method = apiEvent?.requestContext?.http?.method;

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

  attributes['url.scheme'] = 'https';

  if (apiEvent?.requestContext?.http?.protocol) {
    const protocol = apiEvent.requestContext.http.protocol.toLowerCase();
    if (protocol.startsWith('http/')) {
      attributes['network.protocol.version'] = protocol.replace('http/', '');
    }
  }

  // Add route with special handling for $default
  if (apiEvent?.routeKey) {
    if (apiEvent.routeKey === '$default') {
      attributes['http.route'] = apiEvent?.rawPath || '/';
    } else {
      attributes['http.route'] = apiEvent.routeKey;
    }
  } else {
    attributes['http.route'] = '/';
  }

  if (apiEvent?.requestContext?.http?.sourceIp) {
    attributes['client.address'] = apiEvent.requestContext.http.sourceIp;
  }

  if (apiEvent?.requestContext?.http?.userAgent) {
    attributes['user_agent.original'] = apiEvent.requestContext.http.userAgent;
  }

  if (apiEvent?.requestContext?.domainName) {
    attributes['server.address'] = apiEvent.requestContext.domainName;
  }

  // Get method and route for span name
  const spanMethod = attributes['http.request.method'] || 'HTTP';
  const spanRoute = attributes['http.route'];

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: apiEvent?.headers,
    trigger: TriggerType.Http,
    spanName: `${spanMethod} ${spanRoute}`,
  };
}

/**
 * Extract attributes from API Gateway V1 REST API events.
 */
export function apiGatewayV1Extractor(event: unknown, context: unknown): SpanAttributes {
  // Start with default attributes
  const base = defaultExtractor(event, context);
  const attributes = { ...base.attributes };

  const apiEvent = event as APIGatewayV1Event;
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
  if (
    apiEvent?.multiValueQueryStringParameters &&
    Object.keys(apiEvent.multiValueQueryStringParameters).length > 0
  ) {
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

  // Add source IP from identity
  if (apiEvent?.requestContext?.identity?.sourceIp) {
    attributes['client.address'] = apiEvent.requestContext.identity.sourceIp;
  }

  // Add user agent from identity
  if (apiEvent?.requestContext?.identity?.userAgent) {
    attributes['user_agent.original'] = apiEvent.requestContext.identity.userAgent;
  }

  // Add server address from Host header
  if (apiEvent?.headers) {
    const normalizedHeaders = normalizeHeaders(apiEvent.headers);
    if (normalizedHeaders?.['host']) {
      attributes['server.address'] = normalizedHeaders['host'];
    }
  }

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: apiEvent?.headers,
    trigger: TriggerType.Http,
    spanName: `${method} ${route}`,
  };
}

/**
 * Extract attributes from Application Load Balancer target group events.
 */
export function albExtractor(event: unknown, context: unknown): SpanAttributes {
  // Start with default attributes
  const base = defaultExtractor(event, context);
  const attributes = { ...base.attributes };

  const albEvent = event as ALBEvent;
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
  if (albEvent?.queryStringParameters && Object.keys(albEvent.queryStringParameters).length > 0) {
    const queryParts: string[] = [];
    for (const [key, value] of Object.entries(albEvent.queryStringParameters)) {
      queryParts.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`);
    }
    if (queryParts.length > 0) {
      attributes['url.query'] = queryParts.join('&');
    }
  }

  // Add ALB specific attributes
  if (albEvent?.requestContext?.elb?.targetGroupArn) {
    attributes['alb.target_group_arn'] = albEvent.requestContext.elb.targetGroupArn;
  }

  // Extract attributes from headers
  if (albEvent?.headers) {
    const normalizedHeaders = normalizeHeaders(albEvent.headers);
    if (normalizedHeaders) {
      // Set URL scheme based on x-forwarded-proto
      if (normalizedHeaders['x-forwarded-proto']) {
        attributes['url.scheme'] = normalizedHeaders['x-forwarded-proto'];
      } else {
        attributes['url.scheme'] = 'http';
      }

      // Extract user agent
      if (normalizedHeaders['user-agent']) {
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
  }

  // ALB uses HTTP/1.1
  attributes['network.protocol.version'] = '1.1';

  return {
    kind: SpanKind.SERVER,
    attributes,
    carrier: albEvent?.headers,
    trigger: TriggerType.Http,
    spanName: `${method} ${path}`,
  };
}
