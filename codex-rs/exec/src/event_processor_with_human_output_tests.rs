use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_core::config::ConfigBuilder;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use codex_utils_sandbox_summary::summarize_permission_profile;
use owo_colors::Style;
use pretty_assertions::assert_eq;

use super::EventProcessorWithHumanOutput;
use super::config_summary_entries;
use super::final_message_from_turn_items;
use super::reasoning_text;
use super::should_print_final_message_to_stdout;
use super::should_print_final_message_to_tty;
use crate::event_processor::EventProcessor;

#[test]
fn suppresses_final_stdout_message_when_both_streams_are_terminals() {
    assert!(!should_print_final_message_to_stdout(
        Some("hello"),
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn prints_final_stdout_message_when_stdout_is_not_terminal() {
    assert!(should_print_final_message_to_stdout(
        Some("hello"),
        /*stdout_is_terminal*/ false,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn prints_final_stdout_message_when_stderr_is_not_terminal() {
    assert!(should_print_final_message_to_stdout(
        Some("hello"),
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ false
    ));
}

#[test]
fn suppresses_final_stdout_message_when_missing() {
    assert!(!should_print_final_message_to_stdout(
        /*final_message*/ None, /*stdout_is_terminal*/ false,
        /*stderr_is_terminal*/ false
    ));
}

#[test]
fn prints_final_tty_message_when_not_yet_rendered() {
    assert!(should_print_final_message_to_tty(
        Some("hello"),
        /*final_message_rendered*/ false,
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn suppresses_final_tty_message_when_already_rendered() {
    assert!(!should_print_final_message_to_tty(
        Some("hello"),
        /*final_message_rendered*/ true,
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn reasoning_text_prefers_summary_when_raw_reasoning_is_hidden() {
    let text = reasoning_text(
        &["summary".to_string()],
        &["raw".to_string()],
        /*show_raw_agent_reasoning*/ false,
    );

    assert_eq!(text.as_deref(), Some("summary"));
}

#[test]
fn reasoning_text_uses_raw_content_when_enabled() {
    let text = reasoning_text(
        &["summary".to_string()],
        &["raw".to_string()],
        /*show_raw_agent_reasoning*/ true,
    );

    assert_eq!(text.as_deref(), Some("raw"));
}

#[test]
fn summarizes_disabled_permission_profile_as_danger_full_access() {
    let cwd = test_path_buf("/tmp").abs();

    assert_eq!(
        summarize_permission_profile(
            &PermissionProfile::Disabled,
            &cwd,
            std::slice::from_ref(&cwd),
        ),
        "danger-full-access"
    );
}

#[test]
fn summarizes_external_permission_profile() {
    let cwd = test_path_buf("/tmp").abs();

    assert_eq!(
        summarize_permission_profile(
            &PermissionProfile::External {
                network: NetworkSandboxPolicy::Enabled,
            },
            &cwd,
            std::slice::from_ref(&cwd),
        ),
        "external-sandbox (network access enabled)"
    );
}

#[test]
fn summarizes_managed_workspace_write_permission_profile() {
    let cwd = test_path_buf("/tmp/project").abs();
    let cache_root = test_path_buf("/tmp/cache").abs();
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: cwd.clone() },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: cache_root.clone(),
                },
                access: FileSystemAccessMode::Write,
            },
        ]),
        NetworkSandboxPolicy::Restricted,
    );

    assert_eq!(
        summarize_permission_profile(&profile, &cwd, &[cwd.clone(), cache_root.clone()]),
        format!("workspace-write [workdir, {}]", cache_root.display())
    );
}

#[test]
fn summarizes_managed_read_only_permission_profile() {
    let cwd = test_path_buf("/tmp/project").abs();
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(Vec::new()),
        NetworkSandboxPolicy::Restricted,
    );

    assert_eq!(
        summarize_permission_profile(&profile, &cwd, std::slice::from_ref(&cwd)),
        "read-only"
    );
}

#[tokio::test]
async fn config_summary_entries_include_runtime_workspace_roots() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let cwd = tempfile::tempdir().expect("create cwd");
    let extra_root = tempfile::tempdir().expect("create extra root");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(cwd.path().to_path_buf()))
        .build()
        .await
        .expect("build default config");
    let cwd = cwd.path().to_path_buf().abs();
    let extra_root = extra_root.path().to_path_buf().abs();
    let expected_extra_root_name = extra_root
        .file_name()
        .expect("extra root should have file name")
        .to_string_lossy()
        .to_string();
    config.cwd = cwd.clone();
    config.workspace_roots = vec![cwd.clone(), extra_root];
    config
        .permissions
        .set_workspace_roots(config.workspace_roots.clone());
    config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write_with(
            &[],
            NetworkSandboxPolicy::Restricted,
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        ))
        .expect("set permission profile");

    let session_configured_event = SessionConfiguredEvent {
        session_id: SessionId::new(),
        thread_id: ThreadId::new(),
        forked_from_id: None,
        thread_source: None,
        thread_name: None,
        model: "gpt-5.4".to_string(),
        model_provider_id: config.model_provider_id.clone(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: config.approvals_reviewer,
        permission_profile: config.permissions.effective_permission_profile(),
        active_permission_profile: None,
        cwd,
        reasoning_effort: None,
        initial_messages: None,
        network_proxy: None,
        rollout_path: None,
    };

    let summary_entries = config_summary_entries(&config, &session_configured_event);
    let sandbox_summary = summary_entries
        .iter()
        .find_map(|(key, value)| (*key == "sandbox").then_some(value))
        .expect("sandbox summary entry");
    assert!(
        sandbox_summary.starts_with("workspace-write [workdir, ")
            && sandbox_summary.contains(&expected_extra_root_name),
        "expected runtime workspace root in sandbox summary: {summary_entries:?}"
    );
}

#[test]
fn final_message_from_turn_items_uses_latest_agent_message() {
    let message = final_message_from_turn_items(&[
        ThreadItem::AgentMessage {
            id: "msg-1".to_string(),
            text: "first".to_string(),
            phase: None,
            memory_citation: None,
        },
        ThreadItem::Plan {
            id: "plan-1".to_string(),
            text: "plan".to_string(),
        },
        ThreadItem::AgentMessage {
            id: "msg-2".to_string(),
            text: "second".to_string(),
            phase: None,
            memory_citation: None,
        },
    ]);

    assert_eq!(message.as_deref(), Some("second"));
}

#[test]
fn final_message_from_turn_items_falls_back_to_latest_plan() {
    let message = final_message_from_turn_items(&[
        ThreadItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec!["inspect".to_string()],
            content: Vec::new(),
        },
        ThreadItem::Plan {
            id: "plan-1".to_string(),
            text: "first plan".to_string(),
        },
        ThreadItem::Plan {
            id: "plan-2".to_string(),
            text: "final plan".to_string(),
        },
    ]);

    assert_eq!(message.as_deref(), Some("final plan"));
}

#[test]
fn turn_completed_recovers_final_message_from_turn_items() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: None,
        final_message_rendered: false,
        emit_final_message_on_shutdown: false,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: vec![ThreadItem::AgentMessage {
                    id: "msg-1".to_string(),
                    text: "final answer".to_string(),
                    phase: None,
                    memory_citation: None,
                }],
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message.as_deref(), Some("final answer"));
}

