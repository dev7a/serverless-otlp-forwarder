use super::*;
use base64::engine::general_purpose;
use flate2::read::GzDecoder;
use futures::FutureExt;
use opentelemetry::trace::Tracer;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use sealed_test::prelude::*;
use serde_json::Value;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{
    io::{Cursor, Read},
    sync::Arc,
    time::Duration,
};
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;

/// A thread-safe writer for testing that captures output in a buffer
#[derive(Clone)]
struct TestWriter {
    buffer: Arc<Mutex<Cursor<Vec<u8>>>>,
}

impl TestWriter {
    fn new() -> Self {
        TestWriter {
            buffer: Arc::new(Mutex::new(Cursor::new(Vec::new()))),
        }
    }

    async fn get_content(&self) -> String {
        let mut buffer = self.buffer.lock().await;
        buffer.set_position(0);
        let mut content = String::new();
        buffer
            .read_to_string(&mut content)
            .expect("Failed to read buffer");
        content
    }
}

// Instead of implementing Write, implement AsyncWrite
impl AsyncWrite for TestWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let lock_future = self.buffer.lock();
        futures::pin_mut!(lock_future);
        let mut guard = futures::ready!(lock_future.poll_unpin(cx));
        Poll::Ready(std::io::Write::write(&mut *guard, buf))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let lock_future = self.buffer.lock();
        futures::pin_mut!(lock_future);
        let mut guard = futures::ready!(lock_future.poll_unpin(cx));
        Poll::Ready(std::io::Write::flush(&mut *guard))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Runs a tracer test and captures its output as a `String`.
///
/// # Returns
///
/// A `String` containing the captured output from the tracer.
async fn run_tracer_test() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Determine the protocol based on the OTEL_EXPORTER_OTLP_PROTOCOL environment variable
    let protocol = match std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "http/protobuf" => Protocol::HttpBinary,
        "http/json" | "" => Protocol::HttpJson,
        unsupported => {
            eprintln!(
                "Warning: OTEL_EXPORTER_OTLP_PROTOCOL value '{}' is not supported. Defaulting to HTTP JSON.",
                unsupported
            );
            Protocol::HttpJson
        }
    };

    // Initialize the TestWriter
    let test_writer = TestWriter::new();
    let client = StdoutClient::new_with_writer(test_writer.clone());

    // Set up the OTLP exporter with the StdoutClient using the TestWriter
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(protocol)
        .with_http_client(client)
        .build()?;

    // Create a tracer provider and set it as the global provider
    let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();

    opentelemetry::global::set_tracer_provider(tracer_provider);
    let tracer = opentelemetry::global::tracer("my_tracer");

    // Create a sample span
    tracer.in_span("example_span", |_cx| {
        std::thread::sleep(Duration::from_millis(100));
    });

    // Shut down the tracer provider to ensure all spans are flushed
    opentelemetry::global::shutdown_tracer_provider();

    // Add a small delay to ensure all data is written
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Retrieve and convert the captured output
    let content = test_writer.get_content().await;

    Ok(content)
}

/// Verifies the captured output of the tracer test.
fn verify_output(
    captured_output: String,
    expected_content_type: &str,
    expected_content_encoding: Option<&str>,
) {
    if captured_output.is_empty() {
        panic!("Captured output is empty");
    }

    let json_output: Value =
        serde_json::from_str(&captured_output).expect("Failed to parse JSON output");

    // Assert on specific fields
    assert_eq!(
        json_output["content-type"], expected_content_type,
        "Content-Type mismatch"
    );
    assert_eq!(json_output["method"], "POST", "HTTP method mismatch");
    assert!(
        json_output["__otel_otlp_stdout"]
            .as_str()
            .unwrap()
            .contains("otlp-stdout-client@"),
        "OTEL version format mismatch"
    );
    assert_eq!(
        json_output["endpoint"].as_str().unwrap(),
        "http://example.com/v1/traces",
        "Endpoint mismatch"
    );

    // Check if payload exists
    assert!(json_output.get("payload").is_some(), "Payload is missing");

    // Check content encoding
    match expected_content_encoding {
        Some(encoding) => {
            assert_eq!(
                json_output["content-encoding"].as_str(),
                Some(encoding),
                "Content-Encoding mismatch"
            );
        }
        None => {
            assert!(
                json_output.get("content-encoding").is_none(),
                "Content-Encoding should not be present"
            );
        }
    }

    // Check payload based on content type and compression
    match expected_content_type {
        "application/json" => {
            if expected_content_encoding == Some("gzip") {
                assert!(
                    json_output["payload"].is_string(),
                    "Compressed JSON payload should be a string"
                );
                assert!(
                    json_output["base64"].as_bool().unwrap_or(false),
                    "base64 flag should be true for compressed JSON"
                );

                // Decode and decompress the payload
                let decoded = general_purpose::STANDARD
                    .decode(json_output["payload"].as_str().unwrap())
                    .expect("Failed to decode base64");
                let mut decoder = GzDecoder::new(&decoded[..]);
                let mut decompressed = String::new();
                decoder
                    .read_to_string(&mut decompressed)
                    .expect("Failed to decompress");

                // Parse the decompressed JSON
                let decompressed_json: Value =
                    serde_json::from_str(&decompressed).expect("Failed to parse decompressed JSON");

                // Verify that the decompressed payload is a valid JSON object
                assert!(
                    decompressed_json.is_object(),
                    "Decompressed JSON payload should be an object"
                );
            } else {
                assert!(
                    json_output["payload"].is_object(),
                    "Uncompressed JSON payload should be an object"
                );
            }
        }
        "application/x-protobuf" => {
            assert!(
                json_output["payload"].is_string(),
                "Protobuf payload should be a string"
            );
            assert!(
                json_output["base64"].as_bool().unwrap_or(false),
                "base64 flag should be true for Protobuf"
            );
        }
        _ => panic!("Unexpected content type"),
    }

    // Check for essential headers
    assert!(
        json_output["headers"].get("content-type").is_some(),
        "Content-Type header is missing"
    );
}

#[tokio::test]
#[sealed_test(env = [
    ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://example.com/"),
    ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/json"),
])]
async fn test_stdout_client_send_json_plain() {
    let captured_output = run_tracer_test().await.unwrap();
    verify_output(captured_output, "application/json", None);
}
#[tokio::test]
#[sealed_test(env = [
    ("OTEL_EXPORTER_OTLP_COMPRESSION", "gzip"),
    ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://example.com/"),
    ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/json"),
])]
async fn test_stdout_client_send_json_gzip() {
    let captured_output = run_tracer_test().await.unwrap();
    verify_output(captured_output, "application/json", Some("gzip"));
}
#[tokio::test]
#[sealed_test(env = [
    ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://example.com/"),
    ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf"),
])]
async fn test_stdout_client_send_proto_plain() {
    let captured_output = run_tracer_test().await.unwrap();
    verify_output(captured_output, "application/x-protobuf", None);
}

#[tokio::test]
#[sealed_test(env = [
    ("OTEL_EXPORTER_OTLP_COMPRESSION", "gzip"),
    ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://example.com/"),
    ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf"),
])]
async fn test_stdout_client_send_proto_gzip() {
    let captured_output = run_tracer_test().await.unwrap();
    verify_output(captured_output, "application/x-protobuf", Some("gzip"));
}
