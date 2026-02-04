//! Validates that the collaboration mode list endpoint returns the expected default presets.
//!
//! The test drives the app server through the MCP harness and asserts that the list response
//! includes the plan and default modes with their default model and reasoning effort
//! settings, which keeps the API contract visible in one place.

#![allow(clippy::unwrap_used)]

use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::CollaborationModeListResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_core::models_manager::test_builtin_collaboration_mode_presets;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Confirms the server returns the default collaboration mode presets in a stable order.
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

    let expected = vec![plan_preset(), default_preset()];
    assert_eq!(expected, items);
    Ok(())
}

/// Builds the plan preset that the list response is expected to return.
///
/// If the defaults change in the app server, this helper should be updated alongside the
/// contract, or the test will fail in ways that imply a regression in the API.
fn plan_preset() -> CollaborationModeMask {
    let presets = test_builtin_collaboration_mode_presets();
    presets
        .into_iter()
        .find(|p| p.mode == Some(ModeKind::Plan))
        .unwrap()
}

/// Builds the default preset that the list response is expected to return.
fn default_preset() -> CollaborationModeMask {
    let presets = test_builtin_collaboration_mode_presets();
    presets
        .into_iter()
        .find(|p| p.mode == Some(ModeKind::Default))
        .unwrap()
}
