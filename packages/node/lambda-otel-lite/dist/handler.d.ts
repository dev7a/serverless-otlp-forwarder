import { SpanKind, Tracer, Span, Context, Link } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
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
export declare function tracedHandler<T>(options: TracedHandlerOptions<T>): Promise<T>;
//# sourceMappingURL=handler.d.ts.map