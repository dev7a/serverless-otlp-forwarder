use super::*;
use aws_credential_types::Credentials;
use serde_json::json;
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};
use crate::collectors::test_utils::init_test_collectors;

fn create_test_telemetry(source: &str) -> TelemetryData {
    TelemetryData {
        source: source.to_string(),
        endpoint: "http://example.com/v1/traces".to_string(),
        payload: json!({"test": "data"}).to_string().into_bytes(),
        content_type: "application/json".to_string(),
        content_encoding: Some("gzip".to_string()),
    }
}

fn create_test_credentials() -> Credentials {
    Credentials::new(
        "test-key",
        "test-secret",
        None,
        None,
        "test-provider",
    )
}

async fn setup_test_collector() -> MockServer {
    let mock_server = MockServer::start().await;
    
    // Create a test collector
    let collector = Collector {
        name: "test".to_string(),
        endpoint: mock_server.uri(),
        auth: None,
        exclude: None,
    };

    // Initialize collectors with the test collector
    init_test_collectors(collector);

    mock_server
}

#[tokio::test]
async fn test_process_telemetry_batch_empty() {
    let _mock_server = setup_test_collector().await;
    let client = reqwest::Client::new();
    let creds = create_test_credentials();
    let records = vec![];

    let result = process_telemetry_batch(records, &client, &creds, "us-east-1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_telemetry_batch_single_success() {
    let mock_server = setup_test_collector().await;
    
    Mock::given(method("POST"))
        .and(path("/v1/traces"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let creds = create_test_credentials();
    let telemetry = create_test_telemetry("test-service");

    let records = vec![telemetry];
    let result = process_telemetry_batch(records, &client, &creds, "us-east-1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_telemetry_batch_all_fail() {
    let mock_server = setup_test_collector().await;
    
    Mock::given(method("POST"))
        .and(path("/v1/traces"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let creds = create_test_credentials();
    let telemetry = create_test_telemetry("test-service");

    let records = vec![telemetry];
    let result = process_telemetry_batch(records, &client, &creds, "us-east-1").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_telemetry_different_content_types() {
    let mock_server = MockServer::start().await;
    
    // Test JSON content type
    Mock::given(method("POST"))
        .and(path("/v1/traces"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Test Protobuf content type
    Mock::given(method("POST"))
        .and(path("/v1/traces-proto"))
        .and(header("content-type", "application/x-protobuf"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();

    // Test JSON
    let mut json_telemetry = create_test_telemetry("json-service");
    json_telemetry.endpoint = format!("{}/v1/traces", mock_server.uri());
    let result = send_telemetry(&client, &json_telemetry, HeaderMap::new()).await;
    assert!(result.is_ok());

    // Test Protobuf
    let mut proto_telemetry = create_test_telemetry("proto-service");
    proto_telemetry.endpoint = format!("{}/v1/traces-proto", mock_server.uri());
    proto_telemetry.content_type = "application/x-protobuf".to_string();
    let result = send_telemetry(&client, &proto_telemetry, HeaderMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_send_telemetry_with_compression() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/v1/traces"))
        .and(header("content-encoding", "gzip"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let mut telemetry = create_test_telemetry("test-service");
    telemetry.endpoint = format!("{}/v1/traces", mock_server.uri());

    let mut headers = HeaderMap::new();
    headers.insert("content-encoding", "gzip".parse().unwrap());

    let result = send_telemetry(&client, &telemetry, headers).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_send_telemetry_network_failure() {
    let client = reqwest::Client::new();
    let mut telemetry = create_test_telemetry("test-service");
    telemetry.endpoint = "http://non-existent-endpoint:12345".to_string();

    let result = send_telemetry(&client, &telemetry, HeaderMap::new()).await;
    assert!(result.is_err());
} 