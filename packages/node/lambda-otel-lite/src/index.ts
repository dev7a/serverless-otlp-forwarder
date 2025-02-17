// Re-export public API
export { createTracedHandler } from './handler';
export { initTelemetry, getLambdaResource } from './internal/telemetry/init';
export { 
  apiGatewayV1Extractor, 
  apiGatewayV2Extractor, 
  albExtractor,
  TriggerType,
  type SpanAttributes,
  type APIGatewayV2Event,
  type APIGatewayV1Event,
  type ALBEvent
} from './internal/telemetry/extractors';
export * from './mode';

// Export types needed by users
export type { TelemetryCompletionHandler } from './internal/telemetry/completion';
export type { TracerConfig, TracedFunction, LambdaContext } from './handler';
