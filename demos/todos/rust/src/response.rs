use serde_json::{json, Value};
use std::collections::HashMap;

// --- Cache-Control Directives ---
pub const CC_NO_CACHE: &str = "no-store, no-cache, must-revalidate, proxy-revalidate";
pub const CC_PRAGMA_NO_CACHE: &str = "no-cache"; // For Pragma header
pub const CC_EXPIRES_ZERO: &str = "0"; // For Expires header

pub const CC_HTML_SHELL: &str = "no-cache, max-age=0, must-revalidate";
pub const CC_STATIC_ASSET_IMMUTABLE: &str = "public, max-age=31536000, immutable";
pub const CC_FAVICON: &str = "public, max-age=2592000"; // 30 days
pub const CC_GENERIC_STATIC: &str = "public, max-age=86400"; // 1 day

// --- Core Response Structure ---

/// Builds a generic API Gateway response value.
/// Handles differences between V1 (Proxy) and V2 (HTTP API) implicitly through usage.
/// For V2, headers are simpler (direct map). For V1, multiValueHeaders might be needed for complex cases,
/// but simple headers work for most JSON/HTML.
/// The `is_base64_encoded` flag is primarily for V2 when body is binary.
fn build_response(
    status_code: u16,
    body: String,
    content_type: Option<&str>,
    additional_headers: HashMap<String, String>,
    is_base64_encoded: bool,
) -> Value {
    let mut headers = HashMap::new();
    if let Some(ct) = content_type {
        headers.insert("Content-Type".to_string(), ct.to_string());
    }
    headers.extend(additional_headers);

    json!({
        "statusCode": status_code,
        "headers": headers,
        "body": body,
        "isBase64Encoded": is_base64_encoded // Important for V2 with binary, ignored or default false for V1 JSON
    })
}

// --- Backend API Helpers (API Gateway V1 Proxy Style - typically JSON) ---

/// Creates a JSON API response with standard no-cache headers.
pub fn api_json_response(status_code: u16, body_value: Value) -> Value {
    let mut headers = HashMap::new();
    headers.insert("Cache-Control".to_string(), CC_NO_CACHE.to_string());
    headers.insert("Pragma".to_string(), CC_PRAGMA_NO_CACHE.to_string());
    headers.insert("Expires".to_string(), CC_EXPIRES_ZERO.to_string());
    build_response(
        status_code,
        body_value.to_string(),
        Some("application/json"),
        headers,
        false,
    )
}

/// Creates an empty API response (e.g., 204 No Content) with no-cache headers.
pub fn api_empty_response(status_code: u16) -> Value {
    let mut headers = HashMap::new();
    // No-cache headers might not be strictly necessary for 204, but doesn't hurt.
    headers.insert("Cache-Control".to_string(), CC_NO_CACHE.to_string());
    headers.insert("Pragma".to_string(), CC_PRAGMA_NO_CACHE.to_string());
    headers.insert("Expires".to_string(), CC_EXPIRES_ZERO.to_string());
    // V1 expects a body string, even if empty for some tools or strict interpretations.
    // V2 is more lenient.
    build_response(
        status_code,
        "".to_string(),
        Some("application/json"),
        headers,
        false,
    )
}

/// Creates a JSON error response with standard no-cache headers.
pub fn api_error_response(status_code: u16, message: &str) -> Value {
    let error_body = json!({"error": message});
    api_json_response(status_code, error_body)
}

// --- Frontend API Helpers (API Gateway V2 HTTP API Style - HTML, Static Assets) ---

/// Creates an HTML response.
pub fn html_response(status_code: u16, html_body: String) -> Value {
    let mut headers = HashMap::new();
    headers.insert("Cache-Control".to_string(), CC_HTML_SHELL.to_string());
    build_response(status_code, html_body, Some("text/html"), headers, false)
}

/// Creates a response for a static asset (CSS, JS).
pub fn static_asset_response(
    status_code: u16,
    content_type: &str,
    body: String,
    is_immutable: bool, // If true, uses long-term immutable caching
) -> Value {
    let mut headers = HashMap::new();
    if is_immutable {
        headers.insert(
            "Cache-Control".to_string(),
            CC_STATIC_ASSET_IMMUTABLE.to_string(),
        );
    } else {
        headers.insert("Cache-Control".to_string(), CC_GENERIC_STATIC.to_string());
    }
    build_response(status_code, body, Some(content_type), headers, false)
}

/// Creates a response for a binary static asset that needs Base64 encoding for API Gateway V2.
pub fn binary_asset_response(
    status_code: u16,
    content_type: &str,
    base64_encoded_body: String,
    cache_directive: &str,
) -> Value {
    let mut headers = HashMap::new();
    headers.insert("Cache-Control".to_string(), cache_directive.to_string());
    build_response(
        status_code,
        base64_encoded_body,
        Some(content_type),
        headers,
        true,
    )
}

/// Creates a 204 No Content response, often used if a resource like favicon isn't found
/// to prevent repeated browser requests.
pub fn no_content_response(cache_directive: Option<&str>) -> Value {
    let mut headers = HashMap::new();
    if let Some(cc) = cache_directive {
        headers.insert("Cache-Control".to_string(), cc.to_string());
    }
    // For V2, an empty body is fine.
    build_response(204, "".to_string(), None, headers, false)
}
