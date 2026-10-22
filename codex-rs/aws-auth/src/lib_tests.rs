#![cfg(test)]

//! Unit tests for the crate and its related test suites.

use std::time::Duration;
use std::time::UNIX_EPOCH;

use aws_credential_types::Credentials;
use aws_credential_types::provider::error::CredentialsError;
use pretty_assertions::assert_eq;

use super::*;

fn test_context(session_token: Option<&str>) -> AwsAuthContext {
    AwsAuthContext {
        network_policy: Default::default(),
        credentials_provider: SharedCredentialsProvider::new(Credentials::new(
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            session_token.map(str::to_string),
            /*expires_after*/ None,
            "unit-test",
        )),
        region: "us-east-1".to_string(),
        service: "bedrock".to_string(),
    }
}

pub(super) fn test_request() -> AwsRequestToSign {
    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    headers.insert("x-test-header", http::HeaderValue::from_static("present"));
    AwsRequestToSign {
        method: Method::POST,
        url: "https://bedrock-runtime.us-east-1.amazonaws.com/v1/responses".to_string(),
        headers,
        body: Bytes::from_static(br#"{"model":"openai.gpt-oss-120b-1:0"}"#),
    }
}

#[tokio::test]
async fn sign_adds_sigv4_headers_and_preserves_existing_headers() {
    let signed = test_context(/*session_token*/ None)
        .sign_at(
            test_request(),
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .await
        .expect("request should sign");

    assert_eq!(
        signing::header_value(&signed.headers, http::header::CONTENT_TYPE.as_str()),
        Some("application/json".to_string())
    );
    assert_eq!(
        signing::header_value(&signed.headers, "x-test-header"),
        Some("present".to_string())
    );
    assert_eq!(
        signed.url,
        "https://bedrock-runtime.us-east-1.amazonaws.com/v1/responses"
    );
    assert!(
        signing::header_value(&signed.headers, http::header::AUTHORIZATION.as_str())
            .is_some_and(|value| value.starts_with("AWS4-HMAC-SHA256 "))
    );
    assert!(signing::header_value(&signed.headers, "x-amz-date").is_some());
}

#[tokio::test]
async fn credentials_provider_adapter_converts_keys_and_provider_failures() {
    #[derive(Debug)]
    struct TestCredentialsProvider(Result<AwsAccessKeys, std::io::ErrorKind>);

    impl AwsCredentialsProvider for TestCredentialsProvider {
        async fn credentials(&self) -> std::io::Result<AwsAccessKeys> {
            self.0
                .clone()
                .map_err(|kind| std::io::Error::new(kind, "credential export failed"))
        }
    }

    let credentials = AwsCredentialsProviderAdapter(
        Arc::new(TestCredentialsProvider(Ok(AwsAccessKeys {
            access_key_id: "access-key-id".to_string(),
            secret_access_key: "secret-access-key".to_string(),
            session_token: Some("session-token".to_string()),
        }))),
        Default::default(),
    )
    .provide_credentials()
    .await
    .expect("exported credentials should be available");

    assert_eq!(
        credentials,
        Credentials::new(
            "access-key-id",
            "secret-access-key",
            Some("session-token".to_string()),
            /*expires_after*/ None,
            "codex-bedrock-credential-export",
        )
    );

    for (kind, retryable) in [
        (std::io::ErrorKind::Other, true),
        (std::io::ErrorKind::TimedOut, true),
        (std::io::ErrorKind::InvalidData, false),
        (std::io::ErrorKind::InvalidInput, false),
        (std::io::ErrorKind::NotFound, false),
        (std::io::ErrorKind::PermissionDenied, false),
    ] {
        let error = AwsCredentialsProviderAdapter(
            Arc::new(TestCredentialsProvider(Err(kind))),
            Default::default(),
        )
        .provide_credentials()
        .await
        .expect_err("credential export failure should be propagated");
        let error = AwsAuthError::Credentials(error);
        assert_eq!(
            (
                error.is_retryable(),
                error
                    .credentials_provider_error()
                    .map(|error| (error.kind(), error.to_string())),
            ),
            (
                retryable,
                Some((kind, "credential export failed".to_string()))
            ),
        );
    }
}

#[test]
fn credentials_provider_failures_are_retryable() {
    assert!(
        AwsAuthError::Credentials(CredentialsError::provider_error("temporarily unavailable"))
            .is_retryable()
    );
    assert!(
        AwsAuthError::Credentials(CredentialsError::provider_timed_out(Duration::from_secs(1)))
            .is_retryable()
    );
}

#[test]
fn deterministic_aws_auth_errors_are_not_retryable() {
    assert!(!AwsAuthError::EmptyService.is_retryable());
    assert!(!AwsAuthError::MissingProfile.is_retryable());
    assert!(!AwsAuthError::Credentials(CredentialsError::not_loaded_no_source()).is_retryable());
    assert!(
        !AwsAuthError::Credentials(CredentialsError::invalid_configuration("bad profile"))
            .is_retryable()
    );
    assert!(
        !AwsAuthError::Credentials(CredentialsError::unhandled("unexpected response"))
            .is_retryable()
    );
}

#[tokio::test]
async fn sign_includes_session_token_when_credentials_have_one() {
    let signed = test_context(Some("session-token"))
        .sign_at(
            test_request(),
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )
        .await
        .expect("request should sign");

    assert_eq!(
        signing::header_value(&signed.headers, "x-amz-security-token"),
        Some("session-token".to_string())
    );
}

#[tokio::test]
async fn load_rejects_invalid_configuration() {
    let config = AwsAuthConfig {
        profile: None,
        region: None,
        service: "   ".to_string(),
    };
    let err = AwsAuthContext::load(
        config,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await
    .expect_err("empty service should be rejected");

    assert_eq!(err.to_string(), "AWS service name must not be empty");

    let config = AwsAuthConfig {
        profile: None,
        region: Some("us-east-1".to_string()),
        service: "bedrock".to_string(),
    };
    let err = AwsAuthContext::load_profile(
        config,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await
    .expect_err("profile auth should require a configured profile");

    assert_eq!(err.to_string(), "AWS profile must be configured");
}

#[path = "network_policy_tests.rs"]
mod network_policy_tests;
