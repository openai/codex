#![cfg(unix)]
// Support code lives in the `app_test_support` crate under tests/common.

use std::path::Path;

use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::InterruptConversationParams;
use codex_app_server_protocol::InterruptConversationResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_core::protocol::TurnAbortReason;
use core_test_support::skip_if_no_network;
use tempfile::TempDir;
use tokio::time::timeout;

use app_test_support::McpProcess;
use app_test_support::create_mock_chat_completions_server;
use app_test_support::create_shell_sse_response;
use app_test_support::to_response;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_shell_command_interruption() {
    skip_if_no_network!();

    if let Err(err) = shell_command_interruption().await {
        panic!("failure: {err}");
    }
}

async fn shell_command_interruption() -> anyhow::Result<()> {
    // Use a cross-platform blocking command. On Windows plain `sleep` is not guaranteed to exist
    // (MSYS/GNU coreutils may be absent) and the failure causes the tool call to finish immediately,
    // which triggers a second model request before the test sends the explicit follow-up. That
    // prematurely consumes the second mocked SSE response and leads to a third POST (panic: no response for 2).
    // Powershell Start-Sleep is always available on Windows runners. On Unix we keep using `sleep`.
    #[cfg(target_os = "windows")]
    let shell_command = vec![
        "powershell".to_string(),
        "-Command".to_string(),
        "Start-Sleep -Seconds 10".to_string(),
    ];
    #[cfg(not(target_os = "windows"))]
    let shell_command = vec!["sleep".to_string(), "10".to_string()];

    let tmp = TempDir::new()?;
    // Temporary Codex home with config pointing at the mock server.
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    // Create mock server with a single SSE response: the long sleep command
    let server = create_mock_chat_completions_server(vec![create_shell_sse_response(
        shell_command.clone(),
        Some(&working_directory),
        Some(10_000), // 10 seconds timeout in ms
        "call_sleep",
    )?])
    .await;
    create_config_toml(&codex_home, server.uri())?;

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // 1) newConversation
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            cwd: Some(working_directory.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let new_conv_resp = to_response::<NewConversationResponse>(new_conv_resp)?;
    let NewConversationResponse {
        conversation_id, ..
    } = new_conv_resp;

    // 2) addConversationListener
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams { conversation_id })
        .await?;
    let _add_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await??;

    // 3) sendUserMessage (should trigger notifications; we only validate an OK response)
    let send_user_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![codex_app_server_protocol::InputItem::Text {
                text: "run first sleep command".to_string(),
            }],
        })
        .await?;
    let send_user_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_user_id)),
    )
    .await??;
    let SendUserMessageResponse {} = to_response::<SendUserMessageResponse>(send_user_resp)?;

    // Give the command a moment to start
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // 4) send interrupt request
    let interrupt_id = mcp
        .send_interrupt_conversation_request(InterruptConversationParams { conversation_id })
        .await?;
    let interrupt_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(interrupt_id)),
    )
    .await??;
    let InterruptConversationResponse { abort_reason } =
        to_response::<InterruptConversationResponse>(interrupt_resp)?;
    assert_eq!(TurnAbortReason::Interrupted, abort_reason);

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
sandbox_mode = "danger-full-access"

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
