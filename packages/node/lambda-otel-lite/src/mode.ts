/**
 * Controls how spans are processed and exported.
 */
export enum ProcessorMode {
  /**
   * Synchronous flush in handler thread. Best for development.
   */
  Sync = 'sync',

  /**
   * Asynchronous flush via extension. Best for production.
   */
  Async = 'async',

  /**
   * Let processor handle flushing. Best with BatchSpanProcessor.
   */
  Finalize = 'finalize',
}

/**
 * Get processor mode from environment variables
 * @param envVar - Name of the environment variable to read
 * @param defaultMode - Default mode if environment variable is not set
 */
export function processorModeFromEnv(
  envVar: string = 'LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE',
  defaultMode: ProcessorMode = ProcessorMode.Sync
): ProcessorMode {
  const envValue = process.env[envVar];
  // Handle undefined, null, or non-string values
  if (!envValue || typeof envValue !== 'string') {
    return defaultMode;
  }

  const value = envValue.trim().toLowerCase();
  if (!value) {
    return defaultMode;
  }

  if (Object.values(ProcessorMode).includes(value as ProcessorMode)) {
    return value as ProcessorMode;
  }
  throw new Error(
    `Invalid ${envVar}: ${envValue}. Must be one of: ${Object.values(ProcessorMode).join(', ')}`
  );
}
