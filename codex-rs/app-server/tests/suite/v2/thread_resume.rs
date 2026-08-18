use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_apply_patch_sse_response;
use app_test_support::create_fake_paginated_rollout;
use app_test_support::create_fake_rollout;
use app_test_support::create_fake_rollout_with_text_elements;
use app_test_support::create_fake_rollout_with_token_usage;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::rollout_path;
use app_test_support::test_absolute_path;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use chrono::Utc;
use codex_app_server_protocol::ActivePermissionProfile;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpToolCallAppContext;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy as AppSandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadGoalClearResponse;
use codex_app_server_protocol::ThreadGoalSetResponse;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeInitialTurnsPageParams;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadSettingsUpdateResponse;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_features::Feature;
use codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::Settings;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ImageGenerationEndEvent;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource as RolloutSessionSource;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
use codex_rollout::CompactedItem;
use codex_rollout::RolloutItem;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::read_session_meta_line;
use codex_state::StateRuntime;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathExt;
use codex_utils_path_uri::LegacyAppPathString;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::skip_if_wine_exec;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs::FileTimes;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::time::timeout;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::analytics::assert_basic_thread_initialized_event;
use super::analytics::mount_analytics_capture;
use super::analytics::thread_initialized_event;
use super::analytics::wait_for_analytics_payload;
use super::analytics::wait_for_goal_event;
use super::analytics::wait_for_matching_analytics_event;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CODEX_5_2_INSTRUCTIONS_TEMPLATE_DEFAULT: &str = "You are Codex, a coding agent based on GPT-5. You and the user share the same workspace and collaborate to achieve the user's goals.";

#[tokio::test]
async fn thread_resume_paginated_model_context_preserves_original_metadata() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let conversation_id = create_fake_paginated_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let path = rollout_path(codex_home.path(), "2025-01-05T12-00-00", &conversation_id);
    append_rollout_item_to_path(
        &path,
        &RolloutItem::Compacted(CompactedItem {
            message: "compacted history".to_string(),
            replacement_history: Some(Vec::new()),
            mcp_resource_origins: None,
            window_number: Some(1),
            first_window_id: None,
            previous_window_id: None,
            window_id: None,
        }),
    )
    .await?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed, ..
    } = timeout(DEFAULT_READ_TIMEOUT, primary.read_response(resume_id)).await??;
    assert_eq!(resumed.id, conversation_id);
    assert_eq!(resumed.history_mode, ThreadHistoryMode::Paginated);
    assert_eq!(resumed.preview, "Saved user message");
    assert!(resumed.turns.is_empty());

    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: conversation_id.clone(),
            input: vec![UserInput::Text {
                text: "bounded suffix user message".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;
    timeout(DEFAULT_READ_TIMEOUT, primary.shutdown_gracefully()).await??;

    let mut secondary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let resume_id = secondary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id.clone(),
            path: Some(path),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed, ..
    } = timeout(DEFAULT_READ_TIMEOUT, secondary.read_response(resume_id)).await??;
    assert_eq!(resumed.preview, "Saved user message");
    assert!(resumed.turns.is_empty());

    timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: conversation_id.clone(),
            input: vec![UserInput::Text {
                text: "resumed user message".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;

    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    let metadata = state_db
        .get_thread(ThreadId::from_string(&conversation_id)?)
        .await?
        .expect("thread metadata should exist");
    assert_eq!(
        (
            metadata.preview.as_deref(),
            metadata.title.as_str(),
            metadata.first_user_message.as_deref(),
        ),
        (
            Some("Saved user message"),
            "Saved user message",
            Some("Saved user message"),
        )
    );
    Ok(())
}

#[tokio::test]
async fn thread_resume_rejects_legacy_writer_owned_by_another_process() -> Result<()> {
    assert_thread_resume_rejects_writer_owned_by_another_process(ThreadHistoryMode::Legacy).await
}

#[tokio::test]
async fn thread_resume_rejects_paginated_writer_owned_by_another_process() -> Result<()> {
    assert_thread_resume_rejects_writer_owned_by_another_process(ThreadHistoryMode::Paginated).await
}

async fn assert_thread_resume_rejects_writer_owned_by_another_process(
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = primary
        .start_thread(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            history_mode: Some(history_mode),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "first writer".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;

    let secondary_sqlite_home = TempDir::new()?;
    let secondary_sqlite_home_path = secondary_sqlite_home.path().to_string_lossy();
    let mut secondary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[(
            "CODEX_SQLITE_HOME",
            Some(secondary_sqlite_home_path.as_ref()),
        )])
        .build_initialized()
        .await?;
    let resume_id = secondary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        format!("thread {} already has an active writer", thread.id)
    );

    timeout(DEFAULT_READ_TIMEOUT, primary.shutdown_gracefully()).await??;

    let next_resume_id = secondary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let _: ThreadResumeResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.read_response(next_resume_id),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        secondary.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "second writer".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;

    Ok(())
}

fn normalized_existing_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    Ok(AbsolutePathBuf::from_absolute_path(path.as_ref().canonicalize()?)?.into_path_buf())
}

async fn wait_for_responses_request_count(
    server: &wiremock::MockServer,
    expected_count: usize,
) -> Result<()> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let Some(requests) = server.received_requests().await else {
                anyhow::bail!("wiremock did not record requests");
            };
            let responses_request_count = requests
                .iter()
                .filter(|request| {
                    request.method == "POST" && request.url.path().ends_with("/responses")
                })
                .count();
            if responses_request_count == expected_count {
                return Ok::<(), anyhow::Error>(());
            }
            if responses_request_count > expected_count {
                anyhow::bail!(
                    "expected exactly {expected_count} /responses requests, got {responses_request_count}"
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}

#[tokio::test]
async fn thread_resume_rejects_unmaterialized_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    // Start a thread.
    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

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
async fn thread_resume_with_empty_path_uses_running_thread_id() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "materialize rollout".to_string(),
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

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            path: Some(PathBuf::new()),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(resumed.id, thread.id);
    Ok(())
}

#[tokio::test]
async fn thread_resume_running_thread_uses_cached_instruction_sources() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "cached instruction-source fixture is outside the selected remote cwd"
    );

    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let workspace = TempDir::new()?;
    let project_agents = workspace.path().join("AGENTS.md");
    std::fs::write(&project_agents, "project instructions")?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        // TODO(anp): Move the cached instruction-source fixture into the auto environment cwd.
        .without_auto_env()
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse {
        thread,
        instruction_sources,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;
    let project_agents = AbsolutePathBuf::try_from(project_agents)?;
    let project_agents_source = LegacyAppPathString::from_abs_path(&project_agents);
    assert_eq!(instruction_sources, vec![project_agents_source.clone()]);

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "materialize rollout".to_string(),
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

    std::fs::remove_file(project_agents.as_path())?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        instruction_sources,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(instruction_sources, vec![project_agents_source]);

    Ok(())
}

#[tokio::test]
async fn turn_start_updates_runtime_workspace_roots_for_loaded_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let extra_root_tmp = TempDir::new()?;
    let extra_root = extra_root_tmp.path().join("extra-root");
    std::fs::create_dir_all(&extra_root)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            runtime_workspace_roots: Some(vec![
                AbsolutePathBuf::from_absolute_path(&extra_root)?,
                AbsolutePathBuf::from_absolute_path(extra_root.join("."))?,
            ]),
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

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        runtime_workspace_roots,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(
        runtime_workspace_roots,
        vec![AbsolutePathBuf::from_absolute_path(extra_root)?]
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_preserves_persisted_approvals_reviewer() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let thread_id = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_auto_env()
            .build_initialized()
            .await?;

        let start_id = mcp
            .send_thread_start_request(ThreadStartParams {
                model: Some("gpt-5.4".to_string()),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                ..Default::default()
            })
            .await?;
        let ThreadStartResponse { thread, .. } =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

        let turn_id = mcp
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "materialize this thread".to_string(),
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

        thread.id
    };

    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        config.replace(
            "approval_policy = \"never\"\n",
            "approval_policy = \"never\"\napprovals_reviewer = \"user\"\n",
        ),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        approvals_reviewer, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(approvals_reviewer, ApprovalsReviewer::AutoReview);

    Ok(())
}

#[tokio::test]
async fn thread_resume_restores_persisted_approval_policy() -> Result<()> {
    assert_thread_resume_approval_policy(
        ThreadHistoryMode::Legacy,
        /*approval_policy*/ None,
        AskForApproval::Never,
    )
    .await
}

#[tokio::test]
async fn paginated_thread_resume_restores_persisted_approval_policy() -> Result<()> {
    assert_thread_resume_approval_policy(
        ThreadHistoryMode::Paginated,
        /*approval_policy*/ None,
        AskForApproval::Never,
    )
    .await
}

#[tokio::test]
async fn thread_resume_approval_policy_override_wins_over_persisted_policy() -> Result<()> {
    assert_thread_resume_approval_policy(
        ThreadHistoryMode::Legacy,
        Some(AskForApproval::OnRequest),
        AskForApproval::OnRequest,
    )
    .await
}

