use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::common;
use crate::engine::ClaudeHooksEngine;
use crate::engine::ConfiguredHandler;
use crate::engine::HandlerRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::InterruptCommandInput;
use crate::schema::NullableString;

#[derive(Debug, Clone)]
pub struct InterruptRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
}

#[derive(Debug, Default)]
pub struct InterruptOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct InterruptHandlerData;

pub(crate) fn preview(handlers: &[ConfiguredHandler]) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::Interrupt,
        /*matcher_input*/ None,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(engine: &ClaudeHooksEngine, request: InterruptRequest) -> InterruptOutcome {
    let matched = dispatcher::select_handlers(
        &engine.handlers,
        HookEventName::Interrupt,
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return InterruptOutcome::default();
    }

    let InterruptRequest {
        session_id,
        turn_id,
        cwd,
        transcript_path,
        model,
        permission_mode,
    } = request;
    let input_json = match serde_json::to_string(&InterruptCommandInput {
        session_id: session_id.to_string(),
        turn_id: turn_id.clone(),
        transcript_path: NullableString::from_path(transcript_path),
        cwd: cwd.display().to_string(),
        hook_event_name: "Interrupt".to_string(),
        model,
        permission_mode,
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return InterruptOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(turn_id),
                    format!("failed to serialize interrupt hook input: {error}"),
                ),
            };
        }
    };

    let results = dispatcher::execute_handlers(
        engine,
        matched,
        input_json,
        cwd.as_path(),
        Some(turn_id),
        parse_completed,
    )
    .await;

    InterruptOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: HandlerRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<InterruptHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;

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
                } else if let Some(parsed) = output_parser::parse_interrupt(&run_result.stdout) {
                    if let Some(system_message) = parsed.system_message {
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Warning,
                            text: system_message,
                        });
                    }
                } else {
                    status = HookRunStatus::Failed;
                    let text = if output_parser::looks_like_json(&run_result.stdout) {
                        "hook returned invalid interrupt hook JSON output"
                    } else {
                        "Interrupt hook returned non-JSON stdout"
                    };
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: text.to_string(),
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
        data: InterruptHandlerData,
        completion_order: 0,
    }
}

#[cfg(test)]
#[path = "interrupt_tests.rs"]
mod tests;
