//! Exercises OAuth operations over HTTP, including grant binding and rejection diagnostics.

use std::collections::BTreeMap;

use base64::Engine;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

use super::*;
use crate::oauth::AuthorizationRequest;
use crate::oauth::build_authorization_url;
use crate::oauth::generate_pkce;
use crate::oauth::generate_state;

fn oauth<'a>(http: &'a HttpClient, url: &'a str, limit: ErrorBodyLimit) -> OAuthClient<'a> {
    OAuthClient::new(
        http,
        TokenEndpoint {
            url,
            client_id: "client id",
            encoding: TokenEncoding::Form,
            timeout: None,
            error_body_limit: limit,
        },
    )
}

#[tokio::test]
async fn authorization_code_preserves_pkce_and_http_policy() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"access_token": "access"})),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let mut headers = HeaderMap::new();
    headers.insert("x-client-policy", HeaderValue::from_static("preserved"));
    let http = HttpClientBuilder::new()
        .default_headers(headers)
        .build_direct()
        .unwrap();
    let endpoint = server.uri();
    let form = oauth(&http, &endpoint, ErrorBodyLimit::Unlimited);
    let pkce = generate_pkce();
    let state = generate_state();
    let resource = "https://gateway.example.test/a?b=c&d=e";
    let redirect_uri = "http://127.0.0.1:1234/callback";
    let authorization_url = build_authorization_url(AuthorizationRequest {
        endpoint: &format!("{endpoint}/authorize?tenant=existing"),
        client_id: "client id",
        redirect_uri,
        scope: Some("openid gateway.inference"),
        resource: Some(resource),
        pkce: &pkce,
        state: &state,
        extra_parameters: &[("prompt", "login")],
    })
    .unwrap();
    let tokens: Value = form
        .exchange_code(AuthorizationCodeGrant {
            code: "code+with&reserved=characters",
            redirect_uri,
            pkce: &pkce,
            resource: Some(resource),
        })
        .await
        .unwrap();
    assert_eq!(tokens, json!({"access_token": "access"}));
    let requests = server.received_requests().await.unwrap();
    let code_parameters = decode_form(&requests[0].body);
    assert_eq!(
        requests[0].headers["content-type"],
        "application/x-www-form-urlencoded"
    );
    assert_eq!(requests[0].headers["x-client-policy"], "preserved");
    assert_eq!(
        code_parameters,
        json!({
            "grant_type": "authorization_code",
            "client_id": "client id",
            "code": "code+with&reserved=characters",
            "redirect_uri": redirect_uri,
            "code_verifier": pkce.code_verifier,
            "resource": resource,
        })
    );
    // Bind the browser challenge to the verifier actually sent in the code exchange.
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(
        code_parameters["code_verifier"]
            .as_str()
            .unwrap()
            .as_bytes(),
    ));
    assert_eq!(
        decode_form(authorization_url.query().unwrap().as_bytes()),
        json!({
            "response_type": "code",
            "client_id": "client id",
            "redirect_uri": redirect_uri,
            "scope": "openid gateway.inference",
            "resource": resource,
            "state": state,
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            "tenant": "existing",
            "prompt": "login",
        })
    );
}

#[tokio::test]
async fn rejection_preserves_status_and_omits_oversized_diagnostics() {
    let http = HttpClientBuilder::new().build_direct().unwrap();
    const BODY_LIMIT: usize = 8192;
    let refresh_token = "credential+crossing&the=limit /%";
    let form_encoded: String =
        url::form_urlencoded::byte_serialize(refresh_token.as_bytes()).collect();
    let long_body = format!("{}{form_encoded}", "x".repeat(BODY_LIMIT - 2));
    let redacted = long_body.replace(&form_encoded, "[REDACTED]");
    let top_level_error = json!({
        "code": "configuration_error",
        "message": format!("Unknown tenant: {form_encoded}"),
    })
    .to_string();
    let redacted_top_level_error = top_level_error.replace(&form_encoded, "[REDACTED]");
    for (limit, body, expected_display) in [
        (
            ErrorBodyLimit::Bytes(BODY_LIMIT),
            json!({"error": "invalid_grant", "error_description": form_encoded}).to_string(),
            "[REDACTED]",
        ),
        (
            ErrorBodyLimit::Unlimited,
            top_level_error,
            redacted_top_level_error.as_str(),
        ),
        (
            ErrorBodyLimit::Bytes(BODY_LIMIT),
            long_body.clone(),
            "unknown error",
        ),
        (
            ErrorBodyLimit::Unlimited,
            long_body.clone(),
            redacted.as_str(),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(move |request: &wiremock::Request| {
                decode_form(&request.body)
                    == json!({
                        "client_id": "client id", "grant_type": "refresh_token",
                        "refresh_token": refresh_token, "resource": "urn:gateway",
                    })
            })
            .respond_with(
                ResponseTemplate::new(/*s*/ 401)
                    .insert_header("x-request-id", format!("request-{form_encoded}"))
                    .set_body_string(body),
            )
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let endpoint = server.uri();
        let result = oauth(&http, &endpoint, limit)
            .refresh::<Value>(RefreshTokenGrant {
                refresh_token,
                resource: Some("urn:gateway"),
            })
            .await;
        let Err(OAuthError::Rejected(error)) = result else {
            panic!("expected OAuth rejection")
        };
        assert_eq!(
            (
                error.status,
                error.request_id.as_deref(),
                error.detail.to_string(),
                error.body_read_error.is_none()
            ),
            (
                StatusCode::UNAUTHORIZED,
                Some("request-[REDACTED]"),
                expected_display.to_string(),
                true
            )
        );
    }
}

#[tokio::test]
async fn rejection_keeps_status_when_reading_the_body_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"refresh_token=refresh") {
            let mut buffer = [0; 4096];
            let read = stream.read(&mut buffer).await.unwrap();
            assert!(read > 0);
            request.extend_from_slice(&buffer[..read]);
        }
        stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial").await.unwrap();
    });
    let http = HttpClientBuilder::new().build_direct().unwrap();
    let endpoint = format!("http://{address}");
    let result = oauth(&http, &endpoint, ErrorBodyLimit::Unlimited)
        .refresh::<Value>(RefreshTokenGrant {
            refresh_token: "refresh",
            resource: None,
        })
        .await;
    let Err(OAuthError::Rejected(error)) = result else {
        panic!("expected OAuth rejection")
    };
    assert_eq!(error.status, StatusCode::UNAUTHORIZED);
    assert!(error.body_read_error.is_some());
    server.await.unwrap();
}

fn decode_form(body: &[u8]) -> Value {
    serde_json::to_value(
        url::form_urlencoded::parse(body)
            .into_owned()
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}
