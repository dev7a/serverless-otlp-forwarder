/**
 * Controls how spans are processed and exported.
 */
export declare enum ProcessorMode {
    /**
     * Synchronous flush in handler thread. Best for development.
     */
    Sync = "sync",
    /**
     * Asynchronous flush via extension. Best for production.
     */
    Async = "async",
    /**
     * Let processor handle flushing. Best with BatchSpanProcessor.
     */
    Finalize = "finalize"
}
/**
 * Get processor mode from environment variables
 * @param envVar - Name of the environment variable to read
 * @param defaultMode - Default mode if environment variable is not set
 */
export declare function processorModeFromEnv(envVar?: string, defaultMode?: ProcessorMode): ProcessorMode;
//# sourceMappingURL=index.d.ts.map