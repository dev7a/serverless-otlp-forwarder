use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use futures::future::join_all;
use opentelemetry::trace::SpanKind;
use reqwest::{header::HeaderMap, Client as ReqwestClient};
use tracing::{instrument, Instrument};

use crate::{collectors::Collectors, headers::LogRecordHeaders, telemetry::TelemetryData};

/// Sends telemetry data to a collector endpoint.
/// Includes OpenTelemetry instrumentation for request tracking.
#[instrument(skip_all, fields(
    otel.kind = ?SpanKind::Client,
    otel.status_code,
    http.method = "POST",
    http.url = %endpoint,
    http.request.headers.content_type,
    http.request.headers.content_encoding,
    http.status_code,
))]
pub async fn send_telemetry(
    client: &ReqwestClient,
    endpoint: &str,
    telemetry: &TelemetryData,
    headers: HeaderMap,
) -> Result<()> {
    let current_span = tracing::Span::current();

    // Record request headers for tracing
    headers.get("content-type").map(|ct| {
        current_span.record(
            "http.request.headers.content_type",
            ct.to_str().unwrap_or_default(),
        )
    });
    headers.get("content-encoding").map(|ce| {
        current_span.record(
            "http.request.headers.content_encoding",
            ce.to_str().unwrap_or_default(),
        )
    });

    // Log request details at debug level
    use base64::Engine;
    let base64_body = base64::engine::general_purpose::STANDARD.encode(&telemetry.payload);
    tracing::debug!(
        name = "sending telemetry request",
        headers = ?headers,
        body = %base64_body,
        "Request details"
    );
    // Send the request
    let response = client
        .post(endpoint)
        .headers(headers)
        .body(telemetry.payload.clone())
        .send()
        .await
        .context("Failed to send POST request")?;

    let status = response.status();

    // Record the HTTP status code
    current_span.record("http.status_code", status.as_u16());

    if !status.is_success() {
        current_span.record("otel.status_code", "ERROR");
        let error_body = match response.text().await {
            Ok(text) => text,
            Err(_) => "Could not read response body".to_string(),
        };
        tracing::warn!(
            name = "error posting telemetry data",
            endpoint = %telemetry.endpoint,
            status = status.as_u16(),
            status_text = %status.canonical_reason().unwrap_or("Unknown status"),
            error = %error_body,
        );

        return Err(anyhow::anyhow!(
            "Failed to send telemetry data. Status: {}, Error: {}",
            status,
            error_body
        ));
    }

    Ok(())
}

