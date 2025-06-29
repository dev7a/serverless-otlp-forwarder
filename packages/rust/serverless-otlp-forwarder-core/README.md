# Serverless OTLP Forwarder Core

[![Crates.io](https://img.shields.io/crates/v/serverless-otlp-forwarder-core.svg)](https://crates.io/crates/serverless-otlp-forwarder-core)

The `serverless-otlp-forwarder-core` crate provides essential, shared components for building AWS Lambda functions that process and forward OpenTelemetry (OTLP) data. It is a core part of the [Serverless OTLP Forwarder project](https://github.com/dev7a/serverless-otlp-forwarder).

This crate is designed to be used by specific Lambda processor implementations, offering a standardized way to parse, compact, and send OTLP telemetry batches.

## Table of Contents

- [Features](#features)
- [Core Components](#core-components)
    - [`TelemetryData`](#telemetrydata)
    - [`EventParser` Trait](#eventparser-trait)
    - [Span Compaction](#span-compaction)
    - [HTTP Sender](#http-sender)
    - [HTTP Client Options](#http-client-options)
    - [`process_event_batch` Orchestrator](#process_event_batch-orchestrator)
- [Installation](#installation)
- [Usage Example](#usage-example)
- [Environment Variables](#environment-variables)
- [License](#license)

## Features

- **Standardized Telemetry Handling**: Defines a common `TelemetryData` struct for internal OTLP representation.
- **Pluggable Event Parsing**: Uses an `EventParser` trait to allow different Lambda processors for different event sources to implement their specific event decoding logic.
- **Efficient Batching**: Includes a `span_compactor` module to merge multiple OTLP messages into a single batch.
- **Configurable OTLP Export**: Provides an HTTP sender that respects standard OpenTelemetry environment variables for endpoint and header configuration (e.g., `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, `OTEL_EXPORTER_OTLP_TRACES_HEADERS`).
- **Simplified Processor Logic**: Offers a generic `process_event_batch` function to orchestrate the parse-compact-send workflow.
- **Zero-Boilerplate HTTP Clients**: Built-in HTTP client implementations eliminate the need for custom trait implementations in your Lambda functions.
- **Optional Instrumentation**: Feature-gated support for request tracing and middleware integration.

## Core Components

### `TelemetryData`

(Located in `src/telemetry.rs`)

The central struct representing a unit of telemetry data. It normalizes incoming data into an OTLP protobuf format (uncompressed initially) and includes methods for final compression (Gzip). Its fields include `source`, `endpoint` (primarily for context, as the actual target is resolved from env vars), `payload`, `content_type`, and `content_encoding`.

### `EventParser` Trait

(Located in `src/core_parser.rs`)

A trait that specific Lambda processors must implement to convert their incoming AWS event payloads into a `Vec<TelemetryData>`.

```rust,no_run
use anyhow;
use serverless_otlp_forwarder_core::TelemetryData;

pub trait EventParser {
    type EventInput; // The specific AWS event type
    fn parse(&self, event_payload: Self::EventInput, source_identifier: &str) -> anyhow::Result<Vec<TelemetryData>>;
}
```

### Span Compaction

(Located in `src/span_compactor.rs`)

- `SpanCompactionConfig`: Configuration for enabling/disabling compaction, setting max payload size, and Gzip compression level.
- `compact_telemetry_payloads()`: Takes a `Vec<TelemetryData>` (expected to contain uncompressed OTLP protobuf payloads) and merges them into a single `TelemetryData` object, then applies Gzip compression according to the config.

### HTTP Sender

(Located in `src/http_sender.rs`)

- `resolve_otlp_endpoint()`: Determines the target OTLP HTTP endpoint by checking `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, then `OTEL_EXPORTER_OTLP_ENDPOINT`, and finally defaulting to `http://localhost:4318/v1/traces`. It correctly appends `/v1/traces` if a base URL is provided via `OTEL_EXPORTER_OTLP_ENDPOINT`.
- `resolve_otlp_headers()`: Parses custom HTTP headers from `OTEL_EXPORTER_OTLP_TRACES_HEADERS` or `OTEL_EXPORTER_OTLP_HEADERS` (comma-separated `key=value` format).
- `send_telemetry_batch()`: Asynchronously sends a (compacted and compressed) `TelemetryData` payload to the resolved endpoint using the resolved headers.

### HTTP Client Options

The crate provides multiple HTTP client options to minimize boilerplate in your Lambda implementations:

#### Simple ReqwestClient (Recommended for most use cases)
```rust,no_run
use reqwest::Client as ReqwestClient;
use std::sync::Arc;

// ReqwestClient implements HttpOtlpForwarderClient out of the box
let http_client = Arc::new(ReqwestClient::new());
```

#### Builder Functions
```rust,no_run
use serverless_otlp_forwarder_core::client_builder;
use std::sync::Arc;
use std::time::Duration;

// Simple client
let http_client = Arc::new(client_builder::simple());

// With custom timeout
let http_client = Arc::new(client_builder::with_timeout(Duration::from_secs(30)));
```

#### Instrumented Client (Feature: `instrumented-client`)
For Lambda functions that need request tracing and middleware support:

```rust,ignore
use reqwest::Client as ReqwestClient;
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use serverless_otlp_forwarder_core::InstrumentedHttpClient;
use std::sync::Arc;

// Create with custom middleware
let base_client = ReqwestClient::new();
let middleware_client = ClientBuilder::new(base_client)
    .with(TracingMiddleware::default())
    .build();
let http_client = Arc::new(InstrumentedHttpClient::new(middleware_client));

// Or use the builder (when instrumented-client feature is enabled)
// let http_client = Arc::new(client_builder::instrumented());
```

**Use Case**: The instrumented client is particularly useful when you want to instrument the forwarder's own HTTP requests to OTLP collectors. This aligns with the [OpenTelemetry Collector's internal telemetry capabilities](https://opentelemetry.io/docs/collector/internal-telemetry/#activate-internal-telemetry-in-the-collector), allowing you to observe the forwarder's performance, request patterns, and potential issues when sending data to collectors.

**Note**: OpenTelemetry's tracing instrumentation for collectors is still under active development and considered experimental. The instrumented client provides HTTP request tracing that can complement the collector's internal telemetry when debugging data flow issues or monitoring forwarder performance.

### `process_event_batch` Orchestrator

(Located in `src/processor.rs`)

The main generic function that orchestrates the telemetry processing pipeline:

1. Calls the provided `EventParser`'s `parse` method.
2. If telemetry items are produced, calls `compact_telemetry_payloads`.
3. Sends the resulting batch using `send_telemetry_batch`.

Handles errors at each step.

## Installation

This crate is intended to be used as a dependency by other Lambda functions implementing the Serverless OTLP Forwarder architecture. It can be added with the following command:

```bash
cargo add serverless-otlp-forwarder-core
```

### Optional Features

- **`instrumented-client`**: Enables the `InstrumentedHttpClient` for advanced middleware support
  ```toml
  [dependencies]
  serverless-otlp-forwarder-core = { version = "0.1.0", features = ["instrumented-client"] }
  ```

## Usage Example

Implementing a forwarder for AWS CloudWatch Logs containing `ExporterOutput` JSON from the `otlp-stdout-span-exporter` crate:

**1. Define your parser (`src/parser.rs` in your Lambda crate):**

```rust,no_run
use aws_lambda_events::cloudwatch_logs::LogsEvent;
use anyhow::Result;
use serverless_otlp_forwarder_core::{EventParser, TelemetryData};

pub struct MyLogsEventParser;

impl EventParser for MyLogsEventParser {
    type EventInput = LogsEvent;

    fn parse(&self, _event_payload: Self::EventInput, _source_identifier: &str) -> Result<Vec<TelemetryData>> {
        // Simplified example - in real usage you'd parse log events containing OTLP data
        let items = Vec::new();
        // Example parsing logic would go here:
        // for log_event in event_payload.aws_logs.data.log_events {
        //     // Parse JSON containing OTLP data and convert to TelemetryData
        // }
        Ok(items)
    }
}
```

**2. Use in your Lambda's `main.rs`:**

```rust,no_run
use anyhow::Result;
use lambda_runtime::{Error as LambdaError, LambdaEvent, Runtime, service_fn};
use reqwest::Client as ReqwestClient;
use serverless_otlp_forwarder_core::{
    process_event_batch, 
    SpanCompactionConfig,
    client_builder,
    EventParser,
    TelemetryData,
};
use std::sync::Arc;
use aws_lambda_events::cloudwatch_logs::LogsEvent;

// Define the parser inline instead of as a separate module
pub struct MyLogsEventParser;

impl EventParser for MyLogsEventParser {
    type EventInput = LogsEvent;

    fn parse(&self, _event_payload: Self::EventInput, _source_identifier: &str) -> Result<Vec<TelemetryData>> {
        // Simplified example - in real usage you'd parse the actual event
        Ok(vec![])
    }
} 

async fn function_handler(
    event: LambdaEvent<LogsEvent>, 
    http_client: Arc<ReqwestClient>,
) -> Result<(), LambdaError> {
    let log_group = event.payload.aws_logs.data.log_group.clone();
    let parser = MyLogsEventParser;
    let compaction_config = SpanCompactionConfig::default();

    process_event_batch(
        event.payload,
        &parser,
        &log_group,
        http_client.as_ref(),
        &compaction_config,
    )
    .await
    .map_err(|e| LambdaError::from(e.to_string()))?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    // Initialize telemetry (optional)
    // ... telemetry setup code ...
    
    // Create HTTP client - Zero boilerplate required!
    let http_client = Arc::new(client_builder::simple());
    
    // Alternative options:
    // let http_client = Arc::new(ReqwestClient::new());
    // let http_client = Arc::new(client_builder::with_timeout(Duration::from_secs(30)));
    
    // Initialize the Lambda runtime
    Runtime::new(service_fn(move |event: LambdaEvent<LogsEvent>| {
        let client = Arc::clone(&http_client);
        async move { function_handler(event, client).await }
    })).run().await
}
```

### Advanced Example with Instrumentation

For more advanced use cases requiring request tracing and observability of the forwarder itself:

```toml
# Cargo.toml
[dependencies]
serverless-otlp-forwarder-core = { version = "0.1.0", features = ["instrumented-client"] }
reqwest-middleware = "0.3"
reqwest-tracing = "0.5"
```

```rust,ignore
use reqwest::Client as ReqwestClient;
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use serverless_otlp_forwarder_core::InstrumentedHttpClient;
use lambda_runtime::Error as LambdaError;
use anyhow::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    // Create instrumented HTTP client to trace forwarder's own requests
    let base_client = ReqwestClient::new();
    let middleware_client = ClientBuilder::new(base_client)
        .with(TracingMiddleware::default())
        .build();
    let http_client = Arc::new(InstrumentedHttpClient::new(middleware_client));
    
    // Or use the builder function:
    // let http_client = Arc::new(client_builder::instrumented());
    
    // Rest of setup is identical...
    Ok(())
}
```

This setup enables distributed tracing of the forwarder's HTTP requests to OTLP collectors, which is valuable for:
- **Debugging data flow issues** between the forwarder and collectors
- **Monitoring forwarder performance** and request latency patterns  
- **Correlating forwarder behavior** with the [OpenTelemetry Collector's internal telemetry](https://opentelemetry.io/docs/collector/internal-telemetry/#activate-internal-telemetry-in-the-collector)
- **Identifying bottlenecks** in the telemetry pipeline

**Note**: Since collector tracing instrumentation is experimental, this instrumented client provides a stable way to observe the forwarder's side of the telemetry pipeline.

### Manual Implementation vs. Built-in HTTP Clients

**Manual/Custom Setup (Without this crate's built-in clients):**
```rust,ignore
use reqwest::{Client as ReqwestClient, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;
use async_trait::async_trait;
use anyhow::{Result, Context};
use url::Url;
use reqwest::header::HeaderMap;
use bytes::Bytes;
use std::time::Duration;
use std::sync::Arc;

// Custom wrapper type required
pub struct InstrumentedOtlpClient(ClientWithMiddleware);

// Manual trait implementation required (15+ lines per Lambda)
#[async_trait]
trait HttpOtlpForwarderClient {
    async fn post_telemetry(
        &self,
        target_url: Url,
        headers: HeaderMap,
        payload: Bytes,
        timeout: Duration,
    ) -> Result<Response>;
}

#[async_trait]
impl HttpOtlpForwarderClient for InstrumentedOtlpClient {
    async fn post_telemetry(
        &self,
        target_url: Url,
        headers: HeaderMap,
        payload: Bytes,
        timeout: Duration,
    ) -> Result<Response> {
        self.0
            .post(target_url)
            .headers(headers)
            .body(payload)
            .timeout(timeout)
            .send()
            .await
            .context("HTTP request failed via InstrumentedOtlpClient for OTLP export")
    }
}

// Complex setup in main()
let base_client = ReqwestClient::new();
let middleware_client = ClientBuilder::new(base_client)
    .with(TracingMiddleware::default())
    .build();
let http_client = Arc::new(InstrumentedOtlpClient(middleware_client));
```

**Built-in Options (Using this crate's provided clients):**
```rust,no_run
use reqwest::Client as ReqwestClient;
use serverless_otlp_forwarder_core::client_builder;
use std::sync::Arc;

// Option 1: Simple
let http_client = Arc::new(ReqwestClient::new());

// Option 2: Builder functions
let http_client = Arc::new(client_builder::simple());

// Option 3: Instrumented (with feature flag)
// let http_client = Arc::new(client_builder::instrumented());
```

## Environment Variables

The `http_sender` module within this crate respects the following standard OpenTelemetry environment variables for configuring the OTLP export endpoint and headers:

- `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`: The target URL for traces. If not set, `OTEL_EXPORTER_OTLP_ENDPOINT` is used. Defaults to `http://localhost:4318/v1/traces`.
- `OTEL_EXPORTER_OTLP_ENDPOINT`: A base URL for OTLP exports. `/v1/traces` will be appended if not present in the path.
- `OTEL_EXPORTER_OTLP_TRACES_HEADERS`: Custom headers for trace exports (e.g., `key1=value1,key2=value2`).
- `OTEL_EXPORTER_OTLP_HEADERS`: Custom general OTLP headers, used if trace-specific headers are not set.
- `OTEL_EXPORTER_OTLP_COMPRESSION`: Whether to compress the payload. Valid values are `gzip` or `none`. Defaults to `none`.
- `OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL`: The compression level for Gzip compression. Valid values are `0` to `9`. Please note that this is not part of the OTLP specification and is not supported by all OTLP exporters. We are adding support for it because it's useful in a be able to tune it in a constrained Lambda environment. Defaults to `9`.

## License

Licensed under the MIT License. See workspace root.