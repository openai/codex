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
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::NullableString;
use crate::schema::UserPromptSubmitCommandInput;

#[derive(Debug, Clone)]
pub struct UserPromptSubmitRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub prompt: String,
}

#[derive(Debug)]
pub struct UserPromptSubmitOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub should_stop: bool,
    pub stop_reason: Option<String>,
    pub additional_contexts: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct UserPromptSubmitHandlerData {
    should_stop: bool,
    stop_reason: Option<String>,
    additional_contexts_for_model: Vec<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &UserPromptSubmitRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::UserPromptSubmit,
        /*matcher_input*/ None,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: UserPromptSubmitRequest,
) -> UserPromptSubmitOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::UserPromptSubmit,
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return UserPromptSubmitOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            additional_contexts: Vec::new(),
        };
    }

    let input_json = match serde_json::to_string(&UserPromptSubmitCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "UserPromptSubmit".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        prompt: request.prompt.clone(),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return serialization_failure_outcome(common::serialization_failure_hook_events(
                matched,
                Some(request.turn_id),
                format!("failed to serialize user prompt submit hook input: {error}"),
            ));
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

    let should_stop = results.iter().any(|result| result.data.should_stop);
    let stop_reason = results
        .iter()
        .find_map(|result| result.data.stop_reason.clone());
    let additional_contexts = common::flatten_additional_contexts(
        results
            .iter()
            .map(|result| result.data.additional_contexts_for_model.as_slice()),
    );

    UserPromptSubmitOutcome {
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
) -> dispatcher::ParsedHandler<UserPromptSubmitHandlerData> {
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
                } else if let Some(parsed) =
                    output_parser::parse_user_prompt_submit(&run_result.stdout)
                {
                    if let Some(system_message) = parsed.universal.system_message {
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Warning,
                            text: system_message,
                        });
                    }
                    if parsed.invalid_block_reason.is_none()
                        && let Some(additional_context) = parsed.additional_context
                    {
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
                    } else if let Some(invalid_block_reason) = parsed.invalid_block_reason {
                        status = HookRunStatus::Failed;
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Error,
                            text: invalid_block_reason,
                        });
                    } else if parsed.should_block {
                        status = HookRunStatus::Blocked;
                        should_stop = true;
                        stop_reason = parsed.reason.clone();
                        if let Some(reason) = parsed.reason {
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Feedback,
                                text: reason,
                            });
                        }
                    }
                } else if trimmed_stdout.starts_with('{') || trimmed_stdout.starts_with('[') {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid user prompt submit JSON output".to_string(),
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
            Some(2) => {
                if let Some(reason) = common::trimmed_non_empty(&run_result.stderr) {
                    status = HookRunStatus::Blocked;
                    should_stop = true;
                    stop_reason = Some(reason.clone());
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Feedback,
                        text: reason,
                    });
                } else {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "UserPromptSubmit hook exited with code 2 but did not write a blocking reason to stderr".to_string(),
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
        data: UserPromptSubmitHandlerData {
            should_stop,
            stop_reason,
            additional_contexts_for_model,
        },
    }
}

fn serialization_failure_outcome(hook_events: Vec<HookCompletedEvent>) -> UserPromptSubmitOutcome {
    UserPromptSubmitOutcome {
        hook_events,
        should_stop: false,
        stop_reason: None,
        additional_contexts: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookOutputEntry;
    use codex_protocol::protocol::HookOutputEntryKind;
    use codex_protocol::protocol::HookRunStatus;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::UserPromptSubmitHandlerData;
    use super::parse_completed;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn continue_false_preserves_context_for_later_turns() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"continue":false,"stopReason":"pause","hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"do not inject"}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            UserPromptSubmitHandlerData {
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
    fn claude_block_decision_blocks_processing() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"decision":"block","reason":"slow down","hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"do not inject"}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            UserPromptSubmitHandlerData {
                should_stop: true,
                stop_reason: Some("slow down".to_string()),
                additional_contexts_for_model: vec!["do not inject".to_string()],
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
        assert_eq!(
            parsed.completed.run.entries,
            vec![
                HookOutputEntry {
                    kind: HookOutputEntryKind::Context,
                    text: "do not inject".to_string(),
                },
                HookOutputEntry {
                    kind: HookOutputEntryKind::Feedback,
                    text: "slow down".to_string(),
                },
            ]
        );
    }

    #[test]
    fn claude_block_decision_requires_reason() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"decision":"block","hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"do not inject"}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            UserPromptSubmitHandlerData {
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
                text: "UserPromptSubmit hook returned decision:block without a non-empty reason"
                    .to_string(),
            }]
        );
    }

    #[test]
    fn exit_code_two_blocks_processing() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(2), "", "blocked by policy\n"),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            UserPromptSubmitHandlerData {
                should_stop: true,
                stop_reason: Some("blocked by policy".to_string()),
                additional_contexts_for_model: Vec::new(),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Feedback,
                text: "blocked by policy".to_string(),
            }]
        );
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::UserPromptSubmit,
            is_managed: false,
            matcher: None,
            command: "echo hook".to_string(),
            timeout_sec: 5,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: codex_protocol::protocol::HookSource::User,
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
