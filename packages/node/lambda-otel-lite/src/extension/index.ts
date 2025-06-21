import { ProcessorMode, resolveProcessorMode } from '../mode';
import { state } from '../internal/state';
import { createLogger } from '../internal/logger';
import * as http from 'http';

const logger = createLogger('extension');

// Configuration constants
const DEFAULT_HTTP_TIMEOUT_MS = 5000;

// Create HTTP agent for connection pooling
const httpAgent = new http.Agent({
  keepAlive: true,
  maxSockets: 1, // Lambda extensions typically only need 1 connection
});

// Types for better type safety
interface HttpResponse {
  status: number;
  headers: http.IncomingHttpHeaders;
  body: string;
}

interface ExtensionEvent {
  eventType: 'INVOKE' | 'SHUTDOWN';
  deadlineMs: number;
  requestId: string;
  invokedFunctionArn: string;
  tracing?: {
    type: string;
    value: string;
  };
}

interface ExtensionRegistrationRequest {
  events: string[];
}

/**
 * Parse Lambda Runtime API URL into host and port
 */
function parseRuntimeApi(): { host: string; port: number } {
  const runtimeApi = process.env.AWS_LAMBDA_RUNTIME_API;
  if (!runtimeApi) {
    throw new Error('AWS_LAMBDA_RUNTIME_API environment variable is not set');
  }

  const [host, port] = runtimeApi.split(':');
  const parsedPort = parseInt(port, 10);
  const isValidPort = !isNaN(parsedPort) && parsedPort >= 0 && parsedPort <= 65535;
  if (!isValidPort) {
    logger.warn(`[extension] Invalid port value "${port}" in AWS_LAMBDA_RUNTIME_API. Defaulting to 9001.`);
  }
  return {
    host: host || 'localhost',
    port: isValidPort ? parsedPort : 9001,
  };
}

/**
 * Make an HTTP request using the native http module with proper error handling
 * @param options HTTP request options
 * @param data Optional request body
 * @param timeoutMs Optional timeout in milliseconds. If not provided, no additional timeout is applied.
 */
async function httpRequest(
  options: http.RequestOptions,
  data?: string,
  timeoutMs?: number
): Promise<HttpResponse> {
  return new Promise((resolve, reject) => {
    // Use the pre-configured agent for connection pooling
    const requestOptions: http.RequestOptions = {
      ...options,
      agent: httpAgent,
    };

    const req = http.request(requestOptions, (res) => {
      let body = '';

      res.on('data', (chunk) => {
        body += chunk;
      });

      res.on('end', () => {
        resolve({
          status: res.statusCode || 500,
          headers: res.headers,
          body: body,
        });
      });
    });

    // Set custom timeout if provided
    if (timeoutMs !== undefined) {
      req.setTimeout(timeoutMs, () => {
        req.destroy();
        reject(new Error('HTTP request timeout'));
      });
    }

    req.on('error', (error) => {
      const nodeError = error as NodeJS.ErrnoException;
      if (nodeError.code === 'ECONNRESET' || nodeError.code === 'ETIMEDOUT') {
        logger.error('[extension] HTTP request timeout or connection reset');
        reject(new Error('HTTP request timeout or connection reset'));
      } else {
        logger.error('[extension] HTTP request failed:', error.message);
        reject(error);
      }
    });

    if (data) {
      req.write(data);
    }

    req.end();
  });
}

/**
 * Request the next event from the Lambda Extensions API
 * This is a long-polling request that should not timeout
 */
async function requestNextEvent(extensionId: string): Promise<void> {
  try {
    logger.debug('[extension] requesting next event');
    const { host, port } = parseRuntimeApi();

    // No timeout for long-polling - this request blocks until an event occurs
    const response = await httpRequest({
      host,
      port,
      path: '/2020-01-01/extension/event/next',
      method: 'GET',
      headers: {
        'Lambda-Extension-Identifier': extensionId,
      },
    });

    if (response.status !== 200) {
      logger.warn(
        `[extension] unexpected status from next event request: ${response.status}, body: ${response.body}`
      );
      return;
    }

    // Parse the event if needed for logging
    try {
      const event: ExtensionEvent = JSON.parse(response.body);
      logger.debug(`[extension] received event: ${event.eventType}`);
    } catch {
      logger.debug('[extension] received non-JSON event response');
    }
  } catch (error) {
    logger.error('[extension] error requesting next event:', error);
    throw error;
  }
}

/**
 * Handle SIGTERM by flushing spans and shutting down gracefully
 */
