//! Exercises `ChatWidget` event handling and rendering invariants.
//!
//! These tests treat the widget as the adapter between `codex_protocol::protocol::EventMsg` inputs and
//! the TUI output. Many assertions are snapshot-based so that layout regressions and status/header
//! changes show up as stable, reviewable diffs.

pub(super) use super::*;
pub(super) use crate::app_event::AppEvent;
pub(super) use crate::app_event::ExitMode;
#[cfg(not(target_os = "linux"))]
pub(super) use crate::app_event::RealtimeAudioDeviceKind;
pub(super) use crate::app_event_sender::AppEventSender;
pub(super) use crate::bottom_pane::LocalImageAttachment;
pub(super) use crate::bottom_pane::MentionBinding;
pub(super) use crate::chatwidget::realtime::RealtimeConversationPhase;
pub(super) use crate::history_cell::UserHistoryCell;
pub(super) use crate::model_catalog::ModelCatalog;
pub(super) use crate::test_backend::VT100Backend;
pub(super) use crate::test_support::PathBufExt;
pub(super) use crate::test_support::test_path_display;
pub(super) use crate::tui::FrameRequester;
pub(super) use assert_matches::assert_matches;
pub(super) use codex_app_server_protocol::AdditionalFileSystemPermissions as AppServerAdditionalFileSystemPermissions;
pub(super) use codex_app_server_protocol::AdditionalNetworkPermissions as AppServerAdditionalNetworkPermissions;
pub(super) use codex_app_server_protocol::AdditionalPermissionProfile as AppServerAdditionalPermissionProfile;
pub(super) use codex_app_server_protocol::AppSummary;
pub(super) use codex_app_server_protocol::CollabAgentState as AppServerCollabAgentState;
pub(super) use codex_app_server_protocol::CollabAgentStatus as AppServerCollabAgentStatus;
pub(super) use codex_app_server_protocol::CollabAgentTool as AppServerCollabAgentTool;
pub(super) use codex_app_server_protocol::CollabAgentToolCallStatus as AppServerCollabAgentToolCallStatus;
pub(super) use codex_app_server_protocol::CommandAction as AppServerCommandAction;
pub(super) use codex_app_server_protocol::CommandExecutionRequestApprovalParams as AppServerCommandExecutionRequestApprovalParams;
pub(super) use codex_app_server_protocol::CommandExecutionSource as AppServerCommandExecutionSource;
pub(super) use codex_app_server_protocol::CommandExecutionStatus as AppServerCommandExecutionStatus;
pub(super) use codex_app_server_protocol::ErrorNotification;
pub(super) use codex_app_server_protocol::FileUpdateChange;
pub(super) use codex_app_server_protocol::GuardianApprovalReview;
pub(super) use codex_app_server_protocol::GuardianApprovalReviewAction as AppServerGuardianApprovalReviewAction;
pub(super) use codex_app_server_protocol::GuardianApprovalReviewStatus;
pub(super) use codex_app_server_protocol::GuardianCommandSource as AppServerGuardianCommandSource;
pub(super) use codex_app_server_protocol::GuardianRiskLevel as AppServerGuardianRiskLevel;
pub(super) use codex_app_server_protocol::GuardianUserAuthorization as AppServerGuardianUserAuthorization;
pub(super) use codex_app_server_protocol::HookCompletedNotification as AppServerHookCompletedNotification;
pub(super) use codex_app_server_protocol::HookEventName as AppServerHookEventName;
pub(super) use codex_app_server_protocol::HookExecutionMode as AppServerHookExecutionMode;
pub(super) use codex_app_server_protocol::HookHandlerType as AppServerHookHandlerType;
pub(super) use codex_app_server_protocol::HookOutputEntry as AppServerHookOutputEntry;
pub(super) use codex_app_server_protocol::HookOutputEntryKind as AppServerHookOutputEntryKind;
pub(super) use codex_app_server_protocol::HookRunStatus as AppServerHookRunStatus;
pub(super) use codex_app_server_protocol::HookRunSummary as AppServerHookRunSummary;
pub(super) use codex_app_server_protocol::HookScope as AppServerHookScope;
pub(super) use codex_app_server_protocol::HookStartedNotification as AppServerHookStartedNotification;
pub(super) use codex_app_server_protocol::ItemCompletedNotification;
pub(super) use codex_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification;
pub(super) use codex_app_server_protocol::ItemGuardianApprovalReviewStartedNotification;
pub(super) use codex_app_server_protocol::ItemStartedNotification;
pub(super) use codex_app_server_protocol::MarketplaceInterface;
pub(super) use codex_app_server_protocol::McpServerStartupState;
pub(super) use codex_app_server_protocol::McpServerStatusUpdatedNotification;
pub(super) use codex_app_server_protocol::PatchApplyStatus as AppServerPatchApplyStatus;
pub(super) use codex_app_server_protocol::PatchChangeKind;
pub(super) use codex_app_server_protocol::PermissionsRequestApprovalParams as AppServerPermissionsRequestApprovalParams;
pub(super) use codex_app_server_protocol::PluginAuthPolicy;
pub(super) use codex_app_server_protocol::PluginDetail;
pub(super) use codex_app_server_protocol::PluginInstallPolicy;
pub(super) use codex_app_server_protocol::PluginInterface;
pub(super) use codex_app_server_protocol::PluginListResponse;
pub(super) use codex_app_server_protocol::PluginMarketplaceEntry;
pub(super) use codex_app_server_protocol::PluginReadResponse;
pub(super) use codex_app_server_protocol::PluginSource;
pub(super) use codex_app_server_protocol::PluginSummary;
pub(super) use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
pub(super) use codex_app_server_protocol::ServerNotification;
pub(super) use codex_app_server_protocol::SkillSummary;
pub(super) use codex_app_server_protocol::ThreadClosedNotification;
pub(super) use codex_app_server_protocol::ThreadItem as AppServerThreadItem;
pub(super) use codex_app_server_protocol::Turn as AppServerTurn;
pub(super) use codex_app_server_protocol::TurnCompletedNotification;
pub(super) use codex_app_server_protocol::TurnError as AppServerTurnError;
pub(super) use codex_app_server_protocol::TurnStartedNotification;
pub(super) use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
pub(super) use codex_app_server_protocol::UserInput as AppServerUserInput;
pub(super) use codex_config::types::ApprovalsReviewer;
pub(super) use codex_config::types::Notifications;
#[cfg(target_os = "windows")]
pub(super) use codex_config::types::WindowsSandboxModeToml;
pub(super) use codex_core::config::Config;
pub(super) use codex_core::config::ConfigBuilder;
pub(super) use codex_core::config::Constrained;
pub(super) use codex_core::config::ConstraintError;
pub(super) use codex_core::config_loader::AppRequirementToml;
pub(super) use codex_core::config_loader::AppsRequirementsToml;
pub(super) use codex_core::config_loader::ConfigLayerStack;
pub(super) use codex_core::config_loader::ConfigRequirements;
pub(super) use codex_core::config_loader::ConfigRequirementsToml;
pub(super) use codex_core::config_loader::RequirementSource;
pub(super) use codex_core::plugins::OPENAI_CURATED_MARKETPLACE_NAME;
pub(super) use codex_core::skills::model::SkillMetadata;
pub(super) use codex_features::FEATURES;
pub(super) use codex_features::Feature;
pub(super) use codex_git_utils::CommitLogEntry;
pub(super) use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
pub(super) use codex_otel::RuntimeMetricsSummary;
pub(super) use codex_otel::SessionTelemetry;
pub(super) use codex_protocol::ThreadId;
pub(super) use codex_protocol::account::PlanType;
pub(super) use codex_protocol::config_types::CollaborationMode;
pub(super) use codex_protocol::config_types::ModeKind;
pub(super) use codex_protocol::config_types::Personality;
pub(super) use codex_protocol::config_types::ServiceTier;
pub(super) use codex_protocol::config_types::Settings;
pub(super) use codex_protocol::items::AgentMessageContent;
pub(super) use codex_protocol::items::AgentMessageItem;
pub(super) use codex_protocol::items::PlanItem;
pub(super) use codex_protocol::items::TurnItem;
pub(super) use codex_protocol::items::UserMessageItem;
pub(super) use codex_protocol::models::FileSystemPermissions;
pub(super) use codex_protocol::models::MessagePhase;
pub(super) use codex_protocol::models::NetworkPermissions;
pub(super) use codex_protocol::models::PermissionProfile;
pub(super) use codex_protocol::openai_models::ModelInfo;
pub(super) use codex_protocol::openai_models::ModelPreset;
pub(super) use codex_protocol::openai_models::ModelsResponse;
pub(super) use codex_protocol::openai_models::ReasoningEffortPreset;
pub(super) use codex_protocol::openai_models::default_input_modalities;
pub(super) use codex_protocol::parse_command::ParsedCommand;
pub(super) use codex_protocol::plan_tool::PlanItemArg;
pub(super) use codex_protocol::plan_tool::StepStatus;
pub(super) use codex_protocol::plan_tool::UpdatePlanArgs;
pub(super) use codex_protocol::protocol::AddCreditsNudgeEmailStatus;
pub(super) use codex_protocol::protocol::AgentMessageDeltaEvent;
pub(super) use codex_protocol::protocol::AgentMessageEvent;
pub(super) use codex_protocol::protocol::AgentReasoningDeltaEvent;
pub(super) use codex_protocol::protocol::AgentReasoningEvent;
pub(super) use codex_protocol::protocol::AgentStatus;
pub(super) use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
pub(super) use codex_protocol::protocol::BackgroundEventEvent;
pub(super) use codex_protocol::protocol::CodexErrorInfo;
pub(super) use codex_protocol::protocol::CollabAgentSpawnBeginEvent;
pub(super) use codex_protocol::protocol::CollabAgentSpawnEndEvent;
pub(super) use codex_protocol::protocol::CreditsSnapshot;
pub(super) use codex_protocol::protocol::Event;
pub(super) use codex_protocol::protocol::EventMsg;
pub(super) use codex_protocol::protocol::ExecApprovalRequestEvent;
pub(super) use codex_protocol::protocol::ExecCommandBeginEvent;
pub(super) use codex_protocol::protocol::ExecCommandEndEvent;
pub(super) use codex_protocol::protocol::ExecCommandSource;
pub(super) use codex_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
pub(super) use codex_protocol::protocol::ExecPolicyAmendment;
pub(super) use codex_protocol::protocol::ExitedReviewModeEvent;
pub(super) use codex_protocol::protocol::FileChange;
pub(super) use codex_protocol::protocol::GuardianAssessmentAction;
pub(super) use codex_protocol::protocol::GuardianAssessmentEvent;
pub(super) use codex_protocol::protocol::GuardianAssessmentStatus;
pub(super) use codex_protocol::protocol::GuardianCommandSource;
pub(super) use codex_protocol::protocol::GuardianRiskLevel;
pub(super) use codex_protocol::protocol::GuardianUserAuthorization;
pub(super) use codex_protocol::protocol::ImageGenerationEndEvent;
pub(super) use codex_protocol::protocol::ItemCompletedEvent;
pub(super) use codex_protocol::protocol::McpStartupCompleteEvent;
pub(super) use codex_protocol::protocol::McpStartupStatus;
pub(super) use codex_protocol::protocol::McpStartupUpdateEvent;
pub(super) use codex_protocol::protocol::NonSteerableTurnKind;
pub(super) use codex_protocol::protocol::Op;
pub(super) use codex_protocol::protocol::PatchApplyBeginEvent;
pub(super) use codex_protocol::protocol::PatchApplyEndEvent;
pub(super) use codex_protocol::protocol::PatchApplyStatus as CorePatchApplyStatus;
pub(super) use codex_protocol::protocol::RateLimitWindow;
pub(super) use codex_protocol::protocol::ReadOnlyAccess;
pub(super) use codex_protocol::protocol::RealtimeConversationClosedEvent;
pub(super) use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
pub(super) use codex_protocol::protocol::RealtimeEvent;
pub(super) use codex_protocol::protocol::ReviewRequest;
pub(super) use codex_protocol::protocol::ReviewTarget;
pub(super) use codex_protocol::protocol::SessionConfiguredEvent;
pub(super) use codex_protocol::protocol::SessionSource;
pub(super) use codex_protocol::protocol::SkillScope;
pub(super) use codex_protocol::protocol::StreamErrorEvent;
pub(super) use codex_protocol::protocol::TerminalInteractionEvent;
pub(super) use codex_protocol::protocol::ThreadRolledBackEvent;
pub(super) use codex_protocol::protocol::TokenCountEvent;
pub(super) use codex_protocol::protocol::TokenUsage;
pub(super) use codex_protocol::protocol::TokenUsageInfo;
pub(super) use codex_protocol::protocol::TurnCompleteEvent;
pub(super) use codex_protocol::protocol::TurnStartedEvent;
pub(super) use codex_protocol::protocol::UndoCompletedEvent;
pub(super) use codex_protocol::protocol::UndoStartedEvent;
pub(super) use codex_protocol::protocol::ViewImageToolCallEvent;
pub(super) use codex_protocol::protocol::WarningEvent;
pub(super) use codex_protocol::request_permissions::RequestPermissionProfile;
pub(super) use codex_protocol::request_user_input::RequestUserInputEvent;
pub(super) use codex_protocol::request_user_input::RequestUserInputQuestion;
pub(super) use codex_protocol::request_user_input::RequestUserInputQuestionOption;
pub(super) use codex_protocol::user_input::TextElement;
pub(super) use codex_protocol::user_input::UserInput;
pub(super) use codex_terminal_detection::Multiplexer;
pub(super) use codex_terminal_detection::TerminalInfo;
pub(super) use codex_terminal_detection::TerminalName;
pub(super) use codex_utils_absolute_path::AbsolutePathBuf;
pub(super) use codex_utils_approval_presets::builtin_approval_presets;
pub(super) use crossterm::event::KeyCode;
pub(super) use crossterm::event::KeyEvent;
pub(super) use crossterm::event::KeyModifiers;
pub(super) use insta::assert_snapshot;
pub(super) use serde_json::json;
#[cfg(target_os = "windows")]
pub(super) use serial_test::serial;
pub(super) use std::collections::BTreeMap;
pub(super) use std::collections::HashMap;
pub(super) use std::collections::HashSet;
pub(super) use std::path::PathBuf;
pub(super) use tempfile::NamedTempFile;
pub(super) use tempfile::tempdir;
pub(super) use tokio::sync::mpsc::error::TryRecvError;
pub(super) use tokio::sync::mpsc::unbounded_channel;
pub(super) use toml::Value as TomlValue;

