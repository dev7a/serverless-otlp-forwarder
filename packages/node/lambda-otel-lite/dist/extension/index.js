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
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const types_1 = require("../types");
const state_1 = require("../state");
const http = __importStar(require("http"));
const logger_1 = __importDefault(require("./logger"));
/**
 * Make a synchronous HTTP request
 * @param {import('http').RequestOptions} options - HTTP request options
 * @param {string} [data] - Optional request body
 * @returns {Promise<{status: number, headers: import('http').IncomingHttpHeaders, body: string}>}
 */
function syncHttpRequest(options, data) {
    return new Promise((resolve, reject) => {
        const req = http.request(options, (res) => {
            let responseBody = '';
            res.on('data', chunk => responseBody += chunk);
            res.on('end', () => resolve({
                status: res.statusCode || 500,
                headers: res.headers,
                body: responseBody
            }));
        });
        req.on('error', reject);
        if (data) {
            req.write(data);
        }
        req.end();
    });
}
/**
 * Track flush counter and threshold
 * @type {number}
 */
let _flushCounter = 0;
const _flushThreshold = parseInt(process.env.LAMBDA_EXTENSION_SPAN_PROCESSOR_FREQUENCY || '1', 10);
/**
 * Request the next event from the Lambda Extensions API
 * @param {string} extensionId - The extension ID
 */
async function requestNextEvent(extensionId) {
    const nextUrl = `http://${process.env.AWS_LAMBDA_RUNTIME_API}/2020-01-01/extension/event/next`;
    try {
        logger_1.default.debug('extension requesting next event');
        const response = await fetch(nextUrl, {
            method: 'GET',
            headers: {
                'Lambda-Extension-Identifier': extensionId
            }
        });
        // Always consume the response buffer
        await response.arrayBuffer();
        if (response.status !== 200) {
            logger_1.default.warn(`unexpected status from next event request: ${response.status}`);
        }
    }
    catch (error) {
        logger_1.default.error('error requesting next event:', error);
    }
}
/**
 * Handle SIGTERM by flushing spans and shutting down
 */
async function shutdownTelemetry() {
    if (!state_1.state.provider || !state_1.state.provider.forceFlush || !state_1.state.provider.shutdown) {
        logger_1.default.warn('provider not initialized or missing required methods');
        return;
    }
    logger_1.default.debug('SIGTERM received, flushing traces and shutting down');
    await state_1.state.provider.forceFlush();
    await state_1.state.provider.shutdown();
    process.exit(0);
}
// This is called at startup via --require
async function initializeInternalExtension() {
    const processorMode = (0, types_1.processorModeFromEnv)();
    // Get processor mode from env vars
    state_1.state.mode = processorMode;
    logger_1.default.debug(`processor mode: ${processorMode}`);
    // Only initialize extension for async and finalize modes
    if (state_1.state.mode === types_1.ProcessorMode.Sync) {
        logger_1.default.debug('skipping extension initialization in sync mode');
        return false;
    }
    // Only async and finalize modes from this point on
    try {
        // Register SIGTERM handler
        process.on('SIGTERM', shutdownTelemetry);
        logger_1.default.debug('registered SIGTERM handler');
        const events = processorMode === types_1.ProcessorMode.Async ? ['INVOKE'] : [];
        // Use synchronous HTTP request for registration
        const runtimeApi = /** @type {string} */ (process.env.AWS_LAMBDA_RUNTIME_API);
        const [host, port] = runtimeApi.split(':');
        const response = await syncHttpRequest({
            host: host || '169.254.100.1',
            port: port || '9001',
            path: '/2020-01-01/extension/register',
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Lambda-Extension-Name': 'internal'
            }
        }, JSON.stringify({ events }));
        logger_1.default.debug(`extension registration response status: ${response.status}`);
        const extensionId = response.headers['lambda-extension-identifier'];
        if (!extensionId) {
            throw new Error(`Failed to get extension ID from registration. Status: ${response.status}, Body: ${response.body}`);
        }
        logger_1.default.debug(`internal extension '${extensionId}' registered for mode: ${state_1.state.mode}`);
        // Start extension loop if in async mode
        if (processorMode === types_1.ProcessorMode.Async) {
            state_1.state.handlerComplete.on(async () => {
                logger_1.default.debug('handler complete event received');
                try {
                    if (state_1.state.provider && state_1.state.provider.forceFlush) {
                        // Increment flush counter
                        _flushCounter++;
                        // Flush when counter reaches threshold
                        if (_flushCounter >= _flushThreshold) {
                            // Add a small delay with setTimeout to allow runtime's /next request
                            await new Promise(resolve => setTimeout(resolve, 5));
                            await state_1.state.provider.forceFlush();
                            _flushCounter = 0;
                        }
                    }
                }
                finally {
                    // Request next event after handling is complete
                    await requestNextEvent(extensionId.toString());
                }
            });
            // Request first event to start the chain
            await requestNextEvent(extensionId.toString());
            logger_1.default.debug('received first event');
            return true;
        }
        return true;
    }
    catch (error) {
        logger_1.default.error('failed to initialize extension:', error);
        return false;
    }
}
// Initialize immediately when loaded via --require
if (process.env.AWS_LAMBDA_RUNTIME_API) {
    logger_1.default.debug('initializing internal extension');
    // Use an IIFE to make this synchronous
    (async () => {
        try {
            state_1.state.extensionInitialized = await initializeInternalExtension();
        }
        catch (error) {
            logger_1.default.error('failed to initialize extension:', error);
            state_1.state.extensionInitialized = false;
        }
    })();
}
//# sourceMappingURL=index.js.map