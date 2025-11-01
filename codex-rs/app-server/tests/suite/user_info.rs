use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::UserInfoResponse;
use codex_core::auth::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_info_returns_email_from_auth_json() -> Result<()> {
    let codex_home = TempDir::new()?;

    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access")
            .refresh_token("refresh")
            .email("user@example.com"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_user_info_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let received: UserInfoResponse = to_response(response)?;
    let expected = UserInfoResponse {
        alleged_user_email: Some("user@example.com".to_string()),
    };

    assert_eq!(received, expected);
    Ok(())
}
