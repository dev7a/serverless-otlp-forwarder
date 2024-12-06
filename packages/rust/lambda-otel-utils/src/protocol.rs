use opentelemetry_otlp::Protocol;
use std::env;

/// Determines the OpenTelemetry protocol to use based on the OTEL_EXPORTER_OTLP_PROTOCOL environment variable.
///
/// # Returns
///
/// A `Protocol` enum value indicating which protocol format to use:
/// - `Protocol::HttpBinary` if OTEL_EXPORTER_OTLP_PROTOCOL is set to "http/protobuf"
/// - `Protocol::HttpJson` if OTEL_EXPORTER_OTLP_PROTOCOL is set to "http/json" or empty/unset
///
/// # Environment Variables
///
/// - `OTEL_EXPORTER_OTLP_PROTOCOL`: The protocol format to use. Supported values are:
///   - "http/protobuf" - Use Protocol Buffers over HTTP
///   - "http/json" - Use JSON over HTTP (default)
///
/// If an unsupported protocol value is provided, defaults to HTTP JSON with a warning message.
pub fn get_protocol() -> Protocol {
    match env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "http/protobuf" => Protocol::HttpBinary,
        "http/json" | "" => Protocol::HttpJson,
        unsupported => {
            eprintln!(
                "Warning: OTEL_EXPORTER_OTLP_PROTOCOL value '{}' is not supported. Defaulting to HTTP JSON.",
                unsupported
            );
            Protocol::HttpJson
        }
    }
}
