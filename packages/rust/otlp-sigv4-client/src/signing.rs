//! Request signing functionality for AWS SigV4

use aws_credential_types::Credentials;
use aws_sigv4::http_request::{SignableBody, SignableRequest};
use aws_smithy_runtime_api::client::identity::Identity;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use std::{error::Error, result::Result, str::FromStr};

/// Signs an HTTP request with AWS SigV4
///
/// # Arguments
///
/// * `credentials` - AWS credentials to use for signing
/// * `endpoint` - The endpoint URL
/// * `method` - The HTTP method (GET, POST, etc.)
/// * `headers` - The request headers
/// * `payload` - The request body
/// * `region` - AWS region
/// * `service` - AWS service name
///
/// # Returns
///
/// Returns a `HeaderMap` containing the signed headers
pub fn sign_request(
    credentials: &Credentials,
    endpoint: &str,
    method: &str,
    headers: &HeaderMap,
    payload: &[u8],
    region: &str,
    service: &str,
) -> Result<HeaderMap, Box<dyn Error + Send + Sync>> {
    let identity: Identity = <Credentials as Into<Identity>>::into((*credentials).clone());

    let signing_params = aws_sigv4::http_request::SigningParams::V4(
        aws_sigv4::sign::v4::SigningParams::builder()
            .identity(&identity)
            .region(region)
            .name(service)
            .time(std::time::SystemTime::now())
            .settings(aws_sigv4::http_request::SigningSettings::default())
            .build()?,
    );

    let mut header_pairs = Vec::new();
    for (k, v) in headers.iter() {
        let value = match v.to_str() {
            Ok(val) => val,
            Err(e) => return Err(Box::new(e)),
        };
        header_pairs.push((k.as_str().to_owned(), value.to_owned()));
    }

    let signable_request = SignableRequest::new(
        method,
        endpoint,
        header_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        SignableBody::Bytes(payload),
    )?;

    let (signing_instructions, _) =
        aws_sigv4::http_request::sign(signable_request, &signing_params)?.into_parts();

    let (signed_headers, _) = signing_instructions.into_parts();
    let mut final_headers = HeaderMap::new();
    for header in signed_headers.into_iter() {
        final_headers.insert(
            HeaderName::from_str(header.name())?,
            HeaderValue::from_str(header.value())?,
        );
    }
    Ok(final_headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_credential_types::Credentials;
    use http::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_sign_request_basic() {
        let credentials = Credentials::new(
            "test_access_key",
            "test_secret_key",
            None,
            None,
            "test_provider",
        );

        let endpoint = "https://xray.us-east-1.amazonaws.com/";
        let method = "POST";
        let headers = HeaderMap::new();
        let payload = b"test payload";
        let region = "us-east-1";
        let service = "xray";

        let result = sign_request(
            &credentials,
            endpoint,
            method,
            &headers,
            payload,
            region,
            service,
        );

        assert!(result.is_ok());
        let signed_headers = result.unwrap();

        // Verify AWS SigV4 headers are present
        assert!(signed_headers.contains_key("x-amz-date"));
        assert!(signed_headers.contains_key("authorization"));
        assert!(signed_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("AWS4-HMAC-SHA256"));
        assert!(signed_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("Credential=test_access_key"));
        assert!(signed_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("SignedHeaders="));
    }

    #[test]
    fn test_sign_request_with_different_http_methods() {
        let credentials = Credentials::new(
            "test_access_key",
            "test_secret_key",
            None,
            None,
            "test_provider",
        );

        let endpoint = "https://xray.us-east-1.amazonaws.com/";
        let headers = HeaderMap::new();
        let payload = b"";
        let region = "us-east-1";
        let service = "xray";

        // Test with GET method
        let get_result = sign_request(
            &credentials,
            endpoint,
            "GET",
            &headers,
            payload,
            region,
            service,
        );

        assert!(get_result.is_ok());
        let get_headers = get_result.unwrap();
        assert!(get_headers.contains_key("authorization"));
        assert!(get_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("AWS4-HMAC-SHA256"));

        // Test with PUT method
        let put_result = sign_request(
            &credentials,
            endpoint,
            "PUT",
            &headers,
            payload,
            region,
            service,
        );

        assert!(put_result.is_ok());
        let put_headers = put_result.unwrap();
        assert!(put_headers.contains_key("authorization"));
        assert!(put_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("AWS4-HMAC-SHA256"));
    }

    #[test]
    fn test_sign_request_preserves_custom_headers() {
        let credentials = Credentials::new(
            "test_access_key",
            "test_secret_key",
            None,
            None,
            "test_provider",
        );

        let endpoint = "https://xray.us-east-1.amazonaws.com/";
        let method = "POST";

        // Create custom headers
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert("x-custom-header", HeaderValue::from_static("custom-value"));

        let payload = b"{\"key\":\"value\"}";
        let region = "us-east-1";
        let service = "xray";

        let result = sign_request(
            &credentials,
            endpoint,
            method,
            &headers,
            payload,
            region,
            service,
        );

        assert!(result.is_ok());
        let signed_headers = result.unwrap();

        // AWS SigV4 headers should be present
        assert!(signed_headers.contains_key("x-amz-date"));
        assert!(signed_headers.contains_key("authorization"));

        // Original custom headers should be present in the signed headers list
        assert!(signed_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("content-type"));
        assert!(signed_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("x-custom-header"));
    }

    #[test]
    fn test_sign_request_with_different_services() {
        let credentials = Credentials::new(
            "test_access_key",
            "test_secret_key",
            None,
            None,
            "test_provider",
        );

        let endpoint = "https://xray.us-east-1.amazonaws.com/";
        let method = "POST";
        let headers = HeaderMap::new();
        let payload = b"";
        let region = "us-east-1";

        // Test with xray service
        let xray_result = sign_request(
            &credentials,
            endpoint,
            method,
            &headers,
            payload,
            region,
            "xray",
        );

        assert!(xray_result.is_ok());
        let xray_headers = xray_result.unwrap();
        assert!(xray_headers["authorization"]
            .to_str()
            .unwrap()
            .contains("/xray/"));
    }

    #[test]
    fn test_sign_request_error_handling_invalid_headers() {
        let credentials = Credentials::new(
            "test_access_key",
            "test_secret_key",
            None,
            None,
            "test_provider",
        );

        let endpoint = "https://xray.us-east-1.amazonaws.com/";
        let method = "POST";

        // Create headers with invalid values (non-ASCII characters)
        let mut headers = HeaderMap::new();
        let invalid_value = HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap();
        headers.insert("x-invalid-header", invalid_value);

        let payload = b"";
        let region = "us-east-1";
        let service = "xray";

        let result = sign_request(
            &credentials,
            endpoint,
            method,
            &headers,
            payload,
            region,
            service,
        );

        // Should return an error for an invalid header value
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_request_with_empty_credentials() {
        let credentials = Credentials::new(
            "", // Empty access key
            "", // Empty secret key
            None,
            None,
            "test_provider",
        );

        let endpoint = "https://xray.us-east-1.amazonaws.com/";
        let method = "POST";
        let headers = HeaderMap::new();
        let payload = b"";
        let region = "us-east-1";
        let service = "xray";

        let result = sign_request(
            &credentials,
            endpoint,
            method,
            &headers,
            payload,
            region,
            service,
        );

        // Even with empty credentials, the signing process should complete
        // (the signature will be invalid, but the process doesn't fail)
        assert!(result.is_ok());
        let signed_headers = result.unwrap();
        assert!(signed_headers.contains_key("authorization"));
    }
}
