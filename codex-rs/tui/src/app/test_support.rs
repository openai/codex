//! Shared test fixtures and builders for `crate::app` tests.

use super::*;
use crate::app_backtrack::BacktrackState;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::file_search::FileSearchManager;
use crate::legacy_core::config::ConfigOverrides;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::HookCompletedNotification;
use codex_app_server_protocol::HookEventName as AppServerHookEventName;
use codex_app_server_protocol::HookExecutionMode as AppServerHookExecutionMode;
use codex_app_server_protocol::HookHandlerType as AppServerHookHandlerType;
use codex_app_server_protocol::HookOutputEntry as AppServerHookOutputEntry;
use codex_app_server_protocol::HookOutputEntryKind as AppServerHookOutputEntryKind;
use codex_app_server_protocol::HookRunStatus as AppServerHookRunStatus;
use codex_app_server_protocol::HookRunSummary as AppServerHookRunSummary;
use codex_app_server_protocol::HookScope as AppServerHookScope;
use codex_app_server_protocol::HookStartedNotification;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::TokenUsageBreakdown;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use ratatui::prelude::Line;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub(in crate::app) async fn make_test_app() -> App {
    let (chat_widget, app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let config = chat_widget.config_ref().clone();
    let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
    let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
    let session_telemetry = test_session_telemetry(&config, model.as_str());

    App {
        model_catalog: chat_widget.model_catalog(),
        session_telemetry,
        app_event_tx,
        chat_widget,
        config,
        active_profile: None,
        cli_kv_overrides: Vec::new(),
        harness_overrides: ConfigOverrides::default(),
        runtime_approval_policy_override: None,
        runtime_sandbox_policy_override: None,
        file_search,
        transcript_cells: Vec::new(),
        overlay: None,
        deferred_history_lines: Vec::new(),
        has_emitted_history_lines: false,
        enhanced_keys_supported: false,
        commit_anim_running: Arc::new(AtomicBool::new(false)),
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        backtrack: BacktrackState::default(),
        backtrack_render_pending: false,
        feedback: codex_feedback::CodexFeedback::new(),
        feedback_audience: FeedbackAudience::External,
        environment_manager: Arc::new(EnvironmentManager::new(/*exec_server_url*/ None)),
        remote_app_server_url: None,
        remote_app_server_auth_token: None,
        pending_update_action: None,
        pending_shutdown_exit_thread_id: None,
        windows_sandbox: WindowsSandboxState::default(),
        thread_event_channels: HashMap::new(),
        thread_event_listener_tasks: HashMap::new(),
        agent_navigation: AgentNavigationState::default(),
        side_threads: HashMap::new(),
        active_thread_id: None,
        active_thread_rx: None,
        primary_thread_id: None,
        last_subagent_backfill_attempt: None,
        primary_session_configured: None,
        pending_primary_events: VecDeque::new(),
        pending_app_server_requests: PendingAppServerRequests::default(),
        pending_plugin_enabled_writes: HashMap::new(),
    }
}

pub(in crate::app) async fn make_test_app_with_channels() -> (
    App,
    tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    tokio::sync::mpsc::UnboundedReceiver<Op>,
) {
    let (chat_widget, app_event_tx, rx, op_rx) = make_chatwidget_manual_with_sender().await;
    let config = chat_widget.config_ref().clone();
    let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
    let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
    let session_telemetry = test_session_telemetry(&config, model.as_str());

    (
        App {
            model_catalog: chat_widget.model_catalog(),
            session_telemetry,
            app_event_tx,
            chat_widget,
            config,
            active_profile: None,
            cli_kv_overrides: Vec::new(),
            harness_overrides: ConfigOverrides::default(),
            runtime_approval_policy_override: None,
            runtime_sandbox_policy_override: None,
            file_search,
            transcript_cells: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            enhanced_keys_supported: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
            terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            backtrack_render_pending: false,
            feedback: codex_feedback::CodexFeedback::new(),
            feedback_audience: FeedbackAudience::External,
            environment_manager: Arc::new(EnvironmentManager::new(/*exec_server_url*/ None)),
            remote_app_server_url: None,
            remote_app_server_auth_token: None,
            pending_update_action: None,
            pending_shutdown_exit_thread_id: None,
            windows_sandbox: WindowsSandboxState::default(),
            thread_event_channels: HashMap::new(),
            thread_event_listener_tasks: HashMap::new(),
            agent_navigation: AgentNavigationState::default(),
            side_threads: HashMap::new(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            last_subagent_backfill_attempt: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
            pending_app_server_requests: PendingAppServerRequests::default(),
            pending_plugin_enabled_writes: HashMap::new(),
        },
        rx,
        op_rx,
    )
}

pub(in crate::app) fn test_absolute_path(path: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(PathBuf::from(path)).expect("absolute test path")
}

pub(in crate::app) fn test_thread_session(thread_id: ThreadId, cwd: PathBuf) -> ThreadSessionState {
    ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "gpt-test".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        cwd: cwd.abs(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        history_log_id: 0,
        history_entry_count: 0,
        network_proxy: None,
        rollout_path: Some(PathBuf::new()),
    }
}

pub(in crate::app) fn test_turn(turn_id: &str, status: TurnStatus, items: Vec<ThreadItem>) -> Turn {
    Turn {
        id: turn_id.to_string(),
        items,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

pub(in crate::app) fn turn_started_notification(
    thread_id: ThreadId,
    turn_id: &str,
) -> ServerNotification {
    ServerNotification::TurnStarted(TurnStartedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            started_at: Some(0),
            ..test_turn(turn_id, TurnStatus::InProgress, Vec::new())
        },
    })
}

pub(in crate::app) fn turn_completed_notification(
    thread_id: ThreadId,
    turn_id: &str,
    status: TurnStatus,
) -> ServerNotification {
    ServerNotification::TurnCompleted(TurnCompletedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            completed_at: Some(0),
            duration_ms: Some(1),
            ..test_turn(turn_id, status, Vec::new())
        },
    })
}

