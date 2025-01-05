import { Tracer } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { Resource } from '@opentelemetry/resources';
import { SpanProcessor, SpanExporter } from '@opentelemetry/sdk-trace-base';
export declare function isColdStart(): boolean;
export declare function setColdStart(value: boolean): void;
/**
 * Initializes OpenTelemetry telemetry for a Lambda function.
 *
 * @param name - Name of the tracer/service. Used as service.name if not overridden by env vars
 * @param options - Optional configuration options
 * @param options.resource - Custom Resource to use instead of auto-detected resources
 * @param options.spanProcessor - Custom SpanProcessor implementation to use instead of default LambdaSpanProcessor
 * @param options.exporter - Custom SpanExporter to use instead of default StdoutOTLPExporter
 * @returns Object containing the NodeTracerProvider and Tracer instances
 *
 * @example
 * Basic usage:
 * ```ts
 * const { provider, tracer } = initTelemetry('my-lambda');
 * ```
 *
 * @example
 * With custom exporter:
 * ```ts
 * const { provider, tracer } = initTelemetry('my-lambda', {
 *   exporter: new OTLPTraceExporter({
 *     url: 'http://collector:4318/v1/traces'
 *   })
 * });
 * ```
 *
 * @example
 * With custom resource attributes:
 * ```ts
 * const { provider, tracer } = initTelemetry('my-lambda', {
 *   resource: new Resource({
 *     'service.version': '1.0.0',
 *     'deployment.environment': 'production'
 *   })
 * });
 * ```
 *
 * @example
 * With BatchSpanProcessor:
 * ```ts
 * const { provider, tracer } = initTelemetry('my-lambda', {
 *   spanProcessor: new BatchSpanProcessor(new OTLPTraceExporter(), {
 *     maxQueueSize: 2048,
 *     scheduledDelayMillis: 1000,
 *     maxExportBatchSize: 512
 *   })
 * });
 * ```
 */
export declare function initTelemetry(name: string, options?: {
    resource?: Resource;
    spanProcessor?: SpanProcessor;
    exporter?: SpanExporter;
}): {
    provider: NodeTracerProvider;
    tracer: Tracer;
};
export declare function getTracerProvider(): any;
//# sourceMappingURL=init.d.ts.map