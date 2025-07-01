use aws_sdk_kinesis::Client as KinesisClient;
use lambda_extension::{
    service_fn, tracing, Error, Extension, LambdaEvent, LambdaTelemetry, LambdaTelemetryRecord,
    LogBuffering, NextEvent, SharedService,
};
use opentelemetry::{trace::SpanId, Value as OtelValue};
use opentelemetry_sdk::trace::{SpanData, SpanExporter};

// Add nix for mkfifo (Re-add these)
use nix::errno::Errno;
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use std::path::Path;

use lambda_otel_lite::resource::get_lambda_resource;
use opentelemetry::trace::TraceId;
use otlp_stdout_span_exporter::{BufferOutput, OtlpStdoutSpanExporter};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::{mpsc, Mutex};

// Import for pipe reading
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

// Add the modules
mod aggregation;
mod config;
mod events;
mod kinesis;
mod otlp_parsing;
mod types;

// Use the types from the modules
use aggregation::SpanAggregator;
use config::Config;
use events::{ParsedPlatformEvent, PlatformEventData, TelemetrySpan};
use kinesis::KinesisBatch;
use types::ProcessorInput;

// Re-add chrono for timeout logic
use chrono::{Duration, Utc};

// Define the pipe path constant
const PIPE_PATH: &str = "/tmp/otlp-stdout-span-exporter.pipe";

// Application state
struct AppState {
    kinesis_client: KinesisClient,
    stream_name: Option<String>,
    batch: Mutex<KinesisBatch>,
    aggregations: Mutex<HashMap<String, SpanAggregator>>,
    exporter: OtlpStdoutSpanExporter,
    internal_exporter_buffer: Arc<BufferOutput>,
    processor_input_rx: Mutex<mpsc::Receiver<ProcessorInput>>,
    execution_trace_map: Mutex<HashMap<String, (TraceId, SpanId, Instant)>>,
    init_start_time: Mutex<Option<SystemTime>>,
}
impl AppState {
    async fn flush_batch(&self) -> Result<(), Error> {
        if self.stream_name.is_none() {
            tracing::debug!("Kinesis stream not configured, skipping flush.");
            let mut batch = self.batch.lock().await;
            if !batch.is_empty() {
                tracing::warn!(
                    "Clearing {} records from Kinesis batch because Kinesis is disabled.",
                    batch.records.len()
                );
                batch.clear();
            }
            return Ok(());
        }
        let stream_name = self.stream_name.as_ref().unwrap();

        let mut batch = self.batch.lock().await;
        if batch.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            "Sending batch of {} records to Kinesis stream {}",
            batch.records.len(),
            stream_name
        );

        let result = self
            .kinesis_client
            .put_records()
            .stream_name(stream_name)
            .set_records(Some(batch.records.clone()))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Kinesis batch error: {}", e);
                Error::from(format!("Failed to send records to Kinesis: {}", e))
            })?;

        let failed_count = result.failed_record_count.unwrap_or(0);
        if failed_count > 0 {
            tracing::warn!("Failed to put {} records", failed_count);
            let records = result.records();
            for (i, record) in records.iter().enumerate() {
                if let Some(error_code) = &record.error_code {
                    tracing::warn!(
                        "Record {} failed with error: {} - {}",
                        i,
                        error_code,
                        record
                            .error_message
                            .as_deref()
                            .unwrap_or("No error message")
                    );
                }
            }
        } else {
            tracing::debug!("Successfully sent all records to Kinesis");
        }

        batch.clear();
        Ok(())
    }
}

