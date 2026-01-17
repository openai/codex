//! Validates that the collaboration mode list endpoint returns the expected default presets.
//!
//! The test drives the app server through the MCP harness and asserts that the list response
//! includes the plan, collaborate, and execute modes with their default model and reasoning
//! effort settings, which keeps the API contract visible in one place.

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
use codex_protocol::openai_models::ReasoningEffort;
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

    let expected = vec![plan_preset(), collaborate_preset(), execute_preset()];
    assert_eq!(expected, items);
    Ok(())
}

/// Builds the plan preset that the list response is expected to return.
///
/// If the defaults change in the app server, this helper should be updated alongside the
/// contract, or the test will fail in ways that imply a regression in the API.
fn plan_preset() -> CollaborationMode {
    CollaborationMode::Plan(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: None,
    })
}

/// Builds the collaborate preset that the list response is expected to return.
///
/// The helper keeps the expected model and reasoning defaults co-located with the test
/// so that mismatches point directly at the API contract being exercised.
fn collaborate_preset() -> CollaborationMode {
    CollaborationMode::Collaborate(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: None,
    })
}

/// Builds the execute preset that the list response is expected to return.
///
/// The execute preset uses a different reasoning effort to capture the higher-effort
/// execution contract the server currently exposes.
fn execute_preset() -> CollaborationMode {
    CollaborationMode::Execute(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::XHigh),
        developer_instructions: None,
    })
}
