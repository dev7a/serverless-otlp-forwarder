//! Context propagation extensions for AWS Lambda.
//!
//! This module provides specialized context propagators for AWS Lambda environments,
//! including enhanced X-Ray propagation that integrates with Lambda's built-in tracing.

use crate::logger::Logger;
use opentelemetry::{
    propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
    Context,
};
use opentelemetry_aws::trace::XrayPropagator;
use std::{collections::HashMap, env};

// Add module-specific logger
static LOGGER: Logger = Logger::const_new("propagation");

// Define the X-Ray trace header constant since it's not publicly exported
const AWS_XRAY_TRACE_HEADER: &str = "x-amzn-trace-id";

/// A custom propagator that wraps the `XrayPropagator` with Lambda-specific enhancements.
///
/// This propagator extends the standard X-Ray propagator to automatically extract
/// trace context from the Lambda `_X_AMZN_TRACE_ID` environment variable when no
/// valid context is found in the provided carrier.
///
/// # Example
///
/// ```no_run
/// use lambda_otel_lite::{init_telemetry, TelemetryConfig};
/// use lambda_otel_lite::propagation::LambdaXrayPropagator;
/// use opentelemetry::global;
/// use lambda_runtime::Error;
///
/// # async fn example() -> Result<(), Error> {
/// // Add the LambdaXrayPropagator
/// let config = TelemetryConfig::builder()
///     .with_named_propagator("tracecontext")
///     .with_named_propagator("xray-lambda")
///     .build();
///
/// let _ = init_telemetry(config).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct LambdaXrayPropagator {
    /// The wrapped XrayPropagator instance
    inner: XrayPropagator,
}

impl LambdaXrayPropagator {
    /// Create a new instance of the LambdaXrayPropagator.
    pub fn new() -> Self {
        Self {
            inner: XrayPropagator::default(),
        }
    }
}

impl TextMapPropagator for LambdaXrayPropagator {
    fn fields(&self) -> FieldIter<'_> {
        self.inner.fields()
    }

    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
        // First, try to extract from the provided carrier using the inner propagator
        let ctx = self.inner.extract_with_context(cx, extractor);

        // Check if we got a valid context from the carrier
        let has_carrier_context = has_active_span(&ctx);

        // If we didn't get a valid context from the carrier, try the environment variable
        if !has_carrier_context {
            if let Ok(trace_id_value) = env::var("_X_AMZN_TRACE_ID") {
                LOGGER.debug(format!("Found _X_AMZN_TRACE_ID: {}", trace_id_value));

                // Create a carrier from the environment variable
                let mut env_carrier = HashMap::new();
                env_carrier.insert(AWS_XRAY_TRACE_HEADER.to_string(), trace_id_value);

                // Try to extract from the environment variable
                let env_ctx = self.inner.extract_with_context(cx, &env_carrier);
                if has_active_span(&env_ctx) {
                    LOGGER.debug("Successfully extracted context from _X_AMZN_TRACE_ID");
                    return env_ctx;
                }
            }
        }

        // Return the original context
        ctx
    }

    fn extract(&self, extractor: &dyn Extractor) -> Context {
        self.extract_with_context(&Context::current(), extractor)
    }

    fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
        self.inner.inject_context(cx, injector)
    }
}

