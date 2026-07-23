use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::create_fake_rollout_with_token_usage;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::rollout_path;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::append_thread_name;
use codex_rollout::read_session_meta_line;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::analytics::assert_basic_thread_initialized_event;
use super::analytics::mount_analytics_capture;
use super::analytics::thread_initialized_event;
use super::analytics::wait_for_analytics_payload;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

async fn list_threads(mcp: &mut TestAppServer) -> Result<ThreadListResponse> {
    let list_id = mcp
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(50),
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            is_pinned: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let list_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_id)),
    )
    .await??;
    to_response::<ThreadListResponse>(list_resp)
}

#[tokio::test]
async fn thread_fork_creates_new_thread_and_emits_started() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let preview = "Saved user message";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        preview,
        Some("mock_provider"),
        /*git_info*/ None,
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
    let mut session_meta = read_session_meta_line(&original_path).await?;
    session_meta.meta.multi_agent_version = Some(MultiAgentVersion::V1);
    append_rollout_item_to_path(&original_path, &RolloutItem::SessionMeta(session_meta)).await?;
    let original_contents = std::fs::read_to_string(&original_path)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id.clone(),
            thread_source: Some(ThreadSource::User),
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
        thread_json.get("sessionId").and_then(Value::as_str),
        Some(thread.session_id.as_str()),
        "forked threads should serialize `sessionId` on the thread object"
    );
    assert_eq!(
        thread_json.get("name"),
        Some(&Value::Null),
        "forked threads do not inherit a name; expected `name: null`"
    );
    assert_eq!(
        fork_result.get("sessionId"),
        None,
        "thread/fork should not serialize a top-level `sessionId`"
    );

    let after_contents = std::fs::read_to_string(&original_path)?;
    assert_eq!(
        after_contents, original_contents,
        "fork should not mutate the original rollout file"
    );

    assert_ne!(thread.id, conversation_id);
    assert_eq!(thread.session_id, thread.id);
    assert_eq!(thread.forked_from_id, Some(conversation_id.clone()));
    assert_eq!(thread.preview, preview);
    assert_eq!(thread.model_provider, "mock_provider");
    assert_eq!(thread.status, ThreadStatus::Idle);
    let thread_path = thread.path.clone().expect("thread path");
    assert!(thread_path.as_path().is_absolute());
    assert_ne!(thread_path.as_path(), original_path);
    assert!(thread.cwd.as_path().is_absolute());
    assert_eq!(thread.source, SessionSource::VsCode);
    assert_eq!(thread.thread_source, Some(ThreadSource::User));
    assert_eq!(thread.name, None);

    assert_eq!(
        thread.turns.len(),
        1,
        "expected forked thread to include one turn"
    );
    let turn = &thread.turns[0];
    assert_eq!(turn.status, TurnStatus::Interrupted);
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
    let deadline = tokio::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    let notif = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = timeout(remaining, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notif) = message else {
            continue;
        };
        if notif.method == "thread/status/changed" {
            let status_changed: ThreadStatusChangedNotification =
                serde_json::from_value(notif.params.expect("params must be present"))?;
            if status_changed.thread_id == thread.id {
                anyhow::bail!(
                    "thread/fork should introduce the thread without a preceding thread/status/changed"
                );
            }
            continue;
        }
        if notif.method == "thread/started" {
            break notif;
        }
    };
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
    assert_eq!(
        started_thread_json.get("turns"),
        Some(&json!([])),
        "thread/started must not emit copied fork turns"
    );
    assert_eq!(
        started_thread_json
            .get("threadSource")
            .and_then(Value::as_str),
        Some("user"),
        "thread/started should preserve the caller-supplied fork origin"
    );
    let started: ThreadStartedNotification =
        serde_json::from_value(notif.params.expect("params must be present"))?;
    let mut expected_started_thread = thread;
    expected_started_thread.turns.clear();
    assert_eq!(started.thread, expected_started_thread);

    Ok(())
}

