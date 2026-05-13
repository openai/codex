use std::time::Duration;

use anyhow::Result;
use app_test_support::DEFAULT_CLIENT_NAME;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RequestId;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn remote_control_disable_returns_disabled_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_disable_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlDisableResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Disabled);
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_enable_returns_connecting_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlEnableResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Connecting);
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn experimental_feature_enablement_does_not_toggle_remote_control() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("remoteControl/status/changed"),
    )
    .await??;

    let request_id = mcp
        .send_experimental_feature_enablement_set_request(ExperimentalFeatureEnablementSetParams {
            enablement: BTreeMap::from([("remote_control".to_string(), true)]),
        })
        .await?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    timeout(
        Duration::from_millis(200),
        mcp.read_stream_until_notification_message("remoteControl/status/changed"),
    )
    .await
    .expect_err("experimental feature enablement should not emit remote-control status changes");
    Ok(())
}

#[tokio::test]
async fn remote_control_methods_require_experimental_api_capability() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    let init = mcp
        .initialize_with_capabilities(
            ClientInfo {
                name: DEFAULT_CLIENT_NAME.to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: false,
                request_attestation: false,
                opt_out_notification_methods: None,
            }),
        )
        .await?;
    let JSONRPCMessage::Response(_) = init else {
        anyhow::bail!("expected initialize response, got {init:?}");
    };

    let request_id = mcp.send_remote_control_disable_request().await?;
    let error = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert!(error.error.message.contains("remoteControl/disable"));
    assert!(error.error.message.contains("experimentalApi capability"));
    Ok(())
}
