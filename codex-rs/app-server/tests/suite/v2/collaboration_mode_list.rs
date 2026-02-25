//! Validates that the collaboration mode list endpoint returns the expected default presets.
//!
//! The test drives the app server through the MCP harness and asserts that the list response
//! includes the plan and default modes with their default model and reasoning effort
//! settings, which keeps the API contract visible in one place.

#![allow(clippy::unwrap_used)]

use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::CollaborationModeListResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::test_support::builtin_collaboration_mode_presets;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
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
        .send_list_collaboration_modes_request(CollaborationModeListParams::default())
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

#[tokio::test]
async fn list_collaboration_modes_honors_thread_feature_overrides() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
default_mode_request_user_input = false

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
            server.uri()
        ),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let thread_request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            config: Some(HashMap::from([(
                "features.default_mode_request_user_input".to_string(),
                json!(true),
            )])),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_response)?;

    let list_request_id = mcp
        .send_list_collaboration_modes_request(CollaborationModeListParams {
            thread_id: Some(thread.id),
        })
        .await?;
    let list_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_request_id)),
    )
    .await??;
    let CollaborationModeListResponse { data } =
        to_response::<CollaborationModeListResponse>(list_response)?;
    let default_mode = data
        .into_iter()
        .find(|preset| preset.mode == Some(ModeKind::Default))
        .expect("default mode should be present");
    let instructions = default_mode
        .developer_instructions
        .flatten()
        .expect("default mode should include built-in instructions");
    assert!(instructions.contains("The `request_user_input` tool is available in Default mode."));

    Ok(())
}

/// Builds the plan preset that the list response is expected to return.
///
/// If the defaults change in the app server, this helper should be updated alongside the
/// contract, or the test will fail in ways that imply a regression in the API.
fn plan_preset() -> CollaborationModeMask {
    let presets = builtin_collaboration_mode_presets();
    presets
        .into_iter()
        .find(|p| p.mode == Some(ModeKind::Plan))
        .unwrap()
}

/// Builds the default preset that the list response is expected to return.
fn default_preset() -> CollaborationModeMask {
    let presets = builtin_collaboration_mode_presets();
    presets
        .into_iter()
        .find(|p| p.mode == Some(ModeKind::Default))
        .unwrap()
}
