use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout_with_text_elements;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::rollout_path;
use app_test_support::to_response;
use chrono::Utc;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::config_types::Personality;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::fs::FileTimes;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CODEX_5_2_INSTRUCTIONS_TEMPLATE_DEFAULT: &str = "You are Codex, a coding agent based on GPT-5. You and the user share the same workspace and collaborate to achieve the user's goals.";

#[tokio::test]
async fn thread_resume_rejects_unmaterialized_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a thread.
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    // Resume should fail before the first user message materializes rollout storage.
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;
    assert!(
        resume_err
            .error
            .message
            .contains("no rollout found for thread id"),
        "unexpected resume error: {}",
        resume_err.error.message
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_returns_rollout_history() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let preview = "Saved user message";
    let text_elements = vec![TextElement::new(
        ByteRange { start: 0, end: 5 },
        Some("<note>".into()),
    )];
    let conversation_id = create_fake_rollout_with_text_elements(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        preview,
        text_elements
            .iter()
            .map(|elem| serde_json::to_value(elem).expect("serialize text element"))
            .collect(),
        Some("mock_provider"),
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } = to_response::<ThreadResumeResponse>(resume_resp)?;

    assert_eq!(thread.id, conversation_id);
    assert_eq!(thread.preview, preview);
    assert_eq!(thread.model_provider, "mock_provider");
    assert!(thread.path.as_ref().expect("thread path").is_absolute());
    assert_eq!(thread.cwd, PathBuf::from("/"));
    assert_eq!(thread.cli_version, "0.0.0");
    assert_eq!(thread.source, SessionSource::Cli);
    assert_eq!(thread.git_info, None);
    assert_eq!(thread.status, ThreadStatus::Idle);

    assert_eq!(
        thread.turns.len(),
        1,
        "expected rollouts to include one turn"
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
                    text_elements: text_elements.clone().into_iter().map(Into::into).collect(),
                }]
            );
        }
        other => panic!("expected user message item, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn thread_resume_without_overrides_does_not_change_updated_at_or_mtime() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let rollout = setup_rollout_fixture(codex_home.path(), &server.uri())?;
    let thread_id = rollout.conversation_id.clone();

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } = to_response::<ThreadResumeResponse>(resume_resp)?;

    assert_eq!(thread.updated_at, rollout.expected_updated_at);
    assert_eq!(thread.status, ThreadStatus::Idle);

    let after_modified = std::fs::metadata(&rollout.rollout_file_path)?.modified()?;
    assert_eq!(after_modified, rollout.before_modified);

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let after_turn_modified = std::fs::metadata(&rollout.rollout_file_path)?.modified()?;
    assert!(after_turn_modified > rollout.before_modified);

    Ok(())
}

#[tokio::test]
async fn thread_resume_keeps_in_flight_turn_streaming() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut primary = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, primary.initialize()).await??;

    let start_id = primary
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(seed_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    primary.clear_message_buffer();

    let mut secondary = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, secondary.initialize()).await??;

    let turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "respond with docs".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let resume_id = secondary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_ne!(resumed_thread.status, ThreadStatus::NotLoaded);

    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn thread_resume_rejects_history_when_thread_is_running() -> Result<()> {
    let server = responses::start_mock_server().await;
    let first_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let second_response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]))
    .set_delay(std::time::Duration::from_millis(500));
    let _first_response_mock = responses::mount_sse_once(&server, first_body).await;
    let _second_response_mock = responses::mount_response_once(&server, second_response).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut primary = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, primary.initialize()).await??;

    let start_id = primary
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(seed_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    primary.clear_message_buffer();

    let thread_id = thread.id.clone();
    let running_turn_request_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "keep running".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let running_turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(running_turn_request_id)),
    )
    .await??;
    let TurnStartResponse { turn: running_turn } =
        to_response::<TurnStartResponse>(running_turn_resp)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            history: Some(vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "history override".to_string(),
                }],
                end_turn: None,
                phase: None,
            }]),
            ..Default::default()
        })
        .await?;
    let resume_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;
    assert!(
        resume_err.error.message.contains("cannot resume thread")
            && resume_err.error.message.contains("with history")
            && resume_err.error.message.contains("running"),
        "unexpected resume error: {}",
        resume_err.error.message
    );

    primary
        .interrupt_turn_and_wait_for_aborted(thread_id, running_turn.id, DEFAULT_READ_TIMEOUT)
        .await?;

    Ok(())
}

