use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::PostToolUseCommandInput;
use crate::schema::PostToolUseToolInput;

#[derive(Debug, Clone)]
pub struct PostToolUseRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub tool_name: String,
    pub tool_use_id: String,
    pub command: String,
    pub tool_response: Value,
}

#[derive(Debug)]
pub struct PostToolUseOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub should_stop: bool,
    pub stop_reason: Option<String>,
    pub additional_contexts: Vec<String>,
    pub feedback_message: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PostToolUseHandlerData {
    should_stop: bool,
    stop_reason: Option<String>,
    additional_contexts_for_model: Vec<String>,
    feedback_messages_for_model: Vec<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &PostToolUseRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::PostToolUse,
        Some(&request.tool_name),
    )
    .into_iter()
    .map(|handler| {
        common::hook_run_for_tool_use(dispatcher::running_summary(&handler), &request.tool_use_id)
    })
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: PostToolUseRequest,
) -> PostToolUseOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::PostToolUse,
        Some(&request.tool_name),
    );
    if matched.is_empty() {
        return PostToolUseOutcome {
            hook_events: Vec::new(),
            should_stop: false,
            stop_reason: None,
            additional_contexts: Vec::new(),
            feedback_message: None,
        };
    }

    let input_json = match serde_json::to_string(&PostToolUseCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: crate::schema::NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PostToolUse".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        tool_name: "Bash".to_string(),
        tool_input: PostToolUseToolInput {
            command: request.command.clone(),
        },
        tool_response: request.tool_response.clone(),
        tool_use_id: request.tool_use_id.clone(),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            let hook_events = common::serialization_failure_hook_events_for_tool_use(
                matched,
                Some(request.turn_id.clone()),
                format!("failed to serialize post tool use hook input: {error}"),
                &request.tool_use_id,
            );
            return serialization_failure_outcome(hook_events);
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

    let additional_contexts = common::flatten_additional_contexts(
        results
            .iter()
            .map(|result| result.data.additional_contexts_for_model.as_slice()),
    );
    let should_stop = results.iter().any(|result| result.data.should_stop);
    let stop_reason = results
        .iter()
        .find_map(|result| result.data.stop_reason.clone());
    let feedback_message = common::join_text_chunks(
        results
            .iter()
            .flat_map(|result| result.data.feedback_messages_for_model.clone())
            .collect(),
    );

    PostToolUseOutcome {
        hook_events: results
            .into_iter()
            .map(|result| {
                common::hook_completed_for_tool_use(result.completed, &request.tool_use_id)
            })
            .collect(),
        should_stop,
        stop_reason,
        additional_contexts,
        feedback_message,
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<PostToolUseHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut should_stop = false;
    let mut stop_reason = None;
    let mut additional_contexts_for_model = Vec::new();
    let mut feedback_messages_for_model = Vec::new();

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
                } else if let Some(parsed) = output_parser::parse_post_tool_use(&run_result.stdout)
                {
                    if let Some(system_message) = parsed.universal.system_message {
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Warning,
                            text: system_message,
                        });
                    }
                    if parsed.invalid_reason.is_none()
                        && parsed.invalid_block_reason.is_none()
                        && let Some(additional_context) = parsed.additional_context
                    {
                        common::append_additional_context(
                            &mut entries,
                            &mut additional_contexts_for_model,
                            additional_context,
                        );
                    }
                    if !parsed.universal.continue_processing {
                        status = HookRunStatus::Stopped;
                        should_stop = true;
                        stop_reason = parsed.universal.stop_reason.clone();
                        let stop_text = parsed
                            .universal
                            .stop_reason
                            .unwrap_or_else(|| "PostToolUse hook stopped execution".to_string());
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Stop,
                            text: stop_text.clone(),
                        });
                        let model_feedback = parsed
                            .reason
                            .as_deref()
                            .and_then(common::trimmed_non_empty)
                            .unwrap_or(stop_text);
                        feedback_messages_for_model.push(model_feedback);
                    } else if let Some(invalid_reason) = parsed.invalid_reason {
                        status = HookRunStatus::Failed;
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Error,
                            text: invalid_reason,
                        });
                    } else if let Some(invalid_block_reason) = parsed.invalid_block_reason {
                        status = HookRunStatus::Failed;
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Error,
                            text: invalid_block_reason,
                        });
                    } else if parsed.should_block {
                        status = HookRunStatus::Blocked;
                        if let Some(reason) = parsed.reason {
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Feedback,
                                text: reason.clone(),
                            });
                            feedback_messages_for_model.push(reason);
                        }
                    }
                } else if trimmed_stdout.starts_with('{') || trimmed_stdout.starts_with('[') {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid post-tool-use JSON output".to_string(),
                    });
                }
            }
            Some(2) => {
                if let Some(reason) = common::trimmed_non_empty(&run_result.stderr) {
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Feedback,
                        text: reason.clone(),
                    });
                    feedback_messages_for_model.push(reason);
                } else {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "PostToolUse hook exited with code 2 but did not write feedback to stderr".to_string(),
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
        data: PostToolUseHandlerData {
            should_stop,
            stop_reason,
            additional_contexts_for_model,
            feedback_messages_for_model,
        },
    }
}

