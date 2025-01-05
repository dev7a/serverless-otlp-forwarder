import { DiagLogLevel } from '@opentelemetry/api';
export declare const diagLevel: DiagLogLevel;
/**
 * Measure execution time of an async operation if debug logging is enabled
 * @template T
 * @param {() => Promise<T>} operation - The async operation to measure
 * @param {string} description - Description of the operation for logging
 * @returns {Promise<T>}
 */
export declare function withDebugTiming<T>(operation: () => Promise<T>, description: string): Promise<T>;
export * from './telemetry';
export * from './types/index';
export { tracedHandler } from './handler';
export { initTelemetry } from './telemetry/init';
//# sourceMappingURL=index.d.ts.map