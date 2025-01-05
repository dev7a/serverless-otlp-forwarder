"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.handlerComplete = exports.state = void 0;
const logger_1 = __importDefault(require("./extension/logger"));
/**
 * A type-safe event signal that can be signaled and listened to
 */
class Signal {
    constructor() {
        this.listeners = [];
    }
    /**
     * Signal all listeners that the event has occurred
     */
    signal() {
        this.listeners.forEach(listener => listener());
    }
    /**
     * Register a listener to be called when the event is signaled
     */
    on(listener) {
        this.listeners.push(listener);
    }
    /**
     * Remove a listener
     */
    off(listener) {
        const index = this.listeners.indexOf(listener);
        if (index !== -1) {
            this.listeners.splice(index, 1);
        }
    }
}
// Initialize global state if not exists
if (!global._lambdaOtelState) {
    logger_1.default.debug('initializing lambda-otel state');
    global._lambdaOtelState = {
        provider: null,
        mode: null,
        extensionInitialized: false,
        handlerCompleted: false,
        handlerComplete: new Signal()
    };
}
/**
 * Shared state for extension-processor communication
 */
exports.state = global._lambdaOtelState;
// Export the handler complete signal for convenience
exports.handlerComplete = exports.state.handlerComplete;
//# sourceMappingURL=state.js.map