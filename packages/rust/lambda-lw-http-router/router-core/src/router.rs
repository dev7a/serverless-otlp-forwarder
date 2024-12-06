use lambda_runtime::{Error, LambdaEvent};
use lazy_static::lazy_static;
use opentelemetry::{global, trace::Status, Context as OtelContext};
use opentelemetry_http::HeaderExtractor;
use regex::Regex;
use serde_json::{json, Value as JsonValue};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use crate::{RoutableHttpEvent, RouteContext};

lazy_static! {
    static ref ROUTE_REGISTRY: Mutex<HashMap<(TypeId, TypeId), Box<dyn Any + Send + Sync>>> =
        Mutex::new(HashMap::new());
}

/// The main router type that handles HTTP requests and dispatches them to handlers.
///
/// The router matches incoming requests against registered routes and executes
/// the corresponding handlers. It supports path parameters and different HTTP methods.
///
/// # Type Parameters
///
/// * `State` - The type of the application state shared across handlers
/// * `E` - The Lambda event type (must implement `RoutableHttpEvent`)
///
/// # Examples
///
/// ```rust
/// use lambda_lw_http_router_core::{Router, RouteContext};
/// use serde_json::{json, Value};
/// use lambda_runtime::Error;
/// use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
///
/// #[derive(Clone)]
/// struct State {}
///
/// let mut router: Router<State, ApiGatewayV2httpRequest> = Router::new();
/// router.register_route("GET", "/hello/{name}", |ctx| async move {
///     let name = ctx.params.get("name").map_or("World", String::as_str);
///     Ok(json!({ "message": format!("Hello, {}!", name) }))
/// });
/// ```
pub struct Router<State, E>
where
    State: Send + Sync + Clone + 'static,
    E: RoutableHttpEvent,
{
    routes: HashMap<
        String,
        (
            Arc<
                dyn Fn(
                        RouteContext<State, E>,
                    )
                        -> Pin<Box<dyn Future<Output = Result<JsonValue, Error>> + Send>>
                    + Send
                    + Sync,
            >,
            Regex,
        ),
    >,
}

