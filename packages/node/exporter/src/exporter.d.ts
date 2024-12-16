import { OTLPExporterBase } from '@opentelemetry/otlp-exporter-base';
import { OTLPExporterNodeConfigBase } from '@opentelemetry/otlp-exporter-base';
import { ISerializer } from '@opentelemetry/otlp-transformer';
import { IExporterTransport } from '@opentelemetry/otlp-exporter-base';
import { OTLPExporterError } from '@opentelemetry/otlp-exporter-base';
import { IExportTraceServiceResponse } from '@opentelemetry/otlp-transformer';
import { ReadableSpan } from '@opentelemetry/sdk-trace-base';
/**
 * StdoutOTLPExporterNode exports OpenTelemetry spans to stdout in a format
 * compatible with serverless-otlp-forwarder.
 *
 * @example
 * ```typescript
 * const exporter = new StdoutOTLPExporterNode({
 *   compression: CompressionAlgorithm.GZIP,
 *   timeoutMillis: 5000,
 *   url: 'your-endpoint'
 * });
 * ```
 *
 * Configuration can be provided via environment variables:
 * - OTEL_EXPORTER_OTLP_PROTOCOL: 'http/json' or 'http/protobuf'
 * - OTEL_EXPORTER_OTLP_ENDPOINT: Endpoint URL
 * - OTEL_SERVICE_NAME: Service name
 * - OTEL_EXPORTER_OTLP_COMPRESSION: 'gzip' or 'none'
 */
export declare class StdoutOTLPExporterNode<ExportItem extends ReadableSpan> extends OTLPExporterBase<OTLPExporterNodeConfigBase, ExportItem> {
    private static readonly VERSION;
    protected _serializer: ISerializer<ExportItem[], IExportTraceServiceResponse>;
    protected _transport: IExporterTransport;
    protected _timeoutMillis: number;
    constructor(config?: OTLPExporterNodeConfigBase);
    send(objects: ExportItem[], onSuccess: () => void, onError: (error: OTLPExporterError) => void): void;
    onShutdown(): void;
}
export type { StdoutTransportParameters } from './transport';
export { CompressionAlgorithm } from '@opentelemetry/otlp-exporter-base';
