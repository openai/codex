use std::time::Duration;

use codex_aws_auth::AwsAccessKeys;
use http::HeaderValue;
use http::Method;
use pretty_assertions::assert_eq;

use super::*;

async fn test_provider() -> AwsSigV4AuthProvider {
    let context = AwsAuthContext::load_with_access_keys(
        AwsAuthConfig {
            profile: None,
            region: Some("us-east-1".to_string()),
            service: "execute-api".to_string(),
        },
        AwsAccessKeys {
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
            session_token: Some("test-session-token".to_string()),
        },
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await
    .expect("load fixture signing context");
    AwsSigV4AuthProvider { context }
}

#[tokio::test]
async fn signs_requests_without_changing_payload_or_metadata() {
    let provider = test_provider().await;
    let url = "https://executor.example.com/connect?environment_id=environment-1";
    for mut request in [
        Request::new(Method::GET, url.to_string()),
        Request::new(Method::POST, url.to_string())
            .with_json(&serde_json::json!({"transport": "direct_jsonrpc_v1"})),
        Request::new(Method::POST, url.to_string())
            .with_json(&serde_json::json!({"transport": "direct_jsonrpc_v1"}))
            .with_compression(RequestCompression::Zstd),
    ] {
        request
            .headers
            .insert("session_id", HeaderValue::from_static("session-1"));
        request
            .headers
            .insert("x-custom-header", HeaderValue::from_static("preserved"));
        request.timeout = Some(Duration::from_secs(3));
        let method = request.method.clone();
        let expected = request
            .prepare_body_for_send()
            .expect("prepare fixture body");
        let signed = provider
            .apply_auth(request)
            .await
            .expect("sign fixture request");

        assert_eq!(signed.method, method);
        assert_eq!(signed.url, url);
        assert_eq!(signed.timeout, Some(Duration::from_secs(3)));
        assert_eq!(signed.body, expected.body.clone().map(RequestBody::Raw));
        assert_eq!(signed.compression, RequestCompression::None);
        for (name, value) in &expected.headers {
            assert_eq!(signed.headers.get(name), Some(value));
        }
        assert_eq!(signed.headers["x-amz-security-token"], "test-session-token");
        assert!(signed.headers.contains_key("x-amz-date"));
        let authorization = signed.headers[http::header::AUTHORIZATION]
            .to_str()
            .unwrap();
        assert!(authorization.starts_with("AWS4-HMAC-SHA256 "));
        assert!(authorization.contains("/us-east-1/execute-api/aws4_request"));
        assert_eq!(signed.prepare_body_for_send().unwrap().body, expected.body);
    }
}

#[tokio::test]
async fn invalid_signing_request_is_a_permanent_auth_error() {
    let provider = test_provider().await;
    let error = provider
        .apply_auth(Request::new(Method::GET, "not a URL".to_string()))
        .await
        .expect_err("invalid URL should fail signing");
    assert!(matches!(error, AuthError::Build(_)));
}

#[tokio::test]
async fn invalid_signing_configuration_is_rejected() {
    let config = AwsAuthConfig {
        profile: None,
        region: Some("us-east-1".to_string()),
        service: " ".to_string(),
    };
    let error = aws_sigv4_auth_provider(
        config,
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await
    .err()
    .expect("empty service should fail configuration");
    assert!(matches!(error, AwsAuthError::EmptyService));
}
