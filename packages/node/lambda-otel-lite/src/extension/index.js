import { ProcessorMode, processorModeFromEnv } from '../mode';
import { state } from '../internal/state';
import * as http from 'http';
import logger from '../internal/logger';


const httpAgent = new http.Agent({ 
  keepAlive: true,
  maxSockets: 1, // Lambda extensions typically only need 1 connection
  timeout: 3000
});

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
 * Request the next event from the Lambda Extensions API
 * @param {string} extensionId - The extension ID
 */
async function requestNextEvent(extensionId) {
  const nextUrl = `http://${process.env.AWS_LAMBDA_RUNTIME_API}/2020-01-01/extension/event/next`;
  const [host, port] = (process.env.AWS_LAMBDA_RUNTIME_API || '').split(':');

  try {
    logger.debug('[extension] requesting next event');
    const response = await syncHttpRequest({
      host: host || '169.254.100.1',
      port: port || '9001',
      path: '/2020-01-01/extension/event/next',
      method: 'GET',
      headers: {
        'Lambda-Extension-Identifier': extensionId
      },
      agent: httpAgent
    });

    if (response.status !== 200) {
      logger.warn(`[extension] unexpected status from next event request: ${response.status}`);
    }
  } catch (error) {
    logger.error('[extension] error requesting next event:', error);
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

  logger.debug('[extension] SIGTERM received, flushing traces and shutting down');
  await state.provider.forceFlush();
  await state.provider.shutdown();
  process.exit(0);
}


// This is called at startup via --require
async function initializeInternalExtension() {
  const processorMode = processorModeFromEnv();
  // Get processor mode from env vars
  state.mode = processorMode;
  logger.debug(`[extension] processor mode: ${processorMode}`);

  // Only initialize extension for async and finalize modes
  if (state.mode === ProcessorMode.Sync) {
    logger.debug('[extension] skipping extension initialization in sync mode');
    return false;
  }

  // Only async and finalize modes from this point on
  try {

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

    const extensionId = String(response.headers['lambda-extension-identifier']);
    if (!extensionId) {
      throw new Error(`Failed to get extension ID from registration. Status: ${response.status}, Body: ${response.body}`);
    }
    logger.debug(`[extension] registered for mode: ${state.mode}`);
    state.extensionInitialized = true;

    // Register SIGTERM handler
    process.on('SIGTERM', shutdownTelemetry);

    if (processorMode === ProcessorMode.Async) {
      // Set up handler complete listener before making any requests
      state.handlerComplete.on(async () => {
        // this is called when the handler is complete after each invocation in async mode
        // Handle span flushing first
        if (state.provider) {
          await state.provider.forceFlush();
        }
        await requestNextEvent(extensionId);
      });

      // Wait for Lambda initialization to complete
      // Since the extension is loaded via --require, it starts before the main Lambda handler.
      // We use the provider (set during initTelemetry()) as a signal that the Lambda initialization
      // is complete and the runtime is ready to process events.
      // This ensures proper sequencing of initialization and event processing.
      // while (!state.provider) {
      //     await new Promise(resolve => setTimeout(resolve, 1));
      // }
      logger.debug('[extension] initialized');

      // Initial event request to start the chain in async mode
      await requestNextEvent(extensionId);
    } else if (processorMode === ProcessorMode.Finalize) {
      // since we haven't registered events to be processed, this will just wait for SIGTERM
      await requestNextEvent(extensionId);
    }
    return true;
  } catch (error) {
    logger.error('[extension] failed to initialize extension:', error);
    return false;
  }
}

// Initialize immediately when loaded via --require
if (process.env.AWS_LAMBDA_RUNTIME_API) {
  logger.debug('initializing internal extension');
  // Use an IIFE to make this synchronous
  (async () => {
    try {
      await initializeInternalExtension();
      logger.debug(`[extension] initialized with result: ${state.extensionInitialized}`);
    } catch (error) {
      logger.error('[extension] failed to initialize extension:', error);
      state.extensionInitialized = false;
    }
  })();
}
