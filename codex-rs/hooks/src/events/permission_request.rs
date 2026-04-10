//! PermissionRequest hook execution for approval prompts.
//!
//! This event is different from `PreToolUse`: it runs only when Codex is about
//! to ask for permission, and its decision answers that approval prompt rather
//! than blocking normal tool execution. A quiet hook is a no-op so callers can
//! fall back to the existing approval path.

use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::PermissionRequestApprovalReviewDecisionWire;
use crate::schema::PermissionRequestApprovalReviewRiskLevelWire;
use crate::schema::PermissionRequestApprovalReviewStatusWire;
use crate::schema::PermissionRequestApprovalReviewUserAuthorizationWire;
use crate::schema::PermissionRequestApprovalReviewWire;
use crate::schema::PermissionRequestCommandInput;
use crate::schema::PermissionRequestToolInput;

#[derive(Debug, Clone)]
pub struct PermissionRequestRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub tool_name: String,
    /// Suffix used only for hook run ids.
    ///
    /// Claude's PermissionRequest input does not include `tool_use_id`, but Codex
    /// still needs stable begin/end ids for hook UI and transcript bookkeeping.
    pub run_id_suffix: String,
    pub command: String,
    /// Advisory approval context from Codex's automated reviewer, when one ran.
    ///
    /// A hook can use this as another signal, but it is not bound by the
    /// guardian's decision. The hook may allow, deny, or stay quiet; if it stays
    /// quiet, the orchestrator falls back to the guardian's original decision.
    pub approval_review: Option<PermissionRequestApprovalReview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequestDecision {
    Allow,
    Deny { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequestApprovalReview {
    pub status: PermissionRequestApprovalReviewStatus,
    pub decision: Option<PermissionRequestApprovalReviewDecision>,
    pub risk_level: Option<PermissionRequestApprovalReviewRiskLevel>,
    pub user_authorization: Option<PermissionRequestApprovalReviewUserAuthorization>,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequestApprovalReviewStatus {
    Approved,
    Denied,
    Aborted,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequestApprovalReviewDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequestApprovalReviewRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequestApprovalReviewUserAuthorization {
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Debug)]
pub struct PermissionRequestOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub decision: Option<PermissionRequestDecision>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PermissionRequestHandlerData {
    decision: Option<PermissionRequestDecision>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &PermissionRequestRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::PermissionRequest,
        Some(&request.tool_name),
    )
    .into_iter()
    .map(|handler| {
        common::hook_run_for_tool_use(
            dispatcher::running_summary(&handler),
            &request.run_id_suffix,
        )
    })
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: PermissionRequestRequest,
) -> PermissionRequestOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::PermissionRequest,
        Some(&request.tool_name),
    );
    if matched.is_empty() {
        return PermissionRequestOutcome {
            hook_events: Vec::new(),
            decision: None,
        };
    }

    // This first pass is Bash-only. Keep the wire input fixed to Claude's
    // `Bash` shape even though the request carries `tool_name`, so later
    // tool support has to choose its own explicit schema instead of
    // accidentally inheriting Bash fields.
    let input_json = match serde_json::to_string(&PermissionRequestCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: crate::schema::NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PermissionRequest".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        tool_name: "Bash".to_string(),
        tool_input: PermissionRequestToolInput {
            command: request.command.clone(),
        },
        approval_review: request.approval_review.map(Into::into),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            let hook_events = common::serialization_failure_hook_events_for_tool_use(
                matched,
                Some(request.turn_id.clone()),
                format!("failed to serialize permission request hook input: {error}"),
                &request.run_id_suffix,
            );
            return PermissionRequestOutcome {
                hook_events,
                decision: None,
            };
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id.clone()),
        parse_completed,
    )
    .await;

    // Multiple hooks may match the same approval prompt. For now, use the first
    // explicit decision in declaration order and leave richer precedence rules
    // to the follow-up work.
    let decision = results
        .iter()
        .find_map(|result| result.data.decision.clone());

    PermissionRequestOutcome {
        hook_events: results
            .into_iter()
            .map(|result| {
                common::hook_completed_for_tool_use(result.completed, &request.run_id_suffix)
            })
            .collect(),
        decision,
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<PermissionRequestHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut decision = None;

    match run_result.error.as_deref() {
        Some(error) => {
            status = HookRunStatus::Failed;
            entries.push(HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error.to_string(),
            });
        }
        None => match run_result.exit_code {
            Some(0) => {
                let trimmed_stdout = run_result.stdout.trim();
                if trimmed_stdout.is_empty() {
                } else if let Some(parsed) =
                    output_parser::parse_permission_request(&run_result.stdout)
                {
                    if let Some(system_message) = parsed.universal.system_message {
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Warning,
                            text: system_message,
                        });
                    }
                    if let Some(invalid_reason) = parsed.invalid_reason {
                        status = HookRunStatus::Failed;
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Error,
                            text: invalid_reason,
                        });
                    } else if let Some(parsed_decision) = parsed.decision {
                        match parsed_decision {
                            output_parser::PermissionRequestDecision::Allow => {
                                decision = Some(PermissionRequestDecision::Allow);
                            }
                            output_parser::PermissionRequestDecision::Deny { message } => {
                                status = HookRunStatus::Blocked;
                                entries.push(HookOutputEntry {
                                    kind: HookOutputEntryKind::Feedback,
                                    text: message.clone(),
                                });
                                decision = Some(PermissionRequestDecision::Deny { message });
                            }
                        }
                    }
                } else if trimmed_stdout.starts_with('{') || trimmed_stdout.starts_with('[') {
                    // Invalid JSON-like output is treated as a hook failure, not an
                    // approval decision. That keeps malformed hooks fail-open: the
                    // orchestrator can still fall back to normal approval.
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid permission-request JSON output".to_string(),
                    });
                }
            }
            Some(2) => {
                // Match Claude's blocking-hook convention: exit code 2 denies
                // the approval prompt, with stderr as the denial message.
                if let Some(message) = common::trimmed_non_empty(&run_result.stderr) {
                    status = HookRunStatus::Blocked;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Feedback,
                        text: message.clone(),
                    });
                    decision = Some(PermissionRequestDecision::Deny { message });
                } else {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "PermissionRequest hook exited with code 2 but did not write a denial reason to stderr".to_string(),
                    });
                }
            }
            Some(exit_code) => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: format!("hook exited with code {exit_code}"),
                });
            }
            None => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: "hook exited without a status code".to_string(),
                });
            }
        },
    }

    let completed = HookCompletedEvent {
        turn_id,
        run: dispatcher::completed_summary(handler, &run_result, status, entries),
    };

    dispatcher::ParsedHandler {
        completed,
        data: PermissionRequestHandlerData { decision },
    }
}