async fn telemetry_handler(
    events: Vec<LambdaTelemetry>,
    tx: mpsc::Sender<ProcessorInput>,
) -> Result<(), Error> {
    for event in events {
        let timestamp = event.time;
        tracing::debug!("Received event: {:?}", event);
        let parsed_event_opt = match event.record {
            // --- Add Case for PlatformInitStart --- START ---
            LambdaTelemetryRecord::PlatformInitStart {
                initialization_type:_, // Ignore initialization_type for now
                phase:_, // Ignore phase for now
                .. // Ignore other fields like runtime_version for now
            } => {
                // TODO removed - InitStart variant needs no fields currently
                Some(ParsedPlatformEvent {
                    timestamp,
                    // PlatformInitStart doesn't have a request_id, use an empty string for now.
                    request_id: "".to_string(),
                    data: PlatformEventData::InitStart {
                        // No fields needed
                     },
                })
            }
            // --- Add Case for PlatformInitStart --- END ---
            LambdaTelemetryRecord::PlatformStart {
                request_id,
                version,
                tracing: _, // Ignore tracing field
            } => {
                // No need to parse X-Ray header since we'll correlate via the execution_trace_map
                Some(ParsedPlatformEvent {
                    timestamp,
                    request_id,
                    data: PlatformEventData::Start {
                        version,
                        // trace_context field removed
                    },
                })
            }
            LambdaTelemetryRecord::PlatformRuntimeDone {
                request_id,
                status,
                error_type,
                metrics,
                spans,
                tracing: _, // Ignore tracing field
            } => {
                // No need to parse X-Ray header since we'll correlate via the execution_trace_map
                let telemetry_spans = spans.into_iter().map(TelemetrySpan::from).collect();
                let mut attributes = HashMap::new();
                if let Some(m) = metrics {
                    attributes.insert(
                        "runtime.durationMs".to_string(),
                        OtelValue::F64(m.duration_ms),
                    );
                    if let Some(pb) = m.produced_bytes {
                        attributes.insert(
                            "runtime.producedBytes".to_string(),
                            OtelValue::I64(pb as i64),
                        );
                    }
                }

                Some(ParsedPlatformEvent {
                    timestamp,
                    request_id,
                    data: PlatformEventData::RuntimeDone {
                        status,
                        error_type,
                        metrics: attributes,
                        spans: telemetry_spans,
                        // trace_context field removed
                    },
                })
            }
            LambdaTelemetryRecord::PlatformReport {
                request_id,
                status,
                error_type,
                metrics,
                spans,
                tracing: _, // Ignore tracing field
            } => {
                // Check for init duration metric BEFORE creating the ParsedPlatformEvent
                let init_duration_ms_opt = metrics.init_duration_ms;

                // Create the normal ParsedPlatformEvent
                let telemetry_spans = spans.into_iter().map(TelemetrySpan::from).collect();
                let mut attributes = HashMap::new();
                attributes.insert(
                    "report.durationMs".to_string(),
                    OtelValue::F64(metrics.duration_ms),
                );
                attributes.insert(
                    "report.billedDurationMs".to_string(),
                    OtelValue::I64(metrics.billed_duration_ms as i64),
                );
                attributes.insert(
                    "report.memorySizeMB".to_string(),
                    OtelValue::I64(metrics.memory_size_mb as i64),
                );
                attributes.insert(
                    "report.maxMemoryUsedMB".to_string(),
                    OtelValue::I64(metrics.max_memory_used_mb as i64),
                );
                if let Some(id) = metrics.init_duration_ms {
                    attributes.insert("report.initDurationMs".to_string(), OtelValue::F64(id));
                }
                if let Some(rd) = metrics.restore_duration_ms {
                    attributes.insert("report.restoreDurationMs".to_string(), OtelValue::F64(rd));
                }

                let parsed_event = ParsedPlatformEvent {
                    timestamp,
                    request_id: request_id.clone(), // Clone request_id here
                    data: PlatformEventData::Report {
                        status,
                        error_type,
                        metrics: attributes,
                        spans: telemetry_spans,
                    },
                };

                // Send the InitDataAvailable message IF init duration was present
                if let Some(init_duration_ms) = init_duration_ms_opt {
                     tracing::debug!(request_id = %request_id, init_duration_ms, "Found init duration in report, sending InitDataAvailable message");
                     if let Err(e) = tx.send(ProcessorInput::InitDataAvailable { request_id: request_id.clone(), init_duration_ms }).await {
                         tracing::error!("Failed to send InitDataAvailable to processor channel: {}", e);
                     }
                 }

                // Return the normal parsed event to be sent as PlatformTelemetry
                Some(parsed_event)
            }
            // Ignore all init phase and other events
            _ => None,
        };

        // Send the PlatformTelemetry message (if any was created)
        if let Some(parsed_event) = parsed_event_opt {
            if let Err(e) = tx
                .send(ProcessorInput::PlatformTelemetry(parsed_event))
                .await
            {
                tracing::error!("Failed to send platform event to processor channel: {}", e);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    tracing::debug!("Starting OTLP Stdout Kinesis Extension");

    // --- Create Named Pipe ---
    let pipe_path = Path::new(PIPE_PATH);
    if !pipe_path.exists() {
        let pipe_path_str = PIPE_PATH.to_string();
        tokio::task::spawn_blocking(move || {
            match mkfifo(
                pipe_path_str.as_str(),
                Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO,
            ) {
                Ok(_) => tracing::debug!("Created named pipe: {}", pipe_path_str),
                Err(Errno::EEXIST) => {
                    tracing::debug!("Named pipe already exists: {}", pipe_path_str);
                }
                Err(e) => {
                    panic!("Failed to create named pipe {}: {}", pipe_path_str, e);
                }
            }
        })
        .await
        .map_err(|e| Error::from(format!("Pipe creation task failed: {}", e)))?;
    } else {
        tracing::debug!("Named pipe already exists: {}", PIPE_PATH);
    }
    // --- Create Named Pipe --- END ---

    let config = Config::from_env()?;

    let aws_config = aws_config::from_env().load().await;
    let kinesis_client = KinesisClient::new(&aws_config);

    // --- Create Channel for Platform Telemetry ---
    let (telemetry_tx, telemetry_rx) = mpsc::channel::<ProcessorInput>(2048);
    // --- Create Channel for Platform Telemetry --- END ---

    // Create the buffer for the internal exporter
    let internal_exporter_buffer = Arc::new(BufferOutput::new());

    let exporter = OtlpStdoutSpanExporter::builder()
        .resource(get_lambda_resource())
        .output(internal_exporter_buffer.clone())
        .build();

    let aggregations = Mutex::new(HashMap::<String, SpanAggregator>::new());
    let execution_trace_map = Mutex::new(HashMap::<String, (TraceId, SpanId, Instant)>::new());
    let init_start_time = Mutex::new(None::<SystemTime>);

    let app_state = Arc::new(AppState {
        kinesis_client,
        stream_name: config.kinesis_stream_name.clone(),
        batch: Mutex::new(KinesisBatch::default()),
        aggregations,
        exporter,
        internal_exporter_buffer: internal_exporter_buffer.clone(),
        processor_input_rx: Mutex::new(telemetry_rx),
        execution_trace_map,
        init_start_time,
    });

    let telemetry_tx_clone = telemetry_tx.clone();
    let telemetry_handler_fn = move |events: Vec<LambdaTelemetry>| {
        let tx = telemetry_tx_clone.clone();
        async move { telemetry_handler(events, tx).await }
    };

    let processor_state = app_state.clone();

    // Define timeout duration (e.g., 30 minutes)
    // TODO: Make this configurable?
    let aggregation_timeout = Duration::try_minutes(30).unwrap_or(Duration::MAX);

    let events_processor = service_fn(move |event: LambdaEvent| {
        let state = processor_state.clone();

        async move {
            match event.next {
                NextEvent::Invoke(invoke_event) => {
                    let current_request_id = invoke_event.request_id.clone(); // Get request_id
                    tracing::debug!(request_id = %current_request_id, "Received INVOKE event, processing pipe data and platform telemetry");

                    // --- Read from pipe until EOF --- START ---
                    let mut found_trace_info_for_invoke = false; // Flag to parse only once
                    match File::open(PIPE_PATH).await {
                        Ok(pipe_file) => {
                            tracing::debug!("Named pipe opened successfully: {}", PIPE_PATH);
                            let mut reader = BufReader::new(pipe_file);
                            let mut line_buffer = String::new();

                            // Read lines from the pipe until EOF
                            loop {
                                match reader.read_line(&mut line_buffer).await {
                                    Ok(0) => {
                                        tracing::debug!("EOF reached on named pipe for request_id {} - all spans for this invocation processed", current_request_id);
                                        break; // Exit the pipe reading loop
                                    }
                                    Ok(_) => {
                                        let line = line_buffer.trim_end();
                                        if !line.is_empty() {
                                            // --- Attempt to extract trace info ONCE per invoke --- START ---
                                            if !found_trace_info_for_invoke {
                                                match otlp_parsing::extract_trace_info_from_json_line(line) {
                                                    Ok(Some((trace_id, span_id))) => {
                                                        tracing::debug!(%trace_id, %span_id, request_id = %current_request_id, "Storing trace info mapping");
                                                        let mut map = state.execution_trace_map.lock().await;
                                                        map.insert(current_request_id.clone(), (trace_id, span_id, Instant::now()));
                                                        drop(map);
                                                        found_trace_info_for_invoke = true; // Mark as found
                                                    }
                                                    Ok(None) => {
                                                        // Line was valid JSON but not OTLP trace data, or no spans found. Ignore for mapping.
                                                        tracing::trace!("Line did not yield trace info for mapping.");
                                                    }
                                                    Err(e) => {
                                                        // Parsing/decoding error, log it but don't stop processing lines
                                                        tracing::warn!(error = %e, request_id = %current_request_id, "Error extracting trace info from line");
                                                        // Potentially mark found_trace_info_for_invoke = true here too,
                                                        // if we want to stop trying after the first error?
                                                        // For now, let's keep trying on subsequent lines just in case.
                                                    }
                                                }
                                            }
                                            // --- Attempt to extract trace info ONCE per invoke --- END ---

                                            // Existing Kinesis/stdout forwarding logic
                                            if state.stream_name.is_some() {
                                                let mut kinesis_batch = state.batch.lock().await;
                                                if let Err(e) =
                                                    kinesis_batch.add_record(line.to_string())
                                                {
                                                    tracing::error!(error = %e, "Failed to add record to Kinesis batch");
                                                }
                                            } else {
                                                // Maybe use tokio::io::stdout().write_all(line.as_bytes()).await? Careful with async in sync context if not.
                                                // For simplicity, using println! which is blocking but often acceptable in Lambda extensions for low volume.
                                                println!("{}", line);
                                            }
                                        }
                                        line_buffer.clear();
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, path = PIPE_PATH, "Error reading line from named pipe");
                                        break; // Break on error
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, path = PIPE_PATH, "Failed to open named pipe for reading");
                        }
                    }
                    // --- Read from pipe until EOF --- END ---

                    // --- Process any platform telemetry that was received --- START ---
                    // Drain the platform telemetry channel (non-blocking)
                    loop {
                        let mut receiver_guard = state.processor_input_rx.lock().await;
                        match receiver_guard.try_recv() {
                            Ok(ProcessorInput::PlatformTelemetry(parsed_event)) => {
                                drop(receiver_guard); // Drop lock ASAP

                                // --- Handle InitStart Event --- START ---
                                if let PlatformEventData::InitStart { .. } = parsed_event.data {
                                    tracing::debug!(
                                        "Received InitStart platform event, storing start time."
                                    );
                                    let mut init_start_opt = state.init_start_time.lock().await;
                                    *init_start_opt = Some(parsed_event.timestamp.into());
                                    drop(init_start_opt);
                                    // Don't process InitStart further in aggregation
                                    continue;
                                }
                                // --- Handle InitStart Event --- END ---

                                tracing::debug!(
                                    "Processing platform telemetry for request_id: {}",
                                    parsed_event.request_id
                                );

                                // --- Correlation Logic --- START ---
                                // Always attempt to look up trace info for this request_id
                                let map = state.execution_trace_map.lock().await;
                                let trace_info = map.get(&parsed_event.request_id).cloned();
                                drop(map); // Release lock

                                // Pass the trace info to the aggregator during update or creation
                                let mut aggregations_map = state.aggregations.lock().await;
                                let key = parsed_event.request_id.clone();
                                let mut completed_spans: Vec<SpanData> = Vec::new();

                                if let Some(agg) = aggregations_map.get_mut(&key) {
                                    // Pass any found trace info
                                    if let Some((trace_id, parent_span_id, _)) = trace_info {
                                        agg.set_trace_context(trace_id, parent_span_id);
                                    }

                                    agg.update_from_event(&parsed_event);
                                    if agg.is_complete() {
                                        if let Some(span_data) = agg.to_otel_span_data() {
                                            completed_spans.push(span_data);
                                            completed_spans.append(&mut agg.child_spans_data);
                                        }
                                        aggregations_map.remove(&key);
                                    }
                                } else {
                                    let mut new_agg =
                                        SpanAggregator::new(key, parsed_event.timestamp);

                                    // Pass any found trace info
                                    if let Some((trace_id, parent_span_id, _)) = trace_info {
                                        new_agg.set_trace_context(trace_id, parent_span_id);
                                    }

                                    new_agg.update_from_event(&parsed_event);
                                    if new_agg.is_complete() {
                                        if let Some(span_data) = new_agg.to_otel_span_data() {
                                            completed_spans.push(span_data);
                                            completed_spans.append(&mut new_agg.child_spans_data);
                                        }
                                    } else {
                                        aggregations_map
                                            .insert(new_agg.request_id.clone(), new_agg);
                                    }
                                }
                                drop(aggregations_map); // Drop lock before await

                                // --- Export and Buffer Handling --- START ---
                                if !completed_spans.is_empty() {
                                    let span_count = completed_spans.len();
                                    tracing::debug!(count = span_count, "Attempting to export completed/aggregated spans via OtlpStdoutSpanExporter");
                                    match state.exporter.export(completed_spans).await {
                                        Ok(_) => {
                                            tracing::debug!(count = span_count, "Successfully called exporter. Aggregated spans should now be in the buffer.");
                                            // Note: Output will be processed right after this block.
                                        }
                                        Err(e) => {
                                            tracing::error!(count = span_count, error = ?e, "Failed to export aggregated spans")
                                        }
                                    }

                                    // --- Process aggregated spans from the internal exporter's buffer --- START ---
                                    match state.internal_exporter_buffer.take_lines() {
                                        // Get lines & clear buffer
                                        Ok(aggregated_lines) => {
                                            // Successfully got the lines
                                            if !aggregated_lines.is_empty() {
                                                tracing::debug!("Processing {} line(s) from internal exporter buffer", aggregated_lines.len());
                                                if state.stream_name.is_some() {
                                                    // Add to Kinesis batch if Kinesis is enabled
                                                    let mut kinesis_batch =
                                                        state.batch.lock().await;
                                                    for line in aggregated_lines {
                                                        // Iterate over the Vec<String>
                                                        if let Err(e) =
                                                            kinesis_batch.add_record(line)
                                                        {
                                                            tracing::error!(error = %e, "Failed to add aggregated span record to Kinesis batch");
                                                        }
                                                    }
                                                    drop(kinesis_batch);
                                                } else {
                                                    // Otherwise, print to stdout (CloudWatch Logs)
                                                    for line in aggregated_lines {
                                                        // Iterate over the Vec<String>
                                                        println!("{}", line);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to take lines from internal exporter buffer: {:?}", e);
                                        }
                                    }
                                    // --- Process aggregated spans from the internal exporter's buffer --- END ---
                                }
                                // --- Export and Buffer Handling --- END ---
                            }
                            // --- Add Case for InitDataAvailable --- START ---
                            Ok(ProcessorInput::InitDataAvailable {
                                request_id,
                                init_duration_ms,
                            }) => {
                                drop(receiver_guard); // Drop lock ASAP
                                tracing::debug!(request_id = %request_id, init_duration_ms, "Processing InitDataAvailable");

                                let init_start_opt = state.init_start_time.lock().await.take();

                                if let Some(init_start_time) = init_start_opt {
                                    let mut aggregations_map = state.aggregations.lock().await;
                                    if let Some(agg) = aggregations_map.get_mut(&request_id) {
                                        agg.add_init_phase_span(init_start_time, init_duration_ms);
                                    } else {
                                        tracing::warn!(request_id = %request_id, "Aggregator not found when trying to add init phase span. It might have completed or timed out already.");
                                    }
                                    drop(aggregations_map);
                                } else {
                                    tracing::warn!(request_id = %request_id, "Received InitDataAvailable but init_start_time was None.");
                                }
                            }
                            // --- Add Case for InitDataAvailable --- END ---
                            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                                // No more telemetry to process
                                drop(receiver_guard);
                                break;
                            }
                            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                                // Channel closed - shouldn't happen in normal operation
                                drop(receiver_guard);
                                tracing::warn!("Platform telemetry channel disconnected");
                                break;
                            }
                        }
                    }
                    // --- Process platform telemetry --- END ---

                    // --- Handle Aggregation Timeouts --- START ---
                    let mut timed_out_spans: Vec<SpanData> = Vec::new();
                    let mut timed_out_req_ids: Vec<String> = Vec::new(); // To clean up map later
                    {
                        let mut aggregations_map = state.aggregations.lock().await;
                        let now = Utc::now();

                        aggregations_map.retain(|key, agg| {
                            if (now - agg.first_seen_timestamp) > aggregation_timeout {
                                tracing::warn!(request_id = %key, timeout = ?aggregation_timeout, "Aggregation timed out. Emitting.");
                                if let Some(span_data) = agg.to_otel_span_data() {
                                    timed_out_spans.push(span_data);
                                    timed_out_spans.append(&mut agg.child_spans_data);
                                }
                                timed_out_req_ids.push(key.clone()); // Mark for map cleanup
                                false // Remove from aggregation map
                            } else {
                                true // Keep in aggregation map
                            }
                        });
                    } // Aggregation map lock released

                    // --- TTL Eviction for Execution Trace Map --- START ---
                    {
                        let mut map = state.execution_trace_map.lock().await;
                        let cutoff = Instant::now()
                            .checked_sub(std::time::Duration::from_secs(300))
                            .unwrap_or_else(Instant::now); // Use 5 minutes TTL
                        let initial_size = map.len();
                        map.retain(|req_id, (_, _, timestamp)| {
                            // Also remove entries explicitly marked from aggregation timeout
                            if timed_out_req_ids.contains(req_id) {
                                return false;
                            }
                            *timestamp >= cutoff
                        });
                        let removed_count = initial_size - map.len();
                        if removed_count > 0 {
                            tracing::debug!(
                                "Removed {} expired entries from execution trace map",
                                removed_count
                            );
                        }
                    }
                    // --- TTL Eviction for Execution Trace Map --- END ---

                    // Export timed-out spans (if any)
                    if !timed_out_spans.is_empty() {
                        tracing::debug!("Exporting {} timed-out spans", timed_out_spans.len());
                        match state.exporter.export(timed_out_spans).await {
                            Ok(_) => tracing::debug!("Successfully exported timed-out spans"),
                            Err(e) => tracing::error!("Failed to export timed-out spans: {:?}", e),
                        }
                    }
                    // --- Handle Aggregation Timeouts --- END ---

                    // Flush Kinesis batch
                    if let Err(e) = state.flush_batch().await {
                        tracing::error!("Error flushing Kinesis batch on INVOKE: {}", e);
                    }
                }
                NextEvent::Shutdown(_) => {
                    tracing::debug!(
                        "Received SHUTDOWN event, flushing final aggregations and Kinesis batch"
                    );

                    // --- Final Aggregation Flush --- START ---
                    let mut final_spans_to_export: Vec<SpanData> = Vec::new();
                    {
                        let mut aggregations_map = state.aggregations.lock().await;
                        tracing::debug!(
                            "Draining {} remaining aggregations on shutdown",
                            aggregations_map.len()
                        );
                        // Drain the map, converting each remaining aggregator to spans
                        for (key, mut agg) in aggregations_map.drain() {
                            tracing::debug!(
                                "Flushing remaining agg for request_id '{}' on shutdown",
                                key
                            );
                            if let Some(span_data) = agg.to_otel_span_data() {
                                final_spans_to_export.push(span_data);
                                final_spans_to_export.append(&mut agg.child_spans_data);
                            }
                        }
                    } // Aggregations map lock released

                    // Export the final spans (if any) via the exporter (writes to pipe)
                    if !final_spans_to_export.is_empty() {
                        tracing::debug!(
                            "Exporting {} final spans on shutdown",
                            final_spans_to_export.len()
                        );
                        match state.exporter.export(final_spans_to_export).await {
                            Ok(_) => {
                                tracing::debug!("Successfully exported final spans on shutdown")
                            }
                            Err(e) => {
                                tracing::error!("Failed to export final spans on shutdown: {:?}", e)
                            }
                        }
                    }
                    // --- Final Aggregation Flush --- END ---

                    // --- Clear Execution Trace Map on Shutdown --- START ---
                    {
                        let mut map = state.execution_trace_map.lock().await;
                        let count = map.len();
                        if count > 0 {
                            tracing::debug!(
                                "Clearing {} entries from execution trace map on shutdown",
                                count
                            );
                            map.clear();
                        }
                    }
                    // --- Clear Execution Trace Map on Shutdown --- END ---

                    // --- Clear Init Start Time on Shutdown --- START ---
                    {
                        let mut init_start_opt = state.init_start_time.lock().await;
                        if init_start_opt.is_some() {
                            tracing::debug!(
                                "Clearing potentially stale init_start_time on shutdown."
                            );
                            *init_start_opt = None;
                        }
                    }
                    // --- Clear Init Start Time on Shutdown --- END ---

                    // Final Kinesis Flush (already implemented)
                    if let Err(e) = state.flush_batch().await {
                        tracing::error!("Error flushing Kinesis batch on SHUTDOWN: {}", e);
                    }
                }
            }

            Ok::<(), Error>(())
        }
    });

    // Build and run the extension with appropriate configuration
    if config.enable_platform_telemetry {
        tracing::debug!("Platform telemetry processing enabled");
        Extension::new()
            .with_events(&["INVOKE", "SHUTDOWN"])
            .with_events_processor(events_processor)
            .with_telemetry_processor(SharedService::new(service_fn(telemetry_handler_fn)))
            .with_telemetry_types(&["platform"])
            .with_telemetry_buffering(LogBuffering {
                timeout_ms: config.buffer_timeout_ms as usize,
                max_bytes: config.buffer_max_bytes,
                max_items: config.buffer_max_items,
            })
            .run()
            .await
    } else {
        tracing::debug!("Platform telemetry processing disabled");
        Extension::new()
            .with_events(&["INVOKE", "SHUTDOWN"])
            .with_events_processor(events_processor)
            .run()
            .await
    }
}
