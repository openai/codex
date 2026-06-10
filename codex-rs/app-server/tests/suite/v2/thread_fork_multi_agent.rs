use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::RolloutItem;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::read_session_meta_line;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_fork_disable_multi_agent_tools_overrides_v2_history() -> Result<()> {
    const ROOT_USAGE_HINT: &str = "disabled forks must not receive root multi-agent guidance";

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[features.multi_agent_v2]
enabled = true
root_agent_usage_hint_text = "{ROOT_USAGE_HINT}"
"#,
            server_uri = server.uri(),
        ),
    )?;

    let source_thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "parent message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let source_rollout_path =
        rollout_path(codex_home.path(), "2025-01-05T12-00-00", &source_thread_id);
    let mut source_meta = read_session_meta_line(&source_rollout_path).await?;
    source_meta.meta.multi_agent_version = Some(MultiAgentVersion::V2);
    append_rollout_item_to_path(&source_rollout_path, &RolloutItem::SessionMeta(source_meta))
        .await?;

    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let fork_request_id = app_server
        .send_thread_fork_request(ThreadForkParams {
            thread_id: source_thread_id,
            disable_multi_agent_tools: true,
            ..Default::default()
        })
        .await?;
    let fork_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(fork_request_id)),
    )
    .await??;
    let ThreadForkResponse {
        thread,
        multi_agent_tools_disabled,
        ..
    } = to_response::<ThreadForkResponse>(fork_response)?;
    assert!(multi_agent_tools_disabled);

    let fork_rollout_path = thread.path.as_ref().context("fork rollout path")?;
    let fork_meta = read_session_meta_line(fork_rollout_path.as_path()).await?;
    assert_eq!(
        fork_meta.meta.multi_agent_version,
        Some(MultiAgentVersion::Disabled)
    );

    let turn_request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "which tools are available?".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    assert!(!request.body_contains_text(ROOT_USAGE_HINT));
    let tool_names = request.body_json()["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"update_plan".to_string()));
    let present_multi_agent_tools = tool_names
        .into_iter()
        .filter(|tool| {
            matches!(
                tool.as_str(),
                "spawn_agent"
                    | "send_message"
                    | "followup_task"
                    | "wait_agent"
                    | "interrupt_agent"
                    | "list_agents"
                    | "spawn_agents_on_csv"
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(present_multi_agent_tools, Vec::<String>::new());

    Ok(())
}
