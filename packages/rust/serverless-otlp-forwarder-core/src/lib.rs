#![doc = include_str!("../README.md")]
// In: packages/rust/serverless-otlp-forwarder-core/src/lib.rs

pub mod telemetry;
pub use telemetry::TelemetryData;

pub mod span_compactor;
pub use span_compactor::{compact_telemetry_payloads, SpanCompactionConfig};

pub mod http_sender;
pub use http_sender::{client_builder, send_telemetry_batch, HttpClient};

#[cfg(feature = "instrumented-client")]
pub use http_sender::instrumented::InstrumentedHttpClient;

pub mod core_parser;
pub use core_parser::EventParser;

pub mod processor;
pub use processor::process_event_batch;
