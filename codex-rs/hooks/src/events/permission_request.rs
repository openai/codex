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
//! 4. Fold the decisions conservatively: any deny wins, otherwise allow if at
//!    least one handler allowed the request, and accumulate the selected
//!    permission updates, otherwise there is no hook verdict.
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
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSuggestion {
    #[serde(rename = "type")]
    pub suggestion_type: PermissionSuggestionType,
    pub rules: Vec<PermissionSuggestionRule>,
    pub behavior: PermissionSuggestionBehavior,
    pub destination: PermissionSuggestionDestination,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionSuggestionType {
    AddRules,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionSuggestionBehavior {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum PermissionSuggestionDestination {
    #[serde(rename = "session")]
    Session,
    #[serde(rename = "projectSettings")]
    ProjectSettings,
    #[serde(rename = "userSettings")]
    UserSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionSuggestionRule {
    PrefixRule { command: Vec<String> },
}

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
    pub permission_suggestions: Vec<PermissionSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequestDecision {
    Allow {
        updated_permissions: Vec<PermissionSuggestion>,
    },
    Deny {
        message: String,
    },
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
                decision: None,
            };
        }
    };

    let mut results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id.clone()),
        parse_completed,
    )
    .await;

    for result in &mut results {
        if let Some(invalid_reason) = invalid_permission_updates(
            result.data.decision.as_ref(),
            &request.permission_suggestions,
        ) {
            result.completed.run.status = HookRunStatus::Failed;
            result.completed.run.entries.push(HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: invalid_reason,
            });
            result.data.decision = None;
        }
    }

    // Any deny wins immediately. Otherwise, accumulate the selected permission
    // updates from matching allow decisions in precedence order.
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
        decision,
    }
}

/// Resolve matching hook decisions conservatively: any deny wins immediately;
/// otherwise allow if any handler allowed the request and accumulate the
/// distinct selected permission updates in precedence order.
fn resolve_permission_request_decision<'a>(
    decisions: impl IntoIterator<Item = &'a PermissionRequestDecision>,
) -> Option<PermissionRequestDecision> {
    let mut saw_allow = false;
    let mut updated_permissions = Vec::new();
    for decision in decisions {
        match decision {
            PermissionRequestDecision::Allow {
                updated_permissions: selected_permissions,
            } => {
                saw_allow = true;
                for permission in selected_permissions {
                    if !updated_permissions.contains(permission) {
                        updated_permissions.push(permission.clone());
                    }
                }
            }
            PermissionRequestDecision::Deny { message } => {
                return Some(PermissionRequestDecision::Deny {
                    message: message.clone(),
                });
            }
        }
    }
    saw_allow.then_some(PermissionRequestDecision::Allow {
        updated_permissions,
    })
}

fn invalid_permission_updates(
    decision: Option<&PermissionRequestDecision>,
    offered_permissions: &[PermissionSuggestion],
) -> Option<String> {
    let PermissionRequestDecision::Allow {
        updated_permissions,
    } = decision?
    else {
        return None;
    };
    if updated_permissions
        .iter()
        .any(|permission| !offered_permissions.contains(permission))
    {
        Some("PermissionRequest hook returned updatedPermissions that were not offered".to_string())
    } else {
        None
    }
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
        permission_suggestions: (!request.permission_suggestions.is_empty())
            .then_some(request.permission_suggestions.clone()),
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
                            output_parser::PermissionRequestDecision::Allow {
                                updated_permissions,
                            } => {
                                decision = Some(PermissionRequestDecision::Allow {
                                    updated_permissions,
                                });
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
        data: PermissionRequestHandlerData { decision },
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::PermissionRequestDecision;
    use super::PermissionSuggestion;
    use super::PermissionSuggestionBehavior;
    use super::PermissionSuggestionDestination;
    use super::PermissionSuggestionRule;
    use super::PermissionSuggestionType;
    use super::invalid_permission_updates;
    use super::resolve_permission_request_decision;

    #[test]
    fn permission_request_deny_overrides_earlier_allow() {
        let decisions = [
            PermissionRequestDecision::Allow {
                updated_permissions: vec![],
            },
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
            PermissionRequestDecision::Allow {
                updated_permissions: vec![],
            },
            PermissionRequestDecision::Allow {
                updated_permissions: vec![],
            },
        ];

        assert_eq!(
            resolve_permission_request_decision(decisions.iter()),
            Some(PermissionRequestDecision::Allow {
                updated_permissions: vec![],
            })
        );
    }

    #[test]
    fn permission_request_returns_none_when_no_handler_decides() {
        let decisions = Vec::<PermissionRequestDecision>::new();

        assert_eq!(resolve_permission_request_decision(decisions.iter()), None);
    }

    #[test]
    fn permission_request_accumulates_distinct_updated_permissions() {
        let permission = PermissionSuggestion {
            suggestion_type: PermissionSuggestionType::AddRules,
            rules: vec![PermissionSuggestionRule::PrefixRule {
                command: vec!["rm".to_string(), "-f".to_string()],
            }],
            behavior: PermissionSuggestionBehavior::Allow,
            destination: PermissionSuggestionDestination::UserSettings,
        };
        let decisions = [
            PermissionRequestDecision::Allow {
                updated_permissions: vec![permission.clone()],
            },
            PermissionRequestDecision::Allow {
                updated_permissions: vec![permission.clone()],
            },
        ];

        assert_eq!(
            resolve_permission_request_decision(decisions.iter()),
            Some(PermissionRequestDecision::Allow {
                updated_permissions: vec![permission],
            })
        );
    }

    #[test]
    fn permission_request_rejects_unoffered_updated_permissions() {
        let offered = vec![PermissionSuggestion {
            suggestion_type: PermissionSuggestionType::AddRules,
            rules: vec![PermissionSuggestionRule::PrefixRule {
                command: vec!["rm".to_string()],
            }],
            behavior: PermissionSuggestionBehavior::Allow,
            destination: PermissionSuggestionDestination::UserSettings,
        }];
        let selected = PermissionRequestDecision::Allow {
            updated_permissions: vec![PermissionSuggestion {
                suggestion_type: PermissionSuggestionType::AddRules,
                rules: vec![PermissionSuggestionRule::PrefixRule {
                    command: vec!["curl".to_string()],
                }],
                behavior: PermissionSuggestionBehavior::Allow,
                destination: PermissionSuggestionDestination::UserSettings,
            }],
        };

        assert_eq!(
            invalid_permission_updates(Some(&selected), &offered),
            Some(
                "PermissionRequest hook returned updatedPermissions that were not offered"
                    .to_string()
            )
        );
    }
}
