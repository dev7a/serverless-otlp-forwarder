// myotel/src/span_event.rs
use opentelemetry::KeyValue;
use std::{env, time::SystemTime};
use tracing;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use std::sync::OnceLock;

#[repr(i32)]
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum Level { Trace = 1, Debug = 5, Info = 9, Warn = 13, Error = 17 }

impl From<Level> for tracing::Level {
    fn from(level: Level) -> Self {
        match level {
            Level::Trace => tracing::Level::TRACE,
            Level::Debug => tracing::Level::DEBUG,
            Level::Info  => tracing::Level::INFO,
            Level::Warn  => tracing::Level::WARN,
            Level::Error => tracing::Level::ERROR,
        }
    }
}

pub(crate) fn level_text(l: Level) -> &'static str {
    match l {
        Level::Trace => "TRACE",
        Level::Debug => "DEBUG",
        Level::Info  => "INFO",
        Level::Warn  => "WARN",
        Level::Error => "ERROR",
    }
}

pub(crate) static MIN_LEVEL: OnceLock<Level> = OnceLock::new();

pub(crate) fn get_min_level() -> Level {
    *MIN_LEVEL.get_or_init(|| {
        match env::var("EVENTS_LOG_LEVEL").unwrap_or_else(|_| "TRACE".into()).to_uppercase().as_str() {
            "ERROR" => Level::Error,
            "WARN"  => Level::Warn,
            "INFO"  => Level::Info,
            "DEBUG" => Level::Debug,
            _       => Level::Trace,
        }
    })
}

pub fn span_event<N, B>(
    name: N,
    body: B,
    level: Level,
    attrs: Vec<KeyValue>,
    ts: Option<SystemTime>,
) where 
    N: AsRef<str>,
    B: AsRef<str>
{
    if level < get_min_level() { return; }

    let name_string = name.as_ref().to_string();
    let body_string = body.as_ref().to_string();
    
    // Create attributes with cloned values to avoid lifetime issues
    let mut all_attrs = Vec::with_capacity(attrs.len() + 3);
    all_attrs.extend_from_slice(&[
        KeyValue::new("event.severity_text", level_text(level)),
        KeyValue::new("event.severity_number", level as i64),
        KeyValue::new("event.body", body_string.clone()),
    ]);
    all_attrs.extend(attrs);

    // Get current tracing span and convert to OpenTelemetry context
    let current_tracing_span = tracing::Span::current();
    let otel_cx = current_tracing_span.context();

    // Attach the context and add the event
    let _guard = otel_cx.attach();

    // Use the attached context to get the active span
    current_tracing_span.set_attribute("event.name", name_string.clone());

    match ts {
        Some(t) => current_tracing_span.add_event_with_timestamp(name_string, t, all_attrs),
        None => current_tracing_span.add_event(name_string, all_attrs),
    }
} 