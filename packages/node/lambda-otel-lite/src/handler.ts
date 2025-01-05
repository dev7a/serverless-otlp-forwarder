import { SpanKind, SpanStatusCode, Tracer, Span, Context, Link, trace, ROOT_CONTEXT, propagation } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { isColdStart, setColdStart } from './telemetry/init';
import { ProcessorMode } from './types';
import { state, handlerComplete } from './state';
import logger from './extension/logger';

/**
 * Options for the traced handler function
 */
export interface TracedHandlerOptions<T> {
    /** OpenTelemetry tracer instance */
    tracer: Tracer;
    /** OpenTelemetry tracer provider instance */
    provider: NodeTracerProvider;
    /** Name of the span */
    name: string;
    /** Handler function that receives the span instance */
    fn: (span: Span) => Promise<T>;
    /** Optional Lambda event object. Used for extracting HTTP attributes and context */
    event?: any;
    /** Optional Lambda context object. Used for extracting FAAS attributes */
    context?: any;
    /** Optional span kind. Defaults to SERVER */
    kind?: SpanKind;
    /** Optional custom attributes to add to the span */
    attributes?: Record<string, any>;
    /** Optional span links */
    links?: Link[];
    /** Optional span start time */
    startTime?: number;
    /** Optional parent context for trace propagation */
    parentContext?: Context;
    /** Optional function to extract carrier from event for context propagation */
    getCarrier?: (event: any) => Record<string, any>;
}

/**
 * Creates a traced handler for AWS Lambda functions with automatic attribute extraction
 * and context propagation.
 * 
 * Features:
 * - Automatic cold start detection
 * - Lambda context attribute extraction (invocation ID, cloud resource ID, account ID)
 * - API Gateway event attribute extraction (HTTP method, route, etc.)
 * - Automatic context propagation from HTTP headers
 * - Custom context carrier extraction support
 * - HTTP response status code handling
 * - Error handling and recording
 * 
 * @example
 * Basic usage:
 * ```typescript
 * export const handler = async (event: any, context: any) => {
 *   return tracedHandler({
 *     tracer,
 *     provider,
 *     name: 'my-handler',
 *     event,
 *     context,
 *     fn: async (span) => {
 *       // Your handler code here
 *       return {
 *         statusCode: 200,
 *         body: 'Success'
 *       };
 *     }
 *   });
 * };
 * ```
 * 
 * @example
 * With custom context extraction:
 * ```typescript
 * export const handler = async (event: any, context: any) => {
 *   return tracedHandler({
 *     tracer,
 *     provider,
 *     name: 'my-handler',
 *     event,
 *     context,
 *     getCarrier: (evt) => evt.Records[0]?.messageAttributes || {},
 *     fn: async (span) => {
 *       // Your handler code here
 *     }
 *   });
 * };
 * ```
 * 
 * @template T The return type of the handler function
 * @param options Configuration options for the traced handler
 * @returns The result of the handler function
 */
export async function tracedHandler<T>(options: TracedHandlerOptions<T>): Promise<T> {
    let error: Error | undefined;
    let result: T;

    try {
        const wrapCallback = (fn: (span: Span) => Promise<T>) => {
            return async (span: Span) => {
                try {
                    if (isColdStart()) {
                        span.setAttribute('faas.cold_start', true);
                    }

                    // Extract attributes from Lambda context if available
                    if (options.context) {
                        if (options.context.awsRequestId) {
                            span.setAttribute('faas.invocation_id', options.context.awsRequestId);
                        }
                        if (options.context.invokedFunctionArn) {
                            const arnParts = options.context.invokedFunctionArn.split(':');
                            if (arnParts.length >= 5) {
                                span.setAttribute('cloud.resource_id', options.context.invokedFunctionArn);
                                span.setAttribute('cloud.account.id', arnParts[4]);
                            }
                        }
                    }

                    // Extract attributes from Lambda event if available
                    if (options.event && typeof options.event === 'object') {
                        if ('version' in options.event && options.event.version === '2.0') {
                            // API Gateway v2
                            span.setAttribute('faas.trigger', 'http');
                            span.setAttribute('http.route', options.event.routeKey || '');
                            if (options.event.requestContext?.http) {
                                const http = options.event.requestContext.http;
                                span.setAttribute('http.method', http.method || '');
                                span.setAttribute('http.target', http.path || '');
                                span.setAttribute('http.scheme', (http.protocol || '').toLowerCase());
                            }
                        } else if ('httpMethod' in options.event || 'requestContext' in options.event) {
                            // API Gateway v1
                            span.setAttribute('faas.trigger', 'http');
                            span.setAttribute('http.route', options.event.resource || '');
                            span.setAttribute('http.method', options.event.httpMethod || '');
                            span.setAttribute('http.target', options.event.path || '');
                            if (options.event.requestContext?.protocol) {
                                span.setAttribute('http.scheme', options.event.requestContext.protocol.toLowerCase());
                            }
                        }
                    }

                    // Add custom attributes
                    if (options.attributes) {
                        Object.entries(options.attributes).forEach(([key, value]) => {
                            span.setAttribute(key, value);
                        });
                    }

                    result = await fn(span);

                    // Handle HTTP response attributes
                    if (result && typeof result === 'object' && 'statusCode' in result) {
                        const statusCode = result.statusCode as number;
                        span.setAttribute('http.status_code', statusCode);
                        if (statusCode >= 500) {
                            span.setStatus({
                                code: SpanStatusCode.ERROR,
                                message: `HTTP ${statusCode} response`
                            });
                        } else {
                            span.setStatus({ code: SpanStatusCode.OK });
                        }
                    } else {
                        span.setStatus({ code: SpanStatusCode.OK });
                    }

                    return result;
                } catch (e) {
                    error = e as Error;
                    span.recordException(error);
                    span.setStatus({
                        code: SpanStatusCode.ERROR,
                        message: error.message
                    });
                    throw error;
                } finally {
                    span.end();
                    if (isColdStart()) {
                        setColdStart(false);
                    }
                }
            };
        };

        // Extract context from event if available
        let parentContext = options.parentContext;
        if (!parentContext && options.event) {
            try {
                if (options.getCarrier) {
                    const carrier = options.getCarrier(options.event);
                    if (carrier && Object.keys(carrier).length > 0) {
                        parentContext = propagation.extract(ROOT_CONTEXT, carrier);
                    }
                } else if (options.event.headers) {
                    parentContext = propagation.extract(ROOT_CONTEXT, options.event.headers);
                }
            } catch (error) {
                logger.warn('Failed to extract context:', error);
            }
        }

        // Start the span
        result = await options.tracer.startActiveSpan(
            options.name,
            {
                kind: options.kind ?? SpanKind.SERVER,
                links: options.links,
                startTime: options.startTime,
            },
            parentContext ?? ROOT_CONTEXT,
            wrapCallback(options.fn)
        );

        return result;
    } catch (e) {
        error = e as Error;
        throw error;
    } finally {
        // Handle completion based on processor mode
        if (state.mode === ProcessorMode.Sync || !state.extensionInitialized) {
            await options.provider.forceFlush();
        } else if (state.mode === ProcessorMode.Async) {
            handlerComplete.signal();
        }
    }
}

