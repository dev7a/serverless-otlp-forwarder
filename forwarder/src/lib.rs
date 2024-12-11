pub mod collectors;
pub mod headers;
pub mod processing;
pub mod telemetry;
pub mod otlp;

// Re-export commonly used types
pub use collectors::Collectors;
pub use headers::LogRecordHeaders;
pub use processing::send_telemetry;
pub use telemetry::TelemetryData;
