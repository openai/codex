use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;

use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::StopCommandInput;

#[derive(Debug, Clone)]
pub struct StopRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub stop_hook_active: bool,
    pub last_assistant_message: Option<String>,
}

#[derive(Debug)]
pub struct StopOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub should_stop: bool,
    pub stop_reason: Option<String>,
    pub should_block: bool,
    pub block_reason: Option<String>,
    pub block_message_for_model: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct StopHandlerData {
    should_stop: bool,
    stop_reason: Option<String>,
    should_block: bool,
    block_reason: Option<String>,
    block_message_for_model: Option<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &StopRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(handlers, HookEventName::Stop, None)
        .into_iter()
        .map(|handler| dispatcher::running_summary(&handler))
        .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: StopRequest,
) -> StopOutcome {
    let matched = dispatcher::select_handlers(handlers, HookEventName::Stop, None);
    if matched.is_empty() {
        return StopOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            should_block: false,
            block_reason: None,
            block_message_for_model: None,
        };
    }

    let input_json = serde_json::to_string(&StopCommandInput::new(
        request.session_id.to_string(),
        request.transcript_path.clone(),
        request.cwd.display().to_string(),
        request.model.clone(),
        request.permission_mode.clone(),
        request.stop_hook_active,
        request.last_assistant_message.clone(),
    ))
    .expect("serialize stop hook input");

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id),
        parse_completed,
    )
    .await;

    let should_stop = results.iter().any(|result| result.data.should_stop);
    let stop_reason = results
        .iter()
        .find_map(|result| result.data.stop_reason.clone());

    let should_block = !should_stop && results.iter().any(|result| result.data.should_block);
    let block_reason = if should_block {
        results
            .iter()
            .find_map(|result| result.data.block_reason.clone())
    } else {
        None
    };
    let block_message_for_model = if should_block {
        results
            .iter()
            .find_map(|result| result.data.block_message_for_model.clone())
    } else {
        None
    };

    StopOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
        should_stop,
        stop_reason,
        should_block,
        block_reason,
        block_message_for_model,
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<StopHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut should_stop = false;
    let mut stop_reason = None;
    let mut should_block = false;
    let mut block_reason = None;
    let mut block_message_for_model = None;

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
                if let Some(parsed) = output_parser::parse_stop(&run_result.stdout) {
                    if let Some(system_message) = parsed.universal.system_message {
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Warning,
                            text: system_message,
                        });
                    }
                    let _ = parsed.universal.suppress_output;
                    if !parsed.universal.continue_processing {
                        status = HookRunStatus::Stopped;
                        should_stop = true;
                        stop_reason = parsed.universal.stop_reason.clone();
                        if let Some(stop_reason_text) = parsed.universal.stop_reason {
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Stop,
                                text: stop_reason_text,
                            });
                        }
                    } else if parsed.should_block {
                        status = HookRunStatus::Blocked;
                        should_block = true;
                        block_reason = parsed.reason.clone();
                        block_message_for_model = parsed.reason.clone();
                        if let Some(reason) = parsed.reason {
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Feedback,
                                text: reason,
                            });
                        }
                    }
                }
            }
            Some(2) => {
                status = HookRunStatus::Blocked;
                should_block = true;
                let fallback_reason = trimmed_non_empty(&run_result.stderr);
                block_reason = fallback_reason.clone();
                block_message_for_model = fallback_reason.clone();
                if let Some(reason) = fallback_reason {
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
        data: StopHandlerData {
            should_stop,
            stop_reason,
            should_block,
            block_reason,
            block_message_for_model,
        },
    }
}

fn trimmed_non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        return Some(trimmed.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookRunStatus;
    use pretty_assertions::assert_eq;

    use super::StopHandlerData;
    use super::parse_completed;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn continue_false_overrides_block_decision() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"continue":false,"stopReason":"done","decision":"block","reason":"keep going"}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            StopHandlerData {
                should_stop: true,
                stop_reason: Some("done".to_string()),
                should_block: false,
                block_reason: None,
                block_message_for_model: None,
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Stopped);
    }

    #[test]
    fn exit_code_two_uses_stderr_feedback_only() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(2), "ignored stdout", "retry with tests"),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            StopHandlerData {
                should_stop: false,
                stop_reason: None,
                should_block: true,
                block_reason: Some("retry with tests".to_string()),
                block_message_for_model: Some("retry with tests".to_string()),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::Stop,
            matcher: None,
            command: "echo hook".to_string(),
            timeout_sec: 600,
            status_message: None,
            source_path: PathBuf::from("/tmp/hooks.json"),
            display_order: 0,
        }
    }

    fn run_result(exit_code: Option<i32>, stdout: &str, stderr: &str) -> CommandRunResult {
        CommandRunResult {
            started_at: 1,
            completed_at: 2,
            duration_ms: 1,
            exit_code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            error: None,
        }
    }
}