impl<State, E: RoutableHttpEvent> Router<State, E>
where
    State: Send + Sync + Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn register_route<F, Fut>(&mut self, method: &str, path: &str, handler: F)
    where
        F: Fn(RouteContext<State, E>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<JsonValue, Error>> + Send + 'static,
    {
        let regex_pattern = path
            .split('/')
            .map(|segment| {
                if segment.starts_with('{') && segment.ends_with('}') {
                    let param_name = segment[1..segment.len() - 1].trim_end_matches('+');
                    if segment.ends_with("+}") {
                        // Greedy match for proxy+ style parameters
                        format!("(?P<{}>.*)", param_name)
                    } else {
                        // Normal parameter match (non-greedy, no slashes)
                        format!("(?P<{}>[^/]+)", param_name)
                    }
                } else {
                    regex::escape(segment) // Escape regular segments
                }
            })
            .collect::<Vec<_>>()
            .join("/");

        let regex = Regex::new(&format!("^{}$", regex_pattern)).expect("Invalid route pattern");

        let handler = Arc::new(move |ctx| {
            Box::pin(handler(ctx)) as Pin<Box<dyn Future<Output = Result<JsonValue, Error>> + Send>>
        });

        let key = format!("{} {}", method.to_uppercase(), path);
        self.routes.insert(key, (handler, regex));
    }

    // Helper method to handle the response and set span attributes
    fn handle_response(span: &tracing::Span, response: JsonValue) -> Result<JsonValue, Error> {
        // Set response attributes
        if let Some(status) = response.get("statusCode").and_then(|s| s.as_i64()) {
            span.set_attribute("http.response.status_code", status);

            // For server spans:
            // - Leave status unset for 1xx-4xx
            // - Set error only for 5xx
            let otel_status = if (500..600).contains(&(status as u16)) {
                Status::error(format!("Server error {}", status))
            } else {
                Status::Unset
            };
            span.set_status(otel_status);
        }

        Ok(response)
    }

    // Helper method to create context and execute handler
    async fn execute_handler(
        &self,
        handler_fn: &Arc<
            dyn Fn(
                    RouteContext<State, E>,
                )
                    -> Pin<Box<dyn Future<Output = Result<JsonValue, Error>> + Send>>
                + Send
                + Sync,
        >,
        params: HashMap<String, String>,
        route_pattern: String,
        base_ctx: RouteContext<State, E>,
        parent_cx: OtelContext,
        payload: &E,
    ) -> Result<JsonValue, Error> {
        let span = tracing::Span::current();
        span.set_parent(parent_cx);
        payload.set_otel_http_attributes(&span, &route_pattern, &base_ctx.lambda_context);

        let ctx = RouteContext {
            params,
            route_pattern,
            ..base_ctx
        };

        let response = handler_fn(ctx).await?;
        Self::handle_response(&span, response)
    }

    pub async fn handle_request(
        &self,
        event: LambdaEvent<E>,
        state: Arc<State>,
    ) -> Result<JsonValue, Error> {
        let (payload, lambda_context) = event.into_parts();

        // Extract parent context from headers
        let parent_cx = if let Some(headers) = payload.http_headers() {
            global::get_text_map_propagator(|propagator| {
                propagator.extract(&HeaderExtractor(headers))
            })
        } else {
            OtelContext::current()
        };

        let raw_path = payload.path();
        let path = raw_path.as_deref().unwrap_or("/").to_string();
        let method = payload.http_method().to_uppercase();

        let base_ctx = RouteContext {
            path: path.clone(),
            method: method.clone(),
            params: HashMap::new(),
            state,
            event: payload.clone(),
            lambda_context,
            route_pattern: String::new(),
        };

        // Check if we have a matching route
        for (route_key, (handler_fn, regex)) in &self.routes {
            // Extract method and path from route_key
            let parts: Vec<&str> = route_key.split_whitespace().collect();
            let (route_method, route_path) = match parts.as_slice() {
                [method, path] => (method.to_uppercase(), path),
                _ => continue, // Invalid route key format
            };

            // Check if methods match
            if method != route_method {
                continue;
            }

            // If we have both resource and path_parameters, validate against API Gateway configuration
            if let (Some(resource), Some(path_params)) =
                (payload.route(), payload.path_parameters())
            {
                if resource == *route_path {
                    return self
                        .execute_handler(
                            handler_fn,
                            path_params.clone(),
                            route_path.to_string(),
                            base_ctx,
                            parent_cx.clone(),
                            &payload,
                        )
                        .await;
                }
            }

            // Fall back to our own path parameter extraction
            if let Some(captures) = regex.captures(&path) {
                let mut params = HashMap::new();
                for name in regex.capture_names().flatten() {
                    if let Some(value) = captures.name(name) {
                        params.insert(name.to_string(), value.as_str().to_string());
                    }
                }

                return self
                    .execute_handler(
                        handler_fn,
                        params,
                        route_path.to_string(),
                        base_ctx,
                        parent_cx.clone(),
                        &payload,
                    )
                    .await;
            }
        }

        Ok(json!({
            "statusCode": 404,
            "headers": {"Content-Type": "text/plain"},
            "body": "Not Found"
        }))
    }
}

impl<State, E: RoutableHttpEvent> Default for Router<State, E>
where
    State: Send + Sync + Clone + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// A builder for constructing routers with a fluent API.
