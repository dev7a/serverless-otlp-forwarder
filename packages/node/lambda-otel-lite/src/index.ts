import { diag, DiagLogLevel } from '@opentelemetry/api';

// Configure diagnostic logger based on log level
const logLevel = (process.env.AWS_LAMBDA_LOG_LEVEL || process.env.LOG_LEVEL || '').toLowerCase();
export const diagLevel = logLevel === 'debug' ? DiagLogLevel.DEBUG
    : logLevel === 'info' ? DiagLogLevel.INFO
    : logLevel === 'warn' ? DiagLogLevel.WARN
    : logLevel === 'error' ? DiagLogLevel.ERROR
    : DiagLogLevel.NONE;

diag.setLogger({
    verbose: (...args: any[]) => console.debug('[runtime]', ...args),
    debug: (...args: any[]) => console.debug('[runtime]', ...args),
    info: (...args: any[]) => console.info('[runtime]', ...args),
    warn: (...args: any[]) => console.warn('[runtime]', ...args),
    error: (...args: any[]) => console.error('[runtime]', ...args),
}, diagLevel);

/**
 * Measure execution time of an async operation if debug logging is enabled
 * @template T
 * @param {() => Promise<T>} operation - The async operation to measure
 * @param {string} description - Description of the operation for logging
 * @returns {Promise<T>}
 */
export async function withDebugTiming<T>(operation: () => Promise<T>, description: string): Promise<T> {
    // Only measure timing if debug logging is enabled
    if (diagLevel > DiagLogLevel.DEBUG) {
        return operation();
    }

    const start = performance.now();
    try {
        return await operation();
    } finally {
        const duration = Math.round(performance.now() - start);
        diag.debug(`${description} took ${duration}ms`);
    }
}

// Re-export all telemetry functionality
export * from './telemetry';
export * from './types/index';
export { tracedHandler } from './handler';
export { initTelemetry } from './telemetry/init';
