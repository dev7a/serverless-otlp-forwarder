interface Logger {
  debug: (...args: unknown[]) => void;
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error: (...args: unknown[]) => void;
}

// Simple logger with level filtering
const logLevel = (
  process.env.AWS_LAMBDA_LOG_LEVEL ||
  process.env.LOG_LEVEL ||
  'info'
).toLowerCase();

/* eslint-disable no-console */
const logger: Logger = {
  debug: (...args: unknown[]) => {
    if (logLevel === 'debug') {
      console.debug('[lambda-otel-lite]', ...args);
    }
  },
  info: (...args: unknown[]) => {
    if (logLevel === 'debug' || logLevel === 'info') {
      console.info('[lambda-otel-lite]', ...args);
    }
  },
  warn: (...args: unknown[]) => {
    if (logLevel !== 'error' && logLevel !== 'none') {
      console.warn('[lambda-otel-lite]', ...args);
    }
  },
  error: (...args: unknown[]) => {
    if (logLevel !== 'none') {
      console.error('[lambda-otel-lite]', ...args);
    }
  },
};
/* eslint-enable no-console */

export function createLogger(prefix: string): Logger {
  return {
    debug: (...args: unknown[]) => logger.debug(`[${prefix}]`, ...args),
    info: (...args: unknown[]) => logger.info(`[${prefix}]`, ...args),
    warn: (...args: unknown[]) => logger.warn(`[${prefix}]`, ...args),
    error: (...args: unknown[]) => logger.error(`[${prefix}]`, ...args),
  };
}

export default logger;
