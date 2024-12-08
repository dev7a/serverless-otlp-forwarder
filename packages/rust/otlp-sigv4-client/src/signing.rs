//! Request signing functionality for AWS SigV4

use aws_credential_types::Credentials;
use aws_sigv4::http_request::{SignableBody, SignableRequest};
use aws_smithy_runtime_api::client::identity::Identity;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::{error::Error, result::Result, str::FromStr};

/// Signs an HTTP request with AWS SigV4
///
/// # Arguments
///
/// * `credentials` - AWS credentials to use for signing
/// * `endpoint` - The endpoint URL
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

    let headers: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_owned(),
                v.to_str().expect("Header value is None").to_owned(),
            )
        })
        .collect();

    let signable_request = SignableRequest::new(
        "POST",
        endpoint,
        headers.iter().map(|(k, v)| (k.as_str(), v.as_str())),
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