async fn assert_thread_resume_approval_policy(
    history_mode: ThreadHistoryMode,
    approval_policy: Option<AskForApproval>,
    expected_approval_policy: AskForApproval,
) -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
model = "gpt-5.4"
approval_policy = "never"
model_provider = "mock_provider"

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

    let thread_id = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_auto_env()
            .build_initialized()
            .await?;

        let start_id = mcp
            .send_thread_start_request(ThreadStartParams {
                model: Some("gpt-5.4".to_string()),
                history_mode: Some(history_mode),
                ..Default::default()
            })
            .await?;
        let ThreadStartResponse { thread, .. } =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

        let turn_id = mcp
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "materialize this thread".to_string(),
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

        thread.id
    };

    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        config.replace(
            "approval_policy = \"never\"",
            "approval_policy = \"on-request\"",
        ),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            approval_policy,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        approval_policy, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(approval_policy, expected_approval_policy);
    Ok(())
}

#[tokio::test]
async fn thread_resume_preserves_goal_first_and_fork_approvals_reviewer() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;

    let (thread_id, fork_thread_id) = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_managed_config()
            .build_initialized()
            .await?;

        let start_id = mcp
            .send_thread_start_request_with_auto_env(ThreadStartParams {
                model: Some("gpt-5.2-codex".to_string()),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                ..Default::default()
            })
            .await?;
        let ThreadStartResponse { thread, .. } =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;
        let rollout_path = thread.path.clone().expect("thread path");

        for objective in [
            "keep auto review after restart",
            "still keep auto review after restart",
        ] {
            let goal_id = mcp
                .send_raw_request(
                    "thread/goal/set",
                    Some(json!({
                        "threadId": thread.id,
                        "objective": objective,
                        "status": "paused",
                    })),
                )
                .await?;
            let _: ThreadGoalSetResponse =
                timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_notification_message("thread/goal/updated"),
            )
            .await??;
        }

        let persisted_rollout = std::fs::read_to_string(rollout_path)?;
        assert_eq!(
            persisted_rollout
                .matches(r#""type":"thread_settings_applied""#)
                .count(),
            1
        );

        let fork_id = mcp
            .send_thread_fork_request(ThreadForkParams {
                thread_id: thread.id.clone(),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                ..Default::default()
            })
            .await?;
        let ThreadForkResponse {
            thread: fork_thread,
            approvals_reviewer,
            ..
        } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(fork_id)).await??;
        assert_eq!(approvals_reviewer, ApprovalsReviewer::User);

        (thread.id, fork_thread.id)
    };

    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        config.replace(
            "approval_policy = \"never\"\n",
            "approval_policy = \"never\"\napprovals_reviewer = \"user\"\n",
        ),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    for (thread_id, expected_reviewer) in [
        (thread_id, ApprovalsReviewer::AutoReview),
        (fork_thread_id, ApprovalsReviewer::User),
    ] {
        let resume_id = mcp
            .send_thread_resume_request(ThreadResumeParams {
                thread_id,
                ..Default::default()
            })
            .await?;
        let ThreadResumeResponse {
            approvals_reviewer, ..
        } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

        assert_eq!(approvals_reviewer, expected_reviewer);
    }

    Ok(())
}

#[tokio::test]
async fn thread_resume_preserves_acknowledged_model_effort_and_approvals_reviewer_update()
-> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let updated_workspace = TempDir::new()?;
    let persisted_cwd = normalized_existing_path(updated_workspace.path())?;
    let live_cwd = normalized_existing_path(codex_home.path())?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config_toml = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config_toml.replace(
            "model = \"gpt-5.4\"",
            "model = \"gpt-5.4\"\nmodel_reasoning_effort = \"high\"",
        ),
    )?;

    let thread_id = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build_initialized()
            .await?;

        let start_id = mcp
            .send_thread_start_request_with_auto_env(ThreadStartParams {
                model: Some("gpt-5.4".to_string()),
                ..Default::default()
            })
            .await?;
        let ThreadStartResponse { thread, .. } =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

        let turn_id = mcp
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "materialize this thread".to_string(),
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

        let update_id = mcp
            .send_thread_settings_update_request(ThreadSettingsUpdateParams {
                thread_id: thread.id.clone(),
                model: Some("gpt-5.2-codex".to_string()),
                effort: Some(ReasoningEffort::Ultra),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                cwd: Some(persisted_cwd.clone()),
                ..Default::default()
            })
            .await?;
        let _: ThreadSettingsUpdateResponse =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(update_id)).await??;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("thread/settings/updated"),
        )
        .await??;

        let read_id = mcp
            .send_thread_read_request(ThreadReadParams {
                thread_id: thread.id.clone(),
                include_turns: false,
            })
            .await?;
        let ThreadReadResponse { thread: read } =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(read_id)).await??;
        assert_eq!(read.cwd.as_path(), persisted_cwd);

        let list_id = mcp
            .send_raw_request(
                "thread/list",
                Some(json!({ "cwd": persisted_cwd, "useStateDbOnly": true })),
            )
            .await?;
        let ThreadListResponse { data, .. } =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(list_id)).await??;
        assert_eq!(
            data.iter()
                .find(|listed| listed.id == thread.id)
                .map(|listed| &listed.cwd),
            Some(&read.cwd)
        );

        thread.id
    };

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            cwd: Some(live_cwd.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread,
        cwd,
        model,
        reasoning_effort,
        approvals_reviewer,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(model, "gpt-5.2-codex");
    assert_eq!(reasoning_effort, Some(ReasoningEffort::Ultra));
    assert_eq!(approvals_reviewer, ApprovalsReviewer::AutoReview);
    assert_eq!(thread.cwd.as_path(), persisted_cwd);
    assert_eq!(cwd.as_path(), live_cwd);

    let update_id = mcp
        .send_thread_settings_update_request(ThreadSettingsUpdateParams {
            thread_id: thread_id.clone(),
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Default,
                settings: Settings {
                    model: "gpt-5.2-codex".to_string(),
                    reasoning_effort: None,
                    developer_instructions: None,
                },
            }),
            ..Default::default()
        })
        .await?;
    let _: ThreadSettingsUpdateResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(update_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/settings/updated"),
    )
    .await??;
    drop(mcp);

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        reasoning_effort, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(reasoning_effort, None);

    Ok(())
}

#[tokio::test]
async fn cold_resume_reresolves_persisted_active_permission_profile() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    for history_mode in [ThreadHistoryMode::Legacy, ThreadHistoryMode::Paginated] {
        let codex_home = TempDir::new()?;
        let previous_workspace_root = TempDir::new()?;
        write_dev_permission_config(&server.uri(), codex_home.path(), ":workspace")?;
        let thread_id = {
            let mut mcp = TestAppServer::builder()
                .with_codex_home(codex_home.path())
                .without_managed_config()
                .build_initialized()
                .await?;
            let thread_id = materialize_dev_permission_thread(&mut mcp, history_mode).await?;
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.start_turn_and_wait_for_completion(TurnStartParams {
                    thread_id: thread_id.clone(),
                    runtime_workspace_roots: Some(vec![AbsolutePathBuf::from_absolute_path(
                        previous_workspace_root.path(),
                    )?]),
                    input: vec![UserInput::Text {
                        text: "update runtime workspace roots".to_string(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                }),
            )
            .await??;
            thread_id
        };

        write_dev_permission_config(&server.uri(), codex_home.path(), ":read-only")?;
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_managed_config()
            .build_initialized()
            .await?;
        let resume_id = mcp
            .send_thread_resume_request(ThreadResumeParams {
                thread_id,
                ..Default::default()
            })
            .await?;
        let ThreadResumeResponse {
            sandbox,
            active_permission_profile,
            runtime_workspace_roots,
            ..
        } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

        assert!(matches!(sandbox, AppSandboxPolicy::ReadOnly { .. }));
        assert_eq!(
            active_permission_profile,
            Some(ActivePermissionProfile {
                id: "dev".to_string(),
                extends: Some(BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
            })
        );
        assert!(
            !runtime_workspace_roots.contains(&AbsolutePathBuf::from_absolute_path(
                previous_workspace_root.path(),
            )?)
        );
    }
    Ok(())
}

#[tokio::test]
async fn cold_resume_with_removed_permission_profile_uses_configured_default() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    for history_mode in [ThreadHistoryMode::Legacy, ThreadHistoryMode::Paginated] {
        let codex_home = TempDir::new()?;
        write_dev_permission_config(&server.uri(), codex_home.path(), ":workspace")?;
        let thread_id = {
            let mut mcp = TestAppServer::builder()
                .with_codex_home(codex_home.path())
                .without_managed_config()
                .build_initialized()
                .await?;
            materialize_dev_permission_thread(&mut mcp, history_mode).await?
        };

        MockResponsesConfig::new(&server.uri())
            .with_root_config(&format!(
                "default_permissions = \"{BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS}\""
            ))
            .write(codex_home.path())?;
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_managed_config()
            .build_initialized()
            .await?;
        let resume_id = mcp
            .send_thread_resume_request(ThreadResumeParams {
                thread_id,
                ..Default::default()
            })
            .await?;
        let ThreadResumeResponse {
            sandbox,
            active_permission_profile,
            ..
        } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

        assert!(matches!(sandbox, AppSandboxPolicy::DangerFullAccess));
        assert_eq!(
            active_permission_profile,
            Some(ActivePermissionProfile::new(
                BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
            ))
        );
    }
    Ok(())
}

#[tokio::test]
async fn cold_resume_permission_overrides_win_over_persisted_profile() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_dev_permission_config(&server.uri(), codex_home.path(), ":workspace")?;
    let thread_id = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_managed_config()
            .build_initialized()
            .await?;
        materialize_dev_permission_thread(&mut mcp, ThreadHistoryMode::Legacy).await?
    };

    for params in [
        ThreadResumeParams {
            thread_id: thread_id.clone(),
            sandbox: Some(SandboxMode::ReadOnly),
            ..Default::default()
        },
        ThreadResumeParams {
            thread_id: thread_id.clone(),
            permissions: Some(BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
            ..Default::default()
        },
        ThreadResumeParams {
            thread_id: thread_id.clone(),
            config: Some(std::collections::HashMap::from([(
                "default_permissions".to_string(),
                json!(BUILT_IN_PERMISSION_PROFILE_READ_ONLY),
            )])),
            ..Default::default()
        },
    ] {
        let expected_active_permission_profile = params
            .sandbox
            .is_none()
            .then(ActivePermissionProfile::read_only);
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_managed_config()
            .build_initialized()
            .await?;
        let resume_id = mcp.send_thread_resume_request(params).await?;
        let ThreadResumeResponse {
            sandbox,
            active_permission_profile,
            ..
        } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

        assert!(matches!(sandbox, AppSandboxPolicy::ReadOnly { .. }));
        assert_eq!(
            active_permission_profile,
            expected_active_permission_profile
        );
    }
    Ok(())
}

