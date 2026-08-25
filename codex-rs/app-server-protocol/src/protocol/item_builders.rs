//! Shared builders for app-server [`ThreadItem`] values derived from compatibility events.
//!
//! Most live tool items now come from first-class core `ItemStarted` / `ItemCompleted` events.
//! These builders remain for approval flows, rebuilt legacy history, and other pre-execution
//! paths where the underlying tool has not started or never starts at all.
//!
//! Keeping these builders in one place is useful for two reasons:
//! - Live notifications and rebuilt `thread/read` history both need to construct the same
//!   synthetic items, so sharing the logic avoids drift between those paths.
//! - The projection is presentation-specific. Core protocol events stay generic, while the
//!   app-server protocol decides how to surface those events as `ThreadItem`s for clients.
use crate::protocol::common::ServerNotification;
use crate::protocol::v2::AutoReviewDecisionSource;
use crate::protocol::v2::CommandAction;
use crate::protocol::v2::CommandExecutionSource;
use crate::protocol::v2::CommandExecutionStatus;
use crate::protocol::v2::FileUpdateChange;
use crate::protocol::v2::GuardianApprovalReview;
use crate::protocol::v2::GuardianApprovalReviewStatus;
use crate::protocol::v2::ItemGuardianApprovalReviewCompletedNotification;
use crate::protocol::v2::ItemGuardianApprovalReviewStartedNotification;
use crate::protocol::v2::PatchApplyStatus;
use crate::protocol::v2::PatchChangeKind;
use crate::protocol::v2::ThreadItem;
use codex_protocol::ThreadId;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::PatchApplyBeginEvent;
use codex_protocol::protocol::PatchApplyEndEvent;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::review_format::REVIEW_FALLBACK_MESSAGE;
use codex_protocol::review_format::render_review_output_text;
use codex_secrets::redact_secrets;
use codex_shell_command::parse_command::parse_command;
use codex_shell_command::parse_command::shlex_join;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::warn;

/// Client-facing command and parsed actions projected from a raw command.
pub struct CommandExecutionPresentation {
    /// Shell-formatted command with recognizable secrets redacted.
    pub command: String,
    /// Parsed command actions with recognizable secrets redacted.
    pub command_actions: Vec<CommandAction>,
}

impl CommandExecutionPresentation {
    /// Projects a raw command into its client-facing representation.
    pub fn from_raw(command: &[String], parsed_cmd: &[ParsedCommand], cwd: &PathUri) -> Self {
        Self {
            command: redact_secrets(shlex_join(command)),
            command_actions: command_actions_for_path_uri(parsed_cmd, cwd),
        }
    }
}

pub(crate) fn review_output_text(output: Option<&ReviewOutputEvent>) -> String {
    output
        .map(render_review_output_text)
        .unwrap_or_else(|| REVIEW_FALLBACK_MESSAGE.to_string())
}

pub fn build_file_change_approval_request_item(
    payload: &ApplyPatchApprovalRequestEvent,
) -> ThreadItem {
    ThreadItem::FileChange {
        id: payload.call_id.clone(),
        changes: convert_patch_changes(&payload.changes),
        status: PatchApplyStatus::InProgress,
    }
}

pub fn build_file_change_begin_item(payload: &PatchApplyBeginEvent) -> ThreadItem {
    ThreadItem::FileChange {
        id: payload.call_id.clone(),
        changes: convert_patch_changes(&payload.changes),
        status: PatchApplyStatus::InProgress,
    }
}

pub fn build_file_change_end_item(payload: &PatchApplyEndEvent) -> ThreadItem {
    ThreadItem::FileChange {
        id: payload.call_id.clone(),
        changes: convert_patch_changes(&payload.changes),
        status: (&payload.status).into(),
    }
}

pub fn build_command_execution_begin_item(payload: &ExecCommandBeginEvent) -> ThreadItem {
    let presentation =
        CommandExecutionPresentation::from_raw(&payload.command, &payload.parsed_cmd, &payload.cwd);
    ThreadItem::CommandExecution {
        id: payload.call_id.clone(),
        plugin_id: payload.plugin_id.clone(),
        script_path: payload.script_path.clone(),
        command: presentation.command,
        cwd: payload.cwd.clone().into(),
        process_id: payload.process_id.clone(),
        source: payload.source.into(),
        status: CommandExecutionStatus::InProgress,
        command_actions: presentation.command_actions,
        aggregated_output: None,
        exit_code: None,
        duration_ms: None,
    }
}

