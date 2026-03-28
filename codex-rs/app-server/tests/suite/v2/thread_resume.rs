use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::create_apply_patch_sse_response;
use app_test_support::create_fake_rollout_with_text_elements;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::rollout_path;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use chrono::Utc;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_login::AuthCredentialsStoreMode;
use codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use codex_protocol::ThreadId;
use codex_protocol::config_types::Personality;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource as RolloutSessionSource;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
use codex_state::StateRuntime;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs::FileTimes;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;
use tokio::time::sleep;
use tokio::time::timeout;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::analytics::assert_basic_thread_initialized_event;
use super::analytics::enable_analytics_capture;
use super::analytics::thread_initialized_event;
use super::analytics::wait_for_analytics_payload;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
#[cfg(not(windows))]
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
async fn thread_resume_tracks_thread_initialized_analytics() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &server.uri(),
        /*general_analytics_enabled*/ true,
    )?;
    enable_analytics_capture(&server, codex_home.path()).await?;

    let conversation_id = create_fake_rollout_with_text_elements(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Vec::new(),
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } = to_response::<ThreadResumeResponse>(resume_resp)?;

    let payload = wait_for_analytics_payload(&server, DEFAULT_READ_TIMEOUT).await?;
    let event = thread_initialized_event(&payload)?;
    assert_basic_thread_initialized_event(event, &thread.id, "gpt-5.2-codex", "resumed");
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
        /*git_info*/ None,
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
async fn thread_resume_prefers_persisted_git_metadata_for_local_threads() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let config_toml = codex_home.path().join("config.toml");
    std::fs::write(
        &config_toml,
        format!(
            r#"
model = "gpt-5.2-codex"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
personality = true
sqlite = true

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

    let repo_path = codex_home.path().join("repo");
    std::fs::create_dir_all(&repo_path)?;
    assert!(
        Command::new("git")
            .args(["init"])
            .arg(&repo_path)
            .status()?
            .success()
    );
    assert!(
        Command::new("git")
            .current_dir(&repo_path)
            .args(["checkout", "-B", "master"])
            .status()?
            .success()
    );
    assert!(
        Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test User"])
            .status()?
            .success()
    );
    assert!(
        Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@example.com"])
            .status()?
            .success()
    );
    std::fs::write(repo_path.join("README.md"), "test\n")?;
    assert!(
        Command::new("git")
            .current_dir(&repo_path)
            .args(["add", "README.md"])
            .status()?
            .success()
    );
    assert!(
        Command::new("git")
            .current_dir(&repo_path)
            .args(["commit", "-m", "initial"])
            .status()?
            .success()
    );
    let head_branch = Command::new("git")
        .current_dir(&repo_path)
        .args(["branch", "--show-current"])
        .output()?;
    assert_eq!(
        String::from_utf8(head_branch.stdout)?.trim(),
        "master",
        "test repo should stay on master to verify resume ignores live HEAD"
    );

    let thread_id = Uuid::new_v4().to_string();
    let conversation_id = ThreadId::from_string(&thread_id)?;
    let rollout_path = rollout_path(codex_home.path(), "2025-01-05T12-00-00", &thread_id);
    let rollout_dir = rollout_path.parent().expect("rollout parent directory");
    std::fs::create_dir_all(rollout_dir)?;
    let session_meta = SessionMeta {
        id: conversation_id,
        forked_from_id: None,
        timestamp: "2025-01-05T12:00:00Z".to_string(),
        cwd: repo_path.clone(),
        originator: "codex".to_string(),
        cli_version: "0.0.0".to_string(),
        source: RolloutSessionSource::Cli,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
        model_provider: Some("mock_provider".to_string()),
        base_instructions: None,
        dynamic_tools: None,
        memory_mode: None,
    };
    std::fs::write(
        &rollout_path,
        [
            json!({
                "timestamp": "2025-01-05T12:00:00Z",
                "type": "session_meta",
                "payload": serde_json::to_value(SessionMetaLine {
                    meta: session_meta,
                    git: None,
                })?,
            })
            .to_string(),
            json!({
                "timestamp": "2025-01-05T12:00:00Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Saved user message"}]
                }
            })
            .to_string(),
            json!({
                "timestamp": "2025-01-05T12:00:00Z",
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": "Saved user message",
                    "kind": "plain"
                }
            })
            .to_string(),
        ]
        .join("\n")
            + "\n",
    )?;
    let state_db =
        StateRuntime::init(codex_home.path().to_path_buf(), "mock_provider".into()).await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            git_info: Some(ThreadMetadataGitInfoUpdateParams {
                sha: None,
                branch: Some(Some("feature/pr-branch".to_string())),
                origin_url: None,
            }),
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_id)),
    )
    .await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } = to_response::<ThreadResumeResponse>(resume_resp)?;

    assert_eq!(
        thread
            .git_info
            .as_ref()
            .and_then(|git| git.branch.as_deref()),
        Some("feature/pr-branch")
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_and_read_interrupt_incomplete_rollout_turn_when_thread_is_idle() -> Result<()>
{
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let filename_ts = "2025-01-05T12-00-00";
    let meta_rfc3339 = "2025-01-05T12:00:00Z";
    let conversation_id = create_fake_rollout_with_text_elements(
        codex_home.path(),
        filename_ts,
        meta_rfc3339,
        "Saved user message",
        Vec::new(),
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let rollout_file_path = rollout_path(codex_home.path(), filename_ts, &conversation_id);
    let persisted_rollout = std::fs::read_to_string(&rollout_file_path)?;
    let turn_id = "incomplete-turn";
    let appended_rollout = [
        json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.to_string(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }))?,
        })
        .to_string(),
        json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(EventMsg::AgentMessage(AgentMessageEvent {
                message: "Still running".to_string(),
                phase: None,
                memory_citation: None,
            }))?,
        })
        .to_string(),
    ]
    .join("\n");
    std::fs::write(
        &rollout_file_path,
        format!("{persisted_rollout}{appended_rollout}\n"),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } = to_response::<ThreadResumeResponse>(resume_resp)?;

    assert_eq!(thread.status, ThreadStatus::Idle);
    assert_eq!(thread.turns.len(), 2);
    assert_eq!(thread.turns[0].status, TurnStatus::Completed);
    assert_eq!(thread.turns[1].id, turn_id);
    assert_eq!(thread.turns[1].status, TurnStatus::Interrupted);

    let second_resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let second_resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_resume_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed_again,
        ..
    } = to_response::<ThreadResumeResponse>(second_resume_resp)?;

    assert_eq!(resumed_again.status, ThreadStatus::Idle);
    assert_eq!(resumed_again.turns.len(), 2);
    assert_eq!(resumed_again.turns[1].id, turn_id);
    assert_eq!(resumed_again.turns[1].status, TurnStatus::Interrupted);

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: resumed_again.id,
            include_turns: true,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse {
        thread: read_thread,
    } = to_response::<ThreadReadResponse>(read_resp)?;

    assert_eq!(read_thread.status, ThreadStatus::Idle);
    assert_eq!(read_thread.turns.len(), 2);
    assert_eq!(read_thread.turns[1].id, turn_id);
    assert_eq!(read_thread.turns[1].status, TurnStatus::Interrupted);

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
    // The running-thread resume response is queued onto the thread listener task.
    // If the in-flight turn completes before that queued command runs, the response
    // can legitimately observe the thread as idle.
    match &thread.status {
        ThreadStatus::Active { active_flags } => assert!(active_flags.is_empty()),
        ThreadStatus::Idle => {}
        status => panic!("unexpected thread status after running resume: {status:?}"),
    }

    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn thread_resume_replays_pending_command_execution_request_approval() -> Result<()> {
    let responses = vec![
        create_final_assistant_message_sse_response("seeded")?,
        create_shell_command_sse_response(
            fast_shell_command(),
            /*workdir*/ None,
            Some(1000),
            "call-1",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
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
                text: "run command".to_string(),
                text_elements: Vec::new(),
            }],
            approval_policy: Some(AskForApproval::UnlessTrusted),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(running_turn_id)),
    )
    .await??;

    let original_request = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { .. } = &original_request else {
        panic!("expected CommandExecutionRequestApproval request, got {original_request:?}");
    };

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(resumed_thread.id, thread.id);
    assert!(
        resumed_thread
            .turns
            .iter()
            .any(|turn| matches!(turn.status, TurnStatus::InProgress))
    );

    let replayed_request = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_request_message(),
    )
    .await??;
    pretty_assertions::assert_eq!(replayed_request, original_request);

    let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = replayed_request else {
        panic!("expected CommandExecutionRequestApproval request");
    };
    primary
        .send_response(
            request_id,
            serde_json::to_value(CommandExecutionRequestApprovalResponse {
                decision: CommandExecutionApprovalDecision::Accept,
            })?,
        )
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    wait_for_mock_request_count(&server, /*expected*/ 3).await?;

    Ok(())
}

