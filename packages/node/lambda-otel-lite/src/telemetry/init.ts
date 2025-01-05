import { trace, Tracer } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { Resource, detectResourcesSync, envDetectorSync, processDetectorSync } from '@opentelemetry/resources';
import { SpanProcessor, SpanExporter } from '@opentelemetry/sdk-trace-base';
import { awsLambdaDetectorSync } from '@opentelemetry/resource-detector-aws';
import { StdoutOTLPExporterNode } from '@dev7a/otlp-stdout-exporter';
import { state } from '../state';
import { LambdaSpanProcessor } from './processor';

// Track cold start
let _isColdStart = true;

export function isColdStart(): boolean {
    return _isColdStart;
}

export function setColdStart(value: boolean): void {
    _isColdStart = value;
}

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
export function initTelemetry(
    name: string,
    options?: {
        resource?: Resource,
        spanProcessor?: SpanProcessor,
        exporter?: SpanExporter
    }
): { provider: NodeTracerProvider; tracer: Tracer } {
    if (!name) {
        throw new Error('Tracer name must be provided to initTelemetry');
    }

    // Setup resource
    const baseResource = options?.resource || detectResourcesSync({
        detectors: [awsLambdaDetectorSync, envDetectorSync, processDetectorSync],
    });

    // Setup processor
    const processor = options?.spanProcessor || new LambdaSpanProcessor(
        options?.exporter || new StdoutOTLPExporterNode(),
        { maxQueueSize: parseInt(process.env.LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE || '2048', 10) }
    );

    // Create provider with resources
    const provider = new NodeTracerProvider({
        resource: new Resource({
            'service.name': process.env.OTEL_SERVICE_NAME || process.env.AWS_LAMBDA_FUNCTION_NAME || name,
            ...baseResource.attributes
        }),
        spanProcessors: [
            processor
        ]
    });


    // Store in shared state for extension
    state.provider = provider;

    // Register as global tracer
    provider.register();

    return { provider, tracer: trace.getTracer(name) };
}

export function getTracerProvider() {
    return state.provider;
} 