pub fn build_command_execution_end_item(payload: &ExecCommandEndEvent) -> ThreadItem {
    let aggregated_output = if payload.aggregated_output.is_empty() {
        None
    } else {
        Some(payload.aggregated_output.clone())
    };
    let duration_ms = i64::try_from(payload.duration.as_millis()).unwrap_or(i64::MAX);
    let presentation =
        CommandExecutionPresentation::from_raw(&payload.command, &payload.parsed_cmd, &payload.cwd);

    ThreadItem::CommandExecution {
        id: payload.call_id.clone(),
        plugin_id: payload.plugin_id.clone(),
        script_path: payload.script_path.clone(),
        command: presentation.command,
        cwd: payload.cwd.clone().into(),
        process_id: payload.process_id.clone(),
        source: payload.source.into(),
        status: (&payload.status).into(),
        command_actions: presentation.command_actions,
        aggregated_output,
        exit_code: Some(payload.exit_code),
        duration_ms: Some(duration_ms),
    }
}

fn command_actions_for_path_uri(parsed_cmd: &[ParsedCommand], cwd: &PathUri) -> Vec<CommandAction> {
    parsed_cmd
        .iter()
        .cloned()
        .filter_map(|parsed| match parsed {
            ParsedCommand::Read { cmd, name, path } => {
                // Resolve against the executor's URI, not the app-server's filesystem. POSIX
                // non-UTF-8 paths are percent-encoded, not opaque. Expanding `~` or resolving a
                // genuinely opaque cwd would require executor-native state unavailable here.
                match cwd.join(path.to_string_lossy().as_ref()) {
                    Ok(path) => Some(CommandAction::Read {
                        command: redact_secrets(cmd),
                        name,
                        path: path.into(),
                    }),
                    Err(error) => {
                        warn!(
                            command = cmd,
                            %cwd,
                            file_path = %path.display(),
                            %error,
                            "omitting read action: invalid file path or cwd"
                        );
                        None
                    }
                }
            }
            ParsedCommand::ListFiles { cmd, path } => Some(CommandAction::ListFiles {
                command: redact_secrets(cmd),
                path,
            }),
            ParsedCommand::Search { cmd, query, path } => Some(CommandAction::Search {
                command: redact_secrets(cmd),
                query: query.map(redact_secrets),
                path,
            }),
            ParsedCommand::Unknown { cmd } => Some(CommandAction::Unknown {
                command: redact_secrets(cmd),
            }),
        })
        .collect()
}

/// Build a guardian-derived [`ThreadItem`].
///
/// Currently this only synthesizes [`ThreadItem::CommandExecution`] for
/// [`GuardianAssessmentAction::Command`] and [`GuardianAssessmentAction::Execve`].
/// Stdin reviews are child approvals and must not change the parent item lifecycle.
pub fn build_item_from_guardian_event(
    assessment: &GuardianAssessmentEvent,
    status: CommandExecutionStatus,
) -> Option<ThreadItem> {
    match &assessment.action {
        GuardianAssessmentAction::Command { command, cwd, .. } => {
            let id = assessment.target_item_id.as_ref()?;
            let command = command.clone();
            let command_actions = vec![CommandAction::Unknown {
                command: command.clone(),
            }];
            Some(ThreadItem::CommandExecution {
                id: id.clone(),
                plugin_id: assessment.plugin_id.clone(),
                script_path: assessment.script_path.clone(),
                command,
                cwd: cwd.clone().into(),
                process_id: None,
                source: CommandExecutionSource::Agent,
                status,
                command_actions,
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            })
        }
        GuardianAssessmentAction::Execve {
            program, argv, cwd, ..
        } => {
            let id = assessment.target_item_id.as_ref()?;
            let argv = if argv.is_empty() {
                vec![program.clone()]
            } else {
                std::iter::once(program.clone())
                    .chain(argv.iter().skip(1).cloned())
                    .collect::<Vec<_>>()
            };
            let command = shlex_join(&argv);
            let parsed_cmd = parse_command(&argv);
            let command_actions = if parsed_cmd.is_empty() {
                vec![CommandAction::Unknown {
                    command: command.clone(),
                }]
            } else {
                parsed_cmd
                    .into_iter()
                    .map(|parsed| CommandAction::from_core_with_cwd(parsed, cwd))
                    .collect()
            };
            Some(ThreadItem::CommandExecution {
                id: id.clone(),
                plugin_id: assessment.plugin_id.clone(),
                script_path: assessment.script_path.clone(),
                command,
                cwd: cwd.clone().into(),
                process_id: None,
                source: CommandExecutionSource::Agent,
                status,
                command_actions,
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            })
        }
        GuardianAssessmentAction::WriteStdin { .. }
        | GuardianAssessmentAction::ApplyPatch { .. }
        | GuardianAssessmentAction::NetworkAccess { .. }
        | GuardianAssessmentAction::McpToolCall { .. }
        | GuardianAssessmentAction::RequestPermissions { .. } => None,
    }
}