#[tokio::test]
async fn cold_resume_without_active_permission_profile_uses_current_config() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let thread_id = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_managed_config()
            .build_initialized()
            .await?;
        let ThreadStartResponse { thread, .. } = mcp
            .start_thread(ThreadStartParams {
                model: Some("mock-model".to_string()),
                ..Default::default()
            })
            .await?;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "persist full access".to_string(),
                    text_elements: Vec::new(),
                }],
                sandbox_policy: Some(AppSandboxPolicy::DangerFullAccess),
                ..Default::default()
            }),
        )
        .await??;
        thread.id
    };

    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!(
            "default_permissions = \"{BUILT_IN_PERMISSION_PROFILE_WORKSPACE}\""
        ))
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        sandbox,
        active_permission_profile,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert!(matches!(sandbox, AppSandboxPolicy::WorkspaceWrite { .. }));
    assert_eq!(
        active_permission_profile,
        Some(ActivePermissionProfile::new(
            BUILT_IN_PERMISSION_PROFILE_WORKSPACE
        ))
    );
    Ok(())
}

#[tokio::test]
async fn cold_resume_restores_profile_selected_by_settings_update() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let thread_id = {
        let mut mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_managed_config()
            .build_initialized()
            .await?;
        let ThreadStartResponse { thread, .. } = mcp
            .start_thread(ThreadStartParams {
                model: Some("mock-model".to_string()),
                ..Default::default()
            })
            .await?;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "persist permission profile".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;
        let update_id = mcp
            .send_thread_settings_update_request(ThreadSettingsUpdateParams {
                thread_id: thread.id.clone(),
                permissions: Some(BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string()),
                ..Default::default()
            })
            .await?;
        let _: ThreadSettingsUpdateResponse =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(update_id)).await??;
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("thread/settings/updated"),
        )
        .await??;
        thread.id
    };

    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        sandbox,
        active_permission_profile,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert!(matches!(sandbox, AppSandboxPolicy::WorkspaceWrite { .. }));
    assert_eq!(
        active_permission_profile,
        Some(ActivePermissionProfile::new(
            BUILT_IN_PERMISSION_PROFILE_WORKSPACE
        ))
    );
    Ok(())
}

async fn materialize_dev_permission_thread(
    mcp: &mut TestAppServer,
    history_mode: ThreadHistoryMode,
) -> Result<String> {
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            history_mode: Some(history_mode),
            permissions: Some("dev".to_string()),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "persist permission profile".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;
    Ok(thread.id)
}

fn write_dev_permission_config(
    server_uri: &str,
    codex_home: &Path,
    dev_extends: &str,
) -> std::io::Result<()> {
    MockResponsesConfig::new(server_uri)
        .with_root_config("default_permissions = \":danger-full-access\"")
        .with_extra_config(&format!("[permissions.dev]\nextends = \"{dev_extends}\""))
        .write(codex_home)
}

#[tokio::test]
async fn thread_goal_get_rejects_unmaterialized_thread() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/get",
            Some(json!({
                "threadId": thread.id,
            })),
        )
        .await?;
    let goal_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(goal_id)),
    )
    .await??;
    assert!(
        goal_err
            .error
            .message
            .contains("ephemeral thread does not support goals"),
        "unexpected goal/get error: {}",
        goal_err.error.message
    );

    Ok(())
}

#[tokio::test]
async fn thread_goal_mutations_preserve_authoritative_sqlite_metadata() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .enable_feature(Feature::Goals)
        .write(codex_home.path())?;
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Rollout preview",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    let thread_id = ThreadId::from_string(&thread_id)?;
    let mut metadata = state_db
        .get_thread(thread_id)
        .await?
        .expect("thread metadata should exist");
    metadata.preview = Some("SQLite preview before goal set".to_string());
    state_db.upsert_thread(&metadata).await?;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread_id.to_string(),
                "objective": "preserve SQLite metadata",
                "status": "paused",
            })),
        )
        .await?;
    let _: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/updated"),
    )
    .await??;

    let mut metadata = state_db
        .get_thread(thread_id)
        .await?
        .expect("thread metadata should survive goal set");
    assert_eq!(
        metadata.preview.as_deref(),
        Some("SQLite preview before goal set")
    );
    metadata.preview = Some("SQLite preview before goal clear".to_string());
    state_db.upsert_thread(&metadata).await?;

    let clear_id = mcp
        .send_raw_request(
            "thread/goal/clear",
            Some(json!({
                "threadId": thread_id.to_string(),
            })),
        )
        .await?;
    let cleared: ThreadGoalClearResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(clear_id)).await??;
    assert!(cleared.cleared);
    let metadata = state_db
        .get_thread(thread_id)
        .await?
        .expect("thread metadata should survive goal clear");
    assert_eq!(
        metadata.preview.as_deref(),
        Some("SQLite preview before goal clear")
    );

    Ok(())
}

#[tokio::test]
async fn thread_goal_set_repairs_missing_sqlite_metadata() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .enable_feature(Feature::Goals)
        .write(codex_home.path())?;
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Rollout preview",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    let thread_id = ThreadId::from_string(&thread_id)?;
    state_db.delete_thread(thread_id).await?;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread_id.to_string(),
                "objective": "repair missing SQLite metadata",
                "status": "paused",
            })),
        )
        .await?;
    let _: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
    assert!(state_db.get_thread(thread_id).await?.is_some());

    Ok(())
}

#[tokio::test]
async fn goal_first_live_thread_appears_in_state_db_thread_list() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let codex_home_path = normalized_existing_path(codex_home.path())?;
    mock_responses_config(&server.uri()).write(&codex_home_path)?;
    let config_path = codex_home_path.join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;

    let sqlite_home = codex_home_path
        .as_path()
        .to_str()
        .expect("test codex home should be utf-8");
    let mut mcp = TestAppServer::builder()
        .with_codex_home(&codex_home_path)
        .without_managed_config()
        .with_env_overrides(&[("CODEX_SQLITE_HOME", Some(sqlite_home))])
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, cwd, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id.clone(),
                "objective": "keep the goal-first thread visible",
                "status": "paused",
            })),
        )
        .await?;
    let _goal: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/updated"),
    )
    .await??;

    let list_id = mcp
        .send_raw_request(
            "thread/list",
            Some(json!({
                "limit": 10,
                "modelProviders": ["mock_provider"],
                "sourceKinds": ["vscode"],
                "archived": false,
                "cwd": cwd.as_path().to_string_lossy().to_string(),
                "useStateDbOnly": true,
            })),
        )
        .await?;
    let list: ThreadListResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(list_id)).await??;
    assert_eq!(
        list.data
            .iter()
            .map(|thread| &thread.id)
            .collect::<Vec<_>>(),
        vec![&thread.id]
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_tracks_thread_initialized_analytics() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
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
    set_session_meta_on_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        &conversation_id,
        "user",
        "codex_work_desktop",
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    assert!(
        !thread.session_id.is_empty(),
        "session id should not be empty"
    );
    assert_eq!(thread.thread_source, Some(ThreadSource::User));

    let payload = wait_for_analytics_payload(&server, DEFAULT_READ_TIMEOUT).await?;
    let event = thread_initialized_event(&payload)?;
    assert_basic_thread_initialized_event(
        event,
        &thread.id,
        &thread.session_id,
        "codex_work_desktop",
        "gpt-5.4",
        "resumed",
        "user",
    );
    assert_eq!(event["event_params"]["thread_source"], "user");
    Ok(())
}

