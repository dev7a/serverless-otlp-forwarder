import { diag } from '@opentelemetry/api';
import { ExportResult, ExportResultCode } from '@opentelemetry/core';
import { ProtobufTraceSerializer } from '@opentelemetry/otlp-transformer';
import { SpanExporter, ReadableSpan } from '@opentelemetry/sdk-trace-base';
import * as zlib from 'zlib';
import { VERSION } from './version';

const DEFAULT_ENDPOINT = 'http://localhost:4318/v1/traces';
const DEFAULT_COMPRESSION_LEVEL = 6;
const COMPRESSION_LEVEL_ENV_VAR = 'OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL';

/**
 * Configuration options for OTLPStdoutSpanExporter
 */
export interface OTLPStdoutSpanExporterConfig {
  /**
   * GZIP compression level (0-9, where 0 is no compression and 9 is maximum compression).
   * Environment variable OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL takes precedence if set.
   * Defaults to 6 if neither environment variable nor parameter is provided.
   */
  gzipLevel?: number;
}

/**
 * An OpenTelemetry span exporter that writes spans to stdout in OTLP format.
 * 
 * This exporter is particularly useful in serverless environments like AWS Lambda
 * where writing to stdout is a common pattern for exporting telemetry data.
 * 
 * Features:
 * - Uses OTLP Protobuf serialization for efficient encoding
 * - Applies GZIP compression with configurable levels
 * - Detects service name from environment variables
 * - Supports custom headers via environment variables
 * 
 * Configuration Precedence:
 * 1. Environment variables (highest precedence)
 * 2. Constructor parameters in config object
 * 3. Default values (lowest precedence)
 * 
 * Environment Variables:
 * - OTEL_SERVICE_NAME: Service name to use in output
 * - AWS_LAMBDA_FUNCTION_NAME: Fallback service name (if OTEL_SERVICE_NAME not set)
 * - OTEL_EXPORTER_OTLP_HEADERS: Global headers for OTLP export
 * - OTEL_EXPORTER_OTLP_TRACES_HEADERS: Trace-specific headers (takes precedence)
 * - OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL: GZIP compression level (0-9). Defaults to 6.
 * 
 * Output Format:
 * ```json
 * {
 *   "__otel_otlp_stdout": "0.1.0",
 *   "source": "my-service",
 *   "endpoint": "http://localhost:4318/v1/traces",
 *   "method": "POST",
 *   "content-type": "application/x-protobuf",
 *   "content-encoding": "gzip",
 *   "headers": {
 *     "tenant-id": "tenant-12345",
 *     "custom-header": "value"
 *   },
 *   "payload": "<base64-encoded-gzipped-protobuf>",
 *   "base64": true
 * }
 * ```
 * 
 * Example Usage:
 * ```typescript
 * // Basic usage with defaults
 * const exporter = new OTLPStdoutSpanExporter();
 * 
 * // Custom compression level (environment variable takes precedence if set)
 * const exporter = new OTLPStdoutSpanExporter({ gzipLevel: 9 }); // Use maximum compression
 * 
 * // With custom headers via environment variables
 * process.env.OTEL_EXPORTER_OTLP_HEADERS = 'tenant-id=tenant-12345,custom-header=value';
 * const exporter = new OTLPStdoutSpanExporter();
 * ```
 */
export class OTLPStdoutSpanExporter implements SpanExporter {
  private readonly endpoint: string;
  private readonly serviceName: string;
  private readonly gzipLevel: number;
  private readonly headers: Record<string, string>;

  /**
   * Creates a new OTLPStdoutSpanExporter
   * @param config - Optional configuration options for the exporter
   * @param config.gzipLevel - Optional GZIP compression level (0-9).
   *                    Environment variable OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL takes precedence if set.
   *                    Defaults to 6 if neither environment variable nor parameter is provided.
   */
  constructor(config?: OTLPStdoutSpanExporterConfig) {
    this.endpoint = DEFAULT_ENDPOINT;
    this.serviceName = process.env.OTEL_SERVICE_NAME || 
      process.env.AWS_LAMBDA_FUNCTION_NAME || 
      'unknown-service';
    
    // Set compression level with proper precedence:
    // 1. Environment variable (highest precedence)
    // 2. Constructor parameter from config object
    // 3. Default value (lowest precedence)
    const gzipLevel = config?.gzipLevel;
    const envValue = process.env[COMPRESSION_LEVEL_ENV_VAR];
    if (envValue !== undefined) {
      try {
        const level = parseInt(envValue, 10);
        if (!isNaN(level) && level >= 0 && level <= 9) {
          this.gzipLevel = level;
        } else {
          diag.warn(`Invalid compression level in ${COMPRESSION_LEVEL_ENV_VAR}: ${envValue}, using default level ${DEFAULT_COMPRESSION_LEVEL}`);
          this.gzipLevel = gzipLevel !== undefined ? gzipLevel : DEFAULT_COMPRESSION_LEVEL;
        }
      } catch {
        diag.warn(`Failed to parse ${COMPRESSION_LEVEL_ENV_VAR}: ${envValue}, using default level ${DEFAULT_COMPRESSION_LEVEL}`);
        this.gzipLevel = gzipLevel !== undefined ? gzipLevel : DEFAULT_COMPRESSION_LEVEL;
      }
    } else {
      // No environment variable, use parameter from config or default
      this.gzipLevel = gzipLevel !== undefined ? gzipLevel : DEFAULT_COMPRESSION_LEVEL;
    }
    
    this.headers = this.parseHeaders();
  }