async function shutdownTelemetry(): Promise<void> {
  logger.debug('[extension] SIGTERM received, initiating graceful shutdown');

  if (!state.provider) {
    logger.warn('[extension] no provider available for shutdown');
    return;
  }

  if (
    typeof state.provider.forceFlush !== 'function' ||
    typeof state.provider.shutdown !== 'function'
  ) {
    logger.warn('[extension] provider missing required methods (forceFlush/shutdown)');
    return;
  }

  try {
    logger.debug('[extension] flushing spans before shutdown');
    await state.provider.forceFlush();

    logger.debug('[extension] shutting down provider');
    await state.provider.shutdown();

    logger.debug('[extension] graceful shutdown complete');
  } catch (error) {
    logger.error('[extension] error during shutdown:', error);
  } finally {
    // Exit cleanly - this is the expected behavior for Lambda extensions during SIGTERM
    process.exit(0);
  }
}

/**
 * Register the extension with Lambda Runtime API
 * This is an admin operation that should complete quickly
 */
async function registerExtension(events: string[]): Promise<string> {
  const { host, port } = parseRuntimeApi();
  const registrationData: ExtensionRegistrationRequest = { events };

  logger.debug(`[extension] registering extension with events: [${events.join(', ')}]`);

  // Use timeout for admin operations - they should complete quickly
  const response = await httpRequest(
    {
      host,
      port,
      path: '/2020-01-01/extension/register',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Lambda-Extension-Name': 'lambda-otel-lite-internal',
      },
    },
    JSON.stringify(registrationData),
    DEFAULT_HTTP_TIMEOUT_MS
  );

  if (response.status !== 200) {
    throw new Error(
      `Failed to register extension. Status: ${response.status}, Body: ${response.body}`
    );
  }

  let extensionId = response.headers['lambda-extension-identifier'];
  if (Array.isArray(extensionId)) {
    extensionId = extensionId[0]; // Extract the first element if it's an array
  }
  if (!extensionId || typeof extensionId !== 'string') {
    throw new Error(
      `Missing or invalid extension ID in response. Extension ID: ${JSON.stringify(extensionId)}`
    );
  }

  logger.debug(`[extension] successfully registered with ID: ${extensionId}`);
  return extensionId;
}

/**
 * Set up async mode event handling
 */
function setupAsyncMode(extensionId: string): void {
  logger.debug('[extension] setting up async mode event handling');

  state.handlerComplete.on(async () => {
    try {
      logger.debug('[extension] handler complete signal received');

      // Flush spans first
      if (state.provider && typeof state.provider.forceFlush === 'function') {
        logger.debug('[extension] flushing spans after handler completion');
        await state.provider.forceFlush();
      } else {
        logger.warn('[extension] provider not available or missing forceFlush method');
      }

      // Request next event to continue the cycle
      await requestNextEvent(extensionId);
    } catch (error) {
      logger.error('[extension] error in handler complete callback:', error);
      // Continue to request next event even if flush fails
      try {
        await requestNextEvent(extensionId);
      } catch (nextEventError) {
        logger.error('[extension] failed to request next event after error:', nextEventError);
      }
    }
  });
}

/**
 * Initialize the internal extension
 */
async function initializeInternalExtension(): Promise<boolean> {
  try {
    const processorMode = resolveProcessorMode();
    state.mode = processorMode;

    logger.debug(`[extension] processor mode: ${processorMode}`);

    // Only initialize extension for async and finalize modes
    if (state.mode === ProcessorMode.Sync) {
      logger.debug('[extension] skipping extension initialization in sync mode');
      return false;
    }

    // Determine which events to register for based on mode
    const events = processorMode === ProcessorMode.Async ? ['INVOKE'] : [];

    // Register extension
    const extensionId = await registerExtension(events);
    state.extensionInitialized = true;

    // Register SIGTERM handler for graceful shutdown
    process.on('SIGTERM', shutdownTelemetry);

    if (processorMode === ProcessorMode.Async) {
      // Set up async mode event handling
      setupAsyncMode(extensionId);

      // Start the event loop with initial request
      logger.debug('[extension] starting initial event request');
      await requestNextEvent(extensionId);
    } else if (processorMode === ProcessorMode.Finalize) {
      // For finalize mode, just wait for SIGTERM
      logger.debug('[extension] finalize mode - waiting for SIGTERM');
      await requestNextEvent(extensionId);
    }

    logger.debug('[extension] initialization complete');
    return true;
  } catch (error) {
    logger.error('[extension] failed to initialize extension:', error);
    state.extensionInitialized = false;
    return false;
  }
}

// Initialize immediately when loaded via --require
if (process.env.AWS_LAMBDA_RUNTIME_API) {
  logger.debug('[extension] AWS Lambda runtime detected, initializing extension');

  // Use IIFE to handle async initialization
  (async (): Promise<void> => {
    try {
      const result = await initializeInternalExtension();
      logger.debug(`[extension] initialization result: ${result ? 'success' : 'failed'}`);
    } catch (error) {
      logger.error('[extension] fatal error during initialization:', error);
      state.extensionInitialized = false;
    }
  })();
} else {
  logger.debug('[extension] not in Lambda environment, skipping extension initialization');
}