/// Process a batch of telemetry records in parallel
/// Each record is sent to all matching collectors
#[instrument(skip_all)]
pub async fn process_telemetry_batch(
    records: Vec<TelemetryData>,
    client: &ReqwestClient,
    credentials: &Credentials,
    region: &str,
) -> Result<()> {
    let tasks: Vec<_> = records
        .into_iter()
        .map(|telemetry| {
            // Clone what we need for the async block
            let client = client.clone();
            let credentials = credentials.clone();
            let region = region.to_string();
            let source = telemetry.source.clone();
            let span = tracing::info_span!("process_telemetry", source = %source);

            async move {
                // Get all collectors with proper signal paths
                let collectors =
                    Collectors::get_signal_endpoints(&telemetry.endpoint, &source).await?;

                // Create futures for sending to each collector
                let collector_tasks: Vec<_> = collectors
                    .into_iter()
                    .map(|collector| {
                        let client = client.clone();
                        let telemetry = &telemetry;
                        let collector_name = collector.name.clone();
                        let credentials = credentials.clone();
                        let region = region.clone();

                        async move {
                            tracing::info!(
                                "Preparing to send telemetry to collector {}",
                                collector_name
                            );

                            let headers = match LogRecordHeaders::default()
                                .with_telemetry(telemetry)
                                .and_then(|h| {
                                    h.with_collector_auth(
                                        &collector,
                                        &telemetry.payload,
                                        &credentials,
                                        &region,
                                    )
                                }) {
                                Ok(h) => h.build(),
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to build headers for collector {}: {}",
                                        collector_name,
                                        e
                                    );
                                    return Err(e);
                                }
                            };

                            tracing::info!(
                                "Sending telemetry to collector {} at endpoint {}",
                                collector_name,
                                collector.endpoint
                            );
                            if let Err(e) =
                                send_telemetry(&client, &collector.endpoint, telemetry, headers)
                                    .await
                            {
                                tracing::warn!(
                                    "Failed to send to collector {}: {}",
                                    collector_name,
                                    e
                                );
                                return Err(e);
                            }
                            tracing::info!(
                                "Successfully sent telemetry to collector {}",
                                collector_name
                            );
                            Ok(())
                        }
                    })
                    .collect();

                let results = join_all(collector_tasks).await;
                let results: Vec<Result<(), _>> = results.into_iter().collect();

                match results.iter().find(|r| r.is_ok()) {
                    Some(_) => Ok(()), // At least one success
                    None => {
                        // Get the last error, if any
                        let last_error = results
                            .into_iter()
                            .filter_map(|r| r.err())
                            .last()
                            .map(|e| format!("Last error: {}", e))
                            .unwrap_or_else(|| "No error details".to_string());

                        Err(anyhow::anyhow!("All collectors failed. {}", last_error))
                    }
                }
            }
            .instrument(span)
        })
        .map(|future| tokio::spawn(future))
        .collect();

    let results = join_all(tasks).await;
    let mut has_success = false;
    let mut errors = Vec::new();

    for result in results {
        match result {
            Ok(Ok(())) => {
                has_success = true;
            }
            Ok(Err(e)) => {
                tracing::warn!("Task error: {:?}", e);
                errors.push(e);
            }
            Err(e) => {
                tracing::warn!("Task panicked: {:?}", e);
                errors.push(anyhow::anyhow!("Task panicked: {}", e));
            }
        }
    }

    if has_success {
        Ok(())
    } else {
        let error_msg = errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Err(anyhow::anyhow!("All tasks failed: {}", error_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_credential_types::Credentials;
    use reqwest::header::{HeaderValue, CONTENT_ENCODING, CONTENT_TYPE};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn setup_test_client() -> ReqwestClient {
        ReqwestClient::builder()
            .build()
            .expect("Failed to create test client")
    }

    fn create_test_telemetry() -> TelemetryData {
        TelemetryData {
            source: "test-source".to_string(),
            endpoint: "http://example.com".to_string(),
            payload: b"test-payload".to_vec(),
            content_type: "application/json".to_string(),
            content_encoding: Some("gzip".to_string()),
        }
    }

    fn create_test_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        headers
    }

    #[tokio::test]
    async fn test_send_telemetry_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = setup_test_client();
        let mut telemetry = create_test_telemetry();
        telemetry.endpoint = mock_server.uri();

        let result = send_telemetry(
            &client,
            &telemetry.endpoint,
            &telemetry,
            create_test_headers(),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_telemetry_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let client = setup_test_client();
        let mut telemetry = create_test_telemetry();
        telemetry.endpoint = mock_server.uri();

        let result = send_telemetry(
            &client,
            &telemetry.endpoint,
            &telemetry,
            create_test_headers(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    #[tokio::test]
    async fn test_process_telemetry_batch() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/traces")) // Match the exact path
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // Initialize collector with the base URI
        use crate::collectors::{test_utils, Collector};

        let collector = Collector {
            name: "test-collector".to_string(),
            endpoint: mock_server.uri(),
            auth: None,
            exclude: None,
        };
        test_utils::init_test_collectors(collector);

        let client = setup_test_client();
        let mut telemetry = create_test_telemetry();
        telemetry.endpoint = format!("{}/v1/traces", mock_server.uri()); // Add signal path

        let records = vec![telemetry];
        let credentials = Credentials::new("test-key", "test-secret", None, None, "test-provider");

        let result = process_telemetry_batch(records, &client, &credentials, "us-west-2").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_telemetry_batch_partial_failure() {
        let mock_server = MockServer::start().await;

        // Success endpoint
        Mock::given(method("POST"))
            .and(path("/v1/traces")) // Match the base path
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // Initialize collectors for both endpoints
        use crate::collectors::{test_utils, Collector};

        let collector = Collector {
            name: "test-collector".to_string(),
            endpoint: mock_server.uri(), // Use base URI
            auth: None,
            exclude: None,
        };
        test_utils::init_test_collectors(collector);

        let client = setup_test_client();

        let mut success_telemetry = create_test_telemetry();
        success_telemetry.endpoint = format!("{}/v1/traces", mock_server.uri());

        let mut failure_telemetry = create_test_telemetry();
        failure_telemetry.endpoint = format!("{}/v1/traces", mock_server.uri());

        let records = vec![success_telemetry, failure_telemetry];
        let credentials = Credentials::new("test-key", "test-secret", None, None, "test-provider");

        let result = process_telemetry_batch(records, &client, &credentials, "us-west-2").await;

        assert!(result.is_ok());
    }
}
