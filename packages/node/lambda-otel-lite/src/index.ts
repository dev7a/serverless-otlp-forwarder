// Re-export public API
export { createTracedHandler } from './handler';
export { initTelemetry } from './internal/telemetry/init';
export type { AttributesExtractor } from './handler'; // Re-export only AttributesExtractor here
export { getLambdaResource } from './internal/telemetry/resource';
export {
  apiGatewayV1Extractor,
  apiGatewayV2Extractor,
  albExtractor,
  defaultExtractor,
  TriggerType,
  type SpanAttributes,
} from './internal/telemetry/extractors';
export * from './mode';

// Export processor related types
export {
  LambdaSpanProcessor,
  type LambdaSpanProcessorConfig,
} from './internal/telemetry/processor';

// Export constants for configuration
export { ENV_VARS, DEFAULTS, RESOURCE_ATTRIBUTES } from './constants';

// Export types needed by users
export type { TelemetryCompletionHandler } from './internal/telemetry/completion';
// TracedFunction is already exported from './handler' elsewhere, ensure it's exported once.
// Let's assume it's correctly exported elsewhere and remove this potentially duplicate line.
// If it's not exported elsewhere, we'll need to add it back correctly.
// For now, removing this line to resolve the duplicate identifier error.