#[tokio::test]
async fn thread_fork_preserves_persisted_approvals_reviewer() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let (source_thread_id, source_turn_id) = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build_initialized()
            .await?;

        let start_id = mcp
            .send_thread_start_request_with_auto_env(ThreadStartParams::default())
            .await?;
        let start_resp: JSONRPCResponse = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
        )
        .await??;
        let ThreadStartResponse { thread, .. } = to_response(start_resp)?;

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
        let turn_resp: JSONRPCResponse = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
        )
        .await??;
        let TurnStartResponse { turn } = to_response(turn_resp)?;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("turn/completed"),
        )
        .await??;

        let second_turn_id = mcp
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "switch to auto-review".to_string(),
                    text_elements: Vec::new(),
                }],
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                ..Default::default()
            })
            .await?;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(second_turn_id)),
        )
        .await??;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("turn/completed"),
        )
        .await??;

        (thread.id, turn.id)
    };

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: source_thread_id,
            last_turn_id: Some(source_turn_id),
            ..Default::default()
        })
        .await?;
    let fork_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(fork_id)),
    )
    .await??;
    let ThreadForkResponse {
        approvals_reviewer, ..
    } = to_response(fork_resp)?;

    assert_eq!(approvals_reviewer, ApprovalsReviewer::AutoReview);

    Ok(())
}

#[tokio::test]
async fn thread_fork_at_last_turn_id_keeps_only_terminal_prefix() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse {
        thread: source_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;
    let source_thread_id = source_thread.id.clone();
    let source_path = source_thread.path.expect("source thread path");

    let mut turn_ids = Vec::new();
    for text in ["first", "second", "third"] {
        let turn_request_id = mcp
            .send_turn_start_request(TurnStartParams {
                thread_id: source_thread_id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        let TurnStartResponse { turn } =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_request_id)).await??;
        turn_ids.push(turn.id);
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("turn/completed"),
        )
        .await??;
    }

    let original_contents = std::fs::read_to_string(source_path.as_path())?;
    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: source_thread_id.clone(),
            last_turn_id: Some(turn_ids[1].clone()),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse {
        thread: forked_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;

    assert_eq!(
        forked_thread
            .turns
            .iter()
            .map(|turn| turn.id.clone())
            .collect::<Vec<_>>(),
        turn_ids[..2]
    );
    assert!(
        forked_thread
            .turns
            .iter()
            .all(|turn| turn.status == TurnStatus::Completed)
    );
    assert_eq!(forked_thread.forked_from_id, Some(source_thread_id));
    assert_eq!(forked_thread.preview, "first");
    assert_eq!(
        std::fs::read_to_string(source_path.as_path())?,
        original_contents,
        "forking at a turn must not mutate the source rollout"
    );

    let forked_path = forked_thread.path.clone().expect("forked thread path");
    let forked_contents = std::fs::read_to_string(forked_path.as_path())?;
    assert!(forked_contents.contains(turn_ids[1].as_str()));
    assert!(!forked_contents.contains(turn_ids[2].as_str()));

    let started = loop {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("thread/started"),
        )
        .await??;
        let started: ThreadStartedNotification =
            serde_json::from_value(notification.params.expect("params must be present"))?;
        if started.thread.id == forked_thread.id {
            break started;
        }
    };
    assert!(started.thread.turns.is_empty());

    Ok(())
}

