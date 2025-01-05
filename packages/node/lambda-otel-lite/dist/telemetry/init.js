"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.getTracerProvider = exports.initTelemetry = exports.setColdStart = exports.isColdStart = void 0;
const api_1 = require("@opentelemetry/api");
const sdk_trace_node_1 = require("@opentelemetry/sdk-trace-node");
const resources_1 = require("@opentelemetry/resources");
const resource_detector_aws_1 = require("@opentelemetry/resource-detector-aws");
const otlp_stdout_exporter_1 = require("@dev7a/otlp-stdout-exporter");
const state_1 = require("../state");
const processor_1 = require("./processor");
// Track cold start
let _isColdStart = true;
function isColdStart() {
    return _isColdStart;
}
exports.isColdStart = isColdStart;
function setColdStart(value) {
    _isColdStart = value;
}
exports.setColdStart = setColdStart;
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
function initTelemetry(name, options) {
    if (!name) {
        throw new Error('Tracer name must be provided to initTelemetry');
    }
    // Setup resource
    const baseResource = options?.resource || (0, resources_1.detectResourcesSync)({
        detectors: [resource_detector_aws_1.awsLambdaDetectorSync, resources_1.envDetectorSync, resources_1.processDetectorSync],
    });
    // Setup processor
    const processor = options?.spanProcessor || new processor_1.LambdaSpanProcessor(options?.exporter || new otlp_stdout_exporter_1.StdoutOTLPExporterNode(), { maxQueueSize: parseInt(process.env.LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE || '2048', 10) });
    // Create provider with resources
    const provider = new sdk_trace_node_1.NodeTracerProvider({
        resource: new resources_1.Resource({
            'service.name': process.env.OTEL_SERVICE_NAME || process.env.AWS_LAMBDA_FUNCTION_NAME || name,
            ...baseResource.attributes
        }),
        spanProcessors: [
            processor
        ]
    });
    // Store in shared state for extension
    state_1.state.provider = provider;
    // Register as global tracer
    provider.register();
    return { provider, tracer: api_1.trace.getTracer(name) };
}
exports.initTelemetry = initTelemetry;
function getTracerProvider() {
    return state_1.state.provider;
}
exports.getTracerProvider = getTracerProvider;
//# sourceMappingURL=init.js.map