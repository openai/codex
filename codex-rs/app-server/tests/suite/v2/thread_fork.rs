use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::rollout_path;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use codex_protocol::models::MessagePhase;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::fs::FileTimes;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_fork_creates_new_thread_and_emits_started() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let preview = "Saved user message";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        preview,
        Some("mock_provider"),
        None,
    )?;

    let original_path = codex_home
        .path()
        .join("sessions")
        .join("2025")
        .join("01")
        .join("05")
        .join(format!(
            "rollout-2025-01-05T12-00-00-{conversation_id}.jsonl"
        ));
    assert!(
        original_path.exists(),
        "expected original rollout to exist at {}",
        original_path.display()
    );
    let original_contents = std::fs::read_to_string(&original_path)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id.clone(),
            ..Default::default()
        })
        .await?;
    let fork_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(fork_id)),
    )
    .await??;
    let fork_result = fork_resp.result.clone();
    let ThreadForkResponse { thread, .. } = to_response::<ThreadForkResponse>(fork_resp)?;

    // Wire contract: thread title field is `name`, serialized as null when unset.
    let thread_json = fork_result
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/fork result.thread must be an object");
    assert_eq!(
        thread_json.get("name"),
        Some(&Value::Null),
        "forked threads do not inherit a name; expected `name: null`"
    );

    let after_contents = std::fs::read_to_string(&original_path)?;
    assert_eq!(
        after_contents, original_contents,
        "fork should not mutate the original rollout file"
    );

    assert_ne!(thread.id, conversation_id);
    assert_eq!(thread.preview, preview);
    assert_eq!(thread.model_provider, "mock_provider");
    assert_eq!(thread.status, ThreadStatus::Idle);
    let thread_path = thread.path.clone().expect("thread path");
    assert!(thread_path.is_absolute());
    assert_ne!(thread_path, original_path);
    assert!(thread.cwd.is_absolute());
    assert_eq!(thread.source, SessionSource::VsCode);
    assert_eq!(thread.name, None);

    assert_eq!(
        thread.turns.len(),
        1,
        "expected forked thread to include one turn"
    );
    let turn = &thread.turns[0];
    assert_eq!(turn.status, TurnStatus::Completed);
    assert_eq!(turn.items.len(), 1, "expected user message item");
    match &turn.items[0] {
        ThreadItem::UserMessage { content, .. } => {
            assert_eq!(
                content,
                &vec![UserInput::Text {
                    text: preview.to_string(),
                    text_elements: Vec::new(),
                }]
            );
        }
        other => panic!("expected user message item, got {other:?}"),
    }

    // A corresponding thread/started notification should arrive.
    let notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/started"),
    )
    .await??;
    let started_params = notif.params.clone().expect("params must be present");
    let started_thread_json = started_params
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/started params.thread must be an object");
    assert_eq!(
        started_thread_json.get("name"),
        Some(&Value::Null),
        "thread/started must serialize `name: null` when unset"
    );
    let started: ThreadStartedNotification =
        serde_json::from_value(notif.params.expect("params must be present"))?;
    assert_eq!(started.thread, thread);

    Ok(())
}

#[tokio::test]
async fn thread_fork_rejects_unmaterialized_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: thread.id,
            ..Default::default()
        })
        .await?;
    let fork_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(fork_id)),
    )
    .await??;
    assert!(
        fork_err
            .error
            .message
            .contains("no rollout found for thread id"),
        "unexpected fork error: {}",
        fork_err.error.message
    );

    Ok(())
}

