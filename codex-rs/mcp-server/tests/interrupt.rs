mod common;

use std::path::Path;

use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use crate::common::McpProcess;
use crate::common::create_mock_chat_completions_server;
use crate::common::create_shell_sse_response;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_shell_command_interruption() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    if let Err(err) = shell_command_interruption().await {
        panic!("failure: {err}");
    }
}

async fn shell_command_interruption() -> anyhow::Result<()> {
    let shell_command = vec!["sleep".to_string(), "60".to_string()];
    let workdir_for_shell_function_call = TempDir::new()?;

    // Create mock server with a single SSE response: the long sleep command
    let server = create_mock_chat_completions_server(vec![
        create_shell_sse_response(
            shell_command.clone(),
            Some(workdir_for_shell_function_call.path()),
            Some(60_000), // 60 seconds timeout in ms
            "call_sleep",
        )?,
        create_shell_sse_response(
            shell_command.clone(),
            Some(workdir_for_shell_function_call.path()),
            Some(60_000), // 60 seconds timeout in ms
            "call_sleep",
        )?,
    ])
    .await;

    // Create Codex configuration
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), server.uri())?;
    let mut mcp_process = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // Send codex tool call that triggers "sleep 60"
    let codex_request_id = mcp_process
        .send_codex_tool_call(None, "hi run `sleep 60`")
        .await?;

    let session_id = mcp_process
        .read_stream_until_configured_response_message()
        .await?;

    // Give the command a moment to start
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Send interrupt notification
    mcp_process
        .send_notification(
            "notifications/cancelled",
            Some(json!({ "requestId": codex_request_id })),
        )
        .await?;

    // Expect Codex to return an error or interruption response
    let codex_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_response_message(RequestId::Integer(codex_request_id)),
    )
    .await??;

    assert!(
        codex_response
            .result
            .as_object()
            .map(|o| o.contains_key("error"))
            .unwrap_or(false),
        "Expected an interruption or error result, got: {codex_response:?}"
    );

    let codex_reply_request_id = mcp_process
        .send_codex_reply_tool_call(&session_id, "run `sleep 60` please")
        .await?;

    // Give the command a moment to start
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Send interrupt notification
    mcp_process
        .send_notification(
            "notifications/cancelled",
            Some(json!({ "requestId": codex_reply_request_id })),
        )
        .await?;

    // Expect Codex to return an error or interruption response
    let codex_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_response_message(RequestId::Integer(codex_reply_request_id)),
    )
    .await??;

    assert!(
        codex_response
            .result
            .as_object()
            .map(|o| o.contains_key("error"))
            .unwrap_or(false),
        "Expected an interruption or error result, got: {codex_response:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_config_toml(codex_home: &Path, server_uri: String) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_policy = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