pub(super) fn chatwidget_snapshot_dir() -> PathBuf {
    let snapshot_file = codex_utils_cargo_bin::find_resource!(
        "src/chatwidget/snapshots/codex_tui__chatwidget__tests__chatwidget_tall.snap"
    )
    .expect("snapshot file");
    snapshot_file
        .parent()
        .unwrap_or_else(|| panic!("snapshot file has no parent: {}", snapshot_file.display()))
        .to_path_buf()
}

macro_rules! assert_chatwidget_snapshot {
    ($name:expr, $value:expr $(,)?) => {{
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_path(crate::chatwidget::tests::chatwidget_snapshot_dir());
        settings.bind(|| {
            insta::assert_snapshot!(format!("codex_tui__chatwidget__tests__{}", $name), $value);
        });
    }};
    ($name:expr, $value:expr, @$snapshot:literal $(,)?) => {{
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_path(crate::chatwidget::tests::chatwidget_snapshot_dir());
        settings.bind(|| {
            insta::assert_snapshot!(
                format!("codex_tui__chatwidget__tests__{}", $name),
                &($value),
                @$snapshot
            );
        });
    }};
}

#[tokio::test]
async fn exec_approval_uses_approval_id_when_present() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "sub-short".into(),
        msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id: "call-parent".into(),
            approval_id: Some("approval-subcommand".into()),
            turn_id: "turn-short".into(),
            command: vec!["bash".into(), "-lc".into(), "echo hello world".into()],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            reason: Some(
                "this is a test reason such as one that would be produced by the model".into(),
            ),
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            additional_permissions: None,
            available_decisions: None,
            parsed_cmd: vec![],
        }),
    });

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    let mut found = false;
    while let Ok(app_ev) = rx.try_recv() {
        if let AppEvent::SubmitThreadOp {
            op: Op::ExecApproval { id, decision, .. },
            ..
        } = app_ev
        {
            assert_eq!(id, "approval-subcommand");
            assert_matches!(decision, codex_protocol::protocol::ReviewDecision::Approved);
            found = true;
            break;
        }
    }
    assert!(found, "expected ExecApproval op to be sent");
}

#[tokio::test]
async fn exec_approval_decision_truncates_multiline_and_long_commands() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Multiline command: modal should show full command, history records decision only
    let ev_multi = ExecApprovalRequestEvent {
        call_id: "call-multi".into(),
        approval_id: Some("call-multi".into()),
        turn_id: "turn-multi".into(),
        command: vec!["bash".into(), "-lc".into(), "echo line1\necho line2".into()],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        reason: Some(
            "this is a test reason such as one that would be produced by the model".into(),
        ),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-multi".into(),
        msg: EventMsg::ExecApprovalRequest(ev_multi),
    });
    let proposed_multi = drain_insert_history(&mut rx);
    assert!(
        proposed_multi.is_empty(),
        "expected multiline approval request to render via modal without emitting history cells"
    );

    let area = Rect::new(0, 0, 80, chat.desired_height(/*width*/ 80));
    let mut buf = ratatui::buffer::Buffer::empty(area);
    chat.render(area, &mut buf);
    let mut saw_first_line = false;
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if row.contains("echo line1") {
            saw_first_line = true;
            break;
        }
    }
    assert!(
        saw_first_line,
        "expected modal to show first line of multiline snippet"
    );

    // Deny via keyboard; decision snippet should be single-line and elided with " ..."
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    let aborted_multi = drain_insert_history(&mut rx)
        .pop()
        .expect("expected aborted decision cell (multiline)");
    assert_snapshot!(
        "exec_approval_history_decision_aborted_multiline",
        lines_to_single_string(&aborted_multi)
    );

    // Very long single-line command: decision snippet should be truncated <= 80 chars with trailing ...
    let long = format!("echo {}", "a".repeat(200));
    let ev_long = ExecApprovalRequestEvent {
        call_id: "call-long".into(),
        approval_id: Some("call-long".into()),
        turn_id: "turn-long".into(),
        command: vec!["bash".into(), "-lc".into(), long],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        reason: None,
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-long".into(),
        msg: EventMsg::ExecApprovalRequest(ev_long),
    });
    let proposed_long = drain_insert_history(&mut rx);
    assert!(
        proposed_long.is_empty(),
        "expected long approval request to avoid emitting history cells before decision"
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    let aborted_long = drain_insert_history(&mut rx)
        .pop()
        .expect("expected aborted decision cell (long)");
    assert_snapshot!(
        "exec_approval_history_decision_aborted_long",
        lines_to_single_string(&aborted_long)
    );
}

// --- Small helpers to tersely drive exec begin/end and snapshot active cell ---
fn begin_exec_with_source(
    chat: &mut ChatWidget,
    call_id: &str,
    raw_cmd: &str,
    source: ExecCommandSource,
) -> ExecCommandBeginEvent {
    // Build the full command vec and parse it using core's parser,
    // then convert to protocol variants for the event payload.
    let command = vec!["bash".to_string(), "-lc".to_string(), raw_cmd.to_string()];
    let parsed_cmd: Vec<ParsedCommand> =
        codex_shell_command::parse_command::parse_command(&command);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let interaction_input = None;
    let event = ExecCommandBeginEvent {
        call_id: call_id.to_string(),
        process_id: None,
        turn_id: "turn-1".to_string(),
        command,
        cwd,
        parsed_cmd,
        source,
        interaction_input,
    };
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::ExecCommandBegin(event.clone()),
    });
    event
}

fn begin_unified_exec_startup(
    chat: &mut ChatWidget,
    call_id: &str,
    process_id: &str,
    raw_cmd: &str,
) -> ExecCommandBeginEvent {
    let command = vec!["bash".to_string(), "-lc".to_string(), raw_cmd.to_string()];
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let event = ExecCommandBeginEvent {
        call_id: call_id.to_string(),
        process_id: Some(process_id.to_string()),
        turn_id: "turn-1".to_string(),
        command,
        cwd,
        parsed_cmd: Vec::new(),
        source: ExecCommandSource::UnifiedExecStartup,
        interaction_input: None,
    };
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::ExecCommandBegin(event.clone()),
    });
    event
}

fn terminal_interaction(chat: &mut ChatWidget, call_id: &str, process_id: &str, stdin: &str) {
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::TerminalInteraction(TerminalInteractionEvent {
            call_id: call_id.to_string(),
            process_id: process_id.to_string(),
            stdin: stdin.to_string(),
        }),
    });
}

fn complete_assistant_message(
    chat: &mut ChatWidget,
    item_id: &str,
    text: &str,
    phase: Option<MessagePhase>,
) {
    chat.handle_codex_event(Event {
        id: format!("raw-{item_id}"),
        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::AgentMessage(AgentMessageItem {
                id: item_id.to_string(),
                content: vec![AgentMessageContent::Text {
                    text: text.to_string(),
                }],
                phase,
                memory_citation: None,
            }),
        }),
    });
}

fn pending_steer(text: &str) -> PendingSteer {
    PendingSteer {
        user_message: UserMessage::from(text),
        compare_key: PendingSteerCompareKey {
            message: text.to_string(),
            image_count: 0,
        },
    }
}