#[test]
fn turn_completed_overwrites_stale_final_message_from_turn_items() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("stale answer".to_string()),
        final_message_rendered: true,
        emit_final_message_on_shutdown: false,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: vec![ThreadItem::AgentMessage {
                    id: "msg-1".to_string(),
                    text: "final answer".to_string(),
                    phase: None,
                    memory_citation: None,
                }],
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message.as_deref(), Some("final answer"));
    assert!(!processor.final_message_rendered);
}

#[test]
fn turn_completed_preserves_streamed_final_message_when_turn_items_are_empty() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("streamed answer".to_string()),
        final_message_rendered: false,
        emit_final_message_on_shutdown: false,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: Vec::new(),
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message.as_deref(), Some("streamed answer"));
    assert!(processor.emit_final_message_on_shutdown);
}

#[test]
fn turn_failed_clears_stale_final_message() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("partial answer".to_string()),
        final_message_rendered: true,
        emit_final_message_on_shutdown: true,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: Vec::new(),
                status: TurnStatus::Failed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message, None);
    assert!(!processor.final_message_rendered);
    assert!(!processor.emit_final_message_on_shutdown);
}

#[test]
fn turn_interrupted_clears_stale_final_message() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("partial answer".to_string()),
        final_message_rendered: true,
        emit_final_message_on_shutdown: true,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: Vec::new(),
                status: TurnStatus::Interrupted,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message, None);
    assert!(!processor.final_message_rendered);
    assert!(!processor.emit_final_message_on_shutdown);
}