#[tokio::test]
async fn thread_fork_can_fork_after_selected_turn() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let conversation_id = create_fake_rollout_with_explicit_turns(
        codex_home.path(),
        "2025-01-05T12-00-01",
        "2025-01-05T12:00:01Z",
        &[
            ExplicitTurnFixture {
                turn_id: "turn-1",
                user_text: Some("u1"),
                agent_text: Some("a1"),
                state: FixtureTurnState::Completed,
            },
            ExplicitTurnFixture {
                turn_id: "turn-2",
                user_text: Some("u2"),
                agent_text: Some("a2"),
                state: FixtureTurnState::Completed,
            },
            ExplicitTurnFixture {
                // Explicit turn with no user message exercises the exact cut-after-turn logic.
                turn_id: "turn-3-empty",
                user_text: None,
                agent_text: None,
                state: FixtureTurnState::Completed,
            },
        ],
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let fork_cwd = codex_home.path().join("fork-worktree");
    fs::create_dir_all(&fork_cwd)?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            fork_after_turn_id: Some("turn-2".to_string()),
            cwd: Some(fork_cwd.display().to_string()),
            ..Default::default()
        })
        .await?;
    let fork_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(fork_id)),
    )
    .await??;
    let ThreadForkResponse { thread, cwd, .. } = to_response::<ThreadForkResponse>(fork_resp)?;

    assert_eq!(cwd, fork_cwd);
    assert_eq!(thread.cwd, fork_cwd);
    assert_eq!(thread.turns.len(), 2, "later turns should be truncated");
    assert_eq!(thread.turns[0].id, "turn-1");
    assert_eq!(thread.turns[1].id, "turn-2");
    assert_turn_user_text(&thread.turns[0].items, "u1");
    assert_turn_user_text(&thread.turns[1].items, "u2");
    Ok(())
}

#[tokio::test]
async fn thread_fork_rejects_unknown_turn_anchor() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let conversation_id = create_fake_rollout_with_explicit_turns(
        codex_home.path(),
        "2025-01-05T12-00-02",
        "2025-01-05T12:00:02Z",
        &[ExplicitTurnFixture {
            turn_id: "turn-1",
            user_text: Some("u1"),
            agent_text: Some("a1"),
            state: FixtureTurnState::Completed,
        }],
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            fork_after_turn_id: Some("missing-turn".to_string()),
            ..Default::default()
        })
        .await?;
    let fork_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(fork_id)),
    )
    .await??;

    assert!(
        fork_err.error.message.contains("fork turn not found"),
        "unexpected fork error: {}",
        fork_err.error.message
    );
    Ok(())
}

#[tokio::test]
async fn thread_fork_rejects_legacy_turn_anchor() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-03",
        "2025-01-05T12:00:03Z",
        "legacy preview",
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            fork_after_turn_id: Some("legacy-turn".to_string()),
            ..Default::default()
        })
        .await?;
    let fork_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(fork_id)),
    )
    .await??;

    assert!(
        fork_err.error.message.contains("legacy thread history"),
        "unexpected fork error: {}",
        fork_err.error.message
    );
    Ok(())
}

#[tokio::test]
async fn thread_fork_rejects_in_progress_turn_anchor() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let conversation_id = create_fake_rollout_with_explicit_turns(
        codex_home.path(),
        "2025-01-05T12-00-04",
        "2025-01-05T12:00:04Z",
        &[ExplicitTurnFixture {
            turn_id: "turn-in-progress",
            user_text: Some("u1"),
            agent_text: Some("a1"),
            state: FixtureTurnState::InProgress,
        }],
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            fork_after_turn_id: Some("turn-in-progress".to_string()),
            ..Default::default()
        })
        .await?;
    let fork_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(fork_id)),
    )
    .await??;

    assert!(
        fork_err.error.message.contains("not in progress"),
        "unexpected fork error: {}",
        fork_err.error.message
    );
    Ok(())
}

#[tokio::test]
async fn thread_fork_rejects_turn_anchor_without_agent_message() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let conversation_id = create_fake_rollout_with_explicit_turns(
        codex_home.path(),
        "2025-01-05T12-00-05",
        "2025-01-05T12:00:05Z",
        &[ExplicitTurnFixture {
            turn_id: "turn-no-agent",
            user_text: Some("u1"),
            agent_text: None,
            state: FixtureTurnState::Completed,
        }],
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            fork_after_turn_id: Some("turn-no-agent".to_string()),
            ..Default::default()
        })
        .await?;
    let fork_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(fork_id)),
    )
    .await??;

    assert!(
        fork_err.error.message.contains("agent message"),
        "unexpected fork error: {}",
        fork_err.error.message
    );
    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
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
"#
        ),
    )
}

#[derive(Clone, Copy)]
enum FixtureTurnState {
    Completed,
    InProgress,
}