fn complete_user_message(chat: &mut ChatWidget, item_id: &str, text: &str) {
    complete_user_message_for_inputs(
        chat,
        item_id,
        vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
    );
}

fn complete_user_message_for_inputs(chat: &mut ChatWidget, item_id: &str, content: Vec<UserInput>) {
    chat.handle_codex_event(Event {
        id: format!("raw-{item_id}"),
        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::UserMessage(UserMessageItem {
                id: item_id.to_string(),
                content,
            }),
        }),
    });
}

fn begin_exec(chat: &mut ChatWidget, call_id: &str, raw_cmd: &str) -> ExecCommandBeginEvent {
    begin_exec_with_source(chat, call_id, raw_cmd, ExecCommandSource::Agent)
}

fn end_exec(
    chat: &mut ChatWidget,
    begin_event: ExecCommandBeginEvent,
    stdout: &str,
    stderr: &str,
    exit_code: i32,
) {
    let aggregated = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}{stderr}")
    };
    let ExecCommandBeginEvent {
        call_id,
        turn_id,
        command,
        cwd,
        parsed_cmd,
        source,
        interaction_input,
        process_id,
    } = begin_event;
    chat.handle_codex_event(Event {
        id: call_id.clone(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id,
            process_id,
            turn_id,
            command,
            cwd,
            parsed_cmd,
            source,
            interaction_input,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            aggregated_output: aggregated.clone(),
            exit_code,
            duration: std::time::Duration::from_millis(5),
            formatted_output: aggregated,
            status: if exit_code == 0 {
                CoreExecCommandStatus::Completed
            } else {
                CoreExecCommandStatus::Failed
            },
        }),
    });
}

fn active_blob(chat: &ChatWidget) -> String {
    let lines = chat
        .active_cell
        .as_ref()
        .expect("active cell present")
        .display_lines(/*width*/ 80);
    lines_to_single_string(&lines)
}

fn get_available_model(chat: &ChatWidget, model: &str) -> ModelPreset {
    let models = chat
        .model_catalog
        .try_list_models()
        .expect("models lock available");
    models
        .iter()
        .find(|&preset| preset.model == model)
        .cloned()
        .unwrap_or_else(|| panic!("{model} preset not found"))
}

#[tokio::test]
async fn empty_enter_during_task_does_not_queue() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Simulate running task so submissions would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Press Enter with an empty composer.
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Ensure nothing was queued.
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn restore_thread_input_state_syncs_sleep_inhibitor_state() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::PreventIdleSleep, /*enabled*/ true);

    chat.restore_thread_input_state(Some(ThreadInputState {
        composer: None,
        pending_steers: VecDeque::new(),
        rejected_steers_queue: VecDeque::new(),
        queued_user_messages: VecDeque::new(),
        current_collaboration_mode: chat.current_collaboration_mode.clone(),
        active_collaboration_mask: chat.active_collaboration_mask.clone(),
        task_running: true,
        agent_turn_running: true,
    }));

    assert!(chat.agent_turn_running);
    assert!(chat.turn_sleep_inhibitor.is_turn_running());
    assert!(chat.bottom_pane.is_task_running());

    chat.restore_thread_input_state(/*input_state*/ None);

    assert!(!chat.agent_turn_running);
    assert!(!chat.turn_sleep_inhibitor.is_turn_running());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn restore_thread_input_state_restores_pending_steers_without_downgrading_them() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut pending_steers = VecDeque::new();
    pending_steers.push_back(UserMessage::from("pending steer"));
    let mut rejected_steers_queue = VecDeque::new();
    rejected_steers_queue.push_back(UserMessage::from("already rejected"));
    let mut queued_user_messages = VecDeque::new();
    queued_user_messages.push_back(UserMessage::from("queued draft"));

    chat.restore_thread_input_state(Some(ThreadInputState {
        composer: None,
        pending_steers,
        rejected_steers_queue,
        queued_user_messages,
        current_collaboration_mode: chat.current_collaboration_mode.clone(),
        active_collaboration_mask: chat.active_collaboration_mask.clone(),
        task_running: false,
        agent_turn_running: false,
    }));

    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["already rejected", "queued draft"]
    );
    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "pending steer"
    );
}

#[tokio::test]
async fn alt_up_edits_most_recent_queued_message() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.queued_message_edit_binding = crate::key_hint::alt(KeyCode::Up);
    chat.bottom_pane
        .set_queued_message_edit_binding(crate::key_hint::alt(KeyCode::Up));

    // Simulate a running task so messages would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Seed two queued messages.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()));
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()));
    chat.refresh_pending_input_preview();

    // Press Alt+Up to edit the most recent (last) queued message.
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));

    // Composer should now contain the last queued message.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "second queued".to_string()
    );
    // And the queue should now contain only the remaining (older) item.
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "first queued"
    );
}

async fn assert_shift_left_edits_most_recent_queued_message_for_terminal(
    terminal_info: TerminalInfo,
) {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.queued_message_edit_binding = queued_message_edit_binding_for_terminal(terminal_info);
    chat.bottom_pane
        .set_queued_message_edit_binding(chat.queued_message_edit_binding);

    // Simulate a running task so messages would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Seed two queued messages.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()));
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()));
    chat.refresh_pending_input_preview();

    // Press Shift+Left to edit the most recent (last) queued message.
    chat.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));

    // Composer should now contain the last queued message.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "second queued".to_string()
    );
    // And the queue should now contain only the remaining (older) item.
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "first queued"
    );
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_apple_terminal() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::AppleTerminal,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    })
    .await;
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_warp_terminal() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::WarpTerminal,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    })
    .await;
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_vscode_terminal() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::VsCode,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    })
    .await;
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_tmux() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::Iterm2,
        term_program: None,
        version: None,
        term: None,
        multiplexer: Some(Multiplexer::Tmux { version: None }),
    })
    .await;
}

#[test]
fn queued_message_edit_binding_mapping_covers_special_terminals_and_tmux() {
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::AppleTerminal,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::WarpTerminal,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::VsCode,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::Iterm2,
            term_program: None,
            version: None,
            term: None,
            multiplexer: Some(Multiplexer::Tmux { version: None }),
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::Iterm2,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::alt(KeyCode::Up)
    );
}

/// Pressing Up to recall the most recent history entry and immediately queuing
/// it while a task is running should always enqueue the same text, even when it
/// is queued repeatedly.
#[tokio::test]
async fn enqueueing_history_prompt_multiple_times_is_stable() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    // Submit an initial prompt to seed history.
    chat.bottom_pane
        .set_composer_text("repeat me".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Simulate an active task so further submissions are queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    for _ in 0..3 {
        // Recall the prompt from history and ensure it is what we expect.
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(chat.bottom_pane.composer_text(), "repeat me");

        // Queue the prompt while the task is running.
        chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    }

    assert_eq!(chat.queued_user_messages.len(), 3);
    for message in chat.queued_user_messages.iter() {
        assert_eq!(message.text, "repeat me");
    }
}

#[tokio::test]
async fn streaming_final_answer_keeps_task_running_state() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());
    chat.on_commit_tick();
    drain_insert_history(&mut rx);

    assert!(chat.bottom_pane.is_task_running());
    assert!(!chat.bottom_pane.status_indicator_visible());

    chat.bottom_pane
        .set_composer_text("queued submission".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued submission"
    );
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    match op_rx.try_recv() {
        Ok(Op::Interrupt) => {}
        other => panic!("expected Op::Interrupt, got {other:?}"),
    }
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());
}

#[tokio::test]
async fn idle_commit_ticks_do_not_restore_status_without_commentary_completion() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.on_task_started();
    assert!(chat.bottom_pane.status_indicator_visible());

    chat.on_agent_message_delta("Final answer line\n".to_string());
    chat.on_commit_tick();
    drain_insert_history(&mut rx);

    assert!(!chat.bottom_pane.status_indicator_visible());
    assert!(chat.bottom_pane.is_task_running());

    // A second idle tick should not toggle the row back on and cause jitter.
    chat.on_commit_tick();
    assert!(!chat.bottom_pane.status_indicator_visible());
}

#[tokio::test]
async fn commentary_completion_restores_status_indicator_before_exec_begin() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.on_task_started();
    assert!(chat.bottom_pane.status_indicator_visible());

    chat.on_agent_message_delta("Preamble line\n".to_string());
    chat.on_commit_tick();
    drain_insert_history(&mut rx);

    assert!(!chat.bottom_pane.status_indicator_visible());

    complete_assistant_message(
        &mut chat,
        "msg-commentary",
        "Preamble line\n",
        Some(MessagePhase::Commentary),
    );

    assert!(chat.bottom_pane.status_indicator_visible());
    assert!(chat.bottom_pane.is_task_running());

    begin_exec(&mut chat, "call-1", "echo hi");
    assert!(chat.bottom_pane.status_indicator_visible());
}

#[tokio::test]
async fn plan_completion_restores_status_indicator_after_streaming_plan_output() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.on_task_started();
    assert!(chat.bottom_pane.status_indicator_visible());

    chat.on_plan_delta("- Step 1\n".to_string());
    chat.on_commit_tick();
    drain_insert_history(&mut rx);

    assert!(!chat.bottom_pane.status_indicator_visible());
    assert!(chat.bottom_pane.is_task_running());

    chat.on_plan_item_completed("- Step 1\n".to_string());

    assert!(chat.bottom_pane.status_indicator_visible());
    assert!(chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn preamble_keeps_working_status_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    // Regression sequence: a preamble line is committed to history before any exec/tool event.
    // After commentary completes, the status row should be restored before subsequent work.
    chat.on_task_started();
    chat.on_agent_message_delta("Preamble line\n".to_string());
    chat.on_commit_tick();
    drain_insert_history(&mut rx);
    complete_assistant_message(
        &mut chat,
        "msg-commentary-snapshot",
        "Preamble line\n",
        Some(MessagePhase::Commentary),
    );

    let height = chat.desired_height(/*width*/ 80);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, height))
        .expect("create terminal");
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("draw preamble + status widget");
    assert_snapshot!(
        "preamble_keeps_working_status",
        normalized_backend_snapshot(terminal.backend())
    );
}

#[tokio::test]
async fn unified_exec_begin_restores_status_indicator_after_preamble() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.on_task_started();
    assert!(chat.bottom_pane.status_indicator_visible());

    // Simulate a hidden status row during an active turn.
    chat.bottom_pane.hide_status_indicator();
    assert!(!chat.bottom_pane.status_indicator_visible());
    assert!(chat.bottom_pane.is_task_running());

    begin_unified_exec_startup(&mut chat, "call-1", "proc-1", "sleep 2");

    assert!(chat.bottom_pane.status_indicator_visible());
}