#[tokio::test]
async fn thread_resume_rejects_mismatched_path_when_thread_is_running() -> Result<()> {
    let server = responses::start_mock_server().await;
    let first_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let second_response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]))
    .set_delay(std::time::Duration::from_millis(500));
    let _first_response_mock = responses::mount_sse_once(&server, first_body).await;
    let _second_response_mock = responses::mount_response_once(&server, second_response).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut primary = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, primary.initialize()).await??;

    let start_id = primary
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(seed_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    primary.clear_message_buffer();

    let thread_id = thread.id.clone();
    let running_turn_request_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "keep running".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let running_turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(running_turn_request_id)),
    )
    .await??;
    let TurnStartResponse { turn: running_turn } =
        to_response::<TurnStartResponse>(running_turn_resp)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            path: Some(PathBuf::from("/tmp/does-not-match-running-rollout.jsonl")),
            ..Default::default()
        })
        .await?;
    let resume_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;
    assert!(
        resume_err.error.message.contains("mismatched path"),
        "unexpected resume error: {}",
        resume_err.error.message
    );

    primary
        .interrupt_turn_and_wait_for_aborted(thread_id, running_turn.id, DEFAULT_READ_TIMEOUT)
        .await?;

    Ok(())
}

#[tokio::test]
async fn thread_resume_rejoins_running_thread_even_with_override_mismatch() -> Result<()> {
    let server = responses::start_mock_server().await;
    let first_response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]));
    let second_response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]))
    .set_delay(std::time::Duration::from_millis(500));
    let _response_mock =
        responses::mount_response_sequence(&server, vec![first_response, second_response]).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut primary = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, primary.initialize()).await??;

    let start_id = primary
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(seed_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    primary.clear_message_buffer();

    let running_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "keep running".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(running_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            model: Some("not-the-running-model".to_string()),
            cwd: Some("/tmp".to_string()),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, model, .. } =
        to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(model, "gpt-5.1-codex-max");
    assert_eq!(
        thread.status,
        ThreadStatus::Active {
            active_flags: vec![],
        }
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn thread_resume_with_overrides_defers_updated_at_until_turn_start() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let RestartedThreadFixture {
        mut mcp,
        thread_id,
        rollout_file_path,
    } = start_materialized_thread_and_restart(codex_home.path(), "materialize").await?;
    let expected_updated_at_rfc3339 = "2025-01-07T00:00:00Z";
    set_rollout_mtime(rollout_file_path.as_path(), expected_updated_at_rfc3339)?;
    let before_modified = std::fs::metadata(&rollout_file_path)?.modified()?;
    let expected_updated_at = chrono::DateTime::parse_from_rfc3339(expected_updated_at_rfc3339)?
        .with_timezone(&Utc)
        .timestamp();

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = to_response::<ThreadResumeResponse>(resume_resp)?;

    assert_eq!(resumed_thread.updated_at, expected_updated_at);
    assert_eq!(resumed_thread.status, ThreadStatus::Idle);

    let after_resume_modified = std::fs::metadata(&rollout_file_path)?.modified()?;
    assert_eq!(after_resume_modified, before_modified);

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: resumed_thread.id,
            input: vec![UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let after_turn_modified = std::fs::metadata(&rollout_file_path)?.modified()?;
    assert!(after_turn_modified > before_modified);

    Ok(())
}

#[tokio::test]
async fn thread_resume_fails_when_required_mcp_server_fails_to_initialize() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let rollout = setup_rollout_fixture(codex_home.path(), &server.uri())?;
    create_config_toml_with_required_broken_mcp(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: rollout.conversation_id,
            ..Default::default()
        })
        .await?;
    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;

    assert!(
        err.error
            .message
            .contains("required MCP servers failed to initialize"),
        "unexpected error message: {}",
        err.error.message
    );
    assert!(
        err.error.message.contains("required_broken"),
        "unexpected error message: {}",
        err.error.message
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_prefers_path_over_thread_id() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "materialize".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let thread_path = thread.path.clone().expect("thread path");
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: "not-a-valid-thread-id".to_string(),
            path: Some(thread_path),
            ..Default::default()
        })
        .await?;

    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed, ..
    } = to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(resumed.id, thread.id);
    assert_eq!(resumed.path, thread.path);
    assert_eq!(resumed.status, ThreadStatus::Idle);

    Ok(())
}

#[tokio::test]
async fn thread_resume_supports_history_and_overrides() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let RestartedThreadFixture {
        mut mcp, thread_id, ..
    } = start_materialized_thread_and_restart(codex_home.path(), "seed history").await?;

    let history_text = "Hello from history";
    let history = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: history_text.to_string(),
        }],
        end_turn: None,
        phase: None,
    }];

    // Resume with explicit history and override the model.
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            history: Some(history),
            model: Some("mock-model".to_string()),
            model_provider: Some("mock_provider".to_string()),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed,
        model_provider,
        ..
    } = to_response::<ThreadResumeResponse>(resume_resp)?;
    assert!(!resumed.id.is_empty());
    assert_eq!(model_provider, "mock_provider");
    assert_eq!(resumed.preview, history_text);
    assert_eq!(resumed.status, ThreadStatus::Idle);

    Ok(())
}