#[tokio::test]
async fn thread_fork_defers_inherited_active_goal_until_next_turn() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        responses::sse(vec![
            responses::ev_response_created("first-source-turn"),
            responses::ev_completed("first-source-turn"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("second-source-turn"),
            responses::ev_completed("second-source-turn"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("explicit-fork-turn"),
            responses::ev_completed_with_tokens("explicit-fork-turn", /*total_tokens*/ 20),
        ]),
        responses::sse(vec![
            responses::ev_response_created("goal-continuation"),
            responses::ev_completed_with_tokens("goal-continuation", /*total_tokens*/ 100),
        ]),
    ])
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        format!("{config}\n[features]\ngoals = true\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse {
        thread: source_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;
    let source_thread_id = ThreadId::from_string(&source_thread.id)?;

    let mut turn_ids = Vec::new();
    for text in ["first", "second"] {
        let completed = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: source_thread.id.clone(),
                input: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;
        turn_ids.push(completed.turn.id);
    }
    mcp.clear_message_buffer();

    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    let source_goal = state_db
        .thread_goals()
        .replace_thread_goal(
            source_thread_id,
            "continue after the retry",
            codex_state::ThreadGoalStatus::Active,
            /*token_budget*/ Some(150),
        )
        .await?;
    state_db
        .thread_goals()
        .account_thread_goal_usage(
            source_thread_id,
            /*time_delta_seconds*/ 11,
            /*token_delta*/ 37,
            codex_state::GoalAccountingMode::ActiveOnly,
            Some(source_goal.goal_id.as_str()),
        )
        .await?;
    let source_goal = state_db
        .thread_goals()
        .get_thread_goal(source_thread_id)
        .await?
        .expect("source goal");

    let ordinary_fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: source_thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse {
        thread: ordinary_fork,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(ordinary_fork_id)).await??;
    assert_eq!(
        state_db
            .thread_goals()
            .get_thread_goal(ThreadId::from_string(&ordinary_fork.id)?)
            .await?,
        None
    );

    let mut forked_threads = Vec::new();
    for (last_turn_id, before_turn_id, expected_turn_count) in [
        (None, None, 2),
        (Some(turn_ids[0].clone()), None, 1),
        (None, Some(turn_ids[0].clone()), 0),
    ] {
        let fork_id = mcp
            .send_thread_fork_request(ThreadForkParams {
                thread_id: source_thread.id.clone(),
                last_turn_id,
                before_turn_id,
                defer_goal_continuation: true,
                ..Default::default()
            })
            .await?;
        let ThreadForkResponse {
            thread: forked_thread,
            ..
        } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;
        let forked_thread_id = ThreadId::from_string(&forked_thread.id)?;
        assert_eq!(forked_thread.turns.len(), expected_turn_count);
        let mut expected_goal = source_goal.clone();
        expected_goal.thread_id = forked_thread_id;
        assert_eq!(
            state_db
                .thread_goals()
                .get_thread_goal(forked_thread_id)
                .await?,
            Some(expected_goal)
        );
        assert!(
            state_db
                .thread_goals()
                .has_thread_goal_continuation_deferral(forked_thread_id)
                .await?
        );
        forked_threads.push(forked_thread);
    }

    assert_eq!(
        state_db
            .thread_goals()
            .get_thread_goal(source_thread_id)
            .await?,
        Some(source_goal.clone())
    );
    assert!(
        !mcp.pending_notification_methods()
            .iter()
            .any(|method| method == "turn/started"),
        "deferred goal should not start a turn while forking"
    );
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("wiremock requests")
            .iter()
            .filter(|request| request.url.path().ends_with("/responses"))
            .count(),
        2,
        "deferred goal should not issue a model request while forking"
    );

    let forked_thread = forked_threads.pop().expect("empty-prefix fork");
    let forked_thread_id = ThreadId::from_string(&forked_thread.id)?;
    drop(mcp);
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: forked_thread.id.clone(),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    assert!(
        !mcp.pending_notification_methods()
            .iter()
            .any(|method| method == "turn/started"),
        "deferred goal should remain deferred after app-server restart"
    );
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: forked_thread.id,
            input: vec![UserInput::Text {
                text: "retry the interrupted prompt".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;

    assert!(
        !state_db
            .thread_goals()
            .has_thread_goal_continuation_deferral(forked_thread_id)
            .await?,
        "first explicit turn should consume the deferred-goal marker"
    );
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let forked_goal = state_db
        .thread_goals()
        .get_thread_goal(forked_thread_id)
        .await?
        .expect("forked goal");
    assert_eq!(forked_goal.goal_id, source_goal.goal_id);
    assert_eq!(forked_goal.objective, source_goal.objective);
    assert_eq!(forked_goal.token_budget, Some(150));
    assert_eq!(forked_goal.tokens_used, 157);
    assert!(forked_goal.time_used_seconds >= source_goal.time_used_seconds);
    assert_eq!(
        forked_goal.status,
        codex_state::ThreadGoalStatus::BudgetLimited
    );
    assert_eq!(
        state_db
            .thread_goals()
            .get_thread_goal(source_thread_id)
            .await?,
        Some(source_goal)
    );

    Ok(())
}

#[tokio::test]
async fn thread_fork_inherits_explicit_source_name_from_session_index() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let source_thread_id = ThreadId::from_string(&conversation_id)?;
    let source_name = "Renamed parent thread";
    append_thread_name(codex_home.path(), source_thread_id, source_name).await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id.clone(),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;

    let ThreadListResponse { data, .. } = list_threads(&mut mcp).await?;
    let listed = data
        .iter()
        .find(|candidate| candidate.id == thread.id)
        .expect("thread/list should include the forked thread");
    assert_eq!(listed.name.as_deref(), Some(source_name));

    Ok(())
}

