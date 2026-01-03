use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetAuthStatusResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginChatGptResponse;
use codex_app_server_protocol::LogoutChatGptResponse;
use codex_app_server_protocol::RequestId;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_login::login_with_api_key;
use serial_test::serial;
use std::path::Path;
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
async fn logout_chatgpt_removes_auth() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    login_with_api_key(
        codex_home.path(),
        "sk-test-key",
        AuthCredentialsStoreMode::File,
    )?;
    assert!(codex_home.path().join("auth.json").exists());

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let id = mcp.send_logout_chat_gpt_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(id)),
    )
    .await??;
    let _ok: LogoutChatGptResponse = to_response(resp)?;

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
        .await?;
    let status_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(status_id)),
    )
    .await??;
    let status: GetAuthStatusResponse = to_response(status_resp)?;
    assert_eq!(status.auth_method, None);
    assert_eq!(status.auth_token, None);
    Ok(())
}

fn create_config_toml_forced_login(codex_home: &Path, forced_method: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    let contents = format!(
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
forced_login_method = "{forced_method}"
"#
    );
    std::fs::write(config_toml, contents)
}

fn create_config_toml_forced_workspace(
    codex_home: &Path,
    workspace_id: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    let contents = format!(
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
forced_chatgpt_workspace_id = "{workspace_id}"
"#
    );
    std::fs::write(config_toml, contents)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_chatgpt_rejected_when_forced_api() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml_forced_login(codex_home.path(), "api")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_login_chat_gpt_request().await?;
    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(
        err.error.message,
        "ChatGPT login is disabled. Use API key login instead."
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch the login server since it binds to a fixed port.
#[serial(login_port)]
async fn login_chatgpt_includes_forced_workspace_query_param() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml_forced_workspace(codex_home.path(), "ws-forced")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_login_chat_gpt_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let login: LoginChatGptResponse = to_response(resp)?;
    assert!(
        login.auth_url.contains("allowed_workspace_id=ws-forced"),
        "auth URL should include forced workspace"
    );
    Ok(())
}
