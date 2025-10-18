use std::path::Path;
use std::time::Duration;

use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::CancelLoginChatGptParams;
use codex_app_server_protocol::CancelLoginChatGptResponse;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetAuthStatusResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginChatGptResponse;
use codex_app_server_protocol::LogoutChatGptResponse;
use codex_app_server_protocol::RequestId;
use codex_login::login_with_api_key;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// Helper to create a config.toml; mirrors create_conversation.rs
fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "http://127.0.0.1:0/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logout_chatgpt_removes_auth() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml(codex_home.path()).expect("write config.toml");
    login_with_api_key(codex_home.path(), "sk-test-key").expect("seed api key");
    assert!(codex_home.path().join("auth.json").exists());

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)])
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    let id = mcp
        .send_logout_chat_gpt_request()
        .await
        .expect("send logoutChatGpt");
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(id)),
    )
    .await
    .expect("logoutChatGpt timeout")
    .expect("logoutChatGpt response");
    let _ok: LogoutChatGptResponse = to_response(resp).expect("deserialize logout response");

    assert!(
        !codex_home.path().join("auth.json").exists(),
        "auth.json should be deleted"
    );

    // Verify status reflects signed-out state.
    let status_id = mcp
        .send_get_auth_status_request(GetAuthStatusParams {
            include_token: Some(true),
            refresh_token: Some(false),
        })
        .await
        .expect("send getAuthStatus");
    let status_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(status_id)),
    )
    .await
    .expect("getAuthStatus timeout")
    .expect("getAuthStatus response");
    let status: GetAuthStatusResponse = to_response(status_resp).expect("deserialize status");
    assert_eq!(status.auth_method, None);
    assert_eq!(status.auth_token, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_and_cancel_chatgpt() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    let login_id = mcp
        .send_login_chat_gpt_request()
        .await
        .expect("send loginChatGpt");
    let login_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(login_id)),
    )
    .await
    .expect("loginChatGpt timeout")
    .expect("loginChatGpt response");
    let login: LoginChatGptResponse = to_response(login_resp).expect("deserialize login resp");

    let cancel_id = mcp
        .send_cancel_login_chat_gpt_request(CancelLoginChatGptParams {
            login_id: login.login_id,
        })
        .await
        .expect("send cancelLoginChatGpt");
    let cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cancel_id)),
    )
    .await
    .expect("cancelLoginChatGpt timeout")
    .expect("cancelLoginChatGpt response");
    let _ok: CancelLoginChatGptResponse =
        to_response(cancel_resp).expect("deserialize cancel response");

    // Optionally observe the completion notification; do not fail if it races.
    let maybe_note = timeout(
        Duration::from_secs(2),
        mcp.read_stream_until_notification_message("codex/event/login_chat_gpt_complete"),
    )
    .await;
    if maybe_note.is_err() {
        eprintln!("warning: did not observe login_chat_gpt_complete notification after cancel");
    }
}
