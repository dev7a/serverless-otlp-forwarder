"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.processorModeFromEnv = exports.ProcessorMode = void 0;
/**
 * Controls how spans are processed and exported.
 */
var ProcessorMode;
(function (ProcessorMode) {
    /**
     * Synchronous flush in handler thread. Best for development.
     */
    ProcessorMode["Sync"] = "sync";
    /**
     * Asynchronous flush via extension. Best for production.
     */
    ProcessorMode["Async"] = "async";
    /**
     * Let processor handle flushing. Best with BatchSpanProcessor.
     */
    ProcessorMode["Finalize"] = "finalize";
})(ProcessorMode || (exports.ProcessorMode = ProcessorMode = {}));
/**
 * Get processor mode from environment variables
 * @param envVar - Name of the environment variable to read
 * @param defaultMode - Default mode if environment variable is not set
 */
function processorModeFromEnv(envVar = 'LAMBDA_EXTENSION_SPAN_PROCESSOR_MODE', defaultMode = ProcessorMode.Async) {
    const envValue = process.env[envVar];
    // Handle undefined, null, or non-string values
    if (!envValue || typeof envValue !== 'string') {
        return defaultMode;
    }
    const value = envValue.trim().toLowerCase();
    if (!value) {
        return defaultMode;
    }
    if (Object.values(ProcessorMode).includes(value)) {
        return value;
    }
    throw new Error(`Invalid ${envVar}: ${envValue}. Must be one of: ${Object.values(ProcessorMode).join(', ')}`);
}
exports.processorModeFromEnv = processorModeFromEnv;
//# sourceMappingURL=index.js.map