use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
use serde_json::{json, Value};
use lambda_runtime::{service_fn, Runtime, Error, LambdaEvent};
use std::sync::Arc;
use lambda_lw_http_router::{define_router, route};
use lambda_otel_utils::{HttpTracerProviderBuilder, OpenTelemetrySubscriberBuilder, OpenTelemetryLayer, OpenTelemetryFaasTrigger};

// Define your application state
#[derive(Clone)]
struct AppState {
    // your state fields here
}

// Set up the router
define_router!(event = ApiGatewayV2httpRequest, state = AppState);

// Define route handlers
#[route(path = "/hello/{name}")]
async fn handle_hello(ctx: RouteContext) -> Result<Value, Error> {
    let name = ctx.params.get("name").map(|s| s.as_str()).unwrap_or("World");
    Ok(json!({
        "message": format!("Hello, {}!", name)
    }))
}

// Lambda function entry point
#[tokio::main]
    async fn main() -> Result<(), Error> {
        // Initialize tracer
        let tracer_provider = HttpTracerProviderBuilder::default()
        .with_stdout_client()
        .with_default_text_map_propagator()
        .with_batch_exporter()
        .enable_global(true)
        .build()
        .expect("Failed to build tracer provider");

    // Initialize the OpenTelemetry subscriber
    OpenTelemetrySubscriberBuilder::new()
        .with_env_filter(true)
        .with_tracer_provider(tracer_provider.clone())
        .with_service_name("hello-world")
        .init()?;

    let state = Arc::new(AppState {});
    let router = Arc::new(RouterBuilder::from_registry().build());
    
    let lambda = move |event: LambdaEvent<ApiGatewayV2httpRequest>| {
        let state = Arc::clone(&state);
        let router = Arc::clone(&router);
        async move { router.handle_request(event, state).await }
    };
    let runtime = Runtime::new(service_fn(lambda)).layer(
        OpenTelemetryLayer::new(|| {
            tracer_provider.force_flush();
        })
        .with_trigger(OpenTelemetryFaasTrigger::Http),
    );

    runtime.run().await
}