#[tokio::test]
async fn thread_resume_running_thread_tracks_thread_originator_in_analytics() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .with_root_config(&format!(r#"chatgpt_base_url = "{}""#, server.uri()))
        .write(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            thread_source: Some(ThreadSource::User),
            service_name: Some("codex_work_desktop".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "materialize rollout".to_string(),
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

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    let event = wait_for_matching_analytics_event(&server, DEFAULT_READ_TIMEOUT, |event| {
        event["event_type"] == "codex_thread_initialized"
            && event["event_params"]["thread_id"] == resumed.id
            && event["event_params"]["initialization_mode"] == "resumed"
    })
    .await?;
    assert_basic_thread_initialized_event(
        &event,
        &resumed.id,
        &resumed.session_id,
        "codex_work_desktop",
        "mock-model",
        "resumed",
        "user",
    );
    Ok(())
}

fn set_session_meta_on_fake_rollout(
    codex_home: &std::path::Path,
    filename_ts: &str,
    thread_id: &str,
    thread_source: &str,
    originator: &str,
) -> Result<()> {
    let path = rollout_path(codex_home, filename_ts, thread_id);
    let contents = std::fs::read_to_string(&path)?;
    let mut lines = contents.lines();
    let session_meta = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("fake rollout missing session meta"))?;
    let mut session_meta: serde_json::Value = serde_json::from_str(session_meta)?;
    session_meta["payload"]["thread_source"] = serde_json::json!(thread_source);
    session_meta["payload"]["originator"] = serde_json::json!(originator);
    let remaining = lines.collect::<Vec<_>>().join("\n");
    std::fs::write(&path, format!("{session_meta}\n{remaining}\n"))?;
    Ok(())
}

#[tokio::test]
async fn thread_resume_returns_rollout_history() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id.clone(),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(thread.id, conversation_id);
    assert_eq!(thread.preview, preview);
    assert_eq!(thread.model_provider, "mock_provider");
    assert!(thread.path.as_ref().expect("thread path").is_absolute());
    assert_eq!(thread.cwd, test_absolute_path("/"));
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
async fn thread_resume_redacts_payloads_for_chatgpt_remote_clients() -> Result<()> {
    for client_name in ["codex_chatgpt_android_remote", "codex_chatgpt_ios_remote"] {
        let remote_resume = resume_redaction_fixture(Some(client_name)).await?;
        let remote_turn = remote_resume
            .thread
            .turns
            .first()
            .expect("remote resume should include a turn");
        let remote_page_turn = remote_resume
            .initial_turns_page
            .as_ref()
            .expect("remote resume should include the requested initial turns page")
            .data
            .first()
            .expect("remote initial turns page should include a turn");
        for remote_turn in [remote_turn, remote_page_turn] {
            let remote_mcp_item = remote_turn
                .items
                .iter()
                .find(|item| matches!(item, ThreadItem::McpToolCall { .. }))
                .expect("remote resume should include redacted MCP item");
            let ThreadItem::McpToolCall {
                arguments,
                app_context,
                read_only_hint,
                result,
                error,
                ..
            } = remote_mcp_item
            else {
                unreachable!("matched MCP item");
            };
            assert_eq!(arguments, &json!("[redacted]"));
            assert_eq!(
                app_context,
                &Some(McpToolCallAppContext {
                    connector_id: "calendar".to_string(),
                    link_id: Some("link_calendar".to_string()),
                    resource_uri: Some("ui://widget/lookup.html".to_string()),
                    app_name: Some("Calendar".to_string()),
                    action_name: Some("lookup".to_string()),
                })
            );
            assert_eq!(read_only_hint, &Some(false));
            let result = result.as_ref().expect("redacted MCP result");
            assert_eq!(
                result.content,
                vec![json!({
                    "type": "text",
                    "text": "[redacted]",
                })]
            );
            assert_eq!(result.structured_content, None);
            assert_eq!(result.meta, None);
            assert_eq!(error, &None);
            assert!(
                !remote_turn
                    .items
                    .iter()
                    .any(|item| matches!(item, ThreadItem::ImageGeneration(_))),
                "remote resume should drop image generation items for {client_name}"
            );
        }
    }

    let normal_resume = resume_redaction_fixture(Some("some_other_client")).await?;
    let normal_turn = normal_resume
        .thread
        .turns
        .first()
        .expect("normal resume should include a turn");
    let normal_mcp_item = normal_turn
        .items
        .iter()
        .find(|item| matches!(item, ThreadItem::McpToolCall { .. }))
        .expect("normal resume should include MCP item");
    let ThreadItem::McpToolCall {
        arguments,
        app_context,
        read_only_hint,
        result,
        ..
    } = normal_mcp_item
    else {
        unreachable!("matched MCP item");
    };
    assert_eq!(arguments, &json!({"secret":"argument"}));
    assert_eq!(
        app_context,
        &Some(McpToolCallAppContext {
            connector_id: "calendar".to_string(),
            link_id: Some("link_calendar".to_string()),
            resource_uri: Some("ui://widget/lookup.html".to_string()),
            app_name: Some("Calendar".to_string()),
            action_name: Some("lookup".to_string()),
        })
    );
    assert_eq!(read_only_hint, &Some(false));
    let result = result.as_ref().expect("normal MCP result");
    assert_eq!(
        result.content,
        vec![json!({
            "type": "text",
            "text": "secret result",
        })]
    );
    assert_eq!(
        result.structured_content,
        Some(json!({"secret":"structured"}))
    );
    assert_eq!(result.meta, Some(json!({"secret":"meta"})));
    assert!(
        normal_turn.items.iter().any(|item| matches!(
            item,
            ThreadItem::ImageGeneration(item)
                if item.result == "base64-image-result"
                    && item.revised_prompt.as_deref() == Some("secret revised prompt")
        )),
        "normal resume should keep image generation items"
    );

    Ok(())
}

async fn resume_redaction_fixture(client_name: Option<&str>) -> Result<ThreadResumeResponse> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let filename_ts = "2025-01-05T12-00-00";
    let meta_rfc3339 = "2025-01-05T12:00:00Z";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        filename_ts,
        meta_rfc3339,
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    append_resume_redaction_history(
        codex_home.path(),
        filename_ts,
        meta_rfc3339,
        &conversation_id,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    if let Some(client_name) = client_name {
        let _ = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.initialize_with_client_info(ClientInfo {
                name: client_name.to_string(),
                title: None,
                version: "0.1.0".to_string(),
            }),
        )
        .await??;
    } else {
        timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    }

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
                limit: None,
                sort_direction: None,
                items_view: Some(TurnItemsView::Full),
            }),
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    to_response::<ThreadResumeResponse>(resume_resp)
}

fn append_resume_redaction_history(
    codex_home: &Path,
    filename_ts: &str,
    meta_rfc3339: &str,
    conversation_id: &str,
) -> Result<()> {
    let rollout_file_path = rollout_path(codex_home, filename_ts, conversation_id);
    let persisted_rollout = std::fs::read_to_string(&rollout_file_path)?;
    let appended_rollout = [
        EventMsg::McpToolCallEnd(McpToolCallEndEvent {
            call_id: "mcp-1".to_string(),
            invocation: McpInvocation {
                server: "docs".to_string(),
                tool: "lookup".to_string(),
                arguments: Some(json!({"secret":"argument"})),
            },
            connector_id: Some("calendar".to_string()),
            mcp_app_resource_uri: Some("ui://widget/lookup.html".to_string()),
            link_id: Some("link_calendar".to_string()),
            app_name: Some("Calendar".to_string()),
            action_name: Some("lookup".to_string()),
            plugin_id: None,
            read_only_hint: Some(false),
            duration: Duration::from_millis(8),
            result: Ok(CallToolResult {
                content: vec![json!({
                    "type": "text",
                    "text": "secret result",
                })],
                structured_content: Some(json!({"secret":"structured"})),
                is_error: Some(false),
                meta: Some(json!({"secret":"meta"})),
            }),
        }),
        EventMsg::ImageGenerationEnd(ImageGenerationEndEvent {
            call_id: "ig-1".to_string(),
            status: "completed".to_string(),
            revised_prompt: Some("secret revised prompt".to_string()),
            result: "base64-image-result".to_string(),
            transparent_background: None,
            failure: None,
            saved_path: Some(test_absolute_path("/tmp/ig-1.png")),
        }),
    ]
    .into_iter()
    .map(|payload| {
        Ok(json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(payload)?,
        })
        .to_string())
    })
    .collect::<Result<Vec<_>>>()?
    .join("\n");
    std::fs::write(
        &rollout_file_path,
        format!("{persisted_rollout}{appended_rollout}\n"),
    )?;
    Ok(())
}

#[tokio::test]
async fn thread_resume_can_skip_turns_for_metadata_only_resume() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout_with_text_elements(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Vec::new(),
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(thread.id, conversation_id);
    assert!(thread.turns.is_empty());

    Ok(())
}

#[tokio::test]
async fn thread_resume_rejects_archived_session_by_id() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let filename_ts = "2025-01-05T12-00-00";
    let conversation_id = create_fake_rollout_with_text_elements(
        codex_home.path(),
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Archived saved user message",
        Vec::new(),
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let active_rollout_path = rollout_path(codex_home.path(), filename_ts, &conversation_id);
    let archived_dir = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    std::fs::create_dir_all(&archived_dir)?;
    std::fs::rename(
        &active_rollout_path,
        archived_dir.join(active_rollout_path.file_name().expect("rollout file name")),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id.clone(),
            ..Default::default()
        })
        .await?;
    let resume_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(resume_id)),
    )
    .await??;

    let message = resume_err.error.message;
    assert!(
        message.contains(&format!("session {conversation_id} is archived"))
            && message.contains(&format!(
                "codex unarchive {conversation_id}` to unarchive it first"
            )),
        "unexpected resume error: {message}"
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_keeps_paused_goal_paused() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "keep polishing",
                "status": "paused",
            })),
        )
        .await?;
    let _goal: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/updated"),
    )
    .await??;
    mcp.clear_message_buffer();

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let _resume: ThreadResumeResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/updated"),
    )
    .await??;
    let notification: ServerNotification = notification.try_into()?;
    let ServerNotification::ThreadGoalUpdated(notification) = notification else {
        anyhow::bail!("expected thread goal update notification");
    };
    assert_eq!(notification.goal.status, ThreadGoalStatus::Paused);
    assert!(
        !mcp.pending_notification_methods()
            .iter()
            .any(|method| method == "turn/started"),
        "paused goal should not continue after thread resume"
    );

    Ok(())
}

