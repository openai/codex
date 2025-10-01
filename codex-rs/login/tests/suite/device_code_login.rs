#![allow(clippy::unwrap_used)]

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_core::auth::get_auth_file;
use codex_core::auth::try_read_auth_json;
use codex_login::ServerOptions;
use codex_login::run_device_code_login;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use core_test_support::skip_if_no_network;

// ---------- Small helpers  ----------

fn make_jwt(payload: serde_json::Value) -> String {
    let header = json!({ "alg": "none", "typ": "JWT" });
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    let signature_b64 = URL_SAFE_NO_PAD.encode(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}

async fn mock_usercode_success(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/usercode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_auth_id": "device-auth-123",
            "user_code": "CODE-12345",
            // NOTE: Interval is kept 0 in order to avoid waiting for the interval to pass
            "interval": "0"
        })))
        .mount(server)
        .await;
}

async fn mock_usercode_failure(server: &MockServer, status: u16) {
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/usercode"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn mock_poll_token_two_step(
    server: &MockServer,
    counter: Arc<AtomicUsize>,
    first_response_status: u16,
) {
    let c = counter.clone();
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/token"))
        .respond_with(move |_: &Request| {
            let attempt = c.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                ResponseTemplate::new(first_response_status)
            } else {
                ResponseTemplate::new(200).set_body_json(json!({
                    "authorization_code": "poll-code-321",
                    "code_challenge": "code-challenge-321",
                    "code_verifier": "code-verifier-321"
                }))
            }
        })
        .expect(2)
        .mount(server)
        .await;
}

async fn mock_poll_token_single(server: &MockServer, endpoint: &str, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path(endpoint))
        .respond_with(response)
        .mount(server)
        .await;
}

async fn mock_oauth_token_single(server: &MockServer, jwt: String) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id_token": jwt.clone(),
            "access_token": "access-token-123",
            "refresh_token": "refresh-token-123"
        })))
        .mount(server)
        .await;
}

fn server_opts(codex_home: &tempfile::TempDir, issuer: String) -> ServerOptions {
    let mut opts = ServerOptions::new(codex_home.path().to_path_buf(), "client-id".to_string());
    opts.issuer = issuer;
    opts.open_browser = false;
    opts
}

#[tokio::test]
async fn device_code_login_integration_succeeds() {
    skip_if_no_network!();

    let codex_home = tempdir().unwrap();
    let mock_server = MockServer::start().await;

    mock_usercode_success(&mock_server).await;

    mock_poll_token_two_step(&mock_server, Arc::new(AtomicUsize::new(0)), 404).await;

    let jwt = make_jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct_321"
        }
    }));

    mock_oauth_token_single(&mock_server, jwt.clone()).await;

    let issuer = mock_server.uri();
    let opts = server_opts(&codex_home, issuer);

    run_device_code_login(opts)
        .await
        .expect("device code login integration should succeed");

    let auth_path = get_auth_file(codex_home.path());
    let auth = try_read_auth_json(&auth_path).expect("auth.json written");
    // assert_eq!(auth.openai_api_key.as_deref(), Some("api-key-321"));
    let tokens = auth.tokens.expect("tokens persisted");
    assert_eq!(tokens.access_token, "access-token-123");
    assert_eq!(tokens.refresh_token, "refresh-token-123");
    assert_eq!(tokens.id_token.raw_jwt, jwt);
    assert_eq!(tokens.account_id.as_deref(), Some("acct_321"));
}

#[tokio::test]
async fn device_code_login_integration_handles_usercode_http_failure() {
    skip_if_no_network!();

    let codex_home = tempdir().unwrap();
    let mock_server = MockServer::start().await;

    mock_usercode_failure(&mock_server, 503).await;

    let issuer = mock_server.uri();

    let opts = server_opts(&codex_home, issuer);

    let err = run_device_code_login(opts)
        .await
        .expect_err("usercode HTTP failure should bubble up");
    assert!(
        err.to_string()
            .contains("device code request failed with status"),
        "unexpected error: {err:?}"
    );

    let auth_path = get_auth_file(codex_home.path());
    assert!(!auth_path.exists());
}

#[tokio::test]
async fn device_code_login_integration_persists_without_api_key_on_exchange_failure() {
    skip_if_no_network!();

    let codex_home = tempdir().unwrap();

    let mock_server = MockServer::start().await;

    mock_usercode_success(&mock_server).await;

    mock_poll_token_two_step(&mock_server, Arc::new(AtomicUsize::new(0)), 404).await;

    let jwt = make_jwt(json!({}));

    mock_oauth_token_single(&mock_server, jwt.clone()).await;

    let issuer = mock_server.uri();

    let mut opts = ServerOptions::new(codex_home.path().to_path_buf(), "client-id".to_string());
    opts.issuer = issuer;
    opts.open_browser = false;

    run_device_code_login(opts)
        .await
        .expect("device login should succeed without API key exchange");

    let auth_path = get_auth_file(codex_home.path());
    let auth = try_read_auth_json(&auth_path).expect("auth.json written");
    assert!(auth.openai_api_key.is_none());
    let tokens = auth.tokens.expect("tokens persisted");
    assert_eq!(tokens.access_token, "access-token-123");
    assert_eq!(tokens.refresh_token, "refresh-token-123");
    assert_eq!(tokens.id_token.raw_jwt, jwt);
}

#[tokio::test]
async fn device_code_login_integration_handles_error_payload() {
    skip_if_no_network!();

    let codex_home = tempdir().unwrap();

    // Start WireMock
    let mock_server = MockServer::start().await;

    mock_usercode_success(&mock_server).await;

    // // /deviceauth/token → returns error payload with status 401
    mock_poll_token_single(
        &mock_server,
        "/api/accounts/deviceauth/token",
        ResponseTemplate::new(401).set_body_json(json!({
            "error": "authorization_declined",
            "error_description": "Denied"
        })),
    )
    .await;

    // (WireMock will automatically 404 for other paths)

    let issuer = mock_server.uri();

    let mut opts = ServerOptions::new(codex_home.path().to_path_buf(), "client-id".to_string());
    opts.issuer = issuer;
    opts.open_browser = false;

    let err = run_device_code_login(opts)
        .await
        .expect_err("integration failure path should return error");

    // Accept either the specific error payload, a 400, or a 404 (since the client may return 404 if the flow is incomplete)
    assert!(
        err.to_string().contains("authorization_declined") || err.to_string().contains("401"),
        "Expected an authorization_declined / 400 / 404 error, got {err:?}"
    );

    let auth_path = get_auth_file(codex_home.path());
    assert!(
        !auth_path.exists(),
        "auth.json should not be created when device auth fails"
    );
}