#[tokio::test]
async fn unified_exec_begin_restores_working_status_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.on_task_started();
    chat.on_agent_message_delta("Preamble line\n".to_string());
    chat.on_commit_tick();
    drain_insert_history(&mut rx);

    begin_unified_exec_startup(&mut chat, "call-1", "proc-1", "sleep 2");

    let width: u16 = 80;
    let height = chat.desired_height(width);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("create terminal");
    terminal.set_viewport_area(Rect::new(0, 0, width, height));
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("draw chatwidget");
    assert_snapshot!(
        "unified_exec_begin_restores_working_status",
        normalized_backend_snapshot(terminal.backend())
    );
}

#[tokio::test]
async fn single_line_final_answer_hides_working_status_in_transcript_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    chat.on_user_message_event(UserMessageEvent {
        message: "count to 1".to_string(),
        images: None,
        local_images: Vec::new(),
        text_elements: Vec::new(),
    });
    chat.on_task_started();
    complete_assistant_message(
        &mut chat,
        "msg-final-single-line",
        "1",
        Some(MessagePhase::FinalAnswer),
    );

    assert!(!chat.bottom_pane.status_indicator_visible());

    let width: u16 = 40;
    let vt_height: u16 = 10;
    let ui_height: u16 = chat.desired_height(width);
    let viewport = Rect::new(0, vt_height - ui_height - 1, width, ui_height);

    let backend = VT100Backend::new(width, vt_height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    term.set_viewport_area(viewport);

    for lines in drain_insert_history(&mut rx) {
        crate::insert_history::insert_history_lines(&mut term, lines)
            .expect("Failed to insert history lines in test");
    }

    term.draw(|f| {
        chat.render(f.area(), f.buffer_mut());
    })
    .expect("draw chatwidget");

    assert_snapshot!(term.backend().vt100().screen().contents());
}

#[tokio::test]
async fn steer_enter_queues_while_plan_stream_is_active() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.on_task_started();
    chat.on_plan_delta("- Step 1".to_string());
    let _ = drain_insert_history(&mut rx);

    chat.bottom_pane
        .set_composer_text("queued submission".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued submission"
    );
    assert!(chat.pending_steers.is_empty());
    assert_no_submit_op(&mut op_rx);
    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn submit_user_message_queues_while_compaction_turn_is_running() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: thread_id.to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );

    chat.submit_user_message(UserMessage::from("queued while compacting"));

    assert_eq!(chat.pending_steers.len(), 1);
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued while compacting".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected running-turn compact steer submit, got {other:?}"),
    }

    chat.handle_codex_event(Event {
        id: "steer-rejected".into(),
        msg: EventMsg::Error(ErrorEvent {
            message: "cannot steer a compact turn".to_string(),
            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Compact,
            }),
        }),
    });

    assert!(chat.pending_steers.is_empty());
    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["queued while compacting"]
    );

    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::Completed,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued while compacting".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued compact follow-up Op::UserTurn, got {other:?}"),
    }
}

#[tokio::test]
async fn slash_compact_eagerly_queues_follow_up_before_turn_start() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Compact);

    assert!(chat.bottom_pane.is_task_running());
    match rx.try_recv() {
        Ok(AppEvent::CodexOp(Op::Compact)) => {}
        other => panic!("expected compact op to be submitted, got {other:?}"),
    }

    chat.bottom_pane.set_composer_text(
        "queued before compact turn start".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.pending_steers.is_empty());
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued before compact turn start"
    );
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn steer_enter_uses_pending_steers_while_turn_is_running_without_streaming() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();

    chat.bottom_pane
        .set_composer_text("queued while running".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "queued while running"
    );
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    complete_user_message(&mut chat, "user-1", "queued while running");

    assert!(chat.pending_steers.is_empty());
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("queued while running"));
}

#[tokio::test]
async fn steer_enter_uses_pending_steers_while_final_answer_stream_is_active() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    // Keep the assistant stream open (no commit tick/finalize) to model the repro window:
    // user presses Enter while the final answer is still streaming.
    chat.on_agent_message_delta("Final answer line\n".to_string());

    chat.bottom_pane.set_composer_text(
        "queued while streaming".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "queued while streaming"
    );
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    complete_user_message(&mut chat, "user-1", "queued while streaming");

    assert!(chat.pending_steers.is_empty());
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("queued while streaming"));
}

#[tokio::test]
async fn failed_pending_steer_submit_does_not_add_pending_preview() {
    let (mut chat, mut rx, op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    drop(op_rx);

    chat.bottom_pane.set_composer_text(
        "queued while streaming".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.pending_steers.is_empty());
    assert!(chat.queued_user_messages.is_empty());
    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn live_legacy_agent_message_after_item_completed_does_not_duplicate_assistant_message() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    complete_assistant_message(
        &mut chat,
        "msg-live",
        "hello",
        Some(MessagePhase::FinalAnswer),
    );
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("hello"));

    chat.handle_codex_event(Event {
        id: "legacy-live".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "hello".into(),
            phase: Some(MessagePhase::FinalAnswer),
            memory_citation: None,
        }),
    });

    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn live_app_server_user_message_item_completed_does_not_duplicate_rendered_prompt() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    chat.bottom_pane
        .set_composer_text("Hi, are you there?".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("Hi, are you there?"));

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::UserMessage {
                id: "user-1".to_string(),
                content: vec![AppServerUserInput::Text {
                    text: "Hi, are you there?".to_string(),
                    text_elements: Vec::new(),
                }],
            },
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn live_app_server_turn_completed_clears_working_status_after_answer_item() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );

    assert!(chat.bottom_pane.is_task_running());
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::AgentMessage {
                id: "msg-1".to_string(),
                text: "Yes. What do you need?".to_string(),
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: None,
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1);
    assert!(lines_to_single_string(&cells[0]).contains("Yes. What do you need?"));
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::Completed,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );

    assert!(!chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_widget().is_none());
}

#[tokio::test]
async fn live_app_server_file_change_item_started_preserves_changes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::FileChange {
                id: "patch-1".to_string(),
                changes: vec![FileUpdateChange {
                    path: "foo.txt".to_string(),
                    kind: PatchChangeKind::Add,
                    diff: "hello\n".to_string(),
                }],
                status: AppServerPatchApplyStatus::InProgress,
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert!(!cells.is_empty(), "expected patch history to be rendered");
    let transcript = lines_to_single_string(cells.last().expect("patch cell"));
    assert!(
        transcript.contains("Added foo.txt") || transcript.contains("Edited foo.txt"),
        "expected patch summary to include foo.txt, got: {transcript}"
    );
}

#[tokio::test]
async fn live_app_server_command_execution_strips_shell_wrapper() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let script = r#"python3 -c 'print("Hello, world!")'"#;
    let command =
        shlex::try_join(["/bin/zsh", "-lc", script]).expect("round-trippable shell wrapper");

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CommandExecution {
                id: "cmd-1".to_string(),
                command: command.clone(),
                cwd: PathBuf::from("/tmp"),
                process_id: None,
                source: AppServerCommandExecutionSource::UserShell,
                status: AppServerCommandExecutionStatus::InProgress,
                command_actions: vec![AppServerCommandAction::Unknown {
                    command: script.to_string(),
                }],
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CommandExecution {
                id: "cmd-1".to_string(),
                command,
                cwd: PathBuf::from("/tmp"),
                process_id: None,
                source: AppServerCommandExecutionSource::UserShell,
                status: AppServerCommandExecutionStatus::Completed,
                command_actions: vec![AppServerCommandAction::Unknown {
                    command: script.to_string(),
                }],
                aggregated_output: Some("Hello, world!\n".to_string()),
                exit_code: Some(0),
                duration_ms: Some(5),
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected one completed command history cell"
    );
    let blob = lines_to_single_string(cells.first().expect("command cell"));
    assert_snapshot!(
        "live_app_server_command_execution_strips_shell_wrapper",
        blob
    );
}

#[test]
fn app_server_patch_changes_to_core_preserves_diffs() {
    let changes = app_server_patch_changes_to_core(vec![FileUpdateChange {
        path: "foo.txt".to_string(),
        kind: PatchChangeKind::Add,
        diff: "hello\n".to_string(),
    }]);

    assert_eq!(
        changes,
        HashMap::from([(
            PathBuf::from("foo.txt"),
            FileChange::Add {
                content: "hello\n".to_string(),
            },
        )])
    );
}

#[tokio::test]
async fn live_app_server_collab_wait_items_render_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let sender_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b90000000001").expect("valid thread id");
    let receiver_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b958ce5dc1cc").expect("valid thread id");
    let other_receiver_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b96db334332d").expect("valid thread id");
    chat.set_collab_agent_metadata(
        receiver_thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
    );
    chat.set_collab_agent_metadata(
        other_receiver_thread_id,
        Some("Ada".to_string()),
        Some("reviewer".to_string()),
    );

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "wait-1".to_string(),
                tool: AppServerCollabAgentTool::Wait,
                status: AppServerCollabAgentToolCallStatus::InProgress,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![
                    receiver_thread_id.to_string(),
                    other_receiver_thread_id.to_string(),
                ],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::new(),
            },
        }),
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "wait-1".to_string(),
                tool: AppServerCollabAgentTool::Wait,
                status: AppServerCollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![
                    receiver_thread_id.to_string(),
                    other_receiver_thread_id.to_string(),
                ],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::from([
                    (
                        receiver_thread_id.to_string(),
                        AppServerCollabAgentState {
                            status: AppServerCollabAgentStatus::Completed,
                            message: Some("Done".to_string()),
                        },
                    ),
                    (
                        other_receiver_thread_id.to_string(),
                        AppServerCollabAgentState {
                            status: AppServerCollabAgentStatus::Running,
                            message: None,
                        },
                    ),
                ]),
            },
        }),
        /*replay_kind*/ None,
    );

    let combined = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert_snapshot!("app_server_collab_wait_items_render_history", combined);
}

#[tokio::test]
async fn live_app_server_collab_spawn_completed_renders_requested_model_and_effort() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let sender_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b90000000002").expect("valid thread id");
    let spawned_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b91781b41a8e").expect("valid thread id");

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "spawn-1".to_string(),
                tool: AppServerCollabAgentTool::SpawnAgent,
                status: AppServerCollabAgentToolCallStatus::InProgress,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: Vec::new(),
                prompt: Some("Explore the repo".to_string()),
                model: Some("gpt-5".to_string()),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                agents_states: HashMap::new(),
            },
        }),
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "spawn-1".to_string(),
                tool: AppServerCollabAgentTool::SpawnAgent,
                status: AppServerCollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![spawned_thread_id.to_string()],
                prompt: Some("Explore the repo".to_string()),
                model: Some("gpt-5".to_string()),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                agents_states: HashMap::from([(
                    spawned_thread_id.to_string(),
                    AppServerCollabAgentState {
                        status: AppServerCollabAgentStatus::PendingInit,
                        message: None,
                    },
                )]),
            },
        }),
        /*replay_kind*/ None,
    );

    let combined = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert_snapshot!(
        "app_server_collab_spawn_completed_renders_requested_model_and_effort",
        combined
    );
}