#[tokio::test]
async fn thread_goal_set_enforces_configured_maximum_token_budget() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    let config = config.replace("personality = true\n", "personality = true\ngoals = true\n");
    std::fs::write(
        config_path,
        format!("{config}\n[goals]\nmax_goal_token_budget = 200\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            config: Some(
                [("goals.max_goal_token_budget".to_string(), json!(100))]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let oversized_creation_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "oversized goal",
                "tokenBudget": 101,
            })),
        )
        .await?;
    let creation_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(oversized_creation_id)),
    )
    .await??;
    assert_eq!(
        creation_error.error.message,
        "goal token budget 101 exceeds the maximum allowed goal token budget of 100"
    );

    let creation_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "bounded goal",
            })),
        )
        .await?;
    let creation: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(creation_id)).await??;
    assert_eq!(creation.goal.token_budget, Some(100));

    let clear_budget_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({ "threadId": thread.id, "tokenBudget": null })),
        )
        .await?;
    let clear_budget: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(clear_budget_id)).await??;
    assert_eq!(clear_budget.goal.token_budget, Some(100));

    let oversized_update_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "tokenBudget": 101,
            })),
        )
        .await?;
    let update_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(oversized_update_id)),
    )
    .await??;
    assert_eq!(
        update_error.error.message,
        "goal token budget 101 exceeds the maximum allowed goal token budget of 100"
    );

    Ok(())
}

#[tokio::test]
async fn thread_goal_set_preserves_budget_limited_same_objective() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "keep polishing",
                "status": "budgetLimited",
                "tokenBudget": 10,
            })),
        )
        .await?;
    let goal: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
    assert_eq!(goal.goal.status, ThreadGoalStatus::BudgetLimited);

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/updated"),
    )
    .await??;

    let replacement_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "keep polishing",
            })),
        )
        .await?;
    let replacement: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(replacement_id)).await??;

    assert_eq!(replacement.goal.status, ThreadGoalStatus::BudgetLimited);
    assert_eq!(replacement.goal.token_budget, Some(10));
    assert_eq!(replacement.goal.tokens_used, 0);
    assert_eq!(replacement.goal.time_used_seconds, 0);

    Ok(())
}

#[tokio::test]
async fn thread_goal_set_persists_resumable_stopped_statuses() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    for (wire_status, expected_status) in [
        ("blocked", ThreadGoalStatus::Blocked),
        ("usageLimited", ThreadGoalStatus::UsageLimited),
    ] {
        let goal_id = mcp
            .send_raw_request(
                "thread/goal/set",
                Some(json!({
                    "threadId": thread.id.clone(),
                    "objective": "keep polishing",
                    "status": wire_status,
                })),
            )
            .await?;
        let goal: ThreadGoalSetResponse =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
        assert_eq!(goal.goal.status, expected_status);

        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("thread/goal/updated"),
        )
        .await??;
        let notification: ServerNotification = notification.try_into()?;
        let ServerNotification::ThreadGoalUpdated(notification) = notification else {
            anyhow::bail!("expected thread goal update notification");
        };
        assert_eq!(notification.goal.status, expected_status);
    }

    Ok(())
}

#[tokio::test]
async fn thread_goal_set_edits_objective_without_resetting_usage() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread_id,
                "objective": "keep polishing",
                "status": "active",
                "tokenBudget": 40,
            })),
        )
        .await?;
    let goal: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/updated"),
    )
    .await??;

    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    let thread_id = ThreadId::from_string(&thread_id)?;
    let thread_metadata = state_db
        .get_thread(thread_id)
        .await?
        .expect("thread metadata should exist");
    assert_eq!(thread_metadata.preview.as_deref(), Some("keep polishing"));
    let persisted_goal = state_db
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .expect("goal should exist");
    state_db
        .thread_goals()
        .account_thread_goal_usage(
            thread_id,
            /*time_delta_seconds*/ 12,
            /*token_delta*/ 50,
            codex_state::GoalAccountingMode::ActiveOnly,
            Some(persisted_goal.goal_id.as_str()),
        )
        .await?;

    let edit_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread_id.to_string(),
                "objective": "keep polishing with clearer wording",
                "status": "active",
                "tokenBudget": 40,
            })),
        )
        .await?;
    let edit: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(edit_id)).await??;
    let updated_goal = state_db
        .thread_goals()
        .get_thread_goal(thread_id)
        .await?
        .expect("goal should still exist");
    let thread_metadata = state_db
        .get_thread(thread_id)
        .await?
        .expect("thread metadata should still exist");

    assert_eq!(persisted_goal.goal_id, updated_goal.goal_id);
    assert_eq!(thread_metadata.preview.as_deref(), Some("keep polishing"));
    assert_eq!(edit.goal.objective, "keep polishing with clearer wording");
    assert_eq!(edit.goal.status, ThreadGoalStatus::BudgetLimited);
    assert_eq!(edit.goal.token_budget, Some(40));
    assert_eq!(edit.goal.tokens_used, 50);
    assert_eq!(edit.goal.time_used_seconds, 12);
    assert_eq!(edit.goal.created_at, goal.goal.created_at);

    Ok(())
}

#[tokio::test]
async fn thread_goal_lifecycle_emits_analytics_and_clear_deletes_goal() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        responses::sse(vec![
            responses::ev_response_created("materialize-thread"),
            responses::ev_completed("materialize-thread"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("goal-continuation"),
            responses::ev_completed_with_tokens("goal-continuation", /*total_tokens*/ 200),
        ]),
    ])
    .await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .with_root_config(&format!(r#"chatgpt_base_url = "{}""#, server.uri()))
        .write(codex_home.path())?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        config.replace("personality = true\n", "personality = true\ngoals = true\n"),
    )?;
    mount_analytics_capture(&server, codex_home.path()).await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT.saturating_mul(2))
        .await?;

    let start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(start_id)).await??;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let goal_id = mcp
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id,
                "objective": "do not serialize this objective",
                "tokenBudget": 100,
            })),
        )
        .await?;
    let _goal: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(goal_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/updated"),
    )
    .await??;

    let created = wait_for_goal_event(&server, DEFAULT_READ_TIMEOUT, "created", "active").await?;
    let persisted_goal_id = created["event_params"]["goal_id"]
        .as_str()
        .expect("created goal id");
    assert_eq!(created["event_params"]["thread_id"], thread.id);
    assert_eq!(created["event_params"]["turn_id"], serde_json::Value::Null);
    assert_eq!(created["event_params"]["has_token_budget"], true);
    assert!(created["event_params"]["session_id"].is_string());
    assert!(created["event_params"]["app_server_client"].is_object());
    assert!(created["event_params"]["runtime"].is_object());
    assert!(created["event_params"].get("objective").is_none());
    assert!(created["event_params"].get("token_budget").is_none());

    let usage = wait_for_goal_event(
        &server,
        DEFAULT_READ_TIMEOUT,
        "usage_accounted",
        "budget_limited",
    )
    .await?;
    let causal_turn_id = usage["event_params"]["turn_id"]
        .as_str()
        .expect("accounted usage turn id");
    assert_eq!(usage["event_params"]["goal_id"], persisted_goal_id);
    assert_eq!(usage["event_params"]["cumulative_tokens_accounted"], 200);
    assert!(
        usage["event_params"]["cumulative_time_accounted_seconds"]
            .as_i64()
            .is_some()
    );

    let status = wait_for_goal_event(
        &server,
        DEFAULT_READ_TIMEOUT,
        "status_changed",
        "budget_limited",
    )
    .await?;
    assert_eq!(status["event_params"]["goal_id"], persisted_goal_id);
    assert_eq!(status["event_params"]["turn_id"], causal_turn_id);
    assert_eq!(
        status["event_params"]["cumulative_tokens_accounted"],
        serde_json::Value::Null
    );
    assert_eq!(
        status["event_params"]["cumulative_time_accounted_seconds"],
        serde_json::Value::Null
    );

    let clear_id = mcp
        .send_raw_request(
            "thread/goal/clear",
            Some(json!({
                "threadId": thread.id,
            })),
        )
        .await?;
    let clear: ThreadGoalClearResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(clear_id)).await??;
    assert!(clear.cleared);

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/goal/cleared"),
    )
    .await??;

    let cleared =
        wait_for_goal_event(&server, DEFAULT_READ_TIMEOUT, "cleared", "budget_limited").await?;
    assert_eq!(cleared["event_params"]["goal_id"], persisted_goal_id);
    assert_eq!(cleared["event_params"]["turn_id"], serde_json::Value::Null);

    let get_id = mcp
        .send_raw_request(
            "thread/goal/get",
            Some(json!({
                "threadId": thread.id,
            })),
        )
        .await?;
    let get: codex_app_server_protocol::ThreadGoalGetResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(get_id)).await??;
    assert_eq!(None, get.goal);

    let clear_again_id = mcp
        .send_raw_request(
            "thread/goal/clear",
            Some(json!({
                "threadId": thread.id,
            })),
        )
        .await?;
    let clear_again: ThreadGoalClearResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(clear_again_id)).await??;
    assert!(!clear_again.cleared);

    Ok(())
}

