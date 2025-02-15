import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { ProcessorMode } from '../mode';
import logger from './logger';

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

/** Interface for the global state */
interface LambdaOtelState {
  provider: NodeTracerProvider | null;
  mode: ProcessorMode | null;
  extensionInitialized: boolean;
  handlerComplete: Signal;
}

/** Extend NodeJS.Global to include our state */
declare global {
  // eslint-disable-next-line no-var
  var _lambdaOtelState: LambdaOtelState;
}

// Initialize global state if not exists
if (!global._lambdaOtelState) {
  logger.debug('initializing lambda-otel state');
  global._lambdaOtelState = {
    provider: null,
    mode: null,
    extensionInitialized: false,
    handlerComplete: new Signal()
  };
}

/**
 * Shared state for extension-processor communication
 */
export const state = global._lambdaOtelState;

// Export the handler complete signal for convenience
export const handlerComplete = state.handlerComplete; 