#[tokio::test]
async fn thread_fork_can_load_source_by_path() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let preview = "Saved user message";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        preview,
        Some("mock_provider"),
        /*git_info*/ None,
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: "not-a-valid-thread-id".to_string(),
            path: Some(original_path),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;

    assert_ne!(thread.id, conversation_id);
    assert_eq!(thread.forked_from_id, Some(conversation_id));
    assert_eq!(thread.preview, preview);
    assert_eq!(thread.model_provider, "mock_provider");
    assert_eq!(thread.turns.len(), 1, "expected copied fork history");

    Ok(())
}

#[tokio::test]
async fn thread_fork_can_cut_before_unfinished_stored_turn() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let filename_ts = "2025-01-05T12-00-00";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let source_path = rollout_path(codex_home.path(), filename_ts, &conversation_id);
    let unfinished_turn_id = "unfinished-turn";
    append_rollout_item_to_path(
        &source_path,
        &RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: unfinished_turn_id.to_string(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
    )
    .await?;
    append_rollout_item_to_path(
        &source_path,
        &RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "Unfinished user message".to_string(),
            ..Default::default()
        })),
    )
    .await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: conversation_id.clone(),
            include_turns: true,
        })
        .await?;
    let ThreadReadResponse {
        thread: source_thread,
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(read_id)).await??;
    assert_eq!(source_thread.turns.len(), 2);
    assert_eq!(source_thread.turns[1].id, unfinished_turn_id);
    assert_eq!(source_thread.turns[1].status, TurnStatus::Interrupted);

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            before_turn_id: Some(unfinished_turn_id.to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse {
        thread: forked_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;
    assert_eq!(forked_thread.turns.len(), 1);
    assert_eq!(forked_thread.preview, "Saved user message");

    Ok(())
}

#[tokio::test]
async fn thread_fork_emits_restored_token_usage_before_next_turn() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout_with_token_usage(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            thread_source: Some(ThreadSource::User),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;

    let note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/tokenUsage/updated"),
    )
    .await??;
    let parsed: ServerNotification = note.try_into()?;
    let ServerNotification::ThreadTokenUsageUpdated(notification) = parsed else {
        panic!("expected thread/tokenUsage/updated notification");
    };

    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.turn_id, thread.turns[0].id);
    assert_eq!(notification.token_usage.total.total_tokens, 150);
    assert_eq!(notification.token_usage.total.input_tokens, 120);
    assert_eq!(notification.token_usage.total.cached_input_tokens, 20);
    assert_eq!(notification.token_usage.total.output_tokens, 30);
    assert_eq!(notification.token_usage.total.reasoning_output_tokens, 10);
    assert_eq!(notification.token_usage.last.total_tokens, 90);
    assert_eq!(notification.token_usage.model_context_window, Some(200_000));

    Ok(())
}

#[tokio::test]
async fn thread_fork_can_exclude_turns_and_skip_restored_token_usage() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout_with_token_usage(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;

    assert_eq!(thread.forked_from_id, Some(conversation_id));
    assert_eq!(thread.preview, "Saved user message");
    assert!(thread.turns.is_empty());

    let note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/tokenUsage/updated"),
    )
    .await;
    assert!(
        note.is_err(),
        "excludeTurns=true should not replay token usage"
    );

    Ok(())
}