#[tokio::test]
async fn live_app_server_failed_turn_does_not_duplicate_error_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "permission denied".to_string(),
                codex_error_info: None,
                additional_details: None,
            },
            will_retry: false,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        /*replay_kind*/ None,
    );

    let first_cells = drain_insert_history(&mut rx);
    assert_eq!(first_cells.len(), 1);
    assert!(lines_to_single_string(&first_cells[0]).contains("permission denied"));

    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::Failed,
                error: Some(AppServerTurnError {
                    message: "permission denied".to_string(),
                    codex_error_info: None,
                    additional_details: None,
                }),
            },
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn replayed_retryable_app_server_error_keeps_turn_running() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        }),
        Some(ReplayKind::ThreadSnapshot),
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "Reconnecting... 1/5".to_string(),
                codex_error_info: None,
                additional_details: Some("Idle timeout waiting for SSE".to_string()),
            },
            will_retry: true,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        Some(ReplayKind::ThreadSnapshot),
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
    assert_eq!(status.details(), None);
}

#[tokio::test]
async fn live_app_server_stream_recovery_restores_previous_status_header() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "Reconnecting... 1/5".to_string(),
                codex_error_info: Some(CodexErrorInfo::Other.into()),
                additional_details: None,
            },
            will_retry: true,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        /*replay_kind*/ None,
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::AgentMessageDelta(
            codex_app_server_protocol::AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                delta: "hello".to_string(),
            },
        ),
        /*replay_kind*/ None,
    );

    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
    assert_eq!(status.details(), None);
    assert!(chat.retry_status_header.is_none());
}

#[tokio::test]
async fn live_app_server_server_overloaded_error_renders_warning() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "server overloaded".to_string(),
                codex_error_info: Some(CodexErrorInfo::ServerOverloaded.into()),
                additional_details: None,
            },
            will_retry: false,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1);
    assert_eq!(lines_to_single_string(&cells[0]), "⚠ server overloaded\n");
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn live_app_server_invalid_thread_name_update_is_ignored() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.thread_name = Some("original name".to_string());

    chat.handle_server_notification(
        ServerNotification::ThreadNameUpdated(
            codex_app_server_protocol::ThreadNameUpdatedNotification {
                thread_id: "not-a-thread-id".to_string(),
                thread_name: Some("bad update".to_string()),
            },
        ),
        /*replay_kind*/ None,
    );

    assert_eq!(chat.thread_id, Some(thread_id));
    assert_eq!(chat.thread_name, Some("original name".to_string()));
}

#[tokio::test]
async fn live_app_server_thread_closed_requests_immediate_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: "thread-1".to_string(),
        }),
        /*replay_kind*/ None,
    );

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::Immediate)));
}

#[tokio::test]
async fn replayed_thread_closed_notification_does_not_exit_tui() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: "thread-1".to_string(),
        }),
        Some(ReplayKind::ThreadSnapshot),
    );

    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn replayed_reasoning_item_hides_raw_reasoning_when_disabled() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.show_raw_agent_reasoning = false;
    chat.handle_codex_event(Event {
        id: "configured".into(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "test-model".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            cwd: test_project_path(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.replay_thread_item(
        AppServerThreadItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec!["Summary only".to_string()],
            content: vec!["Raw reasoning".to_string()],
        },
        "turn-1".to_string(),
        ReplayKind::ThreadSnapshot,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.transcript_lines(/*width*/ 80))
        }
        other => panic!("expected InsertHistoryCell, got {other:?}"),
    };
    assert!(!rendered.trim().is_empty());
    assert!(!rendered.contains("Raw reasoning"));
}

#[tokio::test]
async fn replayed_reasoning_item_shows_raw_reasoning_when_enabled() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.show_raw_agent_reasoning = true;
    chat.handle_codex_event(Event {
        id: "configured".into(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "test-model".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            cwd: test_project_path(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.replay_thread_item(
        AppServerThreadItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec!["Summary only".to_string()],
            content: vec!["Raw reasoning".to_string()],
        },
        "turn-1".to_string(),
        ReplayKind::ThreadSnapshot,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.transcript_lines(/*width*/ 80))
        }
        other => panic!("expected InsertHistoryCell, got {other:?}"),
    };
    assert!(rendered.contains("Raw reasoning"));
}

#[tokio::test]
async fn live_reasoning_summary_is_not_rendered_twice_when_item_completes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        }),
        /*replay_kind*/ None,
    );
    let _ = drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "reasoning-1".to_string(),
            delta: "Summary only".to_string(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::Reasoning {
                id: "reasoning-1".to_string(),
                summary: vec!["Summary only".to_string()],
                content: Vec::new(),
            },
        }),
        /*replay_kind*/ None,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.transcript_lines(/*width*/ 80))
        }
        other => panic!("expected InsertHistoryCell, got {other:?}"),
    };
    assert_eq!(rendered.matches("Summary only").count(), 1);
}

#[test]
fn rendered_user_message_event_from_inputs_matches_flattened_user_message_shape() {
    let local_image = PathBuf::from("/tmp/local.png");
    let rendered = ChatWidget::rendered_user_message_event_from_inputs(&[
        UserInput::Text {
            text: "hello ".to_string(),
            text_elements: vec![TextElement::new((0..5).into(), /*placeholder*/ None)],
        },
        UserInput::Image {
            image_url: "https://example.com/remote.png".to_string(),
        },
        UserInput::LocalImage {
            path: local_image.clone(),
        },
        UserInput::Skill {
            name: "demo".to_string(),
            path: PathBuf::from("/tmp/skill/SKILL.md"),
        },
        UserInput::Mention {
            name: "repo".to_string(),
            path: "app://repo".to_string(),
        },
        UserInput::Text {
            text: "world".to_string(),
            text_elements: vec![TextElement::new((0..5).into(), Some("planet".to_string()))],
        },
    ]);

    assert_eq!(
        rendered,
        ChatWidget::rendered_user_message_event_from_parts(
            "hello world".to_string(),
            vec![
                TextElement::new((0..5).into(), Some("hello".to_string())),
                TextElement::new((6..11).into(), Some("planet".to_string())),
            ],
            vec![local_image],
            vec!["https://example.com/remote.png".to_string()],
        )
    );
}

#[tokio::test]
async fn item_completed_only_pops_front_pending_steer() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.pending_steers.push_back(pending_steer("first"));
    chat.pending_steers.push_back(pending_steer("second"));
    chat.refresh_pending_input_preview();

    complete_user_message(&mut chat, "user-other", "other");

    assert_eq!(chat.pending_steers.len(), 2);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "first"
    );
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("other"));

    complete_user_message(&mut chat, "user-first", "first");

    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "second"
    );
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("first"));
}

#[tokio::test(flavor = "multi_thread")]
async fn item_completed_pops_pending_steer_with_local_image_and_text_elements() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();

    let temp = tempdir().expect("tempdir");
    let image_path = temp.path().join("pending-steer.png");
    const TINY_PNG_BYTES: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0,
        1, 122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    std::fs::write(&image_path, TINY_PNG_BYTES).expect("write image");

    let text = "note".to_string();
    let text_elements = vec![TextElement::new((0..4).into(), Some("note".to_string()))];
    chat.submit_user_message(UserMessage {
        text: text.clone(),
        local_images: vec![LocalImageAttachment {
            placeholder: "[Image #1]".to_string(),
            path: image_path,
        }],
        remote_image_urls: Vec::new(),
        text_elements,
        mention_bindings: Vec::new(),
    });

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    assert_eq!(chat.pending_steers.len(), 1);
    let pending = chat.pending_steers.front().unwrap();
    assert_eq!(pending.user_message.local_images.len(), 1);
    assert_eq!(pending.user_message.text_elements.len(), 1);
    assert_eq!(pending.compare_key.message, text);
    assert_eq!(pending.compare_key.image_count, 1);

    complete_user_message_for_inputs(
        &mut chat,
        "user-1",
        vec![
            UserInput::Image {
                image_url: "data:image/png;base64,placeholder".to_string(),
            },
            UserInput::Text {
                text,
                text_elements: Vec::new(),
            },
        ],
    );

    assert!(chat.pending_steers.is_empty());

    let mut user_cell = None;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev
            && let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>()
        {
            user_cell = Some((
                cell.message.clone(),
                cell.text_elements.clone(),
                cell.local_image_paths.clone(),
                cell.remote_image_urls.clone(),
            ));
            break;
        }
    }

    let (stored_message, stored_elements, stored_images, stored_remote_image_urls) =
        user_cell.expect("expected pending steer user history cell");
    assert_eq!(stored_message, "note");
    assert_eq!(
        stored_elements,
        vec![TextElement::new((0..4).into(), Some("note".to_string()))]
    );
    assert_eq!(stored_images.len(), 1);
    assert!(stored_images[0].ends_with("pending-steer.png"));
    assert!(stored_remote_image_urls.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_user_message_emits_structured_plugin_mentions_from_bindings() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        cwd: PathBuf::from("/home/user/project"),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);
    chat.bottom_pane.set_plugin_mentions(Some(vec![
        codex_core::plugins::PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "Sample Plugin".to_string(),
            description: None,
            has_skills: true,
            mcp_server_names: Vec::new(),
            app_connector_ids: Vec::new(),
        },
    ]));

    chat.submit_user_message(UserMessage {
        text: "$sample".to_string(),
        local_images: Vec::new(),
        remote_image_urls: Vec::new(),
        text_elements: Vec::new(),
        mention_bindings: vec![MentionBinding {
            mention: "sample".to_string(),
            path: "plugin://sample@test".to_string(),
        }],
    });

    let Op::UserTurn { items, .. } = next_submit_op(&mut op_rx) else {
        panic!("expected Op::UserTurn");
    };
    assert_eq!(
        items,
        vec![
            UserInput::Text {
                text: "$sample".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Mention {
                name: "Sample Plugin".to_string(),
                path: "plugin://sample@test".to_string(),
            },
        ]
    );
}

#[tokio::test]
async fn steer_enter_during_final_stream_preserves_follow_up_prompts_in_order() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    // Simulate "dead mode" repro timing by keeping a final-answer stream active while the
    // user submits multiple follow-up prompts.
    chat.on_agent_message_delta("Final answer line\n".to_string());

    chat.bottom_pane
        .set_composer_text("first follow-up".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.bottom_pane
        .set_composer_text("second follow-up".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.pending_steers.len(), 2);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "first follow-up"
    );
    assert_eq!(
        chat.pending_steers.back().unwrap().user_message.text,
        "second follow-up"
    );

    let first_items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(
        first_items,
        vec![UserInput::Text {
            text: "first follow-up".to_string(),
            text_elements: Vec::new(),
        }]
    );
    let second_items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(
        second_items,
        vec![UserInput::Text {
            text: "second follow-up".to_string(),
            text_elements: Vec::new(),
        }]
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    complete_user_message(&mut chat, "user-1", "first follow-up");

    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "second follow-up"
    );
    let first_insert = drain_insert_history(&mut rx);
    assert_eq!(first_insert.len(), 1);
    assert!(lines_to_single_string(&first_insert[0]).contains("first follow-up"));

    complete_user_message(&mut chat, "user-2", "second follow-up");

    assert!(chat.pending_steers.is_empty());
    let second_insert = drain_insert_history(&mut rx);
    assert_eq!(second_insert.len(), 1);
    assert!(lines_to_single_string(&second_insert[0]).contains("second follow-up"));
}

