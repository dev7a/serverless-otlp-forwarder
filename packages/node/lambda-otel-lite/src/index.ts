// Re-export public API
export { createTracedHandler } from './handler';
export { initTelemetry } from './internal/telemetry/init';
export { getLambdaResource } from './internal/telemetry/resource';
export {
  apiGatewayV1Extractor,
  apiGatewayV2Extractor,
  albExtractor,
  TriggerType,
  type SpanAttributes,
  type APIGatewayV2Event,
  type APIGatewayV1Event,
  type ALBEvent,
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
export type { TracedFunction, LambdaContext } from './handler';