#[tokio::test]
async fn thread_fork_tracks_thread_initialized_analytics() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!(r#"chatgpt_base_url = "{}""#, server.uri()))
        .write(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;

    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .without_managed_config()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            thread_source: Some(ThreadSource::User),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;

    let payload = wait_for_analytics_payload(&server, DEFAULT_READ_TIMEOUT).await?;
    let event = thread_initialized_event(&payload)?;
    assert_basic_thread_initialized_event(
        event,
        &thread.id,
        &thread.session_id,
        "codex",
        "mock-model",
        "forked",
        "user",
    );
    assert_eq!(
        event["event_params"]["forked_from_thread_id"],
        thread
            .forked_from_id
            .as_deref()
            .expect("forked thread has a source thread")
    );
    Ok(())
}

#[tokio::test]
async fn thread_fork_rejects_unmaterialized_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

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
async fn thread_fork_rejects_paginated_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let rollout_path = codex_home
        .path()
        .join("sessions")
        .join("2025")
        .join("01")
        .join("05")
        .join(format!(
            "rollout-2025-01-05T12-00-00-{conversation_id}.jsonl"
        ));
    let mut lines = std::fs::read_to_string(&rollout_path)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    lines[0]["payload"]["history_mode"] = json!("paginated");
    let contents = lines
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&rollout_path, format!("{contents}\n"))?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let fork_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(fork_id)),
    )
    .await??;

    assert_eq!(fork_err.error.code, -32601);
    assert_eq!(
        fork_err.error.message,
        "paginated_threads is not supported yet"
    );
    Ok(())
}

#[tokio::test]
async fn thread_fork_with_empty_path_uses_thread_id() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id.clone(),
            path: Some(std::path::PathBuf::new()),
            thread_source: Some(ThreadSource::User),
            ..Default::default()
        })
        .await?;
    let ThreadForkResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;

    assert_eq!(
        thread.forked_from_id.as_deref(),
        Some(conversation_id.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn thread_fork_surfaces_cloud_config_bundle_load_errors() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("content-type", "text/html")
                .set_body_string("<html>nope</html>"),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": { "code": "refresh_token_invalidated" }
        })))
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let model_server = create_mock_responses_server_repeating_assistant("Done").await;
    let chatgpt_base_url = format!("{}/backend-api", server.uri());
    MockResponsesConfig::new(&model_server.uri())
        .with_root_config(&format!(r#"chatgpt_base_url = "{chatgpt_base_url}""#))
        .write(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .refresh_token("stale-refresh-token")
            .plan_type("business")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123")
            .account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let refresh_token_url = format!("{}/oauth/token", server.uri());
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            (
                REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                Some(refresh_token_url.as_str()),
            ),
        ])
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id,
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
            .contains("failed to load configuration"),
        "unexpected fork error: {}",
        fork_err.error.message
    );
    assert_eq!(
        fork_err.error.data,
        Some(json!({
            "reason": "cloudConfigBundle",
            "errorCode": "Auth",
            "action": "relogin",
            "statusCode": 401,
            "detail": "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.",
        }))
    );

    Ok(())
}