pub(in crate::app) fn thread_closed_notification(thread_id: ThreadId) -> ServerNotification {
    ServerNotification::ThreadClosed(ThreadClosedNotification {
        thread_id: thread_id.to_string(),
    })
}

pub(in crate::app) fn token_usage_notification(
    thread_id: ThreadId,
    turn_id: &str,
    model_context_window: Option<i64>,
) -> ServerNotification {
    ServerNotification::ThreadTokenUsageUpdated(ThreadTokenUsageUpdatedNotification {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        token_usage: ThreadTokenUsage {
            total: TokenUsageBreakdown {
                total_tokens: 10,
                input_tokens: 4,
                cached_input_tokens: 1,
                output_tokens: 5,
                reasoning_output_tokens: 0,
            },
            last: TokenUsageBreakdown {
                total_tokens: 10,
                input_tokens: 4,
                cached_input_tokens: 1,
                output_tokens: 5,
                reasoning_output_tokens: 0,
            },
            model_context_window,
        },
    })
}

pub(in crate::app) fn hook_started_notification(
    thread_id: ThreadId,
    turn_id: &str,
) -> ServerNotification {
    ServerNotification::HookStarted(HookStartedNotification {
        thread_id: thread_id.to_string(),
        turn_id: Some(turn_id.to_string()),
        run: AppServerHookRunSummary {
            id: "user-prompt-submit:0:/tmp/hooks.json".to_string(),
            event_name: AppServerHookEventName::UserPromptSubmit,
            handler_type: AppServerHookHandlerType::Command,
            execution_mode: AppServerHookExecutionMode::Sync,
            scope: AppServerHookScope::Turn,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: codex_app_server_protocol::HookSource::User,
            display_order: 0,
            status: AppServerHookRunStatus::Running,
            status_message: Some("checking go-workflow input policy".to_string()),
            started_at: 1,
            completed_at: None,
            duration_ms: None,
            entries: Vec::new(),
        },
    })
}

pub(in crate::app) fn hook_completed_notification(
    thread_id: ThreadId,
    turn_id: &str,
) -> ServerNotification {
    ServerNotification::HookCompleted(HookCompletedNotification {
        thread_id: thread_id.to_string(),
        turn_id: Some(turn_id.to_string()),
        run: AppServerHookRunSummary {
            id: "user-prompt-submit:0:/tmp/hooks.json".to_string(),
            event_name: AppServerHookEventName::UserPromptSubmit,
            handler_type: AppServerHookHandlerType::Command,
            execution_mode: AppServerHookExecutionMode::Sync,
            scope: AppServerHookScope::Turn,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: codex_app_server_protocol::HookSource::User,
            display_order: 0,
            status: AppServerHookRunStatus::Stopped,
            status_message: Some("checking go-workflow input policy".to_string()),
            started_at: 1,
            completed_at: Some(11),
            duration_ms: Some(10),
            entries: vec![
                AppServerHookOutputEntry {
                    kind: AppServerHookOutputEntryKind::Warning,
                    text: "go-workflow must start from PlanMode".to_string(),
                },
                AppServerHookOutputEntry {
                    kind: AppServerHookOutputEntryKind::Stop,
                    text: "prompt blocked".to_string(),
                },
            ],
        },
    })
}

pub(in crate::app) fn agent_message_delta_notification(
    thread_id: ThreadId,
    turn_id: &str,
    item_id: &str,
    delta: &str,
) -> ServerNotification {
    ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        delta: delta.to_string(),
    })
}

pub(in crate::app) fn exec_approval_request(
    thread_id: ThreadId,
    turn_id: &str,
    item_id: &str,
    approval_id: Option<&str>,
) -> ServerRequest {
    ServerRequest::CommandExecutionRequestApproval {
        request_id: AppServerRequestId::Integer(1),
        params: CommandExecutionRequestApprovalParams {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item_id: item_id.to_string(),
            approval_id: approval_id.map(str::to_string),
            reason: Some("needs approval".to_string()),
            network_approval_context: None,
            command: Some("echo hello".to_string()),
            cwd: Some(test_path_buf("/tmp/project").abs()),
            command_actions: None,
            additional_permissions: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        },
    }
}

pub(in crate::app) fn request_user_input_request(
    thread_id: ThreadId,
    turn_id: &str,
    item_id: &str,
) -> ServerRequest {
    ServerRequest::ToolRequestUserInput {
        request_id: AppServerRequestId::Integer(2),
        params: ToolRequestUserInputParams {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item_id: item_id.to_string(),
            questions: Vec::new(),
        },
    }
}

pub(in crate::app) fn lines_to_single_string(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn test_session_telemetry(config: &Config, model: &str) -> SessionTelemetry {
    let model_info = crate::legacy_core::test_support::construct_model_info_offline(model, config);
    SessionTelemetry::new(
        ThreadId::new(),
        model,
        model_info.slug.as_str(),
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "test".to_string(),
        SessionSource::Cli,
    )
}
