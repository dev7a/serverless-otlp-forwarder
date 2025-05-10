//! Adapts the AWS CloudWatch Logs `StartLiveTail` API for use in `livetrace`.
//!
//! This module is responsible for:
//! - Spawning an asynchronous task that initiates a `StartLiveTail` session for
//!   a given set of log group ARNs.
//! - Receiving `StartLiveTailResponseStream` events (session start, updates with log data).
//! - Processing log event messages from the stream using functions from the `processing` module.
//! - Sending the resulting `TelemetryData` (or errors) over an MPSC channel to the main
//!   application logic.
//! - Handling session timeouts.

use anyhow::Result;
use aws_sdk_cloudwatchlogs::{types::StartLiveTailResponseStream, Client as CwlClient};
use std::time::Duration;
use tokio::pin;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::processing::{process_log_event_message, TelemetryData};

/// Spawns a task that runs StartLiveTail and sends processed TelemetryData over an MPSC channel.
pub fn start_live_tail_task(
    cwl_client: CwlClient,
    arns: Vec<String>,
    sender: mpsc::Sender<Result<TelemetryData>>,
    timeout_minutes: u64,
) {
    tokio::spawn(async move {
        tracing::debug!("Live Tail Adapter: Attempting to start Live Tail stream...");
        let live_tail_result = cwl_client
            .start_live_tail()
            .set_log_group_identifiers(Some(arns))
            .log_event_filter_pattern("{ $.__otel_otlp_stdout = * }")
            .send()
            .await;

        let mut stream = match live_tail_result {
            Ok(output) => {
                tracing::debug!("Live Tail Adapter: Stream started successfully.");
                output.response_stream
            }
            Err(e) => {
                let err_msg = format!("Live Tail Adapter: Failed to start Live Tail: {}", e);
                tracing::error!(%err_msg);
                // Send error over channel and exit task
                let _ = sender
                    .send(Err(
                        anyhow::Error::new(e).context("Failed to start Live Tail stream")
                    ))
                    .await;
                return; // Exit the spawned task
            }
        };

        // Setup timeout
        let timeout_duration = Duration::from_secs(timeout_minutes * 60);
        let timeout_sleep = sleep(timeout_duration);
        pin!(timeout_sleep);

        tracing::debug!(timeout = ?timeout_duration, "Live Tail Adapter: Waiting for stream events with timeout...");
        loop {
            tokio::select! {
                // Branch for receiving stream events
                received = stream.recv() => {
                    match received {
                        Ok(Some(event_stream)) => {
                            match event_stream {
                                StartLiveTailResponseStream::SessionStart(start_info) => {
                                    tracing::debug!(
                                        "Live Tail Adapter: Session started. Request ID: {}, Session ID: {}",
                                        start_info.request_id().unwrap_or("N/A"),
                                        start_info.session_id().unwrap_or("N/A")
                                    );
                                }
                                StartLiveTailResponseStream::SessionUpdate(update) => {
                                    let log_events = update.session_results();
                                    tracing::trace!("Live Tail Adapter: Received update with {} log events.", log_events.len());
                                    for log_event in log_events {
                                        if let Some(msg) = log_event.message() {
                                            match process_log_event_message(msg) {
                                                Ok(Some(telemetry)) => {
                                                    if sender.send(Ok(telemetry)).await.is_err() {
                                                        tracing::warn!("Live Tail Adapter: MPSC channel closed by receiver while sending data.");
                                                        return; // Exit task
                                                    }
                                                }
                                                Ok(None) => {} // Ignore
                                                Err(e) => {
                                                    tracing::warn!(message = ?msg, error = %e, "Live Tail Adapter: Failed to process log event");
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    tracing::warn!("Live Tail Adapter: Received unexpected/unknown event type from stream.");
                                }
                            }
                        }
                        Ok(None) => {
                            tracing::info!("Live Tail Adapter: Stream ended gracefully.");
                            break; // Stream closed
                        }
                        Err(e) => {
                            let err_msg = format!("Live Tail Adapter: Error receiving event from stream: {}", e);
                            tracing::error!(%err_msg);
                            let _ = sender.send(Err(anyhow::Error::new(e).context("Error receiving from Live Tail stream"))).await;
                            break;
                        }
                    }
                }
                // Branch for timeout
                _ = &mut timeout_sleep => {
                    tracing::info!(timeout_minutes, "Live Tail Adapter: Session timeout reached. Stopping stream task.");
                    break; // Exit loop, task will finish
                }
            }
        }
        tracing::debug!("Live Tail Adapter: Task finished.");
        // Sender is dropped here, closing the channel naturally
    });
}