#[derive(Clone, Copy)]
struct ExplicitTurnFixture<'a> {
    turn_id: &'a str,
    user_text: Option<&'a str>,
    agent_text: Option<&'a str>,
    state: FixtureTurnState,
}

fn assert_turn_user_text(items: &[ThreadItem], expected: &str) {
    match items.first() {
        Some(ThreadItem::UserMessage { content, .. }) => assert_eq!(
            content,
            &vec![UserInput::Text {
                text: expected.to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected first turn item to be a user message, got {other:?}"),
    }
}

fn create_fake_rollout_with_explicit_turns(
    codex_home: &Path,
    filename_ts: &str,
    meta_rfc3339: &str,
    turns: &[ExplicitTurnFixture<'_>],
) -> Result<String> {
    let uuid = Uuid::new_v4();
    let uuid_str = uuid.to_string();
    let conversation_id = ThreadId::from_string(&uuid_str)?;
    let file_path = rollout_path(codex_home, filename_ts, &uuid_str);
    let dir = file_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing rollout parent directory"))?;
    fs::create_dir_all(dir)?;

    let meta = SessionMeta {
        id: conversation_id,
        forked_from_id: None,
        timestamp: meta_rfc3339.to_string(),
        cwd: PathBuf::from("/"),
        originator: "codex".to_string(),
        cli_version: "0.0.0".to_string(),
        source: codex_protocol::protocol::SessionSource::Cli,
        agent_nickname: None,
        agent_role: None,
        model_provider: Some("mock_provider".to_string()),
        base_instructions: None,
        dynamic_tools: None,
    };
    let mut lines = vec![rollout_line(
        meta_rfc3339,
        RolloutItem::SessionMeta(SessionMetaLine { meta, git: None }),
    )?];

    for (idx, turn) in turns.iter().enumerate() {
        if let Some(user_text) = turn.user_text {
            lines.push(
                json!({
                    "timestamp": meta_rfc3339,
                    "type":"response_item",
                    "payload": {
                        "type":"message",
                        "role":"user",
                        "content":[{"type":"input_text","text": user_text}]
                    }
                })
                .to_string(),
            );
        }

        lines.push(rollout_line(
            meta_rfc3339,
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn.turn_id.to_string(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: Default::default(),
            })),
        )?);
        if let Some(user_text) = turn.user_text {
            lines.push(rollout_line(
                meta_rfc3339,
                RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                    message: user_text.to_string(),
                    images: None,
                    local_images: Vec::new(),
                    text_elements: Vec::new(),
                })),
            )?);
        }
        if let Some(agent_text) = turn.agent_text {
            lines.push(rollout_line(
                meta_rfc3339,
                RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
                    message: agent_text.to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                })),
            )?);
        }
        if matches!(turn.state, FixtureTurnState::Completed) {
            let last_agent_message = turn.agent_text.map(str::to_string);
            lines.push(rollout_line(
                meta_rfc3339,
                RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                    turn_id: turn.turn_id.to_string(),
                    last_agent_message,
                })),
            )?);
        }

        if idx == 0 && turn.agent_text.is_some() {
            lines.push(
                json!({
                    "timestamp": meta_rfc3339,
                    "type":"response_item",
                    "payload": {
                        "type":"message",
                        "role":"assistant",
                        "content":[{"type":"output_text","text": turn.agent_text.unwrap_or("")}]
                    }
                })
                .to_string(),
            );
        }
    }

    fs::write(&file_path, lines.join("\n") + "\n")?;
    let parsed = chrono::DateTime::parse_from_rfc3339(meta_rfc3339)?.with_timezone(&chrono::Utc);
    let times = FileTimes::new().set_modified(parsed.into());
    fs::OpenOptions::new()
        .append(true)
        .open(&file_path)?
        .set_times(times)?;
    Ok(uuid_str)
}

fn rollout_line(timestamp: &str, item: RolloutItem) -> Result<String> {
    let mut line = serde_json::Map::new();
    line.insert(
        "timestamp".to_string(),
        Value::String(timestamp.to_string()),
    );

    let item_value = serde_json::to_value(item)?;
    let Value::Object(item_map) = item_value else {
        anyhow::bail!("rollout item did not serialize as an object");
    };
    line.extend(item_map);

    Ok(Value::Object(line).to_string())
}