#[tokio::test]
async fn manual_interrupt_restores_pending_steers_to_composer() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta(
        "Final answer line
"
        .to_string(),
    );

    chat.bottom_pane.set_composer_text(
        "queued while streaming".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.pending_steers.len(), 1);
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued while streaming".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    assert!(chat.pending_steers.is_empty());
    assert_eq!(chat.bottom_pane.composer_text(), "queued while streaming");
    assert_no_submit_op(&mut op_rx);

    let inserted = drain_insert_history(&mut rx);
    assert!(
        inserted
            .iter()
            .all(|cell| !lines_to_single_string(cell).contains("queued while streaming"))
    );
}

#[tokio::test]
async fn esc_interrupt_sends_all_pending_steers_immediately_and_keeps_existing_draft() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());

    chat.bottom_pane
        .set_composer_text("first pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "first pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.bottom_pane
        .set_composer_text("second pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "second pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.queued_user_messages
        .push_back(UserMessage::from("queued draft".to_string()));
    chat.refresh_pending_input_preview();
    chat.bottom_pane
        .set_composer_text("still editing".to_string(), Vec::new(), Vec::new());

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    next_interrupt_op(&mut op_rx);

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "first pending steer\nsecond pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected merged pending steers to submit, got {other:?}"),
    }

    assert!(chat.pending_steers.is_empty());
    assert_eq!(chat.bottom_pane.composer_text(), "still editing");
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued draft"
    );

    let inserted = drain_insert_history(&mut rx);
    assert!(
        inserted
            .iter()
            .any(|cell| lines_to_single_string(cell).contains("first pending steer"))
    );
    assert!(
        inserted
            .iter()
            .any(|cell| lines_to_single_string(cell).contains("second pending steer"))
    );
}

#[tokio::test]
async fn esc_with_pending_steers_overrides_agent_command_interrupt_behavior() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();

    chat.bottom_pane
        .set_composer_text("pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.bottom_pane
        .set_composer_text("/agent ".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    next_interrupt_op(&mut op_rx);
    assert_eq!(chat.bottom_pane.composer_text(), "/agent ");
}

#[tokio::test]
async fn manual_interrupt_restores_pending_steer_mention_bindings_to_composer() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());

    let mention_bindings = vec![MentionBinding {
        mention: "figma".to_string(),
        path: "/tmp/skills/figma/SKILL.md".to_string(),
    }];
    chat.bottom_pane.set_composer_text_with_mention_bindings(
        "please use $figma".to_string(),
        vec![TextElement::new(
            (11..17).into(),
            Some("$figma".to_string()),
        )],
        Vec::new(),
        mention_bindings.clone(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "please use $figma".to_string(),
                text_elements: vec![TextElement::new(
                    (11..17).into(),
                    Some("$figma".to_string()),
                )],
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    assert_eq!(chat.bottom_pane.composer_text(), "please use $figma");
    assert_eq!(chat.bottom_pane.take_mention_bindings(), mention_bindings);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn manual_interrupt_restores_pending_steers_before_queued_messages() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta(
        "Final answer line
"
        .to_string(),
    );

    chat.bottom_pane
        .set_composer_text("pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.queued_user_messages
        .push_back(UserMessage::from("queued draft".to_string()));
    chat.refresh_pending_input_preview();

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    assert!(chat.pending_steers.is_empty());
    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "pending steer
queued draft"
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn replaced_turn_clears_pending_steers_but_keeps_queued_drafts() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta(
        "Final answer line
"
        .to_string(),
    );

    chat.bottom_pane
        .set_composer_text("pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.queued_user_messages
        .push_back(UserMessage::from("queued draft".to_string()));
    chat.refresh_pending_input_preview();

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_codex_event(Event {
        id: "replaced".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Replaced,
        }),
    });

    assert!(chat.pending_steers.is_empty());
    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.bottom_pane.composer_text(), "");
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued draft".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued draft Op::UserTurn, got {other:?}"),
    }
}

#[tokio::test]
async fn enter_submits_when_plan_stream_is_not_active() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.on_task_started();

    chat.bottom_pane
        .set_composer_text("submitted immediately".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
}

#[tokio::test]
async fn ctrl_c_shutdown_works_with_caps_lock() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::CONTROL));

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn ctrl_c_closes_realtime_conversation_before_interrupt_or_quit() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    chat.bottom_pane
        .set_composer_text("recording meter".to_string(), Vec::new(), Vec::new());

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    next_realtime_close_op(&mut op_rx);
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );
    assert_eq!(chat.bottom_pane.composer_text(), "recording meter");
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn ctrl_d_quits_without_prompt() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn ctrl_d_with_modal_open_does_not_quit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_approvals_popup();
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));

    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn ctrl_c_cleared_prompt_is_recoverable_via_history() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.bottom_pane.insert_str("draft message ");
    chat.bottom_pane
        .attach_image(PathBuf::from("/tmp/preview.png"));
    let placeholder = "[Image #1]";
    assert!(
        chat.bottom_pane.composer_text().ends_with(placeholder),
        "expected placeholder {placeholder:?} in composer text"
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(chat.bottom_pane.composer_text().is_empty());
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());

    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    let restored_text = chat.bottom_pane.composer_text();
    assert!(
        restored_text.ends_with(placeholder),
        "expected placeholder {placeholder:?} after history recall"
    );
    assert!(restored_text.starts_with("draft message "));
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());

    let images = chat.bottom_pane.take_recent_submission_images();
    assert_eq!(vec![PathBuf::from("/tmp/preview.png")], images);
}

#[tokio::test]
async fn realtime_error_closes_without_followup_closed_info() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;

    chat.on_realtime_conversation_realtime(RealtimeConversationRealtimeEvent {
        payload: RealtimeEvent::Error("boom".to_string()),
    });
    next_realtime_close_op(&mut op_rx);

    chat.on_realtime_conversation_closed(RealtimeConversationClosedEvent {
        reason: Some("error".to_string()),
    });

    let rendered = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>();
    assert_snapshot!(rendered.join("\n\n"), @"■ Realtime voice error: boom");
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn deleted_realtime_meter_uses_shared_stop_path() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    let placeholder_id = chat.bottom_pane.insert_recording_meter_placeholder("⠤⠤⠤⠤");
    chat.realtime_conversation.meter_placeholder_id = Some(placeholder_id.clone());

    assert!(chat.stop_realtime_conversation_for_deleted_meter(&placeholder_id));

    next_realtime_close_op(&mut op_rx);
    assert_eq!(chat.realtime_conversation.meter_placeholder_id, None);
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );
}

#[tokio::test]
async fn exec_history_cell_shows_working_then_completed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Begin command
    let begin = begin_exec(&mut chat, "call-1", "echo done");

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 0, "no exec cell should have been flushed yet");

    // End command successfully
    end_exec(&mut chat, begin, "done", "", /*exit_code*/ 0);

    let cells = drain_insert_history(&mut rx);
    // Exec end now finalizes and flushes the exec cell immediately.
    assert_eq!(cells.len(), 1, "expected finalized exec cell to flush");
    // Inspect the flushed exec cell rendering.
    let lines = &cells[0];
    let blob = lines_to_single_string(lines);
    // New behavior: no glyph markers; ensure command is shown and no panic.
    assert!(
        blob.contains("• Ran"),
        "expected summary header present: {blob:?}"
    );
    assert!(
        blob.contains("echo done"),
        "expected command text to be present: {blob:?}"
    );
}

#[tokio::test]
async fn exec_history_cell_shows_working_then_failed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Begin command
    let begin = begin_exec(&mut chat, "call-2", "false");
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 0, "no exec cell should have been flushed yet");

    // End command with failure
    end_exec(&mut chat, begin, "", "Bloop", /*exit_code*/ 2);

    let cells = drain_insert_history(&mut rx);
    // Exec end with failure should also flush immediately.
    assert_eq!(cells.len(), 1, "expected finalized exec cell to flush");
    let lines = &cells[0];
    let blob = lines_to_single_string(lines);
    assert!(
        blob.contains("• Ran false"),
        "expected command and header text present: {blob:?}"
    );
    assert!(blob.to_lowercase().contains("bloop"), "expected error text");
}

#[tokio::test]
async fn exec_end_without_begin_uses_event_command() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "echo orphaned".to_string(),
    ];
    let parsed_cmd = codex_shell_command::parse_command::parse_command(&command);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    chat.handle_codex_event(Event {
        id: "call-orphan".to_string(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "call-orphan".to_string(),
            process_id: None,
            turn_id: "turn-1".to_string(),
            command,
            cwd,
            parsed_cmd,
            source: ExecCommandSource::Agent,
            interaction_input: None,
            stdout: "done".to_string(),
            stderr: String::new(),
            aggregated_output: "done".to_string(),
            exit_code: 0,
            duration: std::time::Duration::from_millis(5),
            formatted_output: "done".to_string(),
            status: CoreExecCommandStatus::Completed,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected finalized exec cell to flush");
    let blob = lines_to_single_string(&cells[0]);
    assert!(
        blob.contains("• Ran echo orphaned"),
        "expected command text to come from event: {blob:?}"
    );
    assert!(
        !blob.contains("call-orphan"),
        "call id should not be rendered when event has the command: {blob:?}"
    );
}

#[tokio::test]
async fn exec_end_without_begin_does_not_flush_unrelated_running_exploring_cell() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();

    begin_exec(&mut chat, "call-exploring", "cat /dev/null");
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(active_blob(&chat).contains("Read null"));

    let orphan =
        begin_unified_exec_startup(&mut chat, "call-orphan", "proc-1", "echo repro-marker");
    assert!(drain_insert_history(&mut rx).is_empty());

    end_exec(
        &mut chat,
        orphan,
        "repro-marker\n",
        "",
        /*exit_code*/ 0,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "only the orphan end should be inserted");
    let orphan_blob = lines_to_single_string(&cells[0]);
    assert!(
        orphan_blob.contains("• Ran echo repro-marker"),
        "expected orphan end to render a standalone entry: {orphan_blob:?}"
    );
    let active = active_blob(&chat);
    assert!(
        active.contains("• Exploring"),
        "expected unrelated exploring call to remain active: {active:?}"
    );
    assert!(
        active.contains("Read null"),
        "expected active exploring command to remain visible: {active:?}"
    );
    assert!(
        !active.contains("echo repro-marker"),
        "orphaned end should not replace the active exploring cell: {active:?}"
    );
}

#[tokio::test]
async fn exec_end_without_begin_flushes_completed_unrelated_exploring_cell() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();

    let begin_ls = begin_exec(&mut chat, "call-ls", "ls -la");
    end_exec(&mut chat, begin_ls, "", "", /*exit_code*/ 0);
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(active_blob(&chat).contains("ls -la"));

    let orphan = begin_unified_exec_startup(&mut chat, "call-after", "proc-1", "echo after");
    end_exec(&mut chat, orphan, "after\n", "", /*exit_code*/ 0);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        2,
        "completed exploring cell should flush before the orphan entry"
    );
    let first = lines_to_single_string(&cells[0]);
    let second = lines_to_single_string(&cells[1]);
    assert!(
        first.contains("• Explored"),
        "expected flushed exploring cell: {first:?}"
    );
    assert!(
        first.contains("List ls -la"),
        "expected flushed exploring cell: {first:?}"
    );
    assert!(
        second.contains("• Ran echo after"),
        "expected orphan end entry after flush: {second:?}"
    );
    assert!(
        chat.active_cell.is_none(),
        "both entries should be finalized"
    );
}

