use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use tracing::{
    field::{Field, Visit},
    Event, Subscriber,
};
use tracing_subscriber::layer::{Context, Layer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedEvent {
    pub(crate) fields: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
struct EventFieldVisitor {
    fields: BTreeMap<String, String>,
}

impl Visit for EventFieldVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

#[derive(Debug, Default)]
pub(crate) struct EventCaptureLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl EventCaptureLayer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn events(&self) -> Arc<Mutex<Vec<CapturedEvent>>> {
        Arc::clone(&self.events)
    }
}

impl<S> Layer<S> for EventCaptureLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = EventFieldVisitor::default();
        event.record(&mut visitor);
        self.events.lock().unwrap().push(CapturedEvent {
            fields: visitor.fields,
        });
    }
}
