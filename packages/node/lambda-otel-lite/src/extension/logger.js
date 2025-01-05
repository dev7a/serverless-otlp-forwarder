// Simple logger for extension
const logLevel = (process.env.AWS_LAMBDA_LOG_LEVEL || process.env.LOG_LEVEL || 'info').toLowerCase();

/** @type {Record<'debug'|'info'|'warn'|'error', (...args: unknown[]) => void>} */
const logger = {
    debug: (...args) => {
        if (logLevel === 'debug') {
            console.debug('[extension]', ...args);
        }
    },
    info: (...args) => {
        if (logLevel === 'debug' || logLevel === 'info') {
            console.info('[extension]', ...args);
        }
    },
    warn: (...args) => {
        if (logLevel !== 'error' && logLevel !== 'none') {
            console.warn('[extension]', ...args);
        }
    },
    error: (...args) => {
        if (logLevel !== 'none') {
            console.error('[extension]', ...args);
        }
    }
};

export default logger; 