fn serialization_failure_outcome(hook_events: Vec<HookCompletedEvent>) -> PostToolUseOutcome {
    PostToolUseOutcome {
        hook_events,
        should_stop: false,
        stop_reason: None,
        additional_contexts: Vec::new(),
        feedback_message: None,
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookOutputEntry;
    use codex_protocol::protocol::HookOutputEntryKind;
    use codex_protocol::protocol::HookRunStatus;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::PostToolUseHandlerData;
    use super::parse_completed;
    use super::preview;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;
    use crate::events::common;

    #[test]
    fn block_decision_stops_normal_processing() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"decision":"block","reason":"bash output looked sketchy"}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PostToolUseHandlerData {
                should_stop: false,
                stop_reason: None,
                additional_contexts_for_model: Vec::new(),
                feedback_messages_for_model: vec!["bash output looked sketchy".to_string()],
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
    }

    #[test]
    fn additional_context_is_recorded() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"Remember the bash cleanup note."}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PostToolUseHandlerData {
                should_stop: false,
                stop_reason: None,
                additional_contexts_for_model: vec!["Remember the bash cleanup note.".to_string()],
                feedback_messages_for_model: Vec::new(),
            }
        );
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Context,
                text: "Remember the bash cleanup note.".to_string(),
            }]
        );
    }

    #[test]
    fn unsupported_updated_mcp_tool_output_fails_open() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"hookSpecificOutput":{"hookEventName":"PostToolUse","updatedMCPToolOutput":{"ok":true}}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PostToolUseHandlerData {
                should_stop: false,
                stop_reason: None,
                additional_contexts_for_model: Vec::new(),
                feedback_messages_for_model: Vec::new(),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "PostToolUse hook returned unsupported updatedMCPToolOutput".to_string(),
            }]
        );
    }

    #[test]
    fn exit_two_surfaces_feedback_to_model_without_blocking() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(2), "", "post hook says pause"),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PostToolUseHandlerData {
                should_stop: false,
                stop_reason: None,
                additional_contexts_for_model: Vec::new(),
                feedback_messages_for_model: vec!["post hook says pause".to_string()],
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
    }

    #[test]
    fn continue_false_stops_with_reason() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"continue":false,"stopReason":"halt after bash output","reason":"post-tool hook says stop"}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PostToolUseHandlerData {
                should_stop: true,
                stop_reason: Some("halt after bash output".to_string()),
                additional_contexts_for_model: Vec::new(),
                feedback_messages_for_model: vec!["post-tool hook says stop".to_string()],
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Stopped);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Stop,
                text: "halt after bash output".to_string(),
            }]
        );
    }

    #[test]
    fn plain_stdout_is_ignored_for_post_tool_use() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "plain text only", ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PostToolUseHandlerData {
                should_stop: false,
                stop_reason: None,
                additional_contexts_for_model: Vec::new(),
                feedback_messages_for_model: Vec::new(),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
        assert_eq!(parsed.completed.run.entries, Vec::<HookOutputEntry>::new());
    }

    #[test]
    fn preview_and_completed_run_ids_include_tool_use_id() {
        let request = request_for_tool_use("tool-call-456");
        let runs = preview(&[handler()], &request);

        assert_eq!(runs.len(), 1);
        assert_eq!(
            runs[0].id,
            format!(
                "post-tool-use:0:{}:tool-call-456",
                test_path_buf("/tmp/hooks.json").display()
            )
        );

        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "", ""),
            Some("turn-1".to_string()),
        );
        let completed = common::hook_completed_for_tool_use(parsed.completed, &request.tool_use_id);

        assert_eq!(completed.run.id, runs[0].id);
    }

    #[test]
    fn serialization_failure_run_ids_include_tool_use_id() {
        let request = request_for_tool_use("tool-call-456");
        let runs = preview(&[handler()], &request);

        let completed = common::serialization_failure_hook_events_for_tool_use(
            vec![handler()],
            Some(request.turn_id.clone()),
            "serialize failed".into(),
            &request.tool_use_id,
        );

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].run.id, runs[0].id);
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::PostToolUse,
            is_managed: false,
            matcher: Some("^Bash$".to_string()),
            command: "python3 post_tool_use_hook.py".to_string(),
            timeout_sec: 5,
            status_message: Some("running post tool use hook".to_string()),
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: codex_protocol::protocol::HookSource::User,
            display_order: 0,
        }
    }

    fn run_result(exit_code: Option<i32>, stdout: &str, stderr: &str) -> CommandRunResult {
        CommandRunResult {
            started_at: 1_700_000_000,
            completed_at: 1_700_000_001,
            duration_ms: 12,
            exit_code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            error: None,
        }
    }

    fn request_for_tool_use(tool_use_id: &str) -> super::PostToolUseRequest {
        super::PostToolUseRequest {
            session_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            cwd: test_path_buf("/tmp").abs(),
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            tool_name: "Bash".to_string(),
            tool_use_id: tool_use_id.to_string(),
            command: "echo hello".to_string(),
            tool_response: json!({"ok": true}),
        }
    }
}