#[tokio::test]
async fn thread_resume_emits_restored_token_usage_before_next_turn() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout_with_token_usage(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

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
async fn cold_paginated_resume_restores_usage_without_loading_turns() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let conversation_id = create_fake_paginated_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let path = rollout_path(codex_home.path(), "2025-01-05T12-00-00", &conversation_id);
    let canonical_turn_id = "persisted-token-usage-turn";
    append_rollout_item_to_path(
        &path,
        &RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: canonical_turn_id.to_string(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
    )
    .await?;
    append_rollout_item_to_path(
        &path,
        &RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
            info: Some(TokenUsageInfo {
                total_token_usage: TokenUsage {
                    input_tokens: 120,
                    output_tokens: 30,
                    total_tokens: 150,
                    ..Default::default()
                },
                last_token_usage: TokenUsage {
                    input_tokens: 70,
                    output_tokens: 20,
                    total_tokens: 90,
                    ..Default::default()
                },
                model_context_window: Some(200_000),
            }),
            rate_limits: None,
        })),
    )
    .await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(resume_id)).await??;
    assert!(thread.turns.is_empty());

    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("thread/tokenUsage/updated"),
    )
    .await??;
    let ServerNotification::ThreadTokenUsageUpdated(notification) = notification.try_into()? else {
        panic!("expected thread/tokenUsage/updated notification");
    };
    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.turn_id, canonical_turn_id);
    assert_eq!(notification.token_usage.total.total_tokens, 150);

    let turns_id = app_server
        .send_thread_turns_list_request(ThreadTurnsListParams {
            thread_id: thread.id,
            cursor: None,
            limit: Some(1),
            sort_direction: Some(SortDirection::Desc),
            items_view: Some(TurnItemsView::NotLoaded),
        })
        .await?;
    let turns: ThreadTurnsListResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(turns_id)).await??;
    assert_eq!(notification.turn_id, turns.data[0].id);

    Ok(())
}

#[tokio::test]
async fn cold_paginated_resume_omits_usage_when_its_turn_is_ambiguous() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let filename_ts = "2025-01-05T12-00-00";
    let conversation_id = create_fake_paginated_rollout(
        codex_home.path(),
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let path = rollout_path(codex_home.path(), filename_ts, &conversation_id);
    append_rollout_item_to_path(
        &path,
        &RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
            info: Some(TokenUsageInfo {
                total_token_usage: TokenUsage {
                    total_tokens: 150,
                    ..Default::default()
                },
                last_token_usage: TokenUsage {
                    total_tokens: 90,
                    ..Default::default()
                },
                model_context_window: Some(200_000),
            }),
            rate_limits: None,
        })),
    )
    .await?;
    let interrupted_turn_id = "interrupted-turn-after-token-usage";
    append_rollout_item_to_path(
        &path,
        &RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: interrupted_turn_id.to_string(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
    )
    .await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(resume_id)).await??;
    assert!(thread.turns.is_empty());
    let turns_id = app_server
        .send_thread_turns_list_request(ThreadTurnsListParams {
            thread_id: thread.id,
            cursor: None,
            limit: Some(1),
            sort_direction: Some(SortDirection::Desc),
            items_view: Some(TurnItemsView::NotLoaded),
        })
        .await?;
    let turns: ThreadTurnsListResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(turns_id)).await??;
    assert_eq!(turns.data[0].id, interrupted_turn_id);
    assert!(
        timeout(
            Duration::from_millis(100),
            app_server.read_stream_until_notification_message("thread/tokenUsage/updated"),
        )
        .await
        .is_err(),
        "usage owned by an implicit turn must not be attributed to {interrupted_turn_id}"
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_skips_restored_token_usage_when_turns_are_excluded() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let conversation_id = create_fake_rollout_with_token_usage(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Some("mock_provider"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let first_resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id.clone(),
            ..Default::default()
        })
        .await?;
    let first_resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } =
        to_response::<ThreadResumeResponse>(first_resume_resp)?;
    let expected_turn_id = thread.turns[0].id.clone();

    let first_note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/tokenUsage/updated"),
    )
    .await??;
    let parsed: ServerNotification = first_note.try_into()?;
    let ServerNotification::ThreadTokenUsageUpdated(notification) = parsed else {
        panic!("expected thread/tokenUsage/updated notification");
    };
    assert_eq!(notification.turn_id, expected_turn_id);

    let second_resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed_again,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(second_resume_id)).await??;
    assert!(resumed_again.turns.is_empty());

    let second_note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/tokenUsage/updated"),
    )
    .await;
    assert!(
        second_note.is_err(),
        "excludeTurns=true should not replay token usage"
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_token_usage_replay_ignores_stale_interrupted_tail_turn() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let filename_ts = "2025-01-05T12-00-00";
    let meta_rfc3339 = "2025-01-05T12:00:00Z";
    let conversation_id = create_fake_rollout_with_token_usage(
        codex_home.path(),
        filename_ts,
        meta_rfc3339,
        "Saved user message",
        Some("mock_provider"),
    )?;
    let rollout_file_path = rollout_path(codex_home.path(), filename_ts, &conversation_id);
    let persisted_rollout = std::fs::read_to_string(&rollout_file_path)?;
    let stale_turn_id = "incomplete-turn-after-token-usage";
    let appended_rollout = [
        json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: stale_turn_id.to_string(),
                trace_id: None,
                started_at: None,
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
                delivery: None,
            }))?,
        })
        .to_string(),
    ]
    .join("\n");
    std::fs::write(
        &rollout_file_path,
        format!("{persisted_rollout}{appended_rollout}\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(thread.turns.len(), 2);
    assert_eq!(thread.turns[0].status, TurnStatus::Completed);
    assert_eq!(thread.turns[1].id, stale_turn_id);
    assert_eq!(thread.turns[1].status, TurnStatus::Interrupted);

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
    assert_ne!(notification.turn_id, stale_turn_id);
    assert_eq!(notification.token_usage.total.total_tokens, 150);
    assert_eq!(notification.token_usage.last.total_tokens, 90);

    Ok(())
}

#[tokio::test]
async fn thread_resume_token_usage_replay_can_belong_to_interrupted_turn() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let filename_ts = "2025-01-05T12-00-00";
    let meta_rfc3339 = "2025-01-05T12:00:00Z";
    let conversation_id = create_fake_rollout_with_token_usage(
        codex_home.path(),
        filename_ts,
        meta_rfc3339,
        "Saved user message",
        Some("mock_provider"),
    )?;
    let rollout_file_path = rollout_path(codex_home.path(), filename_ts, &conversation_id);
    let persisted_rollout = std::fs::read_to_string(&rollout_file_path)?;
    let interrupted_turn_id = "interrupted-turn-with-token-usage";
    let appended_rollout = [
        json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: interrupted_turn_id.to_string(),
                trace_id: None,
                started_at: None,
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }))?,
        })
        .to_string(),
        json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(EventMsg::AgentMessage(AgentMessageEvent {
                message: "Interrupted after usage".to_string(),
                phase: None,
                memory_citation: None,
                delivery: None,
            }))?,
        })
        .to_string(),
        json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(EventMsg::TokenCount(TokenCountEvent {
                info: Some(TokenUsageInfo {
                    total_token_usage: TokenUsage {
                        input_tokens: 180,
                        cached_input_tokens: 40,
                        cache_write_input_tokens: 0,
                        output_tokens: 50,
                        reasoning_output_tokens: 15,
                        total_tokens: 230,
                        codex_rollout_budget_units: None,
                    },
                    last_token_usage: TokenUsage {
                        input_tokens: 90,
                        cached_input_tokens: 30,
                        cache_write_input_tokens: 0,
                        output_tokens: 40,
                        reasoning_output_tokens: 12,
                        total_tokens: 130,
                        codex_rollout_budget_units: None,
                    },
                    model_context_window: Some(200_000),
                }),
                rate_limits: None,
            }))?,
        })
        .to_string(),
        json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": serde_json::to_value(EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some(interrupted_turn_id.to_string()),
                started_at: None,
                reason: TurnAbortReason::Interrupted,
                completed_at: None,
                duration_ms: None,
            }))?,
        })
        .to_string(),
    ]
    .join("\n");
    std::fs::write(
        &rollout_file_path,
        format!("{persisted_rollout}{appended_rollout}\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(thread.turns.len(), 2);
    assert_eq!(thread.turns[0].status, TurnStatus::Completed);
    assert_eq!(thread.turns[1].id, interrupted_turn_id);
    assert_eq!(thread.turns[1].status, TurnStatus::Interrupted);

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
    assert_eq!(notification.turn_id, interrupted_turn_id);
    assert_eq!(notification.token_usage.total.total_tokens, 230);
    assert_eq!(notification.token_usage.last.total_tokens, 130);

    Ok(())
}

#[tokio::test]
async fn thread_resume_prefers_persisted_git_metadata_for_local_threads() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;

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
        session_id: conversation_id.into(),
        id: conversation_id,
        forked_from_id: None,
        parent_thread_id: None,
        timestamp: "2025-01-05T12:00:00Z".to_string(),
        cwd: repo_path.clone(),
        originator: "codex".to_string(),
        cli_version: "0.0.0".to_string(),
        source: RolloutSessionSource::Cli,
        thread_source: None,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
        model_provider: Some("mock_provider".to_string()),
        base_instructions: None,
        dynamic_tools: None,
        selected_capability_roots: Vec::new(),
        memory_mode: None,
        history_mode: Default::default(),
        history_base: None,
        subagent_history_start_ordinal: None,
        multi_agent_version: None,
        context_window: None,
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
    let state_db = StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let update_id = mcp
        .send_thread_metadata_update_request(ThreadMetadataUpdateParams {
            thread_id: thread_id.clone(),
            project_id: None,
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
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

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
    mock_responses_config(&server.uri()).write(codex_home.path())?;

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
                trace_id: None,
                started_at: None,
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
                delivery: None,
            }))?,
        })
        .to_string(),
    ]
    .join("\n");
    std::fs::write(
        &rollout_file_path,
        format!("{persisted_rollout}{appended_rollout}\n"),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: conversation_id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

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
    let ThreadResumeResponse {
        thread: resumed_again,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(second_resume_id)).await??;

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
    let ThreadReadResponse {
        thread: read_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(read_id)).await??;

    assert_eq!(read_thread.status, ThreadStatus::Idle);
    assert_eq!(read_thread.turns.len(), 2);
    assert_eq!(read_thread.turns[1].id, turn_id);
    assert_eq!(read_thread.turns[1].status, TurnStatus::Interrupted);

    Ok(())
}

