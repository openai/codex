use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use codex_app_server_protocol::AddCreditsNudgeEmailNotification;
use codex_app_server_protocol::AddCreditsNudgeEmailResult;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadAddCreditsNudgeEmailParams;
use codex_app_server_protocol::ThreadAddCreditsNudgeEmailResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_add_credits_nudge_email_submits_core_op_and_emits_completion() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;

    let server = create_mock_responses_server_sequence(vec![]).await;
    create_config_toml(codex_home.as_path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.as_path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let nudge_id = mcp
        .send_thread_add_credits_nudge_email_request(ThreadAddCreditsNudgeEmailParams {
            thread_id: thread.id.clone(),
        })
        .await?;
    let nudge_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(nudge_id)),
    )
    .await??;
    let _: ThreadAddCreditsNudgeEmailResponse =
        to_response::<ThreadAddCreditsNudgeEmailResponse>(nudge_resp)?;

    let notification: AddCreditsNudgeEmailNotification = serde_json::from_value(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("account/addCreditsNudgeEmail/completed"),
        )
        .await??
        .params
        .expect("account/addCreditsNudgeEmail/completed params"),
    )?;

    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(
        notification.result,
        AddCreditsNudgeEmailResult::Failed {
            message: "codex account authentication required to notify workspace owner".to_string(),
        }
    );

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
