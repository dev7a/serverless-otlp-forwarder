use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use include_dir::{include_dir, Dir};
use lambda_lw_http_router::{define_router, route};
use lambda_otel_lite::events::{event, EventLevel};
use lambda_otel_lite::{init_telemetry, OtelTracingLayer, TelemetryConfig};
use lambda_runtime::{tower::ServiceBuilder, Error as LambdaError, LambdaEvent, Runtime};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tera::{Context as TeraContext, Tera};
use tracing::error;

use todo_app::response::{
    binary_asset_response, html_response, no_content_response, static_asset_response, CC_FAVICON,
};

// Embed the templates directory at compile time
static TEMPLATES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/frontend/templates");

static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/frontend/static");

// Application state for the frontend
#[derive(Clone)]
struct AppState {
    tera: Arc<Tera>,
    // No need to store STATIC_DIR in AppState as it's a global static
}

// Define the router for ApiGatewayV2httpRequest and our AppState
define_router!(event = ApiGatewayV2httpRequest, state = AppState);

/// Main function
#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    // Initialize telemetry
    let (_, completion_handler) = init_telemetry(TelemetryConfig::default()).await?;

    // Initialize Tera templates
    let tera = Arc::new(build_tera_from_embedded()?);

    // Create app state
    let state = Arc::new(AppState { tera });

    // Create router
    let router = Arc::new(create_router());

    // Build the service with OtelTracingLayer
    let service = ServiceBuilder::new()
        .layer(OtelTracingLayer::new(completion_handler).with_name("todo-frontend-tower"))
        .service_fn(move |event: LambdaEvent<ApiGatewayV2httpRequest>| {
            let router_clone = router.clone();
            let state_clone = state.clone();
            handlder(event, router_clone, state_clone)
        });

    // Run the Lambda runtime
    Runtime::new(service).run().await?;

    Ok(())
}

// Actual handler function called by the service_fn
async fn handlder(
    event: LambdaEvent<ApiGatewayV2httpRequest>,
    router: Arc<Router>,
    state: Arc<AppState>,
) -> Result<Value, LambdaError> {
    // Returns serde_json::Value for API Gateway V2
    router.handle_request(event, state).await
}

/// Create the router for the frontend API
fn create_router() -> Router {
    RouterBuilder::from_registry().build()
}

#[tracing::instrument(name = "frontend/serve/page", skip_all)]
#[route(method = "GET", path = "/")]
async fn home_page(ctx: RouteContext) -> Result<Value, LambdaError> {
    let mut tera_ctx = TeraContext::new();

    if let Some(css_file) = STATIC_DIR.get_file("css/style.css") {
        if let Some(content) = css_file.contents_utf8() {
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let css_hash = hasher.finalize();
            tera_ctx.insert("style_css_hash", &hex::encode(css_hash));
        } else {
            error!("Could not read css/style.css content as UTF-8");
        }
    } else {
        error!("css/style.css not found in STATIC_DIR");
    }

    if let Some(js_file) = STATIC_DIR.get_file("js/app.js") {
        if let Some(content) = js_file.contents_utf8() {
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let js_hash = hasher.finalize();
            tera_ctx.insert("app_js_hash", &hex::encode(js_hash));
        } else {
            error!("Could not read js/app.js content as UTF-8");
        }
    } else {
        error!("js/app.js not found in STATIC_DIR");
    }

    let rendered = match ctx.state.tera.render("index.html", &tera_ctx) {
        Ok(rendered) => rendered,
        Err(e) => {
            error!("Error rendering template: {}", e);
            return Ok(html_response(500, format!("Error rendering page: {e}")));
        }
    };
    event()
        .level(EventLevel::Info)
        .message("Serving static index.html shell with cache-busting hashes")
        .attribute("event.name", "frontend.static_shell.served")
        .call();
    Ok(html_response(200, rendered))
}

#[tracing::instrument(name = "frontend/serve/css", skip_all)]
#[route(method = "GET", path = "/style.css")]
async fn handle_css(_ctx: RouteContext) -> Result<Value, LambdaError> {
    match STATIC_DIR.get_file("css/style.css") {
        Some(file) => match file.contents_utf8() {
            Some(content) => Ok(static_asset_response(
                200,
                "text/css",
                content.to_string(),
                true,
            )),
            None => Ok(html_response(
                500,
                "Error reading CSS file content".to_string(),
            )),
        },
        None => Ok(html_response(404, "CSS file not found".to_string())),
    }
}

#[tracing::instrument(name = "frontend/serve/js", skip_all)]
#[route(method = "GET", path = "/app.js")]
async fn handle_js(_ctx: RouteContext) -> Result<Value, LambdaError> {
    match STATIC_DIR.get_file("js/app.js") {
        Some(file) => match file.contents_utf8() {
            Some(content) => Ok(static_asset_response(
                200,
                "application/javascript",
                content.to_string(),
                true,
            )),
            None => Ok(html_response(
                500,
                "Error reading JS file content".to_string(),
            )),
        },
        None => Ok(html_response(404, "JS file not found".to_string())),
    }
}

#[tracing::instrument(name = "frontend/serve/favicon", skip_all)]
#[route(method = "GET", path = "/favicon.ico")]
async fn handle_favicon(_ctx: RouteContext) -> Result<Value, LambdaError> {
    match STATIC_DIR.get_file("favicon.ico") {
        Some(file) => {
            let contents = file.contents();
            let body = BASE64_STANDARD.encode(contents);
            Ok(binary_asset_response(200, "image/x-icon", body, CC_FAVICON))
        }
        None => Ok(no_content_response(Some(CC_FAVICON))),
    }
}

fn build_tera_from_embedded() -> Result<Tera, tera::Error> {
    let mut tera = Tera::default();
    if let Some(file) = TEMPLATES_DIR.get_file("index.html") {
        if let Some(contents) = file.contents_utf8() {
            tera.add_raw_template("index.html", contents)?;
        }
    }
    Ok(tera)
}