#[tokio::test]
async fn thread_resume_defers_updated_at_until_turn_start() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let rollout = setup_rollout_fixture(codex_home.path(), &server.uri()).await?;
    let thread_id = rollout.conversation_id.clone();

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.clone(),
            include_turns: false,
        })
        .await?;
    let ThreadReadResponse {
        thread: before_resume,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(read_id)).await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(thread.updated_at, before_resume.updated_at);
    assert_eq!(thread.recency_at, before_resume.recency_at);
    assert_eq!(thread.status, ThreadStatus::Idle);

    let after_modified = std::fs::metadata(&rollout.rollout_file_path)?.modified()?;
    assert_eq!(after_modified, rollout.before_modified);

    let unsubscribe_id = mcp
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unsubscribe_id)),
    )
    .await??;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: "not-a-valid-thread-id".to_string(),
            path: Some(normalized_existing_path(&rollout.rollout_file_path)?),
            cwd: Some(codex_home.path().to_string_lossy().to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { cwd, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    assert_eq!(cwd, AbsolutePathBuf::from_absolute_path(codex_home.path())?);

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
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
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.clone(),
            include_turns: false,
        })
        .await?;
    let ThreadReadResponse {
        thread: after_turn_start,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(read_id)).await??;
    assert!(after_turn_start.recency_at > before_resume.recency_at);

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
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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

    let turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, primary.read_response(resume_id)).await??;
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
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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
            client_user_message_id: None,
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
    assert_eq!(running_turn.items_view, TurnItemsView::NotLoaded);
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
                phase: None,
                internal_chat_message_metadata_passthrough: None,
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
async fn thread_resume_rejects_mismatched_path_for_running_thread_id() -> Result<()> {
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
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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
            client_user_message_id: None,
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

    #[cfg(windows)]
    {
        let active_path = thread.path.as_ref().expect("thread should have path");
        let active_path_display = active_path.as_os_str().to_string_lossy();
        let equivalent_path = if let Some(path) = active_path_display.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{path}"))
        } else if let Some(path) = active_path_display.strip_prefix(r"\\?\") {
            PathBuf::from(path)
        } else if let Some(path) = active_path_display.strip_prefix(r"\\") {
            PathBuf::from(format!(r"\\?\UNC\{path}"))
        } else {
            PathBuf::from(format!(r"\\?\{active_path_display}"))
        };
        let normalized_resume_id = primary
            .send_thread_resume_request(ThreadResumeParams {
                thread_id: thread_id.clone(),
                path: Some(equivalent_path),
                ..Default::default()
            })
            .await?;
        let normalized_resume_resp: JSONRPCResponse = timeout(
            DEFAULT_READ_TIMEOUT,
            primary.read_stream_until_response_message(RequestId::Integer(normalized_resume_id)),
        )
        .await??;
        let ThreadResumeResponse { thread, .. } =
            to_response::<ThreadResumeResponse>(normalized_resume_resp)?;
        assert_eq!(thread.id, thread_id);
    }

    let stale_thread_id = Uuid::new_v4().to_string();
    let stale_path = rollout_path(codex_home.path(), "2025-01-01T00-00-00", &stale_thread_id);
    std::fs::create_dir_all(stale_path.parent().expect("stale path parent"))?;
    let thread_uuid = Uuid::parse_str(&stale_thread_id)?;
    let mut stale_file = std::fs::File::create(&stale_path)?;
    let stale_meta = json!({
        "timestamp": "2025-01-01T00:00:00Z",
        "type": "session_meta",
        "payload": {
            "session_id": thread_uuid,
            "id": thread_uuid,
            "timestamp": "2025-01-01T00:00:00Z",
            "cwd": codex_home.path(),
            "originator": "test_originator",
            "cli_version": "test_version",
            "source": "cli",
            "model_provider": "test-provider",
        },
    });
    writeln!(stale_file, "{stale_meta}")?;
    let stale_user_event = json!({
        "timestamp": "2025-01-01T00:00:00Z",
        "type": "event_msg",
        "payload": {
            "type": "user_message",
            "message": "stale history",
            "kind": "plain",
        },
    });
    writeln!(stale_file, "{stale_user_event}")?;

    let stale_resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            path: Some(stale_path),
            ..Default::default()
        })
        .await?;
    let stale_resume_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_error_message(RequestId::Integer(stale_resume_id)),
    )
    .await??;
    assert!(
        stale_resume_err.error.message.contains("stale path"),
        "unexpected resume error: {}",
        stale_resume_err.error.message
    );

    primary
        .interrupt_turn_and_wait_for_aborted(thread_id, running_turn.id, DEFAULT_READ_TIMEOUT)
        .await?;

    Ok(())
}

#[tokio::test]
async fn thread_resume_rejoins_running_paginated_thread_with_initial_page() -> Result<()> {
    let (release_running_turn, running_turn_gate) = oneshot::channel();
    let (server, _response_completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-1"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: Some(running_turn_gate),
            body: responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-2", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        }],
    ])
    .await;
    let codex_home = TempDir::new()?;
    mock_responses_config(server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            history_mode: Some(ThreadHistoryMode::Paginated),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn: seed_turn } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(seed_turn_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    primary.clear_message_buffer();

    let running_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "keep running".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let running_turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_response_message(RequestId::Integer(running_turn_id)),
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
            thread_id: thread.id.clone(),
            model: Some("not-the-running-model".to_string()),
            cwd: Some("/tmp".to_string()),
            initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
                limit: Some(1),
                sort_direction: Some(SortDirection::Desc),
                items_view: Some(TurnItemsView::NotLoaded),
            }),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread,
        model,
        initial_turns_page,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, primary.read_response(resume_id)).await??;
    assert_eq!(model, "gpt-5.4");
    let initial_turns_page = initial_turns_page.expect("resume should include initial turns page");
    assert_eq!(initial_turns_page.data.len(), 1);
    let resumed_running_turn = initial_turns_page
        .data
        .first()
        .expect("resume page should include the running turn");
    assert_eq!(resumed_running_turn.id, running_turn.id);
    assert_eq!(resumed_running_turn.items_view, TurnItemsView::NotLoaded);
    assert!(resumed_running_turn.items.is_empty());
    assert_eq!(resumed_running_turn.status, TurnStatus::InProgress);
    assert!(initial_turns_page.backwards_cursor.is_some());
    assert!(initial_turns_page.next_cursor.is_some());

    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("thread/tokenUsage/updated"),
    )
    .await??;

    let metadata_resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let metadata_resume: ThreadResumeResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_response(metadata_resume_id),
    )
    .await??;
    assert!(metadata_resume.thread.turns.is_empty());
    assert!(metadata_resume.initial_turns_page.is_none());
    assert!(
        timeout(
            Duration::from_millis(100),
            primary.read_stream_until_notification_message("thread/tokenUsage/updated"),
        )
        .await
        .is_err(),
        "hot paginated resume should wait for a real token usage update"
    );

    let asc_resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            exclude_turns: true,
            initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
                limit: Some(1),
                sort_direction: Some(SortDirection::Asc),
                items_view: Some(TurnItemsView::NotLoaded),
            }),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        initial_turns_page, ..
    } = timeout(DEFAULT_READ_TIMEOUT, primary.read_response(asc_resume_id)).await??;
    let initial_turns_page = initial_turns_page.expect("resume should include initial turns page");
    assert_eq!(initial_turns_page.data.len(), 1);
    assert_eq!(initial_turns_page.data[0].id, seed_turn.id);
    // The running-thread resume response is queued onto the thread listener task.
    // If the in-flight turn completes before that queued command runs, the response
    // can legitimately observe the thread as idle.
    match &thread.status {
        ThreadStatus::Active { active_flags } => assert!(active_flags.is_empty()),
        ThreadStatus::Idle => {}
        status => panic!("unexpected thread status after running resume: {status:?}"),
    }

    release_running_turn
        .send(())
        .expect("release the running model response");
    timeout(
        DEFAULT_READ_TIMEOUT,
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    server.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn thread_resume_can_skip_turns_when_thread_is_running() -> Result<()> {
    let server = responses::start_mock_server().await;
    let _response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "seed history".to_string(),
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
        primary.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed, ..
    } = timeout(DEFAULT_READ_TIMEOUT, primary.read_response(resume_id)).await??;

    assert_eq!(resumed.id, thread.id);
    assert_eq!(resumed.status, ThreadStatus::Idle);
    assert!(resumed.turns.is_empty());

    Ok(())
}

