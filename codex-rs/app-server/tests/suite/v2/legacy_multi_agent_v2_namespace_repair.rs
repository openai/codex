use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::RolloutItem;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::read_session_meta_line;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const CUSTOM_NAMESPACE: &str = "agents";
const LEGACY_CALL_ID: &str = "legacy-spawn-call";
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn resumed_legacy_multi_agent_v2_call_uses_configured_namespace_in_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-after-resume"),
            responses::ev_completed("resp-after-resume"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    write_config(codex_home.path(), &server.uri())?;

    let filename_ts = "2025-01-05T12-00-00";
    let thread_id = create_fake_rollout(
        codex_home.path(),
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let rollout_path = rollout_path(codex_home.path(), filename_ts, &thread_id);
    append_legacy_multi_agent_v2_history(&rollout_path).await?;

    let mut app_server = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;
    let resume_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } = to_response(resume_response)?;
    assert_eq!(thread.id, thread_id);

    let turn_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "Continue the legacy thread".to_string(),
                text_elements: Vec::new(),
            }],
            environments: Some(vec![app_server.auto_env_params()?]),
            ..Default::default()
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let input = response_mock.single_request().input();
    let call_index = input
        .iter()
        .position(|item| item.get("call_id").and_then(Value::as_str) == Some(LEGACY_CALL_ID))
        .expect("legacy function call should be present");
    assert_eq!(
        input.get(call_index),
        Some(&json!({
            "type": "function_call",
            "name": "spawn_agent",
            "namespace": CUSTOM_NAMESPACE,
            "arguments": "{}",
            "call_id": LEGACY_CALL_ID,
        }))
    );
    assert_eq!(
        input.get(call_index + 1),
        Some(&json!({
            "type": "function_call_output",
            "call_id": LEGACY_CALL_ID,
            "output": "legacy spawn result",
        }))
    );

    Ok(())
}

async fn append_legacy_multi_agent_v2_history(rollout_path: &std::path::Path) -> Result<()> {
    // Mark the rollout as MAv2 while preserving the legacy unnamespaced call shape that predates
    // namespace support. The paired output makes this a complete historical tool exchange.
    let mut session_meta = read_session_meta_line(rollout_path).await?;
    session_meta.meta.multi_agent_version = Some(MultiAgentVersion::V2);
    append_rollout_item_to_path(rollout_path, &RolloutItem::SessionMeta(session_meta)).await?;
    append_rollout_item_to_path(
        rollout_path,
        &RolloutItem::ResponseItem(ResponseItem::FunctionCall {
            id: None,
            name: "spawn_agent".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: LEGACY_CALL_ID.to_string(),
            internal_chat_message_metadata_passthrough: None,
        }),
    )
    .await?;
    append_rollout_item_to_path(
        rollout_path,
        &RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: LEGACY_CALL_ID.to_string(),
            output: FunctionCallOutputPayload::from_text("legacy spawn result".to_string()),
            internal_chat_message_metadata_passthrough: None,
        }),
    )
    .await?;
    Ok(())
}

fn write_config(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "gpt-5.3-codex"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[features]
personality = true

[features.multi_agent_v2]
enabled = true
tool_namespace = "{CUSTOM_NAMESPACE}"
non_code_mode_only = false

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
        ),
    )
}
