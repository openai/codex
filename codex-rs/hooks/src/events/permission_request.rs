//! Permission-request hook execution.
//!
//! This event runs in the approval path, before guardian or user approval UI is
//! shown. Unlike `pre_tool_use`, handlers do not rewrite tool input or block by
//! stopping execution outright; instead they can return a concrete allow/deny
//! decision, or decline to decide and let the normal approval flow continue.
//!
//! The event also mirrors the rest of the hook system's lifecycle:
//!
//! 1. Preview matching handlers so the UI can render pending hook rows.
//! 2. Execute every matching handler in precedence order.
//! 3. Parse each handler into transcript-visible output plus an optional
//!    decision.
//! 4. Fold the decisions conservatively: any deny wins, otherwise the last
//!    allow wins, otherwise there is no hook verdict.
use std::path::PathBuf;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::PermissionRequestCommandInput;
use crate::schema::PermissionRequestToolInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;

#[derive(Debug, Clone)]
pub struct PermissionRequestRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub tool_name: String,
    pub run_id_suffix: String,
    pub command: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequestDecision {
    Allow,
    Deny { message: String },
}

#[derive(Debug)]
pub struct PermissionRequestOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub should_stop: bool,
    pub stop_reason: Option<String>,
    pub decision: Option<PermissionRequestDecision>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PermissionRequestHandlerData {
    should_stop: bool,
    stop_reason: Option<String>,
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
            should_stop: false,
            stop_reason: None,
            decision: None,
        };
    }

    let input_json = match serde_json::to_string(&build_command_input(&request)) {
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
                should_stop: false,
                stop_reason: None,
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

    let should_stop = results.iter().any(|result| result.data.should_stop);
    let stop_reason = results
        .iter()
        .find_map(|result| result.data.stop_reason.clone());
    // Preserve the most specific matching allow, but treat any deny as final so
    // broader policy layers cannot accidentally overrule a more specific block.
    let decision = resolve_permission_request_decision(
        results
            .iter()
            .filter_map(|result| result.data.decision.as_ref()),
    );

    PermissionRequestOutcome {
        hook_events: results
            .into_iter()
            .map(|result| {
                common::hook_completed_for_tool_use(result.completed, &request.run_id_suffix)
            })
            .collect(),
        should_stop,
        stop_reason,
        decision: (!should_stop).then_some(decision).flatten(),
    }
}

/// Resolve matching hook decisions conservatively: any deny wins immediately;
/// otherwise keep the highest-precedence allow so more specific handlers
/// override broader ones.
fn resolve_permission_request_decision<'a>(
    decisions: impl IntoIterator<Item = &'a PermissionRequestDecision>,
) -> Option<PermissionRequestDecision> {
    let mut resolved_allow = None;
    for decision in decisions {
        match decision {
            PermissionRequestDecision::Allow => {
                resolved_allow = Some(PermissionRequestDecision::Allow);
            }
            PermissionRequestDecision::Deny { message } => {
                return Some(PermissionRequestDecision::Deny {
                    message: message.clone(),
                });
            }
        }
    }
    resolved_allow
}

fn build_command_input(request: &PermissionRequestRequest) -> PermissionRequestCommandInput {
    PermissionRequestCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: crate::schema::NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PermissionRequest".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        tool_name: request.tool_name.clone(),
        tool_input: PermissionRequestToolInput {
            command: request.command.clone(),
            description: request.description.clone(),
        },
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<PermissionRequestHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut should_stop = false;
    let mut stop_reason = None;
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
                    if !parsed.universal.continue_processing {
                        status = HookRunStatus::Stopped;
                        should_stop = true;
                        stop_reason = parsed.universal.stop_reason.clone();
                        let stop_text = parsed.universal.stop_reason.unwrap_or_else(|| {
                            "PermissionRequest hook stopped execution".to_string()
                        });
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Stop,
                            text: stop_text,
                        });
                    } else if let Some(invalid_reason) = parsed.invalid_reason {
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
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid permission-request JSON output".to_string(),
                    });
                }
            }
            Some(2) => {
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
        data: PermissionRequestHandlerData {
            should_stop,
            stop_reason,
            decision,
        },
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookOutputEntry;
    use codex_protocol::protocol::HookOutputEntryKind;
    use codex_protocol::protocol::HookRunStatus;
    use pretty_assertions::assert_eq;

    use super::PermissionRequestDecision;
    use super::PermissionRequestHandlerData;
    use super::parse_completed;
    use super::resolve_permission_request_decision;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn permission_request_deny_overrides_earlier_allow() {
        let decisions = [
            PermissionRequestDecision::Allow,
            PermissionRequestDecision::Deny {
                message: "repo deny".to_string(),
            },
        ];

        assert_eq!(
            resolve_permission_request_decision(decisions.iter()),
            Some(PermissionRequestDecision::Deny {
                message: "repo deny".to_string(),
            })
        );
    }

    #[test]
    fn permission_request_returns_allow_when_no_handler_denies() {
        let decisions = [
            PermissionRequestDecision::Allow,
            PermissionRequestDecision::Allow,
        ];

        assert_eq!(
            resolve_permission_request_decision(decisions.iter()),
            Some(PermissionRequestDecision::Allow)
        );
    }

    #[test]
    fn permission_request_returns_none_when_no_handler_decides() {
        let decisions = Vec::<PermissionRequestDecision>::new();

        assert_eq!(resolve_permission_request_decision(decisions.iter()), None);
    }

    #[test]
    fn continue_false_stops_permission_request_flow() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"continue":false,"stopReason":"stop now","hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PermissionRequestHandlerData {
                should_stop: true,
                stop_reason: Some("stop now".to_string()),
                decision: None,
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Stopped);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Stop,
                text: "stop now".to_string(),
            }]
        );
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::PermissionRequest,
            matcher: None,
            command: "python3 hook.py".to_string(),
            timeout_sec: 30,
            status_message: None,
            source_path: std::path::PathBuf::from("/tmp/hooks.json"),
            display_order: 0,
        }
    }

    fn run_result(exit_code: Option<i32>, stdout: &str, stderr: &str) -> CommandRunResult {
        CommandRunResult {
            started_at: 0,
            completed_at: 1,
            duration_ms: 1,
            exit_code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            error: None,
        }
    }
}
