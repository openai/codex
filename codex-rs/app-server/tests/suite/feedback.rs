use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_chat_completions_server;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::UploadFeedbackParams;
use codex_app_server_protocol::UploadFeedbackResponse;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_feedback_succeeds() -> Result<()> {
    let server = create_mock_chat_completions_server(vec![]).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let new_id = mcp
        .send_new_conversation_request(NewConversationParams::default())
        .await?;
    let new_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<NewConversationResponse>(new_response)?;

    let upload_id = mcp
        .send_upload_feedback_request(UploadFeedbackParams {
            classification: "bug".to_string(),
            reason: Some("it broke".to_string()),
            conversation_id: Some(conversation_id),
            include_logs: true,
        })
        .await?;

    let upload_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(upload_id)),
    )
    .await??;
    let UploadFeedbackResponse { thread_id } =
        to_response::<UploadFeedbackResponse>(upload_response)?;

    assert!(
        !thread_id.is_empty(),
        "thread id should be returned by upload feedback"
    );
    Ok(())
}

// Removed invalid rollout path test: rollout path is now resolved server-side.

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
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
