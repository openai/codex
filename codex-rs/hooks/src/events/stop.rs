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
    pub continuation_prompt: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct StopHandlerData {
    should_stop: bool,
    stop_reason: Option<String>,
    should_block: bool,
    block_reason: Option<String>,
    continuation_prompt: Option<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &StopRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::Stop,
        /*session_start_source*/ None,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: StopRequest,
) -> StopOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::Stop,
        /*session_start_source*/ None,
    );
    if matched.is_empty() {
        return StopOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            should_block: false,
            block_reason: None,
            continuation_prompt: None,
        };
    }

    let input_json = match serde_json::to_string(&StopCommandInput::new(
        request.session_id.to_string(),
        request.transcript_path.clone(),
        request.cwd.display().to_string(),
        request.model.clone(),
        request.permission_mode.clone(),
        request.stop_hook_active,
        request.last_assistant_message.clone(),
    )) {
        Ok(input_json) => input_json,
        Err(error) => {
            return serialization_failure_outcome(
                matched,
                Some(request.turn_id),
                format!("failed to serialize stop hook input: {error}"),
            );
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

    let aggregate = aggregate_results(results.iter().map(|result| &result.data));

    StopOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
        should_stop: aggregate.should_stop,
        stop_reason: aggregate.stop_reason,
        should_block: aggregate.should_block,
        block_reason: aggregate.block_reason,
        continuation_prompt: aggregate.continuation_prompt,
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
    let mut continuation_prompt = None;

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
                } else if let Some(parsed) = output_parser::parse_stop(&run_result.stdout) {
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
                    } else if let Some(invalid_block_reason) = parsed.invalid_block_reason {
                        status = HookRunStatus::Failed;
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Error,
                            text: invalid_block_reason,
                        });
                    } else if parsed.should_block {
                        if let Some(reason) = parsed.reason.as_deref().and_then(trimmed_non_empty) {
                            status = HookRunStatus::Blocked;
                            should_block = true;
                            block_reason = Some(reason.clone());
                            continuation_prompt = Some(reason.clone());
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Feedback,
                                text: reason,
                            });
                        } else {
                            status = HookRunStatus::Failed;
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Error,
                                text:
                                    "Stop hook returned decision:block without a non-empty reason"
                                        .to_string(),
                            });
                        }
                    }
                } else {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid stop hook JSON output".to_string(),
                    });
                }
            }
            Some(2) => {
                if let Some(reason) = trimmed_non_empty(&run_result.stderr) {
                    status = HookRunStatus::Blocked;
                    should_block = true;
                    block_reason = Some(reason.clone());
                    continuation_prompt = Some(reason.clone());
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Feedback,
                        text: reason,
                    });
                } else {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text:
                            "Stop hook exited with code 2 but did not write a continuation prompt to stderr"
                                .to_string(),
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
            continuation_prompt,
        },
    }
}

fn aggregate_results<'a>(
    results: impl IntoIterator<Item = &'a StopHandlerData>,
) -> StopHandlerData {
    let results = results.into_iter().collect::<Vec<_>>();
    let should_stop = results.iter().any(|result| result.should_stop);
    let stop_reason = results.iter().find_map(|result| result.stop_reason.clone());
    let should_block = !should_stop && results.iter().any(|result| result.should_block);
    let block_reason = if should_block {
        join_block_text(results.iter().copied(), |result| {
            result.block_reason.as_deref()
        })
    } else {
        None
    };
    let continuation_prompt = if should_block {
        join_block_text(results.iter().copied(), |result| {
            result.continuation_prompt.as_deref()
        })
    } else {
        None
    };

    StopHandlerData {
        should_stop,
        stop_reason,
        should_block,
        block_reason,
        continuation_prompt,
    }
}

fn join_block_text<'a>(
    results: impl IntoIterator<Item = &'a StopHandlerData>,
    select: impl Fn(&'a StopHandlerData) -> Option<&'a str>,
) -> Option<String> {
    let parts = results
        .into_iter()
        .filter_map(select)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("\n\n"))
}

fn trimmed_non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        return Some(trimmed.to_string());
    }
    None
}

fn serialization_failure_outcome(
    handlers: Vec<ConfiguredHandler>,
    turn_id: Option<String>,
    error_message: String,
) -> StopOutcome {
    let hook_events = handlers
        .into_iter()
        .map(|handler| {
            let mut run = dispatcher::running_summary(&handler);
            run.status = HookRunStatus::Failed;
            run.completed_at = Some(run.started_at);
            run.duration_ms = Some(0);
            run.entries = vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error_message.clone(),
            }];
            HookCompletedEvent {
                turn_id: turn_id.clone(),
                run,
            }
        })
        .collect();

    StopOutcome {
        hook_events,
        should_stop: false,
        stop_reason: None,
        should_block: false,
        block_reason: None,
        continuation_prompt: None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookOutputEntry;
    use codex_protocol::protocol::HookOutputEntryKind;
    use codex_protocol::protocol::HookRunStatus;
    use pretty_assertions::assert_eq;

    use super::StopHandlerData;
    use super::aggregate_results;
    use super::parse_completed;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn block_decision_with_reason_sets_continuation_prompt() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"decision":"block","reason":"retry with tests"}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            StopHandlerData {
                should_stop: false,
                stop_reason: None,
                should_block: true,
                block_reason: Some("retry with tests".to_string()),
                continuation_prompt: Some("retry with tests".to_string()),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
    }

    #[test]
    fn block_decision_without_reason_is_invalid() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"decision":"block"}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, StopHandlerData::default());
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "Stop hook returned decision:block without a non-empty reason".to_string(),
            }]
        );
    }

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
                continuation_prompt: None,
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
                continuation_prompt: Some("retry with tests".to_string()),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
    }

    #[test]
    fn exit_code_two_without_stderr_does_not_block() {
        let parsed = parse_completed(&handler(), run_result(Some(2), "", "   "), None);

        assert_eq!(parsed.data, StopHandlerData::default());
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text:
                    "Stop hook exited with code 2 but did not write a continuation prompt to stderr"
                        .to_string(),
            }]
        );
    }

    #[test]
    fn block_decision_with_blank_reason_fails_instead_of_blocking() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "{\"decision\":\"block\",\"reason\":\"   \"}", ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, StopHandlerData::default());
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "Stop hook returned decision:block without a non-empty reason".to_string(),
            }]
        );
    }

    #[test]
    fn invalid_stdout_fails_instead_of_silently_nooping() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "not json", ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, StopHandlerData::default());
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid stop hook JSON output".to_string(),
            }]
        );
    }

    #[test]
    fn aggregate_results_concatenates_blocking_reasons_in_declaration_order() {
        let aggregate = aggregate_results([
            &StopHandlerData {
                should_stop: false,
                stop_reason: None,
                should_block: true,
                block_reason: Some("first".to_string()),
                continuation_prompt: Some("first".to_string()),
            },
            &StopHandlerData {
                should_stop: false,
                stop_reason: None,
                should_block: true,
                block_reason: Some("second".to_string()),
                continuation_prompt: Some("second".to_string()),
            },
        ]);

        assert_eq!(
            aggregate,
            StopHandlerData {
                should_stop: false,
                stop_reason: None,
                should_block: true,
                block_reason: Some("first\n\nsecond".to_string()),
                continuation_prompt: Some("first\n\nsecond".to_string()),
            }
        );
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
