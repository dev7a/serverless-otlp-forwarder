/**
 * Environment variable names and default values used for configuration.
 *
 * This file centralizes all constants to ensure consistency across the codebase
 * and provide a single source of truth for configuration parameters.
 */

/**
 * Environment variable names for span processor configuration
 */
export const ENV_VARS = {
  /**
   * Controls span processing strategy:
   * - 'sync': Direct export in handler thread (default)
   * - 'async': Deferred export via extension
   * - 'finalize': Custom export strategy
   */
  PROCESSOR_MODE: 'LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE',

  /**
   * Maximum number of spans that can be queued (default: 2048)
   */
  QUEUE_SIZE: 'LAMBDA_SPAN_PROCESSOR_QUEUE_SIZE',

  /**
   * GZIP compression level for stdout exporter:
   * - 0: No compression
   * - 1: Best speed
   * - 6: Good balance between size and speed (default)
   * - 9: Best compression
   */
  COMPRESSION_LEVEL: 'OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL',

  /**
   * Service name (defaults to function name if not specified)
   */
  SERVICE_NAME: 'OTEL_SERVICE_NAME',

  /**
   * Additional resource attributes in key=value,key2=value2 format
   */
  RESOURCE_ATTRIBUTES: 'OTEL_RESOURCE_ATTRIBUTES',

  /**
   * Comma-separated list of propagators to use
   * Supported values: tracecontext, xray, xray-lambda, none
   */
  OTEL_PROPAGATORS: 'OTEL_PROPAGATORS',
};

/**
 * Default values for configuration parameters
 */
export const DEFAULTS = {
  /**
   * Default maximum number of spans that can be queued
   */
  QUEUE_SIZE: 2048,

  /**
   * Default GZIP compression level
   */
  COMPRESSION_LEVEL: 6,

  /**
   * Default service name if no environment variables are set
   */
  SERVICE_NAME: 'unknown_service',

  /**
   * Default processor mode
   */
  PROCESSOR_MODE: 'sync',
};

/**
 * Resource attribute keys used in the Lambda resource
 */
export const RESOURCE_ATTRIBUTES = {
  /**
   * Current processing mode ('sync', 'async', or 'finalize')
   */
  PROCESSOR_MODE: 'lambda_otel_lite.extension.span_processor_mode',

  /**
   * Maximum number of spans that can be queued
   */
  QUEUE_SIZE: 'lambda_otel_lite.lambda_span_processor.queue_size',

  /**
   * GZIP compression level used for span export
   */
  COMPRESSION_LEVEL: 'lambda_otel_lite.otlp_stdout_span_exporter.compression_level',
};