#[tokio::test]
async fn thread_resume_replays_pending_file_change_request_approval() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let patch = r#"*** Begin Patch
*** Add File: README.md
+new line
*** End Patch
"#;
    let responses = vec![
        create_final_assistant_message_sse_response("seeded")?,
        create_apply_patch_sse_response(patch, "patch-call")?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    create_config_toml(&codex_home, &server.uri())?;

    let mut primary = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, primary.initialize()).await??;

    let start_id = primary
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1-codex-max".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
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
            cwd: Some(workspace.clone()),
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
                text: "apply patch".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            approval_policy: Some(AskForApproval::UnlessTrusted),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(running_turn_id)),
    )
    .await??;

    let original_started = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification = primary
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification =
                serde_json::from_value(notification.params.clone().expect("item/started params"))?;
            if let ThreadItem::FileChange { .. } = started.item {
                return Ok::<ThreadItem, anyhow::Error>(started.item);
            }
        }
    })
    .await??;
    let expected_readme_path = workspace.join("README.md");
    let expected_file_change = ThreadItem::FileChange {
        id: "patch-call".to_string(),
        changes: vec![codex_app_server_protocol::FileUpdateChange {
            path: expected_readme_path.to_string_lossy().into_owned(),
            kind: PatchChangeKind::Add,
            diff: "new line\n".to_string(),
        }],
        status: PatchApplyStatus::InProgress,
    };
    assert_eq!(original_started, expected_file_change);

    let original_request = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::FileChangeRequestApproval { .. } = &original_request else {
        panic!("expected FileChangeRequestApproval request, got {original_request:?}");
    };
    primary.clear_message_buffer();

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(resumed_thread.id, thread.id);
    assert!(
        resumed_thread
            .turns
            .iter()
            .any(|turn| matches!(turn.status, TurnStatus::InProgress))
    );

    let replayed_request = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_request_message(),
    )
    .await??;
    assert_eq!(replayed_request, original_request);

    let ServerRequest::FileChangeRequestApproval { request_id, .. } = replayed_request else {
        panic!("expected FileChangeRequestApproval request");
    };
    primary
        .send_response(
            request_id,
            serde_json::to_value(FileChangeRequestApprovalResponse {
                decision: FileChangeApprovalDecision::Accept,
            })?,
        )
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    wait_for_mock_request_count(&server, /*expected*/ 3).await?;

    Ok(())
}

