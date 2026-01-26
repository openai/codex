//! Validates that the collaboration mode list endpoint returns the expected default presets.
//!
//! The test drives the app server through the MCP harness and asserts that the list response
//! includes the plan, coding, pair programming, and execute modes with their default model and reasoning
//! effort settings, which keeps the API contract visible in one place.

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

    let expected = [
        plan_preset(),
        code_preset(),
        pair_programming_preset(),
        execute_preset(),
    ];
    assert_eq!(expected.len(), items.len());
    for (expected_mask, actual_mask) in expected.iter().zip(items.iter()) {
        assert_eq!(expected_mask.name, actual_mask.name);
        assert_eq!(expected_mask.mode, actual_mask.mode);
        assert_eq!(expected_mask.model, actual_mask.model);
        assert_eq!(expected_mask.reasoning_effort, actual_mask.reasoning_effort);
        assert_eq!(
            expected_mask.developer_instructions,
            actual_mask.developer_instructions
        );
    }
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

/// Builds the pair programming preset that the list response is expected to return.
///
/// The helper keeps the expected model and reasoning defaults co-located with the test
/// so that mismatches point directly at the API contract being exercised.
fn pair_programming_preset() -> CollaborationModeMask {
    let presets = test_builtin_collaboration_mode_presets();
    presets
        .into_iter()
        .find(|p| p.mode == Some(ModeKind::PairProgramming))
        .unwrap()
}

/// Builds the code preset that the list response is expected to return.
fn code_preset() -> CollaborationModeMask {
    let presets = test_builtin_collaboration_mode_presets();
    presets
        .into_iter()
        .find(|p| p.mode == Some(ModeKind::Code))
        .unwrap()
}

/// Builds the execute preset that the list response is expected to return.
///
/// The execute preset uses a different reasoning effort to capture the higher-effort
/// execution contract the server currently exposes.
fn execute_preset() -> CollaborationModeMask {
    let presets = test_builtin_collaboration_mode_presets();
    presets
        .into_iter()
        .find(|p| p.mode == Some(ModeKind::Execute))
        .unwrap()
}
