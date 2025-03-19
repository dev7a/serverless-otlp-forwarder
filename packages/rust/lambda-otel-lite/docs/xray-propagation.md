# X-Ray Propagation Support for lambda-otel-lite

## Overview

This document outlines the implementation of AWS X-Ray propagation support in the Rust `lambda-otel-lite` crate. This enhancement allows the library to:

1. Extract AWS X-Ray trace headers from incoming requests
2. Extract trace context from Lambda's environment variables
3. Propagate X-Ray trace context to downstream services
4. Maintain compatibility with the W3C Trace Context standard
5. Provide a unified tracing experience across AWS services

## Background

AWS X-Ray uses a different header format than the W3C Trace Context standard:

- **X-Ray**: Uses `X-Amzn-Trace-Id` header with format: `Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1`
- **W3C Trace Context**: Uses `traceparent` and `tracestate` headers

In addition, AWS Lambda automatically sets the `_X_AMZN_TRACE_ID` environment variable for X-Ray tracing when active tracing is enabled. This environment variable contains the same format as the X-Ray header and is set for all Lambda invocations, regardless of how the function was invoked.

## Implementation

The implementation leverages the existing `opentelemetry-aws` crate for X-Ray propagation, but extends it with Lambda-specific functionality:

### 1. LambdaXrayPropagator

A wrapper propagator that enhances the standard `XrayPropagator` with automatic Lambda environment variable handling:

```rust
/// A custom propagator that wraps the XrayPropagator with Lambda-specific enhancements.
pub struct LambdaXrayPropagator {
    /// The wrapped XrayPropagator instance
    inner: XrayPropagator,
}

impl TextMapPropagator for LambdaXrayPropagator {
    fn extract(&self, carrier: &dyn Extractor) -> Context {
        // First, try to extract from the provided carrier (headers from event)
        let ctx = self.inner.extract(carrier);
        
        // If no valid span context, try to extract from Lambda environment variable
        if !ctx.span().span_context().is_valid() {
            if let Ok(trace_id_value) = env::var("_X_AMZN_TRACE_ID") {
                let env_carrier = HashMap::from([(AWS_XRAY_TRACE_HEADER.to_string(), trace_id_value)]);
                let env_ctx = self.inner.extract(&env_carrier);
                if env_ctx.span().span_context().is_valid() {
                    return env_ctx;
                }
            }
        }
        
        ctx
    }

    // Other methods delegate to the inner propagator
}
```

### 2. Propagator Configuration

The propagator setup is handled in the `init_telemetry` function, allowing different propagation options via the standard OpenTelemetry environment variable:

```rust
// In init_telemetry function
if config.propagators.is_empty() {
    // Default to both W3C and Lambda-enhanced X-Ray if nothing specified
    if let Ok(propagators_str) = env::var("OTEL_PROPAGATORS") {
        // Parse comma-separated list of propagators
        for propagator in propagators_str.split(',').map(|s| s.trim().to_lowercase()) {
            match propagator.as_str() {
                "tracecontext" => {
                    config.propagators.push(Box::new(TraceContextPropagator::new()));
                }
                "xray" => {
                    config.propagators.push(Box::new(XrayPropagator::default()));
                }
                "xray-lambda" => {
                    config.propagators.push(Box::new(LambdaXrayPropagator::default()));
                }
                // ...
            }
        }
    } else {
        // Default: both W3C and Lambda-enhanced X-Ray
        config.propagators.push(Box::new(TraceContextPropagator::new()));
        config.propagators.push(Box::new(LambdaXrayPropagator::default()));
    }
}
```

## Configuration Options

The library supports the OpenTelemetry standard `OTEL_PROPAGATORS` environment variable for configuring context propagation:

- `tracecontext` - W3C Trace Context
- `xray` - Standard AWS X-Ray propagation
- `xray-lambda` - AWS X-Ray propagation with Lambda environment variable support
- `none` - No propagation (disables all context propagation)

Examples:

1. Default behavior (no environment variable set):
   Both W3C Trace Context and Lambda-enhanced X-Ray propagation are enabled

2. Environment variable for W3C only:
   ```bash
   OTEL_PROPAGATORS=tracecontext
   ```

3. Environment variable for standard X-Ray only:
   ```bash
   OTEL_PROPAGATORS=xray
   ```

4. Environment variable for Lambda-enhanced X-Ray only:
   ```bash
   OTEL_PROPAGATORS=xray-lambda
   ```

5. No propagation:
   ```bash
   OTEL_PROPAGATORS=none
   ```

## Benefits

1. **Improved AWS Integration**: Better tracing continuity with AWS managed services
2. **Distributed Tracing**: End-to-end visibility across services using different propagation formats
3. **Compatibility**: Support for both industry-standard (W3C) and AWS-specific (X-Ray) formats
4. **Standards Compliance**: Following OpenTelemetry specifications for configuration
5. **Lambda Integration**: Proper handling of Lambda's built-in X-Ray trace context

## References

1. [AWS X-Ray Trace Header Format](https://docs.aws.amazon.com/xray/latest/devguide/xray-concepts.html#xray-concepts-tracingheader)
2. [OpenTelemetry Context Propagation](https://opentelemetry.io/docs/concepts/context-propagation/)
3. [W3C Trace Context Specification](https://www.w3.org/TR/trace-context/)
4. [AWS Lambda Context Propagation](https://docs.aws.amazon.com/lambda/latest/dg/services-xray.html)
5. [OpenTelemetry SDK Environment Variables](https://opentelemetry.io/docs/specs/otel/configuration/sdk-environment-variables/)
6. [OpenTelemetry AWS Crate](https://docs.rs/opentelemetry-aws/latest/opentelemetry_aws/) 