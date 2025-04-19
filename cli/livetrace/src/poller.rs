use anyhow::Result;
use aws_sdk_cloudwatchlogs::Client as CwlClient;
use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

use crate::processing::{process_log_event_message, TelemetryData};

/// Spawns a task that polls FilterLogEvents for multiple log groups and sends results over a channel.
pub fn start_polling_task(
    cwl_client: CwlClient,
    arns: Vec<String>,
    interval_secs: u64,
    sender: mpsc::Sender<Result<TelemetryData>>,
) {
    tokio::spawn(async move {
        let mut last_timestamps: HashMap<String, i64> = HashMap::new();
        let poll_duration = Duration::from_secs(interval_secs);
        let mut ticker = interval(poll_duration);

        let initial_start_time_ms = Utc::now().timestamp_millis();
        tracing::debug!(
            start_time = initial_start_time_ms,
            "Polling will start from current time."
        );

        tracing::debug!(
            interval_seconds = interval_secs,
            num_groups = arns.len(),
            "Polling Adapter: Starting polling loop."
        );

        loop {
            ticker.tick().await;
            tracing::trace!("Polling Adapter: Tick");

            for arn in &arns {
                let start_time = *last_timestamps.get(arn).unwrap_or(&initial_start_time_ms);
                let arn_clone = arn.clone();
                let client_clone = cwl_client.clone();
                let sender_clone = sender.clone();

                tracing::debug!(log_group_arn = %arn_clone, %start_time, "Polling Adapter: Fetching events for group.");

                match filter_log_events_for_group(
                    &client_clone,
                    arn_clone.clone(),
                    start_time,
                    sender_clone,
                )
                .await
                {
                    Ok(Some(new_timestamp)) => {
                        tracing::trace!(log_group_arn=%arn_clone, %new_timestamp, "Polling Adapter: Updating timestamp.");
                        last_timestamps.insert(arn_clone, new_timestamp);
                    }
                    Ok(None) => {
                        tracing::trace!(log_group_arn=%arn_clone, "Polling Adapter: No new events found.");
                    }
                    Err(e) => {
                        tracing::error!(log_group_arn = %arn_clone, error = %e, "Polling Adapter: Error polling log group.");
                    }
                }
            }
        }
    });
}

/// Fetches and processes events for a single log group using FilterLogEvents.
/// Handles pagination and sends TelemetryData or errors over the channel.
/// Returns Ok(Some(timestamp)) of the last processed event if successful, Ok(None) if no events, Err on failure.
async fn filter_log_events_for_group(
    client: &CwlClient,
    log_group_identifier: String,
    start_time_ms: i64,
    sender: mpsc::Sender<Result<TelemetryData>>,
) -> Result<Option<i64>> {
    let mut next_token: Option<String> = None;
    let mut latest_event_timestamp = start_time_ms;
    let mut events_found = false;

    loop {
        let mut request_builder = client
            .filter_log_events()
            .log_group_identifier(log_group_identifier.clone())
            .start_time(start_time_ms + 1);

        if let Some(token) = next_token {
            request_builder = request_builder.next_token(token);
        }

        match request_builder.send().await {
            Ok(output) => {
                if let Some(events) = output.events {
                    if !events.is_empty() {
                        events_found = true;
                    }
                    for event in events {
                        if let Some(timestamp) = event.timestamp {
                            latest_event_timestamp = latest_event_timestamp.max(timestamp);
                        }

                        if let Some(msg) = event.message {
                            match process_log_event_message(&msg) {
                                Ok(Some(telemetry)) => {
                                    if sender.send(Ok(telemetry)).await.is_err() {
                                        tracing::warn!("Polling Adapter: MPSC channel closed by receiver while sending data.");
                                        return Err(anyhow::anyhow!("MPSC receiver closed"));
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    tracing::warn!(message = ?msg, error = %e, "Polling Adapter: Failed to process polled log event");
                                }
                            }
                        }
                    }
                }

                if let Some(token) = output.next_token {
                    next_token = Some(token);
                    tracing::trace!(log_group=%log_group_identifier, "Polling Adapter: Got next token, continuing pagination.");
                } else {
                    break;
                }
            }
            Err(e) => {
                let context_msg = format!(
                    "Polling Adapter: Failed to filter log events for {}",
                    log_group_identifier
                );
                tracing::error!(error = %e, %context_msg);
                let _ = sender
                    .send(Err(anyhow::Error::new(e).context(context_msg)))
                    .await;
                return Err(anyhow::anyhow!(
                    "Failed to filter log events for group {}",
                    log_group_identifier
                ));
            }
        }
    }

    if events_found {
        Ok(Some(latest_event_timestamp))
    } else {
        Ok(None)
    }
}
