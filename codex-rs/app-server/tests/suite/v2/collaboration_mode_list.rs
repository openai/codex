//! Validates that the collaboration mode list endpoint returns the expected default presets.
//!
//! The test drives the app server through the MCP harness and asserts that the list response
//! includes the plan, pair programming, and execute modes with their default model and reasoning
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

    assert_eq!(items.len(), 3, "should return exactly 3 presets");

    // Verify plan preset
    let plan = items
        .iter()
        .find(|item| matches!(item, CollaborationMode::Plan(_)))
        .expect("should include plan preset");
    assert_eq!(plan.model(), "gpt-5.2-codex");
    assert_eq!(plan.reasoning_effort(), Some(ReasoningEffort::Medium));
    match plan {
        CollaborationMode::Plan(settings) => {
            assert!(
                settings.developer_instructions.is_some(),
                "plan preset should include developer instructions"
            );
            assert!(
                !settings.developer_instructions.as_ref().unwrap().is_empty(),
                "plan preset developer instructions should not be empty"
            );
        }
        _ => unreachable!(),
    }

    // Verify pair programming preset
    let pair_programming = items
        .iter()
        .find(|item| matches!(item, CollaborationMode::PairProgramming(_)))
        .expect("should include pair programming preset");
    assert_eq!(pair_programming.model(), "gpt-5.2-codex");
    assert_eq!(
        pair_programming.reasoning_effort(),
        Some(ReasoningEffort::Medium)
    );
    match pair_programming {
        CollaborationMode::PairProgramming(settings) => {
            assert!(
                settings.developer_instructions.is_some(),
                "pair programming preset should include developer instructions"
            );
            assert!(
                !settings.developer_instructions.as_ref().unwrap().is_empty(),
                "pair programming preset developer instructions should not be empty"
            );
        }
        _ => unreachable!(),
    }

    // Verify execute preset
    let execute = items
        .iter()
        .find(|item| matches!(item, CollaborationMode::Execute(_)))
        .expect("should include execute preset");
    assert_eq!(execute.model(), "gpt-5.2-codex");
    assert_eq!(execute.reasoning_effort(), Some(ReasoningEffort::XHigh));
    match execute {
        CollaborationMode::Execute(settings) => {
            assert!(
                settings.developer_instructions.is_some(),
                "execute preset should include developer instructions"
            );
            assert!(
                !settings.developer_instructions.as_ref().unwrap().is_empty(),
                "execute preset developer instructions should not be empty"
            );
        }
        _ => unreachable!(),
    }

    Ok(())
}
