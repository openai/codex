use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadGoalSetResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn thread_goal_set_applies_named_permissions_before_continuation() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("materialize-thread"),
                responses::ev_assistant_message("materialize-message", "done"),
                responses::ev_completed("materialize-thread"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("goal-continuation"),
                responses::ev_assistant_message("goal-message", "done"),
                responses::ev_completed("goal-continuation"),
            ]),
        ],
    )
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(start_response)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "keep improving",
                "approvalPolicy": "never",
                "approvalsReviewer": "user",
                "permissions": "dev",
            })),
        )
        .await?;
    let goal_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(goal_id)),
    )
    .await??;
    let _: ThreadGoalSetResponse = to_response(goal_response)?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected materialization and continuation"
    );
    let latest_permissions_instructions = requests[1]
        .message_input_texts("developer")
        .into_iter()
        .rev()
        .find(|text| text.contains("<permissions instructions>"))
        .context("goal continuation should include permissions instructions")?;
    assert!(
        latest_permissions_instructions.contains("Approval policy is currently never"),
        "goal continuation should use the current approval policy: {latest_permissions_instructions}"
    );
    assert!(
        latest_permissions_instructions.contains("workspace-write"),
        "goal continuation should use the selected named profile: {latest_permissions_instructions}"
    );
    assert!(
        !latest_permissions_instructions.contains("read-only"),
        "goal continuation should not reuse the stale sandbox: {latest_permissions_instructions}"
    );

    Ok(())
}

#[tokio::test]
async fn thread_goal_set_applies_explicit_sandbox_before_continuation() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("materialize-thread"),
                responses::ev_assistant_message("materialize-message", "done"),
                responses::ev_completed("materialize-thread"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("goal-continuation"),
                responses::ev_assistant_message("goal-message", "done"),
                responses::ev_completed("goal-continuation"),
            ]),
        ],
    )
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(start_response)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "keep improving",
                "approvalPolicy": "never",
                "approvalsReviewer": "user",
                "sandboxPolicy": { "type": "dangerFullAccess" },
            })),
        )
        .await?;
    let goal_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(goal_id)),
    )
    .await??;
    let _: ThreadGoalSetResponse = to_response(goal_response)?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected materialization and continuation"
    );
    let latest_permissions_instructions = requests[1]
        .message_input_texts("developer")
        .into_iter()
        .rev()
        .find(|text| text.contains("<permissions instructions>"))
        .context("goal continuation should include permissions instructions")?;
    assert!(
        latest_permissions_instructions.contains("Approval policy is currently never"),
        "goal continuation should use the current approval policy: {latest_permissions_instructions}"
    );
    assert!(
        latest_permissions_instructions.contains("danger-full-access"),
        "goal continuation should use the explicit sandbox: {latest_permissions_instructions}"
    );
    assert!(
        !latest_permissions_instructions.contains("read-only"),
        "goal continuation should not reuse the stale sandbox: {latest_permissions_instructions}"
    );

    Ok(())
}

#[tokio::test]
async fn thread_goal_set_rejects_permissions_with_sandbox_for_unloaded_thread() -> Result<()> {
    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread_id,
                "objective": "keep improving",
                "sandboxPolicy": { "type": "dangerFullAccess" },
                "permissions": "dev",
            })),
        )
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(
        error.error.message,
        "`permissions` cannot be combined with `sandboxPolicy`"
    );
    Ok(())
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "on-request"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[features]
goals = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[permissions.dev.filesystem.":workspace_roots"]
"." = "write"
"#
        ),
    )
}
