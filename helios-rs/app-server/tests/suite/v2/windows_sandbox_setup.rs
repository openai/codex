use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use helios_app_server_protocol::JSONRPCResponse;
use helios_app_server_protocol::RequestId;
use helios_app_server_protocol::WindowsSandboxSetupCompletedNotification;
use helios_app_server_protocol::WindowsSandboxSetupMode;
use helios_app_server_protocol::WindowsSandboxSetupStartParams;
use helios_app_server_protocol::WindowsSandboxSetupStartResponse;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn windows_sandbox_setup_start_emits_completion_notification() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let helios_home = TempDir::new()?;
    write_mock_responses_config_toml(
        helios_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        500_000,
        Some(false),
        "mock_provider",
        "compact prompt",
    )?;
    let mut mcp = McpProcess::new(helios_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_windows_sandbox_setup_start_request(WindowsSandboxSetupStartParams {
            mode: WindowsSandboxSetupMode::Unelevated,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let start_payload: WindowsSandboxSetupStartResponse = to_response(response)?;
    assert!(start_payload.started);

    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("windowsSandbox/setupCompleted"),
    )
    .await??;
    let payload: WindowsSandboxSetupCompletedNotification = serde_json::from_value(
        notification
            .params
            .context("missing windowsSandbox/setupCompleted params")?,
    )?;

    assert_eq!(payload.mode, WindowsSandboxSetupMode::Unelevated);
    Ok(())
}
