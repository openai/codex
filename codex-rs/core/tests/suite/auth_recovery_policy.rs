//! A denied credential refresh ends 401 recovery without changing stored credentials.

use std::sync::Arc;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyController;
use codex_http_client::NetworkPolicyDenied;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::AuthRouteConfig;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denied_oauth_refresh_terminates_401_recovery_and_preserves_auth() -> Result<()> {
    let home = Arc::new(TempDir::new()?);
    let auth_json = serde_json::to_vec(&json!({
        "auth_mode": "chatgpt",
        "tokens": {
            "id_token": "e30.e30.signature",
            "access_token": "current-access-token",
            "refresh_token": "current-refresh-token",
            "account_id": "workspace-one"
        },
        "last_refresh": chrono::Utc::now()
    }))?;
    std::fs::write(home.path().join("auth.json"), &auth_json)?;
    let auth = AuthManager::shared(
        home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::default(),
        AuthRouteConfig::from_http_client_factory(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
                .with_network_policy(NetworkPolicyController::default().policy()),
        ),
    )
    .await;
    let original = auth.auth_cached().expect("stored ChatGPT auth");
    let server = start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer current-access-token"))
        .and(header("chatgpt-account-id", "workspace-one"))
        .respond_with(ResponseTemplate::new(401))
        .expect(/*r*/ 2) // Initial request and the existing disk-reload recovery step.
        .mount(&server)
        .await;
    let mut builder = test_codex().with_home(home).with_auth_manager(auth.clone());
    let test = builder.build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut policy_error = None;
    wait_for_event(&test.codex, |event| {
        if let EventMsg::Error(error) = event {
            policy_error.get_or_insert_with(|| error.message.clone());
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        policy_error,
        Some(format!("Fatal error: {}", NetworkPolicyDenied::Unavailable))
    );
    assert_eq!(auth.auth().await, Some(original));
    assert_eq!(
        std::fs::read(test.home.path().join("auth.json"))?,
        auth_json
    );
    server.verify().await;
    Ok(())
}
