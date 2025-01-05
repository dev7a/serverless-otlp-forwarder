import { ProcessorMode, processorModeFromEnv } from '../types';
import { state } from '../state';
import * as http from 'http';
import logger from './logger';


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
        logger.debug('extension requesting next event');
        const response = await fetch(nextUrl, {
            method: 'GET',
            headers: {
                'Lambda-Extension-Identifier': extensionId
            }
        });
        // Always consume the response buffer
        await response.arrayBuffer();
        if (response.status !== 200) {
            logger.warn(`unexpected status from next event request: ${response.status}`);
        }
    } catch (error) {
        logger.error('error requesting next event:', error);
    }
}

/**
 * Handle SIGTERM by flushing spans and shutting down
 */
async function shutdownTelemetry() {
    if (!state.provider || !state.provider.forceFlush || !state.provider.shutdown) {
        logger.warn('provider not initialized or missing required methods');
        return;
    }

    logger.debug('SIGTERM received, flushing traces and shutting down');
    await state.provider.forceFlush();
    await state.provider.shutdown();
    process.exit(0);
}


// This is called at startup via --require
async function initializeInternalExtension() {
    const processorMode = processorModeFromEnv();
    // Get processor mode from env vars
    state.mode = processorMode;
    logger.debug(`processor mode: ${processorMode}`);

    // Only initialize extension for async and finalize modes
    if (state.mode === ProcessorMode.Sync) {
        logger.debug('skipping extension initialization in sync mode');
        return false;
    }

    // Only async and finalize modes from this point on
    try {
        // Register SIGTERM handler
        process.on('SIGTERM', shutdownTelemetry);
        logger.debug('registered SIGTERM handler');

        const events = processorMode === ProcessorMode.Async ? ['INVOKE'] : [];

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

        logger.debug(`extension registration response status: ${response.status}`);

        const extensionId = response.headers['lambda-extension-identifier'];
        if (!extensionId) {
            throw new Error(`Failed to get extension ID from registration. Status: ${response.status}, Body: ${response.body}`);
        }
        logger.debug(`internal extension '${extensionId}' registered for mode: ${state.mode}`);

        // Start extension loop if in async mode
        if (processorMode === ProcessorMode.Async) {
            state.handlerComplete.on(async () => {
                logger.debug('handler complete event received');
                try {
                    if (state.provider && state.provider.forceFlush) {
                        // Increment flush counter
                        _flushCounter++;
                        // Flush when counter reaches threshold
                        if (_flushCounter >= _flushThreshold) {
                            // Add a small delay with setTimeout to allow runtime's /next request
                            await new Promise(resolve => setTimeout(resolve, 5));
                            await state.provider.forceFlush();
                            _flushCounter = 0;
                        }
                    }
                } finally {
                    // Request next event after handling is complete
                    await requestNextEvent(extensionId.toString());
                }
            });                   
            // Request first event to start the chain
            await requestNextEvent(extensionId.toString());
            logger.debug('received first event');
            return true;
        } 
        return true;
    } catch (error) {
        logger.error('failed to initialize extension:', error);
        return false;
    }
}

// Initialize immediately when loaded via --require
if (process.env.AWS_LAMBDA_RUNTIME_API) {
    logger.debug('initializing internal extension');
    // Use an IIFE to make this synchronous
    (async () => {
        try {
            state.extensionInitialized = await initializeInternalExtension();
        } catch (error) {
            logger.error('failed to initialize extension:', error);
            state.extensionInitialized = false;
        }
    })();
}
