//! Shared builders for synthetic [`ThreadItem`] values emitted by the app-server layer.
//!
//! These items do not come from first-class core `ItemStarted` / `ItemCompleted` events.
//! Instead, the app-server synthesizes them so clients can render a coherent lifecycle for
//! approvals and other pre-execution flows before the underlying tool has started or when the
//! tool never starts at all.
//!
//! Keeping these builders in one place is useful for two reasons:
//! - Live notifications and rebuilt `thread/read` history both need to construct the same
//!   synthetic items, so sharing the logic avoids drift between those paths.
//! - The projection is presentation-specific. Core protocol events stay generic, while the
//!   app-server protocol decides how to surface those events as `ThreadItem`s for clients.
use crate::protocol::common::ServerNotification;
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
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::PatchApplyBeginEvent;
use codex_protocol::protocol::PatchApplyEndEvent;
use codex_shell_command::parse_command::parse_command;
use codex_shell_command::parse_command::shlex_join;
use std::collections::HashMap;
use std::path::PathBuf;

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

pub fn build_command_execution_approval_request_item(
    payload: &ExecApprovalRequestEvent,
) -> ThreadItem {
    ThreadItem::CommandExecution {
        id: payload.call_id.clone(),
        command: shlex_join(&payload.command),
        cwd: payload.cwd.clone(),
        process_id: None,
        source: CommandExecutionSource::Agent,
        status: CommandExecutionStatus::InProgress,
        command_actions: payload
            .parsed_cmd
            .iter()
            .cloned()
            .map(CommandAction::from)
            .collect(),
        aggregated_output: None,
        exit_code: None,
        duration_ms: None,
    }
}

pub fn build_command_execution_begin_item(payload: &ExecCommandBeginEvent) -> ThreadItem {
    ThreadItem::CommandExecution {
        id: payload.call_id.clone(),
        command: shlex_join(&payload.command),
        cwd: payload.cwd.clone(),
        process_id: payload.process_id.clone(),
        source: payload.source.into(),
        status: CommandExecutionStatus::InProgress,
        command_actions: payload
            .parsed_cmd
            .iter()
            .cloned()
            .map(CommandAction::from)
            .collect(),
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

    ThreadItem::CommandExecution {
        id: payload.call_id.clone(),
        command: shlex_join(&payload.command),
        cwd: payload.cwd.clone(),
        process_id: payload.process_id.clone(),
        source: payload.source.into(),
        status: (&payload.status).into(),
        command_actions: payload
            .parsed_cmd
            .iter()
            .cloned()
            .map(CommandAction::from)
            .collect(),
        aggregated_output,
        exit_code: Some(payload.exit_code),
        duration_ms: Some(duration_ms),
    }
}

/// Build a guardian-derived [`ThreadItem`].
///
/// Currently this only synthesizes [`ThreadItem::CommandExecution`] for
/// [`GuardianAssessmentAction::Command`] and [`GuardianAssessmentAction::Execve`].
pub fn build_item_from_guardian_event(
    assessment: &GuardianAssessmentEvent,
    status: CommandExecutionStatus,
) -> Option<ThreadItem> {
    match &assessment.action {
        GuardianAssessmentAction::Command { command, cwd, .. } => {
            let command = command.clone();
            let command_actions = vec![CommandAction::Unknown {
                command: command.clone(),
            }];
            Some(ThreadItem::CommandExecution {
                id: assessment.id.clone(),
                command,
                cwd: cwd.clone(),
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
                parsed_cmd.into_iter().map(CommandAction::from).collect()
            };
            Some(ThreadItem::CommandExecution {
                id: assessment.id.clone(),
                command,
                cwd: cwd.clone(),
                process_id: None,
                source: CommandExecutionSource::Agent,
                status,
                command_actions,
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            })
        }
        GuardianAssessmentAction::ApplyPatch { .. }
        | GuardianAssessmentAction::NetworkAccess { .. }
        | GuardianAssessmentAction::McpToolCall { .. } => None,
    }
}

pub fn guardian_auto_approval_review_notification(
    conversation_id: &ThreadId,
    event_turn_id: &str,
    assessment: &GuardianAssessmentEvent,
) -> ServerNotification {
    // TODO(ccunningham): Attach guardian review state to the reviewed tool
    // item's lifecycle instead of sending standalone review notifications so
    // the app-server API can persist and replay review state via `thread/read`.
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
            codex_protocol::protocol::GuardianAssessmentStatus::Aborted => {
                GuardianApprovalReviewStatus::Aborted
            }
        },
        risk_score: assessment.risk_score,
        risk_level: assessment.risk_level.map(Into::into),
        rationale: assessment.rationale.clone(),
    };
    let action = assessment.action.clone().into();
    match assessment.status {
        codex_protocol::protocol::GuardianAssessmentStatus::InProgress => {
            ServerNotification::ItemGuardianApprovalReviewStarted(
                ItemGuardianApprovalReviewStartedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id,
                    target_item_id: assessment.id.clone(),
                    review,
                    action,
                },
            )
        }
        codex_protocol::protocol::GuardianAssessmentStatus::Approved
        | codex_protocol::protocol::GuardianAssessmentStatus::Denied
        | codex_protocol::protocol::GuardianAssessmentStatus::Aborted => {
            ServerNotification::ItemGuardianApprovalReviewCompleted(
                ItemGuardianApprovalReviewCompletedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id,
                    target_item_id: assessment.id.clone(),
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
