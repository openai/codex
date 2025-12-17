use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_chat_completions_server_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::CompactStartParams;
use codex_app_server_protocol::ContextCompactedNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn compact_start_emits_context_compacted_notification() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response(
        "compacted summary",
    )?];
    let server = create_mock_chat_completions_server_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_default_thread(&mut mcp).await?;

    let compact_req = mcp
        .send_compact_start_request(CompactStartParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let compact_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(compact_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(compact_resp)?;
    assert_eq!(turn.status, TurnStatus::InProgress);
    let turn_id = turn.id.clone();

    let compacted_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/compacted"),
    )
    .await??;
    let compacted: ContextCompactedNotification =
        serde_json::from_value(compacted_notif.params.expect("params must be present"))?;
    assert_eq!(compacted.thread_id, thread_id);
    assert_eq!(compacted.turn_id, turn_id);

    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification =
        serde_json::from_value(completed_notif.params.expect("params must be present"))?;
    assert_eq!(completed.thread_id, compacted.thread_id);
    assert_eq!(completed.turn.id, turn_id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);

    Ok(())
}

async fn start_default_thread(mcp: &mut McpProcess) -> Result<String> {
    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;
    Ok(thread.id)
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider"
base_url = "{server_uri}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