#[tokio::test]
async fn overlapping_exploring_exec_end_is_not_misclassified_as_orphan() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let begin_ls = begin_exec(&mut chat, "call-ls", "ls -la");
    let begin_cat = begin_exec(&mut chat, "call-cat", "cat foo.txt");
    assert!(drain_insert_history(&mut rx).is_empty());

    end_exec(&mut chat, begin_ls, "foo.txt\n", "", /*exit_code*/ 0);

    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "tracked end inside an exploring cell should not render as an orphan"
    );
    let active = active_blob(&chat);
    assert!(
        active.contains("List ls -la"),
        "expected first command still grouped: {active:?}"
    );
    assert!(
        active.contains("Read foo.txt"),
        "expected second running command to stay in the same active cell: {active:?}"
    );
    assert!(
        active.contains("• Exploring"),
        "expected grouped exploring header to remain active: {active:?}"
    );

    end_exec(&mut chat, begin_cat, "hello\n", "", /*exit_code*/ 0);
}

#[tokio::test]
async fn exec_history_shows_unified_exec_startup_commands() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();

    let begin = begin_exec_with_source(
        &mut chat,
        "call-startup",
        "echo unified exec startup",
        ExecCommandSource::UnifiedExecStartup,
    );
    assert!(
        drain_insert_history(&mut rx).is_empty(),
        "exec begin should not flush until completion"
    );

    end_exec(
        &mut chat,
        begin,
        "echo unified exec startup\n",
        "",
        /*exit_code*/ 0,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected finalized exec cell to flush");
    let blob = lines_to_single_string(&cells[0]);
    assert!(
        blob.contains("• Ran echo unified exec startup"),
        "expected startup command to render: {blob:?}"
    );
}

#[tokio::test]
async fn exec_history_shows_unified_exec_tool_calls() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();

    let begin = begin_exec_with_source(
        &mut chat,
        "call-startup",
        "ls",
        ExecCommandSource::UnifiedExecStartup,
    );
    end_exec(&mut chat, begin, "", "", /*exit_code*/ 0);

    let blob = active_blob(&chat);
    assert_eq!(blob, "• Explored\n  └ List ls\n");
}

#[tokio::test]
async fn unified_exec_unknown_end_with_active_exploring_cell_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();

    begin_exec(&mut chat, "call-exploring", "cat /dev/null");
    let orphan =
        begin_unified_exec_startup(&mut chat, "call-orphan", "proc-1", "echo repro-marker");
    end_exec(
        &mut chat,
        orphan,
        "repro-marker\n",
        "",
        /*exit_code*/ 0,
    );

    let cells = drain_insert_history(&mut rx);
    let history = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    let active = active_blob(&chat);
    let snapshot = format!("History:\n{history}\nActive:\n{active}");
    assert_snapshot!(
        "unified_exec_unknown_end_with_active_exploring_cell",
        snapshot
    );
}

#[tokio::test]
async fn unified_exec_end_after_task_complete_is_suppressed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();

    let begin = begin_exec_with_source(
        &mut chat,
        "call-startup",
        "echo unified exec startup",
        ExecCommandSource::UnifiedExecStartup,
    );
    drain_insert_history(&mut rx);

    chat.on_task_complete(/*last_agent_message*/ None, /*from_replay*/ false);
    end_exec(&mut chat, begin, "", "", /*exit_code*/ 0);

    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "expected unified exec end after task complete to be suppressed"
    );
}

#[tokio::test]
async fn unified_exec_interaction_after_task_complete_is_suppressed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    chat.on_task_complete(/*last_agent_message*/ None, /*from_replay*/ false);

    chat.handle_codex_event(Event {
        id: "call-1".to_string(),
        msg: EventMsg::TerminalInteraction(TerminalInteractionEvent {
            call_id: "call-1".to_string(),
            process_id: "proc-1".to_string(),
            stdin: "ls\n".to_string(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "expected unified exec interaction after task complete to be suppressed"
    );
}

#[tokio::test]
async fn unified_exec_wait_after_final_agent_message_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    begin_unified_exec_startup(&mut chat, "call-wait", "proc-1", "cargo test -p codex-core");
    terminal_interaction(&mut chat, "call-wait-stdin", "proc-1", "");

    complete_assistant_message(&mut chat, "msg-1", "Final response.", /*phase*/ None);
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Final response.".into()),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_snapshot!("unified_exec_wait_after_final_agent_message", combined);
}

#[tokio::test]
async fn unified_exec_wait_before_streamed_agent_message_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    begin_unified_exec_startup(
        &mut chat,
        "call-wait-stream",
        "proc-1",
        "cargo test -p codex-core",
    );
    terminal_interaction(&mut chat, "call-wait-stream-stdin", "proc-1", "");

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "Streaming response.".into(),
        }),
    });
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_snapshot!("unified_exec_wait_before_streamed_agent_message", combined);
}

#[tokio::test]
async fn unified_exec_wait_status_header_updates_on_late_command_display() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    chat.unified_exec_processes.push(UnifiedExecProcessSummary {
        key: "proc-1".to_string(),
        call_id: "call-1".to_string(),
        command_display: "sleep 5".to_string(),
        recent_chunks: Vec::new(),
    });

    chat.on_terminal_interaction(TerminalInteractionEvent {
        call_id: "call-1".to_string(),
        process_id: "proc-1".to_string(),
        stdin: String::new(),
    });

    assert!(chat.active_cell.is_none());
    assert_eq!(
        chat.current_status.header,
        "Waiting for background terminal"
    );
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Waiting for background terminal");
    assert_eq!(status.details(), Some("sleep 5"));
}

#[tokio::test]
async fn unified_exec_waiting_multiple_empty_snapshots() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    begin_unified_exec_startup(&mut chat, "call-wait-1", "proc-1", "just fix");

    terminal_interaction(&mut chat, "call-wait-1a", "proc-1", "");
    terminal_interaction(&mut chat, "call-wait-1b", "proc-1", "");
    assert_eq!(
        chat.current_status.header,
        "Waiting for background terminal"
    );
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Waiting for background terminal");
    assert_eq!(status.details(), Some("just fix"));

    chat.handle_codex_event(Event {
        id: "turn-wait-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_snapshot!("unified_exec_waiting_multiple_empty_after", combined);
}

#[tokio::test]
async fn unified_exec_wait_status_renders_command_in_single_details_row_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    begin_unified_exec_startup(
        &mut chat,
        "call-wait-ui",
        "proc-ui",
        "cargo test -p codex-core -- --exact some::very::long::test::name",
    );

    terminal_interaction(&mut chat, "call-wait-ui-stdin", "proc-ui", "");

    let rendered = render_bottom_popup(&chat, /*width*/ 48);
    assert_snapshot!(
        "unified_exec_wait_status_renders_command_in_single_details_row",
        normalize_snapshot_paths(rendered)
    );
}

#[tokio::test]
async fn unified_exec_empty_then_non_empty_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    begin_unified_exec_startup(&mut chat, "call-wait-2", "proc-2", "just fix");

    terminal_interaction(&mut chat, "call-wait-2a", "proc-2", "");
    terminal_interaction(&mut chat, "call-wait-2b", "proc-2", "ls\n");

    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_snapshot!("unified_exec_empty_then_non_empty_after", combined);
}

#[tokio::test]
async fn unified_exec_non_empty_then_empty_snapshots() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    begin_unified_exec_startup(&mut chat, "call-wait-3", "proc-3", "just fix");

    terminal_interaction(&mut chat, "call-wait-3a", "proc-3", "pwd\n");
    terminal_interaction(&mut chat, "call-wait-3b", "proc-3", "");
    assert_eq!(
        chat.current_status.header,
        "Waiting for background terminal"
    );
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Waiting for background terminal");
    assert_eq!(status.details(), Some("just fix"));
    let pre_cells = drain_insert_history(&mut rx);
    let active_combined = pre_cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_snapshot!("unified_exec_non_empty_then_empty_active", active_combined);

    chat.handle_codex_event(Event {
        id: "turn-wait-3".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });

    let post_cells = drain_insert_history(&mut rx);
    let mut combined = pre_cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    let post = post_cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    if !combined.is_empty() && !post.is_empty() {
        combined.push('\n');
    }
    combined.push_str(&post);
    assert_snapshot!("unified_exec_non_empty_then_empty_after", combined);
}

/// Selecting the custom prompt option from the review popup sends
/// OpenReviewCustomPrompt to the app event channel.
#[tokio::test]
async fn review_popup_custom_prompt_action_sends_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Open the preset selection popup
    chat.open_review_popup();

    // Move selection down to the fourth item: "Custom review instructions"
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    // Activate
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Drain events and ensure we saw the OpenReviewCustomPrompt request
    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::OpenReviewCustomPrompt = ev {
            found = true;
            break;
        }
    }
    assert!(found, "expected OpenReviewCustomPrompt event to be sent");
}

#[tokio::test]
async fn slash_init_skips_when_project_doc_exists() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let tempdir = tempdir().unwrap();
    let existing_path = tempdir.path().join(DEFAULT_PROJECT_DOC_FILENAME);
    std::fs::write(&existing_path, "existing instructions").unwrap();
    chat.config.cwd = tempdir.path().to_path_buf().abs();

    chat.dispatch_command(SlashCommand::Init);

    match op_rx.try_recv() {
        Err(TryRecvError::Empty) => {}
        other => panic!("expected no Codex op to be sent, got {other:?}"),
    }

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(DEFAULT_PROJECT_DOC_FILENAME),
        "info message should mention the existing file: {rendered:?}"
    );
    assert!(
        rendered.contains("Skipping /init"),
        "info message should explain why /init was skipped: {rendered:?}"
    );
    assert_eq!(
        std::fs::read_to_string(existing_path).unwrap(),
        "existing instructions"
    );
}