// Helper function to check if a context has an active span
fn has_active_span(cx: &Context) -> bool {
    use opentelemetry::trace::TraceContextExt;
    cx.span().span_context().is_valid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{
        SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState,
    };
    use std::env;

    #[test]
    fn test_extract_from_carrier() {
        // Create a valid X-Ray header
        let trace_id = "1-5759e988-bd862e3fe1be46a994272793";
        let parent_id = "53995c3f42cd8ad8";
        let header_value = format!("Root={};Parent={};Sampled=1", trace_id, parent_id);

        // Create a carrier with the header
        let carrier = HashMap::from([(AWS_XRAY_TRACE_HEADER.to_string(), header_value)]);

        // Extract context
        let propagator = LambdaXrayPropagator::default();
        let context = propagator.extract(&carrier);

        // Verify the extracted context is valid using TraceContextExt trait
        assert!(context.span().span_context().is_valid());
    }

    #[test]
    fn test_extract_from_env_var() {
        // Save the original environment variable if it exists
        let original_env = env::var("_X_AMZN_TRACE_ID").ok();

        // Set up a test environment variable
        // Using a format that's known to be valid with the XrayPropagator
        let trace_id = "1-5759e988-bd862e3fe1be46a994272793";
        let parent_id = "53995c3f42cd8ad8";
        let header_value = format!("Root={};Parent={};Sampled=1", trace_id, parent_id);
        env::set_var("_X_AMZN_TRACE_ID", &header_value);

        // First verify the XrayPropagator itself can parse the header
        let xray_propagator = XrayPropagator::default();
        let env_carrier =
            HashMap::from([(AWS_XRAY_TRACE_HEADER.to_string(), header_value.clone())]);
        let direct_context = xray_propagator.extract(&env_carrier);
        assert!(
            direct_context.span().span_context().is_valid(),
            "XrayPropagator itself should be able to parse the header"
        );

        // Create an empty carrier - there are no headers, but the env var should be used
        let empty_carrier = HashMap::<String, String>::new();

        // Extract using our custom propagator's extract_with_context method directly
        let propagator = LambdaXrayPropagator::default();
        let context = propagator.extract_with_context(&Context::current(), &empty_carrier);

        // Verify the extracted context is valid
        assert!(
            context.span().span_context().is_valid(),
            "Expected valid context from env var via extract_with_context"
        );

        // Restore the original environment variable
        if let Some(val) = original_env {
            env::set_var("_X_AMZN_TRACE_ID", val);
        } else {
            env::remove_var("_X_AMZN_TRACE_ID");
        }
    }

    #[test]
    fn test_inject_context() {
        // Create a test span context
        let span_context = SpanContext::new(
            TraceId::from_hex("5759e988bd862e3fe1be46a994272793").unwrap(),
            SpanId::from_hex("53995c3f42cd8ad8").unwrap(),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        );

        // Create context with the span context
        let context = Context::current().with_remote_span_context(span_context);

        // Create an injector
        let mut injector = HashMap::<String, String>::new();

        // Inject context
        let propagator = LambdaXrayPropagator::default();
        propagator.inject_context(&context, &mut injector);

        // Verify the injected header
        assert!(injector.contains_key(AWS_XRAY_TRACE_HEADER));
        let header = injector.get(AWS_XRAY_TRACE_HEADER).unwrap();
        assert!(header.contains("Root=1-5759e988-bd862e3fe1be46a994272793"));
        assert!(header.contains("Parent=53995c3f42cd8ad8"));
        assert!(header.contains("Sampled=1"));
    }

    #[test]
    fn test_precedence() {
        // Save the original environment variable if it exists
        let original_env = env::var("_X_AMZN_TRACE_ID").ok();

        // Set up a test environment variable (this should NOT be used if carrier is valid)
        let env_trace_id = "1-5759e988-bd862e3fe1be46a994272793";
        let env_parent_id = "53995c3f42cd8ad8";
        let env_header = format!("Root={};Parent={};Sampled=1", env_trace_id, env_parent_id);
        env::set_var("_X_AMZN_TRACE_ID", &env_header);

        // Create a different valid X-Ray header for the carrier
        let carrier_trace_id = "1-58406520-a006649127e371903a2de979";
        let carrier_parent_id = "4c721bf33e3caf8f";
        let carrier_header = format!(
            "Root={};Parent={};Sampled=1",
            carrier_trace_id, carrier_parent_id
        );

        // Create a carrier with the header
        let carrier = HashMap::from([(AWS_XRAY_TRACE_HEADER.to_string(), carrier_header)]);

        // Extract context
        let propagator = LambdaXrayPropagator::default();
        let context = propagator.extract(&carrier);

        // Verify the extracted context used the carrier, not the env var
        let span = context.span();
        let span_context = span.span_context();
        assert!(span_context.is_valid());
        assert_eq!(
            span_context.trace_id(),
            TraceId::from_hex("58406520a006649127e371903a2de979").unwrap()
        );

        // Restore the original environment variable
        if let Some(val) = original_env {
            env::set_var("_X_AMZN_TRACE_ID", val);
        } else {
            env::remove_var("_X_AMZN_TRACE_ID");
        }
    }
}
