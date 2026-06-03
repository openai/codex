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

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &InterruptRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::Interrupt,
        /*matcher_input*/ None,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: InterruptRequest,
) -> InterruptOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
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
        shell,
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
    run_result: CommandRunResult,
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
mod tests {
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookOutputEntry;
    use codex_protocol::protocol::HookOutputEntryKind;
    use codex_protocol::protocol::HookRunStatus;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::InterruptHandlerData;
    use super::parse_completed;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn empty_stdout_succeeds() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "", ""),
            /*turn_id*/ None,
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
        assert!(parsed.completed.run.entries.is_empty());
    }

    #[test]
    fn system_message_becomes_warning() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"systemMessage":"watch the tide"}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Warning,
                text: "watch the tide".to_string(),
            }]
        );
    }

    #[test]
    fn control_fields_fail_with_invalid_json_output() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"continue":true}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid interrupt hook JSON output".to_string(),
            }]
        );
    }

    #[test]
    fn stop_reason_field_fails_with_invalid_json_output() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"stopReason":null}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid interrupt hook JSON output".to_string(),
            }]
        );
    }

    #[test]
    fn suppress_output_field_fails_with_invalid_json_output() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"suppressOutput":false}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid interrupt hook JSON output".to_string(),
            }]
        );
    }

    #[test]
    fn decision_field_fails_with_invalid_json_output() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"decision":"block"}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid interrupt hook JSON output".to_string(),
            }]
        );
    }

    #[test]
    fn malformed_json_fails_with_invalid_json_output() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"systemMessage":"watch the tide""#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid interrupt hook JSON output".to_string(),
            }]
        );
    }

    #[test]
    fn non_json_stdout_fails() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "aloha", ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "Interrupt hook returned non-JSON stdout".to_string(),
            }]
        );
    }

    #[test]
    fn exit_code_two_is_ordinary_failure() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(2), "", "ignored"),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.data, InterruptHandlerData);
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook exited with code 2".to_string(),
            }]
        );
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::Interrupt,
            matcher: None,
            command: "echo hook".to_string(),
            timeout_sec: 600,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: codex_protocol::protocol::HookSource::User,
            display_order: 0,
            env: std::collections::HashMap::new(),
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