impl From<PermissionRequestApprovalReview> for PermissionRequestApprovalReviewWire {
    fn from(value: PermissionRequestApprovalReview) -> Self {
        Self {
            source: "guardian".to_string(),
            status: value.status.into(),
            decision: value.decision.map(Into::into),
            risk_level: value.risk_level.map(Into::into),
            user_authorization: value.user_authorization.map(Into::into),
            rationale: value.rationale,
        }
    }
}

impl From<PermissionRequestApprovalReviewStatus> for PermissionRequestApprovalReviewStatusWire {
    fn from(value: PermissionRequestApprovalReviewStatus) -> Self {
        match value {
            PermissionRequestApprovalReviewStatus::Approved => Self::Approved,
            PermissionRequestApprovalReviewStatus::Denied => Self::Denied,
            PermissionRequestApprovalReviewStatus::Aborted => Self::Aborted,
            PermissionRequestApprovalReviewStatus::Failed => Self::Failed,
            PermissionRequestApprovalReviewStatus::TimedOut => Self::TimedOut,
        }
    }
}

impl From<PermissionRequestApprovalReviewDecision> for PermissionRequestApprovalReviewDecisionWire {
    fn from(value: PermissionRequestApprovalReviewDecision) -> Self {
        match value {
            PermissionRequestApprovalReviewDecision::Allow => Self::Allow,
            PermissionRequestApprovalReviewDecision::Deny => Self::Deny,
        }
    }
}

impl From<PermissionRequestApprovalReviewRiskLevel>
    for PermissionRequestApprovalReviewRiskLevelWire
{
    fn from(value: PermissionRequestApprovalReviewRiskLevel) -> Self {
        match value {
            PermissionRequestApprovalReviewRiskLevel::Low => Self::Low,
            PermissionRequestApprovalReviewRiskLevel::Medium => Self::Medium,
            PermissionRequestApprovalReviewRiskLevel::High => Self::High,
            PermissionRequestApprovalReviewRiskLevel::Critical => Self::Critical,
        }
    }
}

impl From<PermissionRequestApprovalReviewUserAuthorization>
    for PermissionRequestApprovalReviewUserAuthorizationWire
{
    fn from(value: PermissionRequestApprovalReviewUserAuthorization) -> Self {
        match value {
            PermissionRequestApprovalReviewUserAuthorization::Unknown => Self::Unknown,
            PermissionRequestApprovalReviewUserAuthorization::Low => Self::Low,
            PermissionRequestApprovalReviewUserAuthorization::Medium => Self::Medium,
            PermissionRequestApprovalReviewUserAuthorization::High => Self::High,
        }
    }
}
