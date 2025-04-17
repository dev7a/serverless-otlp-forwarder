import {
  Context,
  TextMapGetter,
  TextMapPropagator,
  TextMapSetter,
  propagation,
  trace,
} from '@opentelemetry/api';
import { CompositePropagator, W3CTraceContextPropagator } from '@opentelemetry/core';
import { AWSXRayPropagator } from '@opentelemetry/propagator-aws-xray';
import { ENV_VARS } from '../constants';
import { getStringValue } from './config';
import { createLogger } from './logger';

const logger = createLogger('propagation');

/**
 * Checks if the context has a valid span.
 * @param context The context to check
 * @returns True if the context has a valid span, false otherwise
 */
function hasValidSpan(context: Context): boolean {
  const span = trace.getSpan(context);
  if (!span) {
    return false;
  }

  const spanContext = span.spanContext();
  // Check if the span context is valid (has valid trace ID and span ID)
  return spanContext.traceId !== '' && spanContext.spanId !== '';
}

/**
 * No-op propagator that does nothing.
 */
export class NoopPropagator implements TextMapPropagator {
  /**
   * Extract from carrier (no-op).
   */
  extract<Carrier>(context: Context, _carrier: Carrier, _getter: TextMapGetter<Carrier>): Context {
    return context;
  }

  /**
   * Inject into carrier (no-op).
   */
  inject<Carrier>(_context: Context, _carrier: Carrier, _setter: TextMapSetter<Carrier>): void {
    // No-op
  }

  /**
   * Get fields (no-op).
   */
  fields(): string[] {
    return [];
  }
}

/**
 * Lambda-aware AWS X-Ray propagator.
 *
 * This propagator extends the standard AWS X-Ray propagator to also check
 * the _X_AMZN_TRACE_ID environment variable if no trace context is found
 * in the carrier. This is useful in AWS Lambda environments where the
 * trace context is often provided via environment variables.
 *
 * It also respects the "Sampled=0" flag in the X-Ray trace header, which
 * indicates that the request should not be sampled.
 */
export class LambdaXRayPropagator implements TextMapPropagator {
  private readonly xrayPropagator: AWSXRayPropagator;

  constructor() {
    this.xrayPropagator = new AWSXRayPropagator();
  }

  /**
   * Injects context into carrier by adding X-Ray headers.
   * This delegates to the standard AWS X-Ray propagator.
   */
  inject<Carrier>(context: Context, carrier: Carrier, setter: TextMapSetter<Carrier>): void {
    this.xrayPropagator.inject(context, carrier, setter);
  }

  /**
   * Extract context from carrier or environment variable.
   *
   * First tries to extract from the carrier using the standard X-Ray propagator.
   * If that fails (no valid span context), then tries to extract from the
   * _X_AMZN_TRACE_ID environment variable with special handling for the Sampled flag.
   *
   * AWS X-Ray Lambda propagator with special handling for Sampled=0 in the environment variable.
   *
   * Why this is needed:
   * - When X-Ray is not enabled for the Lambda, AWS still sets the _X_AMZN_TRACE_ID environment
   *   variable, but always with Sampled=0.
   * - The stock AWSXRayPropagator will extract this and create a non-sampled context, which disables
   *   tracing for the execution—even if other propagators (like W3C tracecontext) are enabled and
   *   would otherwise create a root span.
   * - By skipping extraction if Sampled=0 is present, this propagator allows the next propagator or
   *   the default sampler to create a root span, ensuring traces are captured even when X-Ray is not
   *   enabled or not sampled.
   * - This is essential for environments where you want to use W3C or other propagation mechanisms
   *   alongside (or instead of) X-Ray.
   */
  extract<Carrier>(context: Context, carrier: Carrier, getter: TextMapGetter<Carrier>): Context {
    // First try to extract from carrier
    const xrayContext = this.xrayPropagator.extract(context, carrier, getter);

    // Check for a valid span in the extracted context
    if (hasValidSpan(xrayContext)) {
      return xrayContext;
    }

    // Check the environment variable
    const traceHeader = process.env._X_AMZN_TRACE_ID;

    // If no env var or Sampled=0, do not extract further
    if (!traceHeader || traceHeader.includes('Sampled=0')) {
      return xrayContext;
    }

    // Fallback: extract from the environment variable
    const envCarrier = { 'x-amzn-trace-id': traceHeader } as unknown as Carrier;
    return this.xrayPropagator.extract(xrayContext, envCarrier, getter);
  }

  /**
   * Get propagation fields.
   */
  fields(): string[] {
    return this.xrayPropagator.fields();
  }
}

/**
 * Create a composite propagator based on the OTEL_PROPAGATORS environment variable.
 *
 * The environment variable should be a comma-separated list of propagator names.
 * Supported propagators:
 * - "tracecontext" - W3C Trace Context propagator
 * - "xray" - AWS X-Ray propagator
 * - "xray-lambda" - AWS X-Ray propagator with Lambda support
 * - "none" - No propagation
 *
 * If the environment variable is not set, defaults to ["xray-lambda", "tracecontext"]
 * with tracecontext taking precedence.
 *
 * @returns A composite propagator with the specified propagators
 */
export function createPropagator(): TextMapPropagator {
  // Use a Set to deduplicate propagator names, just like the Python implementation
  const propagatorNames = new Set(
    getStringValue(ENV_VARS.OTEL_PROPAGATORS, undefined, 'xray-lambda,tracecontext')
      .split(',')
      .map((name) => name.trim().toLowerCase())
      .filter(Boolean)
  );

  if (propagatorNames.has('none')) {
    logger.debug('Using no propagator as requested by OTEL_PROPAGATORS=none');
    return new NoopPropagator();
  }

  const propagators: TextMapPropagator[] = [];

  if (propagatorNames.has('tracecontext')) {
    propagators.push(new W3CTraceContextPropagator());
  }

  if (propagatorNames.has('xray') || propagatorNames.has('xray-lambda')) {
    propagators.push(new LambdaXRayPropagator());
  }

  if (propagators.length === 0) {
    // Default to LambdaXRayPropagator and W3CTraceContextPropagator
    logger.info('No valid propagators specified, using defaults');
    propagators.push(new LambdaXRayPropagator());
    propagators.push(new W3CTraceContextPropagator());
  }

  logger.debug(`Using propagators: ${propagators.map((p) => p.constructor.name).join(', ')}`);

  return new CompositePropagator({ propagators });
}

/**
 * Set up the global propagator based on environment variables.
 * This should be called during initialization.
 */
export function setupPropagator(): void {
  const propagator = createPropagator();
  propagation.setGlobalPropagator(propagator);
}
