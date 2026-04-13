use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ShareRevokeParams;
use codex_app_server_protocol::ThreadShareParams;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_share_requires_chatgpt_auth_for_persisted_threads() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let filename_ts = "2025-01-05T12-00-00";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let share_request_id = mcp
        .send_thread_share_request(ThreadShareParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let share_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(share_request_id)),
    )
    .await??;

    assert!(
        share_error
            .error
            .message
            .contains("chatgpt authentication required to create share link"),
        "unexpected error: {}",
        share_error.error.message
    );

    Ok(())
}

#[tokio::test]
async fn thread_share_rejects_unmaterialized_threads() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_response)?;
    assert!(
        !thread.path.as_ref().expect("thread path").exists(),
        "fresh thread rollout should not be materialized yet"
    );

    let share_request_id = mcp
        .send_thread_share_request(ThreadShareParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let share_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(share_request_id)),
    )
    .await??;

    assert!(
        share_error
            .error
            .message
            .contains("thread/share is only available for persisted conversations"),
        "unexpected error: {}",
        share_error.error.message
    );
    assert_eq!(
        share_error.error.data,
        Some(json!({"reason": "thread_not_persisted"}))
    );

    Ok(())
}

#[tokio::test]
async fn share_revoke_requires_chatgpt_auth() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_minimal_config(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let revoke_request_id = mcp
        .send_share_revoke_request(ShareRevokeParams {
            share_id: "share_123".to_string(),
        })
        .await?;
    let revoke_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(revoke_request_id)),
    )
    .await??;

    assert!(
        revoke_error
            .error
            .message
            .contains("chatgpt authentication required to revoke share link"),
        "unexpected error: {}",
        revoke_error.error.message
    );

    Ok(())
}

fn create_minimal_config(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "mock-model"
approval_policy = "never"
"#,
    )?;
    Ok(())
}
