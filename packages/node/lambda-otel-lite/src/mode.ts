import { ENV_VARS } from './constants';
import { createLogger } from './internal/logger';

const logger = createLogger('mode');

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
 * Resolves the processor mode with proper precedence:
 * 1. Environment variable (if set and valid)
 * 2. Programmatic configuration (if provided)
 * 3. Default value (sync)
 *
 * @param configMode - Optional processor mode from programmatic configuration
 * @param envVar - Name of the environment variable to read (default: LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE)
 * @returns The resolved processor mode
 */
export function resolveProcessorMode(
  configMode?: ProcessorMode,
  envVar: string = ENV_VARS.PROCESSOR_MODE
): ProcessorMode {
  const envValue = process.env[envVar];

  // If environment variable is set and not empty
  if (envValue && typeof envValue === 'string') {
    const value = envValue.trim().toLowerCase();
    if (value && Object.values(ProcessorMode).includes(value as ProcessorMode)) {
      return value as ProcessorMode;
    }

    // Invalid environment variable value - log warning and continue
    if (value) {
      logger.warn(
        `Invalid ${envVar}: ${envValue}. Must be one of: ${Object.values(ProcessorMode).join(', ')}. Using fallback.`
      );
    }
  }

  // Use config value if provided, otherwise default to Sync
  return configMode ?? ProcessorMode.Sync;
}