#[tokio::test]
async fn thread_fork_ephemeral_remains_pathless_and_omits_listing() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let preview = "Saved user message";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        preview,
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let fork_id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: conversation_id.clone(),
            ephemeral: true,
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
    let fork_thread_id = thread.id.clone();

    assert!(
        thread.ephemeral,
        "ephemeral forks should be marked explicitly"
    );
    assert_eq!(
        thread.path, None,
        "ephemeral forks should not expose a path"
    );
    assert_eq!(thread.preview, preview);
    assert_eq!(thread.status, ThreadStatus::Idle);
    assert_eq!(thread.name, None);
    assert_eq!(thread.turns.len(), 1, "expected copied fork history");

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

    let thread_json = fork_result
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/fork result.thread must be an object");
    assert_eq!(
        thread_json.get("ephemeral").and_then(Value::as_bool),
        Some(true),
        "ephemeral forks should serialize `ephemeral: true`"
    );

    let deadline = tokio::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    let notif = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = timeout(remaining, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notif) = message else {
            continue;
        };
        if notif.method == "thread/status/changed" {
            let status_changed: ThreadStatusChangedNotification =
                serde_json::from_value(notif.params.expect("params must be present"))?;
            if status_changed.thread_id == fork_thread_id {
                anyhow::bail!(
                    "thread/fork should introduce the thread without a preceding thread/status/changed"
                );
            }
            continue;
        }
        if notif.method == "thread/started" {
            break notif;
        }
    };
    let started_params = notif.params.clone().expect("params must be present");
    let started_thread_json = started_params
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/started params.thread must be an object");
    assert_eq!(
        started_thread_json
            .get("ephemeral")
            .and_then(Value::as_bool),
        Some(true),
        "thread/started should serialize `ephemeral: true` for ephemeral forks"
    );
    assert_eq!(
        started_thread_json.get("turns"),
        Some(&json!([])),
        "thread/started must not emit copied ephemeral fork turns"
    );
    let started: ThreadStartedNotification =
        serde_json::from_value(notif.params.expect("params must be present"))?;
    let mut expected_started_thread = thread;
    expected_started_thread.turns.clear();
    assert_eq!(started.thread, expected_started_thread);

    let ThreadListResponse { data, .. } = list_threads(&mut mcp).await?;
    assert!(
        data.iter().all(|candidate| candidate.id != fork_thread_id),
        "ephemeral forks should not appear in thread/list"
    );
    assert!(
        data.iter().any(|candidate| candidate.id == conversation_id),
        "persistent source thread should remain listed"
    );

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: fork_thread_id,
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "continue".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn thread_fork_rejects_incompatible_boundaries_and_ephemeral_goal_deferral() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    for (params, expected_message) in [
        (
            ThreadForkParams {
                thread_id: thread_id.clone(),
                last_turn_id: Some("turn-1".to_string()),
                before_turn_id: Some("turn-2".to_string()),
                ..Default::default()
            },
            "`beforeTurnId` cannot be combined with `lastTurnId`",
        ),
        (
            ThreadForkParams {
                thread_id: thread_id.clone(),
                ephemeral: true,
                defer_goal_continuation: true,
                ..Default::default()
            },
            "`deferGoalContinuation` cannot be combined with `ephemeral`",
        ),
    ] {
        let fork_id = mcp.send_thread_fork_request(params).await?;
        let error = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_error_message(RequestId::Integer(fork_id)),
        )
        .await??;
        assert_eq!(error.error.message, expected_message);
    }

    Ok(())
}

#[tokio::test]
async fn pathless_ephemeral_thread_rejects_codex_home_path_after_reload() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let parent_thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Parent message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let side_thread_id = {
        let mut app_server = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_auto_env()
            .build_initialized()
            .await?;

        let fork_id = app_server
            .send_thread_fork_request(ThreadForkParams {
                thread_id: parent_thread_id,
                ephemeral: true,
                ..Default::default()
            })
            .await?;
        let ThreadForkResponse { thread, .. } =
            timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(fork_id)).await??;
        assert!(thread.ephemeral);
        assert_eq!(thread.path, None);

        let turn_id = app_server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "continue".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        let _: TurnStartResponse =
            timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(turn_id)).await??;
        timeout(
            DEFAULT_READ_TIMEOUT,
            app_server.read_stream_until_notification_message("turn/completed"),
        )
        .await??;

        thread.id
    };

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let codex_home_path = codex_home.path().to_path_buf();

    let resume_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: side_thread_id.clone(),
            path: Some(codex_home_path.clone()),
            ..Default::default()
        })
        .await?;
    let resume_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;
    assert!(
        resume_err.error.message.contains("path is a directory"),
        "unexpected resume error: {}",
        resume_err.error.message
    );
    assert!(
        !resume_err.error.message.contains("Is a directory"),
        "resume should reject the directory before rollout reading: {}",
        resume_err.error.message
    );

    let fork_id = app_server
        .send_thread_fork_request(ThreadForkParams {
            thread_id: side_thread_id,
            path: Some(codex_home_path),
            ..Default::default()
        })
        .await?;
    let fork_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(fork_id)),
    )
    .await??;
    assert!(
        fork_err.error.message.contains("path is a directory"),
        "unexpected fork error: {}",
        fork_err.error.message
    );
    assert!(
        !fork_err.error.message.contains("Is a directory"),
        "fork should reject the directory before rollout reading: {}",
        fork_err.error.message
    );

    Ok(())
}
