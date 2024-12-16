import { OTLPExporterBase } from '@opentelemetry/otlp-exporter-base';
import { OTLPExporterNodeConfigBase } from '@opentelemetry/otlp-exporter-base';
import { ISerializer } from '@opentelemetry/otlp-transformer';
import { IExporterTransport } from '@opentelemetry/otlp-exporter-base';
import { diag } from '@opentelemetry/api';
import { OTLPExporterError } from '@opentelemetry/otlp-exporter-base';
import {
  JsonTraceSerializer,
  ProtobufTraceSerializer,
  IExportTraceServiceResponse
} from '@opentelemetry/otlp-transformer';
import { ReadableSpan } from '@opentelemetry/sdk-trace-base';
import { createStdoutTransport } from './transport';
import { VERSION } from './version';

interface HttpConfiguration {
  headers?: Record<string, string>;
  endpoint?: string;
}

/**
 * Parses a comma-separated string of key=value pairs into a headers object.
 * 
 * @example
 * ```typescript
 * parseHeaders('key1=value1,key2=value2') 
 * // Returns: { key1: 'value1', key2: 'value2' }
 * ```
 * 
 * @param headersString - String in format 'key1=value1,key2=value2'
 * @returns Record of header key-value pairs
 */
function parseHeaders(headersString?: string): Record<string, string> {
  if (!headersString) {
    return {};
  }
  
  try {
    return Object.fromEntries(
      headersString.split(',').map(headerPair => {
        const [key, value] = headerPair.split('=');
        return [key.trim(), value.trim()];
      })
    );
  } catch (e) {
    diag.warn(`Failed to parse headers: ${headersString}. Error: ${e}`);
    return {};
  }
}

/**
 * Appends a resource path to a base URL, ensuring the resulting URL is valid.
 * Handles URL validation and proper path joining with slashes.
 * 
 * @example
 * ```typescript
 * appendResourcePathToUrl('http://example.com', 'v1/traces')
 * // Returns: 'http://example.com/v1/traces'
 * ```
 * 
 * @param url - Base URL to append to
 * @param path - Resource path to append
 * @returns Complete URL string or undefined if invalid
 */
function appendResourcePathToUrl(url: string, path: string): string | undefined {
  try {
    // Validate the URL first
    new URL(url);
  } catch {
    diag.warn(
      `Configuration: Could not parse environment-provided export URL: '${url}', falling back to undefined`
    );
    return undefined;
  }

  if (!url.endsWith('/')) {
    url = url + '/';
  }
  url += path;

  try {
    // Validate the final URL
    new URL(url);
    return url;
  } catch {
    diag.warn(
      `Configuration: Provided URL appended with '${path}' is not a valid URL, using 'undefined' instead of '${url}'`
    );
    return undefined;
  }
}

/**
 * Retrieves and processes OpenTelemetry configuration from environment variables.
 * Handles both generic OTLP configuration and trace-specific settings.
 * 
 * Environment variables processed:
 * - OTEL_EXPORTER_OTLP_HEADERS
 * - OTEL_EXPORTER_OTLP_TRACES_HEADERS
 * - OTEL_EXPORTER_OTLP_TRACES_ENDPOINT
 * - OTEL_EXPORTER_OTLP_ENDPOINT
 * 
 * @returns Configuration object with processed headers and endpoint
 */
function getEnvConfig(): HttpConfiguration {
  const headers = {
    ...parseHeaders(process.env.OTEL_EXPORTER_OTLP_HEADERS),
    ...parseHeaders(process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS)
  };

  // Handle signal-specific endpoint first
  let endpoint = process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT?.trim();
  
  // If no signal-specific endpoint, handle generic endpoint with path append
  if (!endpoint) {
    const genericEndpoint = process.env.OTEL_EXPORTER_OTLP_ENDPOINT?.trim();
    if (genericEndpoint) {
      endpoint = appendResourcePathToUrl(genericEndpoint, 'v1/traces');
    }
  }

  return { headers, endpoint: endpoint || '' };
}

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
export class StdoutOTLPExporterNode<
  ExportItem extends ReadableSpan
> extends OTLPExporterBase<OTLPExporterNodeConfigBase, ExportItem> {
  private static readonly VERSION = VERSION;
  protected _serializer: ISerializer<ExportItem[], IExportTraceServiceResponse>;
  protected _transport: IExporterTransport;
  protected _timeoutMillis: number;

  constructor(
    config: OTLPExporterNodeConfigBase = {}
  ) {
    super(config);
    
    if (config.timeoutMillis !== undefined && config.timeoutMillis <= 0) {
      throw new Error('timeoutMillis must be positive');
    }

    const protocol = process.env.OTEL_EXPORTER_OTLP_PROTOCOL?.toLowerCase();
    if (protocol && !['http/json', 'http/protobuf'].includes(protocol)) {
      throw new Error('Invalid OTEL_EXPORTER_OTLP_PROTOCOL value');
    }

    const contentType = protocol === 'http/json' ? 'application/json' : 'application/x-protobuf';
    
    // Get environment configuration
    const envConfig = getEnvConfig();
    
    // Merge configurations with precedence:
    // 1. Constructor config
    // 2. Environment variables
    // 3. Defaults
    const headers = {
      'content-type': contentType,
      ...envConfig.headers,
      ...config.headers,
    };

    this._serializer = protocol === 'http/json' ? JsonTraceSerializer : ProtobufTraceSerializer;

    this._transport = createStdoutTransport({
      config: {
        ...config,
        endpoint: config.url || envConfig.endpoint,
      },
      contentType,
      headers
    });
    this._timeoutMillis = config.timeoutMillis ?? 10000;
  }

  send(
    objects: ExportItem[],
    onSuccess: () => void,
    onError: (error: OTLPExporterError) => void
  ): void {
    if (this._shutdownOnce.isCalled) {
      diag.debug('Shutdown already started. Cannot send objects');
      return;
    }

    const data = this._serializer.serializeRequest(objects);

    if (data == null) {
      onError(new OTLPExporterError('Could not serialize message'));
      return;
    }

    const promise = this._transport
      .send(data, this._timeoutMillis)
      .then(response => {
        if (response.status === 'success') {
          onSuccess();
        } else if (response.status === 'failure' && response.error) {
          onError(response.error);
        } else if (response.status === 'retryable') {
          onError(new OTLPExporterError('Export failed with retryable status'));
        } else {
          onError(new OTLPExporterError('Export failed with unknown error'));
        }
      }, onError);

    this._sendingPromises.push(promise);
    const popPromise = () => {
      const index = this._sendingPromises.indexOf(promise);
      this._sendingPromises.splice(index, 1);
    };
    promise.then(popPromise, popPromise);
  }

  onShutdown(): void {
    // Nothing to clean up
  }
}
export type { StdoutTransportParameters } from './transport';
export { CompressionAlgorithm } from '@opentelemetry/otlp-exporter-base';

