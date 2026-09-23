//! Application policy is enforced through public thread and turn RPCs.

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn standalone_startup_applies_local_policy_before_fetching_stored_account_requirements()
-> Result<()> {
    let backend = MockServer::start().await;
    let bundle_path = "/backend-api/wham/config/bundle";
    Mock::given(method("GET"))
        .and(path(bundle_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&backend)
        .await;
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "chatgpt_base_url = '{}/backend-api/'\ncli_auth_credentials_store = 'file'\n",
            backend.uri()
        ),
    )?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "[application.network]",
    )?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("stored-token")
            .account_id("selected")
            .plan_type("enterprise"),
        AuthCredentialsStoreMode::File,
    )?;
    let _app = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized()
        .await?;
    assert!(
        backend
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.url.path() != bundle_path)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_reloads_apply_local_edits_and_cancel_active_responses() -> Result<()> {
    for requirements in ["[application.network]", "[malformed"] {
        let (_release, gate) = tokio::sync::oneshot::channel();
        let (destination, _) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
            gate: Some(gate),
            body: String::new(),
        }]])
        .await;
        let home = TempDir::new()?;
        MockResponsesConfig::new(destination.uri()).write(home.path())?;
        write_models_cache(home.path()).await?;
        let mut app = TestAppServer::builder()
            .with_codex_home(home.path())
            .build_initialized()
            .await?;
        let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
        let params = serde_json::json!({
            "threadId": thread.id, "input": [{"type": "text", "text": "Hello"}]
        });
        app.send_request("turn/start", Some(params)).await?;
        timeout(
            Duration::from_secs(/*secs*/ 30),
            destination.wait_for_request_count(/*count*/ 1),
        )
        .await?;
        std::fs::write(home.path().join("requirements.toml"), requirements)?;
        app.send_request("config/read", Some(serde_json::json!({})))
            .await?;
        let completed: TurnCompletedNotification = timeout(
            Duration::from_secs(/*secs*/ 5),
            app.read_notification("turn/completed"),
        )
        .await??;
        assert_eq!(completed.turn.status, TurnStatus::Failed);
        destination.shutdown().await;
    }
    Ok(())
}
