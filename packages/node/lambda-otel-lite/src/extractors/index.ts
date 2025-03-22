/**
 * Extractors for Lambda events.
 *
 * This module provides extractors for common Lambda triggers to automatically
 * extract trace information and span attributes from Lambda events.
 */

// Re-export all extractors from the internal module
export {
  apiGatewayV1Extractor,
  apiGatewayV2Extractor,
  albExtractor,
  defaultExtractor,
  TriggerType,
  type SpanAttributes,
  type APIGatewayV2Event,
  type APIGatewayV1Event,
  type ALBEvent,
} from '../internal/telemetry/extractors';
