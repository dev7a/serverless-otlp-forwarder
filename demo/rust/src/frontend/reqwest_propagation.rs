//! OpenTelemetry context propagation middleware for reqwest
//!
//! This code is adapted from the reqwest-tracing crate (https://docs.rs/reqwest-tracing/latest/reqwest_tracing/)
//! to support newer versions of OpenTelemetry. The original implementation and design patterns are preserved,
//! with modifications to work with OpenTelemetry 0.27.
//!
//! Original crate's license: Apache-2.0 OR MIT

use async_trait::async_trait;
use http::Extensions;
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next, Result as ReqwestResult};
use reqwest_tracing::{
    default_on_request_end, default_span_name, reqwest_otel_span, ReqwestOtelSpanBackend,
};
use tracing::Instrument;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// A custom span backend that preserves the parent context while including URL information
#[derive(Clone, Debug, Default)]
struct ParentAwareSpanBackend;

impl ReqwestOtelSpanBackend for ParentAwareSpanBackend {
    fn on_request_start(req: &Request, ext: &mut Extensions) -> Span {
        let name = default_span_name(req, ext);
        reqwest_otel_span!(name = name, req,
            url.full = %req.url(),
            net.peer.name = %req.url().host_str().unwrap_or_default(),
            net.peer.port = %req.url().port().unwrap_or_default(),
            http.target = %req.url().path(), //deprecated but still used by app signals
            http.method = %req.method(), //deprecated but still used by app signals
        )
    }

    fn on_request_end(span: &Span, outcome: &ReqwestResult<Response>, _: &mut Extensions) {
        default_on_request_end(span, outcome)
    }
}

/// Custom middleware to handle OpenTelemetry context propagation
#[derive(Clone, Debug, Default)]
pub struct OtelPropagationMiddleware;

impl OtelPropagationMiddleware {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Middleware for OtelPropagationMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> ReqwestResult<Response> {
        let request_span = ParentAwareSpanBackend::on_request_start(&req, extensions);

        let outcome_future = async {
            let mut req = req;
            let cx = request_span.context();

            global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&cx, &mut HeaderInjector(req.headers_mut()));
            });

            let outcome = next.run(req, extensions).await;
            ParentAwareSpanBackend::on_request_end(&request_span, &outcome, extensions);
            outcome
        };

        outcome_future.instrument(request_span.clone()).await
    }
}
