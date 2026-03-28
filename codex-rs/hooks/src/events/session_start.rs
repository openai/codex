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

    let mut results = Vec::new();
    let mut tiers = dispatcher::select_handlers_by_trust_precedence(
        &matched,
        HookEventName::SessionStart,
        Some(request.source.as_str()),
    )
    .into_iter()
    .peekable();
    while let Some(tier) = tiers.next() {
        let tier_results = dispatcher::execute_handlers(
            shell,
            tier,
            input_json.clone(),
            request.cwd.as_path(),
            turn_id.clone(),
            parse_completed,
        )
        .await;
        let tier_should_stop = tier_results.iter().any(|result| result.data.should_stop);
        results.extend(tier_results);
        if tier_should_stop {
            let skipped_message =
                "skipped because a higher-precedence SessionStart hook stopped processing"
                    .to_string();
            for skipped_handler in tiers.flatten() {
                results.push(dispatcher::ParsedHandler {
                    completed: dispatcher::skipped_completed_event(
                        &skipped_handler,
                        turn_id.clone(),
                        skipped_message.clone(),
                    ),
                    data: SessionStartHandlerData {
                        should_stop: false,
                        stop_reason: None,
                        additional_contexts_for_model: Vec::new(),
                    },
                });
            }
            break;
        }
    }

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

    use codex_protocol::ThreadId;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookOutputEntry;
    use codex_protocol::protocol::HookOutputEntryKind;
    use codex_protocol::protocol::HookRunStatus;
    use pretty_assertions::assert_eq;

    use super::SessionStartHandlerData;
    use super::SessionStartRequest;
    use super::SessionStartSource;
    use super::parse_completed;
    use crate::engine::CommandShell;
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

    #[tokio::test]
    async fn higher_precedence_stop_skips_lower_precedence_handlers() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let marker_path = temp.path().join("project-ran");
        let (shell_program, shell_args, stopping_command, project_command) = if cfg!(windows) {
            (
                "powershell.exe".to_string(),
                vec!["-NoProfile".to_string(), "-Command".to_string()],
                "Write-Output '{\"continue\":false,\"stopReason\":\"pause\",\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"trusted context\"}}'".to_string(),
                "$null = New-Item -ItemType File -Path project-ran -Force; Write-Output 'project context'".to_string(),
            )
        } else {
            (
                "/bin/sh".to_string(),
                vec!["-c".to_string()],
                "printf '%s' '{\"continue\":false,\"stopReason\":\"pause\",\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"trusted context\"}}'".to_string(),
                "touch project-ran && printf 'project context'".to_string(),
            )
        };
        let handlers = vec![
            ConfiguredHandler {
                event_name: HookEventName::SessionStart,
                matcher: Some("^startup$".to_string()),
                command: stopping_command,
                timeout_sec: 5,
                status_message: None,
                source_path: PathBuf::from("/tmp/home/.codex/hooks.json"),
                is_project: false,
                display_order: 0,
            },
            ConfiguredHandler {
                event_name: HookEventName::SessionStart,
                matcher: Some("^startup$".to_string()),
                command: project_command,
                timeout_sec: 5,
                status_message: None,
                source_path: PathBuf::from("/tmp/project/.codex/hooks.json"),
                is_project: true,
                display_order: 1,
            },
        ];

        let outcome = super::run(
            &handlers,
            &CommandShell {
                program: shell_program,
                args: shell_args,
            },
            SessionStartRequest {
                session_id: ThreadId::new(),
                cwd: temp.path().to_path_buf(),
                transcript_path: None,
                model: "gpt-5".to_string(),
                permission_mode: "default".to_string(),
                source: SessionStartSource::Startup,
            },
            Some("turn-1".to_string()),
        )
        .await;

        assert!(outcome.should_stop);
        assert_eq!(outcome.stop_reason, Some("pause".to_string()));
        assert_eq!(
            outcome.additional_contexts,
            vec!["trusted context".to_string()]
        );
        assert!(!marker_path.exists());
        assert_eq!(outcome.hook_events.len(), 2);
        assert_eq!(outcome.hook_events[0].run.status, HookRunStatus::Stopped);
        assert_eq!(outcome.hook_events[1].run.status, HookRunStatus::Failed);
        assert_eq!(
            outcome.hook_events[1].run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "skipped because a higher-precedence SessionStart hook stopped processing"
                    .to_string(),
            }]
        );

        Ok(())
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