struct RestartedThreadFixture {
    mcp: McpProcess,
    thread_id: String,
    rollout_file_path: PathBuf,
}

async fn start_materialized_thread_and_restart(
    codex_home: &Path,
    seed_text: &str,
) -> Result<RestartedThreadFixture> {
    let mut first_mcp = McpProcess::new(codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, first_mcp.initialize()).await??;

    let start_id = first_mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        first_mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let materialize_turn_id = first_mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: seed_text.to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        first_mcp.read_stream_until_response_message(RequestId::Integer(materialize_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        first_mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let thread_id = thread.id;
    let rollout_file_path = thread
        .path
        .ok_or_else(|| anyhow::anyhow!("thread path missing from thread/start response"))?;

    drop(first_mcp);

    let mut second_mcp = McpProcess::new(codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, second_mcp.initialize()).await??;

    Ok(RestartedThreadFixture {
        mcp: second_mcp,
        thread_id,
        rollout_file_path,
    })
}

#[tokio::test]
async fn thread_resume_accepts_personality_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let first_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let second_body = responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]);
    let response_mock = responses::mount_sse_sequence(&server, vec![first_body, second_body]).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut primary = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, primary.initialize()).await??;

    let start_id = primary
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let materialize_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(materialize_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let mut secondary = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, secondary.initialize()).await??;

    let resume_id = secondary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            model: Some("gpt-5.2-codex".to_string()),
            personality: Some(Personality::Friendly),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let resume: ThreadResumeResponse = to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(resume.thread.status, ThreadStatus::Idle);

    let turn_id = secondary
        .send_turn_start_request(TurnStartParams {
            thread_id: resume.thread.id,
            input: vec![UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    let request = requests
        .last()
        .expect("expected request for resumed thread turn");
    let developer_texts = request.message_input_texts("developer");
    assert!(
        developer_texts
            .iter()
            .any(|text| text.contains("<personality_spec>")),
        "expected a personality update message in developer input, got {developer_texts:?}"
    );
    let instructions_text = request.instructions_text();
    assert!(
        instructions_text.contains(CODEX_5_2_INSTRUCTIONS_TEMPLATE_DEFAULT),
        "expected default base instructions from history, got {instructions_text:?}"
    );

    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "gpt-5.2-codex"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
personality = true

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

fn create_config_toml_with_required_broken_mcp(
    codex_home: &std::path::Path,
    server_uri: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "gpt-5.2-codex"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
personality = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[mcp_servers.required_broken]
command = "codex-definitely-not-a-real-binary"
required = true
"#
        ),
    )
}

#[allow(dead_code)]
fn set_rollout_mtime(path: &Path, updated_at_rfc3339: &str) -> Result<()> {
    let parsed = chrono::DateTime::parse_from_rfc3339(updated_at_rfc3339)?.with_timezone(&Utc);
    let times = FileTimes::new().set_modified(parsed.into());
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .set_times(times)?;
    Ok(())
}

struct RolloutFixture {
    conversation_id: String,
    rollout_file_path: PathBuf,
    before_modified: std::time::SystemTime,
    expected_updated_at: i64,
}

fn setup_rollout_fixture(codex_home: &Path, server_uri: &str) -> Result<RolloutFixture> {
    create_config_toml(codex_home, server_uri)?;

    let preview = "Saved user message";
    let filename_ts = "2025-01-05T12-00-00";
    let meta_rfc3339 = "2025-01-05T12:00:00Z";
    let expected_updated_at_rfc3339 = "2025-01-07T00:00:00Z";
    let conversation_id = create_fake_rollout_with_text_elements(
        codex_home,
        filename_ts,
        meta_rfc3339,
        preview,
        Vec::new(),
        Some("mock_provider"),
        None,
    )?;
    let rollout_file_path = rollout_path(codex_home, filename_ts, &conversation_id);
    set_rollout_mtime(rollout_file_path.as_path(), expected_updated_at_rfc3339)?;
    let before_modified = std::fs::metadata(&rollout_file_path)?.modified()?;
    let expected_updated_at = chrono::DateTime::parse_from_rfc3339(expected_updated_at_rfc3339)?
        .with_timezone(&Utc)
        .timestamp();

    Ok(RolloutFixture {
        conversation_id,
        rollout_file_path,
        before_modified,
        expected_updated_at,
    })
}