#[tokio::test]
async fn thread_resume_replays_pending_command_execution_request_approval() -> Result<()> {
    // TODO(anp): Remove after shell approval replay can route target-native cwd across host OSes.
    skip_if_wine_exec!(
        Ok(()),
        "shell approval replay rejects the Windows cwd on the Linux host"
    );

    let responses = vec![
        create_final_assistant_message_sse_response("seeded")?,
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            /*workdir*/ None,
            Some(5000),
            "call-1",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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
            client_user_message_id: None,
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
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, primary.read_response(resume_id)).await??;
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
    wait_for_responses_request_count(&server, /*expected_count*/ 3).await?;

    Ok(())
}

#[tokio::test]
async fn thread_resume_replays_pending_file_change_request_approval() -> Result<()> {
    // TODO(anp): Remove after apply-patch approval fixtures use a target-native workspace.
    skip_if_remote!(
        Ok(()),
        "apply-patch approval fixture is only materialized on the host"
    );

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
    mock_responses_config(&server.uri())
        .disable_feature(Feature::ShellSnapshot)
        .write(&codex_home)?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(&codex_home)
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let seed_turn_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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
            client_user_message_id: None,
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

    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification: ThreadStatusChangedNotification =
                primary.read_notification("thread/status/changed").await?;
            if notification.thread_id == thread.id
                && matches!(
                    notification.status,
                    ThreadStatus::Active { active_flags }
                        if active_flags.contains(&ThreadActiveFlag::WaitingOnApproval)
                )
            {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await??;

    let resume_id = primary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, primary.read_response(resume_id)).await??;
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
    wait_for_responses_request_count(&server, /*expected_count*/ 3).await?;
    let status = timeout(DEFAULT_READ_TIMEOUT, primary.shutdown_gracefully()).await??;
    anyhow::ensure!(
        status.success(),
        "app-server exited unsuccessfully: {status}"
    );

    Ok(())
}

#[tokio::test]
async fn thread_resume_with_overrides_defers_updated_at_until_turn_start() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let RestartedThreadFixture {
        mut mcp,
        thread_id,
        rollout_file_path,
        updated_at,
    } = start_materialized_thread_and_restart(codex_home.path(), "materialize").await?;
    let expected_updated_at_rfc3339 = "2025-01-07T00:00:00Z";
    set_rollout_mtime(rollout_file_path.as_path(), expected_updated_at_rfc3339)?;
    let before_modified = std::fs::metadata(&rollout_file_path)?.modified()?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse {
        thread: resumed_thread,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;

    assert_eq!(resumed_thread.updated_at, updated_at);
    assert_eq!(resumed_thread.status, ThreadStatus::Idle);

    let after_resume_modified = std::fs::metadata(&rollout_file_path)?.modified()?;
    assert_eq!(after_resume_modified, before_modified);

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: resumed_thread.id,
            client_user_message_id: None,
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
    let rollout = setup_rollout_fixture(codex_home.path(), &server.uri()).await?;
    mock_responses_config(&server.uri())
        .with_extra_config(
            r#"[mcp_servers.required_broken]
command = "codex-definitely-not-a-real-binary"
required = true"#,
        )
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

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
async fn thread_resume_surfaces_cloud_config_bundle_load_errors() -> Result<()> {
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
    mock_responses_config(&model_server.uri())
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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            (
                REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                Some(refresh_token_url.as_str()),
            ),
        ])
        .build_initialized()
        .await?;

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
async fn thread_resume_uses_path_over_non_running_thread_id() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let RestartedThreadFixture {
        mut mcp,
        thread_id,
        rollout_file_path,
        ..
    } = start_materialized_thread_and_restart(codex_home.path(), "materialize").await?;

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: ThreadId::new().to_string(),
            path: Some(rollout_file_path),
            ..Default::default()
        })
        .await?;

    let ThreadResumeResponse {
        thread: resumed, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    assert_eq!(resumed.id, thread_id);

    Ok(())
}

#[tokio::test]
async fn thread_resume_can_load_source_by_external_path() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let external_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    let thread_id = create_fake_rollout(
        external_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "external path history",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let thread_path = rollout_path(external_home.path(), "2025-01-05T12-00-00", &thread_id);

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: "not-a-valid-thread-id".to_string(),
            path: Some(thread_path.clone()),
            ..Default::default()
        })
        .await?;

    let ThreadResumeResponse {
        thread: resumed, ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    assert_eq!(resumed.id, thread_id);
    let resumed_path = resumed.path.as_ref().expect("resumed thread path");
    assert_eq!(
        normalized_existing_path(resumed_path)?,
        normalized_existing_path(&thread_path)?
    );
    assert_eq!(resumed.preview, "external path history");
    assert_eq!(resumed.status, ThreadStatus::Idle);

    Ok(())
}

#[tokio::test]
async fn thread_resume_supports_history_and_overrides() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;

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
        phase: None,
        internal_chat_message_metadata_passthrough: None,
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
    let ThreadResumeResponse {
        thread: resumed,
        model_provider,
        ..
    } = timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    assert!(!resumed.id.is_empty());
    assert_eq!(model_provider, "mock_provider");
    assert_eq!(resumed.preview, history_text);
    assert_eq!(resumed.status, ThreadStatus::Idle);

    Ok(())
}

struct RestartedThreadFixture {
    mcp: TestAppServer,
    thread_id: String,
    rollout_file_path: PathBuf,
    updated_at: i64,
}

async fn start_materialized_thread_and_restart(
    codex_home: &Path,
    seed_text: &str,
) -> Result<RestartedThreadFixture> {
    let mut first_mcp = TestAppServer::builder()
        .with_codex_home(codex_home)
        .build_initialized()
        .await?;

    let start_id = first_mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, first_mcp.read_response(start_id)).await??;

    let materialize_turn_id = first_mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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

    let read_id = first_mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id.clone(),
            include_turns: false,
        })
        .await?;
    let ThreadReadResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, first_mcp.read_response(read_id)).await??;

    let thread_id = thread.id;
    let rollout_file_path = thread
        .path
        .ok_or_else(|| anyhow::anyhow!("thread path missing from thread/start response"))?;
    let updated_at = thread.updated_at;

    drop(first_mcp);

    let second_mcp = TestAppServer::builder()
        .with_codex_home(codex_home)
        .build_initialized()
        .await?;

    Ok(RestartedThreadFixture {
        mcp: second_mcp,
        thread_id,
        rollout_file_path: rollout_file_path.to_path_buf(),
        updated_at,
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
    mock_responses_config(&server.uri()).write(codex_home.path())?;

    let mut primary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let start_id = primary
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, primary.read_response(start_id)).await??;

    let materialize_id = primary
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
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

    timeout(DEFAULT_READ_TIMEOUT, primary.shutdown_gracefully()).await??;
    let mut secondary = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let resume_id = secondary
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            model: Some("gpt-5.4".to_string()),
            personality: Some(Personality::Friendly),
            ..Default::default()
        })
        .await?;
    let resume: ThreadResumeResponse =
        timeout(DEFAULT_READ_TIMEOUT, secondary.read_response(resume_id)).await??;
    assert_eq!(resume.thread.status, ThreadStatus::Idle);

    let turn_id = secondary
        .send_turn_start_request(TurnStartParams {
            thread_id: resume.thread.id,
            client_user_message_id: None,
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

fn mock_responses_config(server_uri: &str) -> MockResponsesConfig {
    MockResponsesConfig::new(server_uri)
        .with_model("gpt-5.4")
        .enable_feature(Feature::Personality)
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
}

async fn setup_rollout_fixture(codex_home: &Path, server_uri: &str) -> Result<RolloutFixture> {
    mock_responses_config(server_uri).write(codex_home)?;

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
    let mut session_meta = read_session_meta_line(&rollout_file_path).await?;
    session_meta.meta.multi_agent_version = Some(MultiAgentVersion::V1);
    append_rollout_item_to_path(&rollout_file_path, &RolloutItem::SessionMeta(session_meta))
        .await?;
    set_rollout_mtime(rollout_file_path.as_path(), expected_updated_at_rfc3339)?;
    let before_modified = std::fs::metadata(&rollout_file_path)?.modified()?;
    Ok(RolloutFixture {
        conversation_id,
        rollout_file_path,
        before_modified,
    })
}
