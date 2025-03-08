pub mod app_state;
pub mod collectors;
pub mod headers;
pub mod processing;
pub mod span_compactor;
pub mod telemetry;
pub mod wrappers;

// Re-export commonly used types
pub use app_state::AppState;
pub use collectors::Collectors;
pub use headers::LogRecordHeaders;
pub use processing::send_telemetry;
pub use span_compactor::{compact_telemetry_payloads, SpanCompactionConfig};
pub use telemetry::TelemetryData;
pub use wrappers::{KinesisEventWrapper, LogsEventWrapper};
