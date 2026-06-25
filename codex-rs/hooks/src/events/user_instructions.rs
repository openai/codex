//! User-instruction hook execution.
//!
//! Unlike ordinary context-injecting hooks, successful stdout becomes the
//! thread's durable user-instruction snapshot. Every active handler may
//! contribute text to that snapshot.

use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::schema::NullableString;
use crate::schema::UserInstructionsCommandInput;

/// Input supplied to the active `UserInstructions` hook.
#[derive(Debug, Clone)]
pub struct UserInstructionsRequest {
    pub session_id: ThreadId,
    pub cwd: PathUri,
    pub command_cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
}

/// User instructions returned by a hook and their runtime-owned provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserInstructionsResult {
    pub text: String,
    pub source_path: PathUri,
}

/// Result of resolving and running active `UserInstructions` hooks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserInstructionsOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub results: Vec<UserInstructionsResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct UserInstructionsHandlerData {
    result: Option<UserInstructionsResult>,
    warning: Option<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &UserInstructionsRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::UserInstructions,
        /*matcher_input*/ None,
    )
    .iter()
    .map(dispatcher::running_summary)
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: UserInstructionsRequest,
) -> UserInstructionsOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::UserInstructions,
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return UserInstructionsOutcome::default();
    }

    let input = UserInstructionsCommandInput {
        session_id: request.session_id.to_string(),
        transcript_path: NullableString::from_path(request.transcript_path),
        cwd: request.cwd.inferred_native_path_string(),
        hook_event_name: "UserInstructions".to_string(),
        model: request.model,
        permission_mode: request.permission_mode,
    };
    let input_json = match serde_json::to_string(&input) {
        Ok(input_json) => input_json,
        Err(error) => {
            let warning = format!("failed to serialize UserInstructions hook input: {error}");
            return UserInstructionsOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    /*turn_id*/ None,
                    warning.clone(),
                ),
                results: Vec::new(),
                warnings: vec![warning],
            };
        }
    };

    let parsed = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.command_cwd.as_path(),
        /*turn_id*/ None,
        parse_completed,
    )
    .await;
    let mut outcome = UserInstructionsOutcome::default();
    for parsed in parsed {
        outcome.hook_events.push(parsed.completed);
        if let Some(result) = parsed.data.result {
            outcome.results.push(result);
        }
        if let Some(warning) = parsed.data.warning {
            outcome.warnings.push(warning);
        }
    }
    outcome
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<UserInstructionsHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut text = None;
    let mut warning = None;

    match run_result.error.as_deref() {
        Some(error) => {
            status = HookRunStatus::Failed;
            entries.push(HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error.to_string(),
            });
            warning = Some(handler_warning(handler, &format!("failed: {error}")));
        }
        None => match run_result.exit_code {
            Some(0) => {
                let trimmed = run_result.stdout.trim();
                if trimmed.is_empty() {
                    let message = "returned no instructions";
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Warning,
                        text: message.to_string(),
                    });
                    warning = Some(handler_warning(handler, message));
                } else {
                    text = Some(trimmed.to_string());
                }
            }
            Some(exit_code) => {
                status = HookRunStatus::Failed;
                let mut message = format!("hook exited with code {exit_code}");
                if let Some(stderr) = common::trimmed_non_empty(&run_result.stderr) {
                    message.push_str(": ");
                    message.push_str(&stderr);
                }
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: message.clone(),
                });
                warning = Some(handler_warning(handler, &format!("failed: {message}")));
            }
            None => {
                status = HookRunStatus::Failed;
                let message = "hook exited without a status code";
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: message.to_string(),
                });
                warning = Some(handler_warning(handler, &format!("failed: {message}")));
            }
        },
    }

    let result = text.map(|text| UserInstructionsResult {
        text,
        source_path: PathUri::from_abs_path(&handler.source_path),
    });
    dispatcher::ParsedHandler {
        completed: HookCompletedEvent {
            turn_id,
            run: dispatcher::completed_summary(handler, &run_result, status, entries),
        },
        data: UserInstructionsHandlerData { result, warning },
        completion_order: 0,
    }
}

fn handler_warning(handler: &ConfiguredHandler, message: &str) -> String {
    format!(
        "UserInstructions hook from {} {message}",
        PathUri::from_abs_path(&handler.source_path)
    )
}

#[cfg(test)]
#[path = "user_instructions_tests.rs"]
mod tests;
