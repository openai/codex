use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::ReviewDecision;
use serde_json::Value;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::engine::output_parser::PermissionRequestDecision;
use crate::schema::NullableString;
use crate::schema::PermissionRequestCommandInput;

#[derive(Debug, Clone)]
pub struct PermissionRequestRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub permission_mode: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub permission_suggestions: Vec<Value>,
    pub codex_permission_context: Value,
}

#[derive(Debug)]
pub struct PermissionRequestOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub decision: Option<ReviewDecision>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PermissionRequestHandlerData {
    decision: Option<ReviewDecision>,
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
    .map(|handler| dispatcher::running_summary(&handler))
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

    let input_json = match serde_json::to_string(&PermissionRequestCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PermissionRequest".to_string(),
        permission_mode: request.permission_mode,
        tool_name: request.tool_name,
        tool_input: request.tool_input,
        permission_suggestions: request.permission_suggestions,
        codex_permission_context: request.codex_permission_context,
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            let hook_events = common::serialization_failure_hook_events(
                matched,
                Some(request.turn_id),
                format!("failed to serialize permission request hook input: {error}"),
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
        Some(request.turn_id),
        parse_completed,
    )
    .await;

    let decision = if results
        .iter()
        .any(|result| result.data.decision == Some(ReviewDecision::Denied))
    {
        Some(ReviewDecision::Denied)
    } else if results
        .iter()
        .any(|result| result.data.decision == Some(ReviewDecision::Approved))
    {
        Some(ReviewDecision::Approved)
    } else {
        None
    };

    PermissionRequestOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
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
                        decision = Some(match parsed_decision {
                            PermissionRequestDecision::Allow => ReviewDecision::Approved,
                            PermissionRequestDecision::Deny => ReviewDecision::Denied,
                        });
                        if matches!(parsed_decision, PermissionRequestDecision::Deny) {
                            status = HookRunStatus::Blocked;
                        }
                    }
                } else if trimmed_stdout.starts_with('{') || trimmed_stdout.starts_with('[') {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid permission-request JSON output".to_string(),
                    });
                }
            }
            Some(2) => {
                status = HookRunStatus::Blocked;
                decision = Some(ReviewDecision::Denied);
                if let Some(reason) = common::trimmed_non_empty(&run_result.stderr) {
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Feedback,
                        text: reason,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookOutputEntry;
    use codex_protocol::protocol::HookOutputEntryKind;
    use codex_protocol::protocol::HookRunStatus;
    use codex_protocol::protocol::ReviewDecision;
    use pretty_assertions::assert_eq;

    use super::PermissionRequestHandlerData;
    use super::parse_completed;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn allow_decision_approves_request() {
        let parsed = parse_completed(
            &ConfiguredHandler {
                event_name: HookEventName::PermissionRequest,
                matcher: Some("^Bash$".to_string()),
                command: "python3 hook.py".to_string(),
                timeout_sec: 5,
                status_message: Some("running permission hook".to_string()),
                source_path: PathBuf::from("/tmp/hooks.json"),
                display_order: 0,
            },
            CommandRunResult {
                stdout: r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#.to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                error: None,
                started_at: 10,
                completed_at: 15,
                duration_ms: 5,
            },
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PermissionRequestHandlerData {
                decision: Some(ReviewDecision::Approved),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
    }

    #[test]
    fn unsupported_updated_permissions_fails_open() {
        let parsed = parse_completed(
            &ConfiguredHandler {
                event_name: HookEventName::PermissionRequest,
                matcher: Some("^Bash$".to_string()),
                command: "python3 hook.py".to_string(),
                timeout_sec: 5,
                status_message: Some("running permission hook".to_string()),
                source_path: PathBuf::from("/tmp/hooks.json"),
                display_order: 0,
            },
            CommandRunResult {
                stdout: r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedPermissions":[]}}}"#.to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                error: None,
                started_at: 10,
                completed_at: 15,
                duration_ms: 5,
            },
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, PermissionRequestHandlerData { decision: None });
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "PermissionRequest hook returned unsupported updatedPermissions".to_string(),
            }]
        );
    }
}
