use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ExecRunParams;
use codex_app_server_protocol::ExecRunResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn exec_run_completes_turn_and_returns_final_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;

    let request_id = mcp
        .send_exec_run_request(ExecRunParams {
            input: vec![V2UserInput::Text {
                text: "Generate a title".to_string(),
                text_elements: Vec::new(),
            }],
            model: None,
            model_provider: None,
            cwd: None,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
            config: None,
            base_instructions: None,
            developer_instructions: None,
            output_schema: None,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExecRunResponse = to_response(response)?;

    assert_eq!(response.status, TurnStatus::Completed);
    assert_eq!(response.last_agent_message, Some("Done".to_string()));
    assert_eq!(response.error, None);
    assert!(!response.thread_id.is_empty(), "thread_id should be set");
    assert!(!response.turn_id.is_empty(), "turn_id should be set");

    Ok(())
}

#[tokio::test]
async fn exec_run_rejects_empty_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;

    let request_id = mcp
        .send_exec_run_request(ExecRunParams {
            input: Vec::new(),
            model: None,
            model_provider: None,
            cwd: None,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
            config: None,
            base_instructions: None,
            developer_instructions: None,
            output_schema: None,
        })
        .await?;

    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(error.error.message, "input must not be empty".to_string());

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
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
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
