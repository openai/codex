use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::CollaborationModeListResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Settings;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn list_collaboration_modes_returns_presets() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_collaboration_modes_request(CollaborationModeListParams {})
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let CollaborationModeListResponse { data: items } =
        to_response::<CollaborationModeListResponse>(response)?;

    let settings = Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: None,
        developer_instructions: None,
    };
    let expected = vec![
        CollaborationMode::Plan(settings.clone()),
        CollaborationMode::Collaborate(settings.clone()),
        CollaborationMode::Execute(settings),
    ];
    assert_eq!(expected, items);
    Ok(())
}