fn fast_shell_command() -> Vec<String> {
    if cfg!(windows) {
        vec![
            "cmd".to_string(),
            "/d".to_string(),
            "/c".to_string(),
            "echo 42".to_string(),
        ]
    } else {
        vec![
            "python3".to_string(),
            "-c".to_string(),
            "print(42)".to_string(),
        ]
    }
}

async fn wait_for_mock_request_count(server: &MockServer, expected: usize) -> Result<()> {
    let deadline = tokio::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    loop {
        let requests = server
            .received_requests()
            .await
            .ok_or_else(|| anyhow::anyhow!("failed to fetch received requests"))?;
        if requests.len() >= expected {
            return Ok(());
        }

        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!(
                "expected at least {expected} mock requests, observed {}",
                requests.len()
            );
        }

        sleep(std::time::Duration::from_millis(50)).await;
    }
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
async fn thread_resume_surfaces_cloud_requirements_load_errors() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/requirements"))
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
    create_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &model_server.uri(),
        &chatgpt_base_url,
        /*general_analytics_enabled*/ false,
    )?;
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
    let conversation_id = create_fake_rollout_with_text_elements(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Vec::new(),
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let refresh_token_url = format!("{}/oauth/token", server.uri());
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("OPENAI_API_KEY", None),
            (
                REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                Some(refresh_token_url.as_str()),
            ),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;

    assert!(
        err.error.message.contains("failed to load configuration"),
        "unexpected error message: {}",
        err.error.message
    );
    assert_eq!(
        err.error.data,
        Some(json!({
            "reason": "cloudRequirements",
            "errorCode": "Auth",
            "action": "relogin",
            "statusCode": 401,
            "detail": "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.",
        }))
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
general_analytics = true

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

fn create_config_toml_with_chatgpt_base_url(
    codex_home: &std::path::Path,
    server_uri: &str,
    chatgpt_base_url: &str,
    general_analytics_enabled: bool,
) -> std::io::Result<()> {
    let general_analytics_toml = if general_analytics_enabled {
        "\ngeneral_analytics = true".to_string()
    } else {
        String::new()
    };
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "gpt-5.2-codex"
approval_policy = "never"
sandbox_mode = "read-only"
chatgpt_base_url = "{chatgpt_base_url}"

model_provider = "mock_provider"

[features]
personality = true
{general_analytics_toml}

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
        /*git_info*/ None,
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
