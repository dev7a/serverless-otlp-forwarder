use anyhow::Result;
use aws_credential_types::Credentials;
use aws_sigv4::http_request::{SignableBody, SignableRequest};
use aws_smithy_runtime_api::client::identity::Identity;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::env;
use std::str::FromStr;

pub fn sign_request(
    credentials: &Credentials,
    endpoint: &str,
    headers: &HeaderMap,
    payload: &[u8],
) -> Result<HeaderMap> {
    let identity: Identity = <Credentials as Into<Identity>>::into((*credentials).clone());
    
    let region = env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let signing_params = aws_sigv4::http_request::SigningParams::V4(
        aws_sigv4::sign::v4::SigningParams::builder()
            .identity(&identity)
            .region(region.as_str())
            .name("xray")
            .time(std::time::SystemTime::now())
            .settings(aws_sigv4::http_request::SigningSettings::default())
            .build()?
    );
    let headers: Vec<(String, String)> = headers.iter().map(|(k, v)| {
        (
            k.as_str().to_owned(),
            v.to_str().expect("Header value is None").to_owned()
        )
    }).collect();

    let signable_request = SignableRequest::new(
        "POST",
        endpoint,
        headers.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        SignableBody::Bytes(payload),
    )?;

    let (signing_instructions, _) = aws_sigv4::http_request::sign(
        signable_request,
        &signing_params,
    )?.into_parts();

    let (signed_headers, _) = signing_instructions.into_parts();
    let mut final_headers = HeaderMap::new();
    for header in signed_headers.into_iter() {
        final_headers.insert(
            HeaderName::from_str(header.name())?,
            HeaderValue::from_str(header.value())?
        );
    }
    Ok(final_headers)
}