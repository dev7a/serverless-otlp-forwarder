import { ProcessorMode } from './types';
import logger from './extension/logger';

/**
 * A type-safe event signal that can be signaled and listened to
 */
class Signal {
    private listeners: Array<() => void> = [];

    /**
     * Signal all listeners that the event has occurred
     */
    signal(): void {
        this.listeners.forEach(listener => listener());
    }

    /**
     * Register a listener to be called when the event is signaled
     */
    on(listener: () => void): void {
        this.listeners.push(listener);
    }

    /**
     * Remove a listener
     */
    off(listener: () => void): void {
        const index = this.listeners.indexOf(listener);
        if (index !== -1) {
            this.listeners.splice(index, 1);
        }
    }
}

// Initialize global state if not exists
if (!(global as any)._lambdaOtelState) {
    logger.debug('initializing lambda-otel state');
    (global as any)._lambdaOtelState = {
        provider: null,
        mode: null as ProcessorMode | null,
        extensionInitialized: false,
        handlerCompleted: false,
        handlerComplete: new Signal()
    };
}

/**
 * Shared state for extension-processor communication
 */
export const state = (global as any)._lambdaOtelState;

// Export the handler complete signal for convenience
export const handlerComplete = state.handlerComplete; 