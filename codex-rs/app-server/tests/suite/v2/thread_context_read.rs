use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadContextReadParams;
use codex_app_server_protocol::ThreadContextReadResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_context_read_returns_live_breakdown_for_loaded_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let context_id = mcp
        .send_thread_context_read_request(ThreadContextReadParams {
            thread_id: thread.id,
            verbose: true,
        })
        .await?;
    let context_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(context_id)),
    )
    .await??;
    let ThreadContextReadResponse { context } =
        to_response::<ThreadContextReadResponse>(context_resp)?;

    assert!(
        context.total_tokens > 0,
        "expected non-zero base instruction usage"
    );
    assert_eq!(
        context
            .sections
            .iter()
            .map(|section| section.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Built-in"]
    );
    assert_eq!(
        context.sections[0]
            .details
            .iter()
            .map(|detail| detail.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Base instructions"]
    );
    assert_eq!(
        context.sections[0].tokens,
        context.sections[0]
            .details
            .iter()
            .map(|detail| detail.tokens)
            .sum::<i64>()
    );

    Ok(())
}

#[tokio::test]
async fn thread_context_read_rejects_unloaded_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let context_id = mcp
        .send_thread_context_read_request(ThreadContextReadParams {
            thread_id: "12345678-1234-1234-1234-123456789012".to_string(),
            verbose: false,
        })
        .await?;
    let context_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(context_id)),
    )
    .await??;

    assert!(
        context_err
            .error
            .message
            .contains("thread not loaded: 12345678-1234-1234-1234-123456789012"),
        "unexpected error: {}",
        context_err.error.message
    );

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
