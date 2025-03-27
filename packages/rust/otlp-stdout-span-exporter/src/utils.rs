use std::{collections::HashMap, env};

use crate::consts::{defaults, env_vars};

/// Get the service name from environment variables.
///
/// The service name is determined in the following order:
///
/// 1. OTEL_SERVICE_NAME
/// 2. AWS_LAMBDA_FUNCTION_NAME
/// 3. "unknown-service" (fallback)
pub(crate) fn get_service_name() -> String {
    env::var(env_vars::SERVICE_NAME)
        .or_else(|_| env::var(env_vars::AWS_LAMBDA_FUNCTION_NAME))
        .unwrap_or_else(|_| defaults::SERVICE_NAME.to_string())
}

/// Parse headers from environment variables
///
/// This function reads headers from both global and trace-specific
/// environment variables, with trace-specific headers taking precedence.
pub(crate) fn parse_headers() -> HashMap<String, String> {
    let mut headers = HashMap::new();

    // Parse global headers first
    if let Ok(global_headers) = env::var("OTEL_EXPORTER_OTLP_HEADERS") {
        parse_header_string(&global_headers, &mut headers);
    }

    // Parse trace-specific headers (these take precedence)
    if let Ok(trace_headers) = env::var("OTEL_EXPORTER_OTLP_TRACES_HEADERS") {
        parse_header_string(&trace_headers, &mut headers);
    }

    headers
}

/// Parse a header string in the format key1=value1,key2=value2
///
/// # Arguments
///
/// * `header_str` - The header string to parse
/// * `headers` - The map to store parsed headers in
pub(crate) fn parse_header_string(header_str: &str, headers: &mut HashMap<String, String>) {
    for pair in header_str.split(',') {
        if let Some((key, value)) = pair.split_once('=') {
            let key = key.trim().to_lowercase();
            // Skip content-type and content-encoding as they are fixed
            if key != "content-type" && key != "content-encoding" {
                headers.insert(key, value.trim().to_string());
            }
        }
    }
}
