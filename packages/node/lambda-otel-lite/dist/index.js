"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __exportStar = (this && this.__exportStar) || function(m, exports) {
    for (var p in m) if (p !== "default" && !Object.prototype.hasOwnProperty.call(exports, p)) __createBinding(exports, m, p);
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.initTelemetry = exports.tracedHandler = exports.withDebugTiming = exports.diagLevel = void 0;
const api_1 = require("@opentelemetry/api");
// Configure diagnostic logger based on log level
const logLevel = (process.env.AWS_LAMBDA_LOG_LEVEL || process.env.LOG_LEVEL || '').toLowerCase();
exports.diagLevel = logLevel === 'debug' ? api_1.DiagLogLevel.DEBUG
    : logLevel === 'info' ? api_1.DiagLogLevel.INFO
        : logLevel === 'warn' ? api_1.DiagLogLevel.WARN
            : logLevel === 'error' ? api_1.DiagLogLevel.ERROR
                : api_1.DiagLogLevel.NONE;
api_1.diag.setLogger({
    verbose: (...args) => console.debug('[runtime]', ...args),
    debug: (...args) => console.debug('[runtime]', ...args),
    info: (...args) => console.info('[runtime]', ...args),
    warn: (...args) => console.warn('[runtime]', ...args),
    error: (...args) => console.error('[runtime]', ...args),
}, exports.diagLevel);
/**
 * Measure execution time of an async operation if debug logging is enabled
 * @template T
 * @param {() => Promise<T>} operation - The async operation to measure
 * @param {string} description - Description of the operation for logging
 * @returns {Promise<T>}
 */
async function withDebugTiming(operation, description) {
    // Only measure timing if debug logging is enabled
    if (exports.diagLevel > api_1.DiagLogLevel.DEBUG) {
        return operation();
    }
    const start = performance.now();
    try {
        return await operation();
    }
    finally {
        const duration = Math.round(performance.now() - start);
        api_1.diag.debug(`${description} took ${duration}ms`);
    }
}
exports.withDebugTiming = withDebugTiming;
// Re-export all telemetry functionality
__exportStar(require("./telemetry"), exports);
__exportStar(require("./types/index"), exports);
var handler_1 = require("./handler");
Object.defineProperty(exports, "tracedHandler", { enumerable: true, get: function () { return handler_1.tracedHandler; } });
var init_1 = require("./telemetry/init");
Object.defineProperty(exports, "initTelemetry", { enumerable: true, get: function () { return init_1.initTelemetry; } });
//# sourceMappingURL=index.js.map