///
/// This type provides a more ergonomic way to create and configure routers
/// compared to using `Router` directly. It supports method chaining and
/// handles all the type complexity internally.
///
/// # Examples
///
/// ```rust
/// use lambda_lw_http_router_core::{RouterBuilder, RouteContext};
/// use serde_json::{json, Value};
/// use lambda_runtime::Error;
/// use aws_lambda_events::apigw::ApiGatewayV2httpRequest;
///
/// #[derive(Clone)]
/// struct State {}
///
/// async fn get_users(ctx: RouteContext<State, ApiGatewayV2httpRequest>) -> Result<Value, Error> {
///     Ok(json!({ "users": [] }))
/// }
///
/// async fn create_user(ctx: RouteContext<State, ApiGatewayV2httpRequest>) -> Result<Value, Error> {
///     Ok(json!({ "status": "created" }))
/// }
///
/// let router = RouterBuilder::<State, ApiGatewayV2httpRequest>::new()
///     .route("GET", "/users", get_users)
///     .route("POST", "/users", create_user)
///     .build();
/// ```
pub struct RouterBuilder<State, E: RoutableHttpEvent>
where
    State: Send + Sync + Clone + 'static,
{
    routes: Vec<(
        String,
        String,
        Box<
            dyn Fn(
                    RouteContext<State, E>,
                )
                    -> Pin<Box<dyn Future<Output = Result<JsonValue, Error>> + Send>>
                + Send
                + Sync,
        >,
    )>,
}

impl<State, E: RoutableHttpEvent> RouterBuilder<State, E>
where
    State: Send + Sync + Clone + 'static,
{
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn route<F, Fut>(mut self, method: &str, path: &str, handler: F) -> Self
    where
        F: Fn(RouteContext<State, E>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<JsonValue, Error>> + Send + 'static,
    {
        let handler = Box::new(move |ctx| {
            Box::pin(handler(ctx)) as Pin<Box<dyn Future<Output = Result<JsonValue, Error>> + Send>>
        });
        self.routes
            .push((method.to_string(), path.to_string(), handler));
        self
    }

    pub fn build(self) -> Router<State, E> {
        let mut router = Router::new();
        for (method, path, handler) in self.routes {
            let handler = move |ctx: RouteContext<State, E>| (handler)(ctx);
            router.register_route(&method, &path, handler);
        }
        router
    }

    pub fn from_registry() -> Self {
        let mut builder = Self::new();

        let routes = {
            let registry = ROUTE_REGISTRY.lock().unwrap();
            registry
                .get(&(TypeId::of::<State>(), TypeId::of::<E>()))
                .and_then(|routes| routes.downcast_ref::<Vec<RouteEntry<State, E>>>())
                .map(|routes| {
                    routes
                        .iter()
                        .map(|entry| (entry.method, entry.path, Arc::clone(&entry.handler)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };

        for (method, path, handler) in routes {
            let handler = Box::new(move |ctx: RouteContext<State, E>| (handler)(ctx));
            builder
                .routes
                .push((method.to_string(), path.to_string(), handler));
        }

        builder
    }
}

impl<State, E: RoutableHttpEvent> Default for RouterBuilder<State, E>
where
    State: Send + Sync + Clone + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

type BoxedHandler<State, E> = dyn Fn(RouteContext<State, E>) -> Pin<Box<dyn Future<Output = Result<JsonValue, Error>> + Send>>
    + Send
    + Sync;

#[derive(Clone)]
struct RouteEntry<State: Clone, E: RoutableHttpEvent> {
    method: &'static str,
    path: &'static str,
    handler: Arc<BoxedHandler<State, E>>,
}

pub fn register_route<State, E: RoutableHttpEvent>(
    method: &'static str,
    path: &'static str,
    handler: impl Fn(RouteContext<State, E>) -> Pin<Box<dyn Future<Output = Result<JsonValue, Error>> + Send>>
        + Send
        + Sync
        + 'static,
) where
    State: Send + Sync + Clone + 'static,
{
    let state_type_id = TypeId::of::<State>();
    let event_type_id = TypeId::of::<E>();
    let handler = Arc::new(handler) as Arc<BoxedHandler<State, E>>;
    let entry = RouteEntry {
        method,
        path,
        handler,
    };

    let mut registry = ROUTE_REGISTRY.lock().unwrap();
    let routes = registry
        .entry((state_type_id, event_type_id))
        .or_insert_with(|| Box::new(Vec::<RouteEntry<State, E>>::new()));

    let routes = routes
        .downcast_mut::<Vec<RouteEntry<State, E>>>()
        .expect("Registry type mismatch");
    routes.push(entry);
}