#[tokio::test]
async fn collab_mode_shift_tab_cycles_only_when_idle() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let initial = chat.current_collaboration_mode().clone();
    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert_eq!(chat.current_collaboration_mode(), &initial);

    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_collaboration_mode(), &initial);

    chat.on_task_started();
    let before = chat.active_collaboration_mode_kind();
    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    assert_eq!(chat.active_collaboration_mode_kind(), before);
}

#[tokio::test]
async fn mode_switch_surfaces_model_change_notification_when_effective_model_changes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let default_model = chat.current_model().to_string();

    let mut plan_mask =
        collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
            .expect("expected plan collaboration mode");
    plan_mask.model = Some("gpt-5.1-codex-mini".to_string());
    chat.set_collaboration_mask(plan_mask);

    let plan_messages = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        plan_messages.contains("Model changed to gpt-5.1-codex-mini medium for Plan mode."),
        "expected Plan-mode model switch notice, got: {plan_messages:?}"
    );

    let default_mask = collaboration_modes::default_mask(chat.model_catalog.as_ref())
        .expect("expected default collaboration mode");
    chat.set_collaboration_mask(default_mask);

    let default_messages = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    let expected_default_message =
        format!("Model changed to {default_model} default for Default mode.");
    assert!(
        default_messages.contains(&expected_default_message),
        "expected Default-mode model switch notice, got: {default_messages:?}"
    );
}

#[tokio::test]
async fn mode_switch_surfaces_reasoning_change_notification_when_model_stays_same() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));

    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);

    let plan_messages = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        plan_messages.contains("Model changed to gpt-5.3-codex medium for Plan mode."),
        "expected reasoning-change notice in Plan mode, got: {plan_messages:?}"
    );
}

#[tokio::test]
async fn collab_slash_command_opens_picker_and_updates_mode() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    chat.dispatch_command(SlashCommand::Collab);
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Select Collaboration Mode"),
        "expected collaboration picker: {popup}"
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let selected_mask = match rx.try_recv() {
        Ok(AppEvent::UpdateCollaborationMode(mask)) => mask,
        other => panic!("expected UpdateCollaborationMode event, got {other:?}"),
    };
    chat.set_collaboration_mask(selected_mask);

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with code collab mode, got {other:?}")
        }
    }

    chat.bottom_pane
        .set_composer_text("follow up".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with code collab mode, got {other:?}")
        }
    }
}

#[tokio::test]
async fn plan_slash_command_switches_to_plan_mode() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let initial = chat.current_collaboration_mode().clone();

    chat.dispatch_command(SlashCommand::Plan);

    while let Ok(event) = rx.try_recv() {
        assert!(
            matches!(event, AppEvent::InsertHistoryCell(_)),
            "plan should not emit a non-history app event: {event:?}"
        );
    }
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert_eq!(chat.current_collaboration_mode(), &initial);
}

#[tokio::test]
async fn plan_slash_command_with_args_submits_prompt_in_plan_mode() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: ThreadId::new(),
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        cwd: PathBuf::from("/home/user/project"),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: None,
    };
    chat.handle_codex_event(Event {
        id: "configured".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    chat.bottom_pane
        .set_composer_text("/plan build the plan".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        UserInput::Text {
            text: "build the plan".to_string(),
            text_elements: Vec::new(),
        }
    );
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn collaboration_modes_defaults_to_code_on_startup() {
    let codex_home = tempdir().expect("tempdir");
    let cfg = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(vec![(
            "features.collaboration_modes".to_string(),
            TomlValue::Boolean(true),
        )])
        .build()
        .await
        .expect("config");
    let resolved_model = codex_core::test_support::get_model_offline(cfg.model.as_deref());
    let session_telemetry = test_session_telemetry(&cfg, resolved_model.as_str());
    let init = ChatWidgetInit {
        config: cfg.clone(),
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: AppEventSender::new(unbounded_channel::<AppEvent>().0),
        initial_user_message: None,
        enhanced_keys_supported: false,
        has_chatgpt_account: false,
        model_catalog: test_model_catalog(&cfg),
        feedback: codex_feedback::CodexFeedback::new(),
        is_first_run: true,
        status_account_display: None,
        initial_plan_type: None,
        model: Some(resolved_model.clone()),
        startup_tooltip_override: None,
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        session_telemetry,
    };

    let chat = ChatWidget::new_with_app_event(init);
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_model(), resolved_model);
}

#[tokio::test]
async fn experimental_mode_plan_is_ignored_on_startup() {
    let codex_home = tempdir().expect("tempdir");
    let cfg = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(vec![
            (
                "features.collaboration_modes".to_string(),
                TomlValue::Boolean(true),
            ),
            (
                "tui.experimental_mode".to_string(),
                TomlValue::String("plan".to_string()),
            ),
        ])
        .build()
        .await
        .expect("config");
    let resolved_model = codex_core::test_support::get_model_offline(cfg.model.as_deref());
    let session_telemetry = test_session_telemetry(&cfg, resolved_model.as_str());
    let init = ChatWidgetInit {
        config: cfg.clone(),
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: AppEventSender::new(unbounded_channel::<AppEvent>().0),
        initial_user_message: None,
        enhanced_keys_supported: false,
        has_chatgpt_account: false,
        model_catalog: test_model_catalog(&cfg),
        feedback: codex_feedback::CodexFeedback::new(),
        is_first_run: true,
        status_account_display: None,
        initial_plan_type: None,
        model: Some(resolved_model.clone()),
        startup_tooltip_override: None,
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        session_telemetry,
    };

    let chat = ChatWidget::new_with_app_event(init);
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_model(), resolved_model);
}

#[tokio::test]
async fn set_model_updates_active_collaboration_mask() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.set_model("gpt-5.1-codex-mini");

    assert_eq!(chat.current_model(), "gpt-5.1-codex-mini");
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn set_reasoning_effort_updates_active_collaboration_mask() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.set_reasoning_effort(/*effort*/ None);

    assert_eq!(
        chat.current_reasoning_effort(),
        Some(ReasoningEffortConfig::Medium)
    );
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn set_reasoning_effort_does_not_override_active_plan_override() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::High));
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Low));

    assert_eq!(
        chat.current_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn collab_mode_is_sent_after_enabling() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn, got {other:?}")
        }
    }
}

#[tokio::test]
async fn collab_mode_applies_default_preset() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with default collaboration_mode, got {other:?}")
        }
    }

    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_collaboration_mode().mode, ModeKind::Default);
}

#[tokio::test]
async fn user_turn_includes_personality_from_config() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.set_feature_enabled(Feature::Personality, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    chat.set_model("gpt-5.2-codex");
    chat.set_personality(Personality::Friendly);

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            personality: Some(Personality::Friendly),
            ..
        } => {}
        other => panic!("expected Op::UserTurn with friendly personality, got {other:?}"),
    }
}

#[tokio::test]
async fn slash_quit_requests_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Quit);

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn slash_copy_state_tracks_turn_complete_final_reply() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Final reply **markdown**".to_string()),
        }),
    });

    assert_eq!(
        chat.last_copyable_output,
        Some("Final reply **markdown**".to_string())
    );
}

#[tokio::test]
async fn slash_copy_state_tracks_plan_item_completion() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let plan_text = "## Plan\n\n1. Build it\n2. Test it".to_string();

    chat.handle_codex_event(Event {
        id: "item-plan".into(),
        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::Plan(PlanItem {
                id: "plan-1".to_string(),
                text: plan_text.clone(),
            }),
        }),
    });
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });

    assert_eq!(chat.last_copyable_output, Some(plan_text));
}

#[tokio::test]
async fn slash_copy_reports_when_no_copyable_output_exists() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert_snapshot!("slash_copy_no_output_info_message", rendered);
    assert!(
        rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected no-output message, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_copy_state_is_preserved_during_running_task() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Previous completed reply".to_string()),
        }),
    });
    chat.on_task_started();

    assert_eq!(
        chat.last_copyable_output,
        Some("Previous completed reply".to_string())
    );
}

#[tokio::test]
async fn slash_copy_state_clears_on_thread_rollback() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Reply that will be rolled back".to_string()),
        }),
    });
    chat.handle_codex_event(Event {
        id: "rollback-1".into(),
        msg: EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 1 }),
    });

    assert_eq!(chat.last_copyable_output, None);
}

#[tokio::test]
async fn slash_copy_is_unavailable_when_legacy_agent_message_is_not_repeated_on_turn_complete() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "turn-1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Legacy final message".into(),
            phase: None,
            memory_citation: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected unavailable message, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_copy_uses_agent_message_item_when_turn_complete_omits_final_text() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    complete_assistant_message(
        &mut chat,
        "msg-1",
        "Legacy item final message",
        /*phase*/ None,
    );
    let _ = drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        !rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected copy state to be available, got {rendered:?}"
    );
    assert_eq!(
        chat.last_copyable_output,
        Some("Legacy item final message".to_string())
    );
}

#[tokio::test]
async fn slash_copy_does_not_return_stale_output_after_thread_rollback() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    complete_assistant_message(
        &mut chat,
        "msg-1",
        "Reply that will be rolled back",
        /*phase*/ None,
    );
    let _ = drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.handle_codex_event(Event {
        id: "rollback-1".into(),
        msg: EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 1 }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected rollback-cleared copy state message, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_exit_requests_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Exit);

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn slash_stop_submits_background_terminal_cleanup() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Stop);

    assert_matches!(op_rx.try_recv(), Ok(Op::CleanBackgroundTerminals));
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected cleanup confirmation message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Stopping all background terminals."),
        "expected cleanup confirmation, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_clear_requests_ui_clear_when_idle() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Clear);

    assert_matches!(rx.try_recv(), Ok(AppEvent::ClearUi));
}

#[tokio::test]
async fn slash_clear_is_disabled_while_task_running() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);

    chat.dispatch_command(SlashCommand::Clear);

    let event = rx.try_recv().expect("expected disabled command error");
    match event {
        AppEvent::InsertHistoryCell(cell) => {
            let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
            assert!(
                rendered.contains("'/clear' is disabled while a task is in progress."),
                "expected /clear task-running error, got {rendered:?}"
            );
        }
        other => panic!("expected InsertHistoryCell error, got {other:?}"),
    }
    assert!(rx.try_recv().is_err(), "expected no follow-up events");
}

mod app_server;
mod approval_requests;
mod background_events;
mod composer_submission;
mod exec_flow;
mod guardian;
mod helpers;
mod history_replay;
mod mcp_startup;
mod permissions;
mod plan_mode;
mod popups_and_settings;
mod review_mode;
mod slash_commands;
mod status_and_layout;
mod status_command_tests;

pub(crate) use helpers::make_chatwidget_manual_with_sender;
pub(crate) use helpers::set_chatgpt_auth;
pub(crate) use helpers::set_fast_mode_test_catalog;
pub(super) use helpers::*;