pub fn guardian_auto_approval_review_notification(
    conversation_id: &ThreadId,
    event_turn_id: &str,
    assessment: &GuardianAssessmentEvent,
) -> ServerNotification {
    let turn_id = if assessment.turn_id.is_empty() {
        event_turn_id.to_string()
    } else {
        assessment.turn_id.clone()
    };
    let review = GuardianApprovalReview {
        status: match assessment.status {
            codex_protocol::protocol::GuardianAssessmentStatus::InProgress => {
                GuardianApprovalReviewStatus::InProgress
            }
            codex_protocol::protocol::GuardianAssessmentStatus::Approved => {
                GuardianApprovalReviewStatus::Approved
            }
            codex_protocol::protocol::GuardianAssessmentStatus::Denied => {
                GuardianApprovalReviewStatus::Denied
            }
            codex_protocol::protocol::GuardianAssessmentStatus::TimedOut => {
                GuardianApprovalReviewStatus::TimedOut
            }
            codex_protocol::protocol::GuardianAssessmentStatus::Aborted => {
                GuardianApprovalReviewStatus::Aborted
            }
        },
        risk_level: assessment.risk_level.map(Into::into),
        user_authorization: assessment.user_authorization.map(Into::into),
        rationale: assessment.rationale.clone(),
    };
    let action = assessment.action.clone().into();
    match assessment.status {
        codex_protocol::protocol::GuardianAssessmentStatus::InProgress => {
            ServerNotification::ItemGuardianApprovalReviewStarted(
                ItemGuardianApprovalReviewStartedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id,
                    review_id: assessment.id.clone(),
                    started_at_ms: assessment.started_at_ms,
                    target_item_id: assessment.target_item_id.clone(),
                    review,
                    action,
                },
            )
        }
        codex_protocol::protocol::GuardianAssessmentStatus::Approved
        | codex_protocol::protocol::GuardianAssessmentStatus::Denied
        | codex_protocol::protocol::GuardianAssessmentStatus::TimedOut
        | codex_protocol::protocol::GuardianAssessmentStatus::Aborted => {
            ServerNotification::ItemGuardianApprovalReviewCompleted(
                ItemGuardianApprovalReviewCompletedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id,
                    review_id: assessment.id.clone(),
                    started_at_ms: assessment.started_at_ms,
                    completed_at_ms: assessment
                        .completed_at_ms
                        .unwrap_or(assessment.started_at_ms),
                    target_item_id: assessment.target_item_id.clone(),
                    decision_source: assessment
                        .decision_source
                        .map(AutoReviewDecisionSource::from)
                        .unwrap_or(AutoReviewDecisionSource::Agent),
                    review,
                    action,
                },
            )
        }
    }
}

pub fn convert_patch_changes(changes: &HashMap<PathBuf, FileChange>) -> Vec<FileUpdateChange> {
    let mut converted: Vec<FileUpdateChange> = changes
        .iter()
        .map(|(path, change)| FileUpdateChange {
            path: path.to_string_lossy().into_owned(),
            kind: map_patch_change_kind(change),
            diff: format_file_change_diff(change),
        })
        .collect();
    converted.sort_by(|a, b| a.path.cmp(&b.path));
    converted
}

fn map_patch_change_kind(change: &FileChange) -> PatchChangeKind {
    match change {
        FileChange::Add { .. } => PatchChangeKind::Add,
        FileChange::Delete { .. } => PatchChangeKind::Delete,
        FileChange::Update { move_path, .. } => PatchChangeKind::Update {
            move_path: move_path.clone(),
        },
    }
}

fn format_file_change_diff(change: &FileChange) -> String {
    match change {
        FileChange::Add { content } => content.clone(),
        FileChange::Delete { content } => content.clone(),
        FileChange::Update {
            unified_diff,
            move_path,
        } => {
            if let Some(path) = move_path {
                format!("{unified_diff}\n\nMoved to: {}", path.display())
            } else {
                unified_diff.clone()
            }
        }
    }
}

#[cfg(test)]
#[path = "item_builders_tests.rs"]
mod tests;
