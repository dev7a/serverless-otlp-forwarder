use crate::telemetry::TelemetryData;
use anyhow::Result;

pub trait EventParser {
    // The specific AWS event type (e.g., LogsEvent, KinesisEvent)
    // that this parser instance knows how to handle.
    type EventInput;

    fn parse(
        &self,
        event_payload: Self::EventInput,
        source_identifier: &str,
    ) -> Result<Vec<TelemetryData>>;
}
