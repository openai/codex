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
use crate::schema::SessionStartCommandInput;

#[derive(Debug, Clone, Copy)]
pub enum SessionStartSource {
    Startup,
    Resume,
}

impl SessionStartSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Resume => "resume",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionStartRequest {
    pub session_id: ThreadId,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub source: SessionStartSource,
}

#[derive(Debug)]
pub struct SessionStartOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub should_stop: bool,
    pub stop_reason: Option<String>,
    pub additional_contexts: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct SessionStartHandlerData {
    should_stop: bool,
    stop_reason: Option<String>,
    additional_contexts_for_model: Vec<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &SessionStartRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::SessionStart,
        Some(request.source.as_str()),
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: SessionStartRequest,
    turn_id: Option<String>,
) -> SessionStartOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::SessionStart,
        Some(request.source.as_str()),
    );
    if matched.is_empty() {
        return SessionStartOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            additional_contexts: Vec::new(),
        };
    }

    let input_json = match serde_json::to_string(&SessionStartCommandInput::new(
        request.session_id.to_string(),
        request.transcript_path.clone(),
        request.cwd.display().to_string(),
        request.model.clone(),
        request.permission_mode.clone(),
        request.source.as_str().to_string(),
    )) {
        Ok(input_json) => input_json,
        Err(error) => {
            return serialization_failure_outcome(common::serialization_failure_hook_events(
                matched,
                turn_id,
                format!("failed to serialize session start hook input: {error}"),
            ));
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        turn_id,
        parse_completed,
    )
    .await;

    let should_stop = results.iter().any(|result| result.data.should_stop);
    let stop_reason = results
        .iter()
        .find_map(|result| result.data.stop_reason.clone());
    let additional_contexts = common::flatten_additional_contexts(
        results
            .iter()
            .map(|result| result.data.additional_contexts_for_model.as_slice()),
    );

    SessionStartOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
        should_stop,
        stop_reason,
        additional_contexts,
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<SessionStartHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut should_stop = false;
    let mut stop_reason = None;
    let mut additional_contexts_for_model = Vec::new();

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
                } else if let Some(parsed) = output_parser::parse_session_start(&run_result.stdout)
                {
                    if let Some(system_message) = parsed.universal.system_message {
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Warning,
                            text: system_message,
                        });
                    }
                    if let Some(additional_context) = parsed.additional_context {
                        common::append_additional_context(
                            &mut entries,
                            &mut additional_contexts_for_model,
                            additional_context,
                        );
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
                    }
                // Preserve plain-text context support without treating malformed JSON as context.
                } else if trimmed_stdout.starts_with('{') || trimmed_stdout.starts_with('[') {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid session start JSON output".to_string(),
                    });
                } else {
                    let additional_context = trimmed_stdout.to_string();
                    common::append_additional_context(
                        &mut entries,
                        &mut additional_contexts_for_model,
                        additional_context,
                    );
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
        data: SessionStartHandlerData {
            should_stop,
            stop_reason,
            additional_contexts_for_model,
        },
    }
}

fn serialization_failure_outcome(hook_events: Vec<HookCompletedEvent>) -> SessionStartOutcome {
    SessionStartOutcome {
        hook_events,
        should_stop: false,
        stop_reason: None,
        additional_contexts: Vec::new(),
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

    use super::SessionStartHandlerData;
    use super::parse_completed;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn plain_stdout_becomes_model_context() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "hello from hook\n", ""),
            None,
        );

        assert_eq!(
            parsed.data,
            SessionStartHandlerData {
                should_stop: false,
                stop_reason: None,
                additional_contexts_for_model: vec!["hello from hook".to_string()],
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Context,
                text: "hello from hook".to_string(),
            }]
        );
    }

    #[test]
    fn continue_false_preserves_context_for_later_turns() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"continue":false,"stopReason":"pause","hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"do not inject"}}"#,
                "",
            ),
            None,
        );

        assert_eq!(
            parsed.data,
            SessionStartHandlerData {
                should_stop: true,
                stop_reason: Some("pause".to_string()),
                additional_contexts_for_model: vec!["do not inject".to_string()],
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Stopped);
        assert_eq!(
            parsed.completed.run.entries,
            vec![
                HookOutputEntry {
                    kind: HookOutputEntryKind::Context,
                    text: "do not inject".to_string(),
                },
                HookOutputEntry {
                    kind: HookOutputEntryKind::Stop,
                    text: "pause".to_string(),
                },
            ]
        );
    }

    #[test]
    fn invalid_json_like_stdout_fails_instead_of_becoming_model_context() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"hookSpecificOutput":{"hookEventName":"SessionStart""#,
                "",
            ),
            None,
        );

        assert_eq!(
            parsed.data,
            SessionStartHandlerData {
                should_stop: false,
                stop_reason: None,
                additional_contexts_for_model: Vec::new(),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid session start JSON output".to_string(),
            }]
        );
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::SessionStart,
            matcher: None,
            command: "echo hook".to_string(),
            timeout_sec: 600,
            status_message: None,
            source_path: PathBuf::from("/tmp/hooks.json"),
            is_project: false,
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