  /**
   * Parse headers from environment variables.
   * Headers should be in the format: key1=value1,key2=value2
   * Filters out content-type and content-encoding as they are fixed.
   * If both OTLP_TRACES_HEADERS and OTLP_HEADERS are defined, merges them with
   * OTLP_TRACES_HEADERS taking precedence.
   * 
   * @returns Record of header key-value pairs
   */
  private parseHeaders(): Record<string, string> {
    return [
      process.env.OTEL_EXPORTER_OTLP_HEADERS,        // General headers first
      process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS  // Trace-specific headers override
    ]
      .filter((str): str is string => !!str)  // Type guard to filter out undefined
      .reduce((acc, headerStr) => ({
        ...acc,
        ...this.parseHeaderString(headerStr)
      }), {});
  }

  /**
   * Parse a header string in the format key1=value1,key2=value2
   * 
   * @param headerStr - The header string to parse
   * @returns Record of header key-value pairs
   */
  private parseHeaderString(headerStr: string): Record<string, string> {
    return headerStr
      .split(',')
      .map(pair => {
        const [key, ...valueParts] = pair.trim().split('=');
        return [key, valueParts.join('=')];  // Rejoin value parts with '='
      })
      .filter(([key, value]) => key && value && 
        !['content-type', 'content-encoding'].includes(key.toLowerCase().trim()))
      .reduce((acc, [key, value]) => ({
        ...acc,
        [key.trim()]: value.trim()
      }), {});
  }

  /**
   * Exports the spans by serializing them to OTLP Protobuf format, compressing with GZIP,
   * and writing to stdout as a structured JSON object.
   * 
   * @param spans - The spans to export
   * @param resultCallback - Callback to report the success/failure of the export
   */
  export(spans: ReadableSpan[], resultCallback: (result: ExportResult) => void): void {
    try {
      // Serialize spans to protobuf format using the OTLP transformer
      const serializedData = ProtobufTraceSerializer.serializeRequest(spans);
      if (!serializedData) {
        diag.error('Failed to serialize spans');
        return resultCallback({ code: ExportResultCode.FAILED });
      }

      // Compress the serialized data using GZIP with configured compression level
      const compressedData = zlib.gzipSync(serializedData, { level: this.gzipLevel });
      
      // Create the output object with metadata and payload
      const output: Record<string, any> = {
        __otel_otlp_stdout: VERSION,      // Package version for compatibility checking
        source: this.serviceName,          // Service name for identifying the source
        endpoint: this.endpoint,           // Target endpoint (for informational purposes)
        method: 'POST',                    // HTTP method that would be used in OTLP
        'content-type': 'application/x-protobuf',
        'content-encoding': 'gzip',
        payload: compressedData.toString('base64'),  // Base64 encoded GZIP'd protobuf
        base64: true                                 // Indicates payload encoding
      };

      // Add headers section only if there are custom headers
      if (Object.keys(this.headers).length > 0) {
        output.headers = this.headers;
      }

      // Write the formatted output to stdout
      process.stdout.write(JSON.stringify(output) + '\n', (err) => {
        if (err) {
          diag.error('Failed to write to stdout:', err);
          resultCallback({ code: ExportResultCode.FAILED });
        } else {
          resultCallback({ code: ExportResultCode.SUCCESS });
        }
      });
    } catch (e) {
      diag.error('Error in OTLPStdoutSpanExporter:', e);
      resultCallback({ code: ExportResultCode.FAILED });
    }
  }

  forceFlush(): Promise<void> {
    // Nothing to flush as we write immediately
    return Promise.resolve();
  }

  /**
   * Shutdown the exporter. This implementation is a no-op as stdout doesn't need cleanup.
   * @returns A promise that resolves immediately as there's nothing to clean up
   */
  shutdown(): Promise<void> {
    return Promise.resolve();
  }
} 