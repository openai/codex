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
pub(super) use codex_app_server_protocol::GuardianApprovalReviewStatus;
pub(super) use codex_app_server_protocol::GuardianRiskLevel as AppServerGuardianRiskLevel;
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
pub(super) use codex_core::config::ApprovalsReviewer;
pub(super) use codex_core::config::Config;
pub(super) use codex_core::config::ConfigBuilder;
pub(super) use codex_core::config::Constrained;
pub(super) use codex_core::config::ConstraintError;
pub(super) use codex_core::config::types::Notifications;
#[cfg(target_os = "windows")]
pub(super) use codex_core::config::types::WindowsSandboxModeToml;
pub(super) use codex_core::config_loader::AppRequirementToml;
pub(super) use codex_core::config_loader::AppsRequirementsToml;
pub(super) use codex_core::config_loader::ConfigLayerStack;
pub(super) use codex_core::config_loader::ConfigRequirements;
pub(super) use codex_core::config_loader::ConfigRequirementsToml;
pub(super) use codex_core::config_loader::RequirementSource;
pub(super) use codex_core::models_manager::collaboration_mode_presets::CollaborationModesConfig;
pub(super) use codex_core::plugins::OPENAI_CURATED_MARKETPLACE_NAME;
pub(super) use codex_core::skills::model::SkillMetadata;
pub(super) use codex_features::FEATURES;
pub(super) use codex_features::Feature;
pub(super) use codex_git_utils::CommitLogEntry;
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
pub(super) use codex_protocol::models::MessagePhase;
pub(super) use codex_protocol::openai_models::ModelPreset;
pub(super) use codex_protocol::openai_models::ReasoningEffortPreset;
pub(super) use codex_protocol::openai_models::default_input_modalities;
pub(super) use codex_protocol::parse_command::ParsedCommand;
pub(super) use codex_protocol::plan_tool::PlanItemArg;
pub(super) use codex_protocol::plan_tool::StepStatus;
pub(super) use codex_protocol::plan_tool::UpdatePlanArgs;
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
pub(super) use codex_protocol::protocol::GuardianAssessmentEvent;
pub(super) use codex_protocol::protocol::GuardianAssessmentStatus;
pub(super) use codex_protocol::protocol::GuardianRiskLevel;
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
    codex_utils_cargo_bin::find_resource!("src/chatwidget/snapshots").expect("snapshot dir")
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

mod app_server;
mod composer_submission;
mod exec_flow;
mod helpers;
mod history_replay;
mod mcp_guardian;
mod permissions;
mod plan_mode;
mod popups_and_settings;
mod review_mode;
mod slash_commands;
mod status_and_layout;

pub(crate) use helpers::make_chatwidget_manual_with_sender;
pub(crate) use helpers::set_chatgpt_auth;
pub(super) use helpers::*;
