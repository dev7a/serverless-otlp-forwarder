"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CompressionAlgorithm = exports.StdoutOTLPExporterNode = void 0;
const otlp_exporter_base_1 = require("@opentelemetry/otlp-exporter-base");
const api_1 = require("@opentelemetry/api");
const otlp_exporter_base_2 = require("@opentelemetry/otlp-exporter-base");
const otlp_transformer_1 = require("@opentelemetry/otlp-transformer");
const transport_1 = require("./transport");
const version_1 = require("./version");
/**
 * Parses a comma-separated string of key=value pairs into a headers object.
 *
 * @example
 * ```typescript
 * parseHeaders('key1=value1,key2=value2')
 * // Returns: { key1: 'value1', key2: 'value2' }
 * ```
 *
 * @param headersString - String in format 'key1=value1,key2=value2'
 * @returns Record of header key-value pairs
 */
function parseHeaders(headersString) {
    if (!headersString)
        return {};
    try {
        return Object.fromEntries(headersString.split(',').map(headerPair => {
            const [key, value] = headerPair.split('=');
            return [key.trim(), value.trim()];
        }));
    }
    catch (e) {
        api_1.diag.warn(`Failed to parse headers: ${headersString}. Error: ${e}`);
        return {};
    }
}
/**
 * Appends a resource path to a base URL, ensuring the resulting URL is valid.
 * Handles URL validation and proper path joining with slashes.
 *
 * @example
 * ```typescript
 * appendResourcePathToUrl('http://example.com', 'v1/traces')
 * // Returns: 'http://example.com/v1/traces'
 * ```
 *
 * @param url - Base URL to append to
 * @param path - Resource path to append
 * @returns Complete URL string or undefined if invalid
 */
function appendResourcePathToUrl(url, path) {
    try {
        // Validate the URL first
        new URL(url);
    }
    catch {
        api_1.diag.warn(`Configuration: Could not parse environment-provided export URL: '${url}', falling back to undefined`);
        return undefined;
    }
    if (!url.endsWith('/')) {
        url = url + '/';
    }
    url += path;
    try {
        // Validate the final URL
        new URL(url);
        return url;
    }
    catch {
        api_1.diag.warn(`Configuration: Provided URL appended with '${path}' is not a valid URL, using 'undefined' instead of '${url}'`);
        return undefined;
    }
}
/**
 * Retrieves and processes OpenTelemetry configuration from environment variables.
 * Handles both generic OTLP configuration and trace-specific settings.
 *
 * Environment variables processed:
 * - OTEL_EXPORTER_OTLP_HEADERS
 * - OTEL_EXPORTER_OTLP_TRACES_HEADERS
 * - OTEL_EXPORTER_OTLP_TRACES_ENDPOINT
 * - OTEL_EXPORTER_OTLP_ENDPOINT
 *
 * @returns Configuration object with processed headers and endpoint
 */
function getEnvConfig() {
    const headers = {
        ...parseHeaders(process.env.OTEL_EXPORTER_OTLP_HEADERS),
        ...parseHeaders(process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS)
    };
    // Handle signal-specific endpoint first
    let endpoint = process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT?.trim();
    // If no signal-specific endpoint, handle generic endpoint with path append
    if (!endpoint) {
        const genericEndpoint = process.env.OTEL_EXPORTER_OTLP_ENDPOINT?.trim();
        if (genericEndpoint) {
            endpoint = appendResourcePathToUrl(genericEndpoint, 'v1/traces');
        }
    }
    return { headers, endpoint: endpoint || '' };
}
/**
 * StdoutOTLPExporterNode exports OpenTelemetry spans to stdout in a format
 * compatible with serverless-otlp-forwarder.
 *
 * @example
 * ```typescript
 * const exporter = new StdoutOTLPExporterNode({
 *   compression: CompressionAlgorithm.GZIP,
 *   timeoutMillis: 5000,
 *   url: 'your-endpoint'
 * });
 * ```
 *
 * Configuration can be provided via environment variables:
 * - OTEL_EXPORTER_OTLP_PROTOCOL: 'http/json' or 'http/protobuf'
 * - OTEL_EXPORTER_OTLP_ENDPOINT: Endpoint URL
 * - OTEL_SERVICE_NAME: Service name
 * - OTEL_EXPORTER_OTLP_COMPRESSION: 'gzip' or 'none'
 */
class StdoutOTLPExporterNode extends otlp_exporter_base_1.OTLPExporterBase {
    constructor(config = {}) {
        super(config);
        if (config.timeoutMillis !== undefined && config.timeoutMillis <= 0) {
            throw new Error('timeoutMillis must be positive');
        }
        const protocol = process.env.OTEL_EXPORTER_OTLP_PROTOCOL?.toLowerCase();
        if (protocol && !['http/json', 'http/protobuf'].includes(protocol)) {
            throw new Error('Invalid OTEL_EXPORTER_OTLP_PROTOCOL value');
        }
        const contentType = protocol === 'http/json' ? 'application/json' : 'application/x-protobuf';
        // Get environment configuration
        const envConfig = getEnvConfig();
        // Merge configurations with precedence:
        // 1. Constructor config
        // 2. Environment variables
        // 3. Defaults
        const headers = {
            'content-type': contentType,
            ...envConfig.headers,
            ...config.headers,
        };
        this._serializer = protocol === 'http/json' ? otlp_transformer_1.JsonTraceSerializer : otlp_transformer_1.ProtobufTraceSerializer;
        this._transport = (0, transport_1.createStdoutTransport)({
            config: {
                ...config,
                endpoint: config.url || envConfig.endpoint,
            },
            contentType,
            headers
        });
        this._timeoutMillis = config.timeoutMillis ?? 10000;
    }
    send(objects, onSuccess, onError) {
        if (this._shutdownOnce.isCalled) {
            api_1.diag.debug('Shutdown already started. Cannot send objects');
            return;
        }
        const data = this._serializer.serializeRequest(objects);
        if (data == null) {
            onError(new otlp_exporter_base_2.OTLPExporterError('Could not serialize message'));
            return;
        }
        const promise = this._transport
            .send(data, this._timeoutMillis)
            .then(response => {
            if (response.status === 'success') {
                onSuccess();
            }
            else if (response.status === 'failure' && response.error) {
                onError(response.error);
            }
            else if (response.status === 'retryable') {
                onError(new otlp_exporter_base_2.OTLPExporterError('Export failed with retryable status'));
            }
            else {
                onError(new otlp_exporter_base_2.OTLPExporterError('Export failed with unknown error'));
            }
        }, onError);
        this._sendingPromises.push(promise);
        const popPromise = () => {
            const index = this._sendingPromises.indexOf(promise);
            this._sendingPromises.splice(index, 1);
        };
        promise.then(popPromise, popPromise);
    }
    onShutdown() {
        // Nothing to clean up
    }
}
exports.StdoutOTLPExporterNode = StdoutOTLPExporterNode;
StdoutOTLPExporterNode.VERSION = version_1.VERSION;
var otlp_exporter_base_3 = require("@opentelemetry/otlp-exporter-base");
Object.defineProperty(exports, "CompressionAlgorithm", { enumerable: true, get: function () { return otlp_exporter_base_3.CompressionAlgorithm; } });
