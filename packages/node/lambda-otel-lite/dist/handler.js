"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.tracedHandler = void 0;
const api_1 = require("@opentelemetry/api");
const init_1 = require("./telemetry/init");
const types_1 = require("./types");
const state_1 = require("./state");
const logger_1 = __importDefault(require("./extension/logger"));
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
async function tracedHandler(options) {
    let error;
    let result;
    try {
        const wrapCallback = (fn) => {
            return async (span) => {
                try {
                    if ((0, init_1.isColdStart)()) {
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
                        }
                        else if ('httpMethod' in options.event || 'requestContext' in options.event) {
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
                        const statusCode = result.statusCode;
                        span.setAttribute('http.status_code', statusCode);
                        if (statusCode >= 500) {
                            span.setStatus({
                                code: api_1.SpanStatusCode.ERROR,
                                message: `HTTP ${statusCode} response`
                            });
                        }
                        else {
                            span.setStatus({ code: api_1.SpanStatusCode.OK });
                        }
                    }
                    else {
                        span.setStatus({ code: api_1.SpanStatusCode.OK });
                    }
                    return result;
                }
                catch (e) {
                    error = e;
                    span.recordException(error);
                    span.setStatus({
                        code: api_1.SpanStatusCode.ERROR,
                        message: error.message
                    });
                    throw error;
                }
                finally {
                    span.end();
                    if ((0, init_1.isColdStart)()) {
                        (0, init_1.setColdStart)(false);
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
                        parentContext = api_1.propagation.extract(api_1.ROOT_CONTEXT, carrier);
                    }
                }
                else if (options.event.headers) {
                    parentContext = api_1.propagation.extract(api_1.ROOT_CONTEXT, options.event.headers);
                }
            }
            catch (error) {
                logger_1.default.warn('Failed to extract context:', error);
            }
        }
        // Start the span
        result = await options.tracer.startActiveSpan(options.name, {
            kind: options.kind ?? api_1.SpanKind.SERVER,
            links: options.links,
            startTime: options.startTime,
        }, parentContext ?? api_1.ROOT_CONTEXT, wrapCallback(options.fn));
        return result;
    }
    catch (e) {
        error = e;
        throw error;
    }
    finally {
        // Handle completion based on processor mode
        if (state_1.state.mode === types_1.ProcessorMode.Sync || !state_1.state.extensionInitialized) {
            await options.provider.forceFlush();
        }
        else if (state_1.state.mode === types_1.ProcessorMode.Async) {
            state_1.handlerComplete.signal();
        }
    }
}
exports.tracedHandler = tracedHandler;
//# sourceMappingURL=handler.js.map