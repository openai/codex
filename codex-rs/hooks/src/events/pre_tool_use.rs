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
use crate::schema::PreToolUseCommandInput;

#[derive(Debug, Clone)]
pub struct PreToolUseRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub tool_name: String,
    pub tool_use_id: String,
    pub command: String,
}

#[derive(Debug)]
pub struct PreToolUseOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub should_block: bool,
    pub block_reason: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PreToolUseHandlerData {
    should_block: bool,
    block_reason: Option<String>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &PreToolUseRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::PreToolUse,
        Some(&request.tool_name),
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: PreToolUseRequest,
) -> PreToolUseOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::PreToolUse,
        Some(&request.tool_name),
    );
    if matched.is_empty() {
        return PreToolUseOutcome {
            hook_events: Vec::new(),
            should_block: false,
            block_reason: None,
        };
    }

    let input_json = match serde_json::to_string(&PreToolUseCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: crate::schema::NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PreToolUse".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        tool_name: "Bash".to_string(),
        tool_input: crate::schema::PreToolUseToolInput {
            command: request.command.clone(),
        },
        tool_use_id: request.tool_use_id.clone(),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return serialization_failure_outcome(common::serialization_failure_hook_events(
                matched,
                Some(request.turn_id),
                format!("failed to serialize pre tool use hook input: {error}"),
            ));
        }
    };

    let mut results = Vec::new();
    let mut tiers = dispatcher::select_handlers_by_trust_precedence(
        &matched,
        HookEventName::PreToolUse,
        Some(&request.tool_name),
    )
    .into_iter()
    .peekable();
    while let Some(tier) = tiers.next() {
        let tier_results = dispatcher::execute_handlers(
            shell,
            tier,
            input_json.clone(),
            request.cwd.as_path(),
            Some(request.turn_id.clone()),
            parse_completed,
        )
        .await;
        let tier_should_block = tier_results.iter().any(|result| result.data.should_block);
        results.extend(tier_results);
        if tier_should_block {
            let skipped_message =
                "skipped because a higher-precedence PreToolUse hook blocked the command"
                    .to_string();
            for skipped_handler in tiers.flatten() {
                results.push(dispatcher::ParsedHandler {
                    completed: dispatcher::skipped_completed_event(
                        &skipped_handler,
                        Some(request.turn_id.clone()),
                        skipped_message.clone(),
                    ),
                    data: PreToolUseHandlerData::default(),
                });
            }
            break;
        }
    }

    let should_block = results.iter().any(|result| result.data.should_block);
    let block_reason = results
        .iter()
        .find_map(|result| result.data.block_reason.clone());

    PreToolUseOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
        should_block,
        block_reason,
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<PreToolUseHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;
    let mut should_block = false;
    let mut block_reason = None;

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
                } else if let Some(parsed) = output_parser::parse_pre_tool_use(&run_result.stdout) {
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
                    } else if let Some(reason) = parsed.block_reason {
                        status = HookRunStatus::Blocked;
                        should_block = true;
                        block_reason = Some(reason.clone());
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Feedback,
                            text: reason,
                        });
                    }
                } else if trimmed_stdout.starts_with('{') || trimmed_stdout.starts_with('[') {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "hook returned invalid pre-tool-use JSON output".to_string(),
                    });
                }
            }
            Some(2) => {
                if let Some(reason) = common::trimmed_non_empty(&run_result.stderr) {
                    status = HookRunStatus::Blocked;
                    should_block = true;
                    block_reason = Some(reason.clone());
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Feedback,
                        text: reason,
                    });
                } else {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: "PreToolUse hook exited with code 2 but did not write a blocking reason to stderr".to_string(),
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
        data: PreToolUseHandlerData {
            should_block,
            block_reason,
        },
    }
}

fn serialization_failure_outcome(hook_events: Vec<HookCompletedEvent>) -> PreToolUseOutcome {
    PreToolUseOutcome {
        hook_events,
        should_block: false,
        block_reason: None,
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

    use super::PreToolUseHandlerData;
    use super::PreToolUseRequest;
    use super::parse_completed;
    use crate::engine::CommandShell;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn permission_decision_deny_blocks_processing() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"do not run that"}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PreToolUseHandlerData {
                should_block: true,
                block_reason: Some("do not run that".to_string()),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Feedback,
                text: "do not run that".to_string(),
            }]
        );
    }

    #[test]
    fn deprecated_block_decision_blocks_processing() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"decision":"block","reason":"do not run that"}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PreToolUseHandlerData {
                should_block: true,
                block_reason: Some("do not run that".to_string()),
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Blocked);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Feedback,
                text: "do not run that".to_string(),
            }]
        );
    }

    #[test]
    fn unsupported_permission_decision_fails_open() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"please confirm"}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PreToolUseHandlerData {
                should_block: false,
                block_reason: None,
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "PreToolUse hook returned unsupported permissionDecision:ask".to_string(),
            }]
        );
    }

    #[test]
    fn deprecated_approve_decision_fails_open() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), r#"{"decision":"approve"}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PreToolUseHandlerData {
                should_block: false,
                block_reason: None,
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "PreToolUse hook returned unsupported decision:approve".to_string(),
            }]
        );
    }

    #[test]
    fn unsupported_additional_context_fails_open() {
        let parsed = parse_completed(
            &handler(),
            run_result(
                Some(0),
                r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"do not run that","additionalContext":"nope"}}"#,
                "",
            ),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PreToolUseHandlerData {
                should_block: false,
                block_reason: None,
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "PreToolUse hook returned unsupported additionalContext".to_string(),
            }]
        );
    }

    #[test]
    fn plain_stdout_is_ignored() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "hook ran successfully\n", ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PreToolUseHandlerData {
                should_block: false,
                block_reason: None,
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
        assert_eq!(parsed.completed.run.entries, vec![]);
    }

    #[test]
    fn invalid_json_like_stdout_fails_instead_of_becoming_noop() {
        let parsed = parse_completed(
            &handler(),
            run_result(Some(0), "{\"decision\":\n", ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(
            parsed.data,
            PreToolUseHandlerData {
                should_block: false,
                block_reason: None,
            }
        );
        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "hook returned invalid pre-tool-use JSON output".to_string(),
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
            PreToolUseHandlerData {
                should_block: true,
                block_reason: Some("blocked by policy".to_string()),
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

    #[tokio::test]
    async fn higher_precedence_block_skips_lower_precedence_handlers() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let marker_path = temp.path().join("project-ran");
        let (shell_program, shell_args, blocking_command, project_command) = if cfg!(windows) {
            (
                "cmd".to_string(),
                vec!["/C".to_string()],
                "echo blocked by policy 1>&2 && exit /b 2".to_string(),
                "type nul > project-ran".to_string(),
            )
        } else {
            (
                "/bin/sh".to_string(),
                vec!["-c".to_string()],
                "printf 'blocked by policy' >&2; exit 2".to_string(),
                "touch project-ran".to_string(),
            )
        };
        let handlers = vec![
            ConfiguredHandler {
                event_name: HookEventName::PreToolUse,
                matcher: Some("^Bash$".to_string()),
                command: blocking_command,
                timeout_sec: 5,
                status_message: None,
                source_path: PathBuf::from("/tmp/home/.codex/hooks.json"),
                is_project: false,
                display_order: 0,
            },
            ConfiguredHandler {
                event_name: HookEventName::PreToolUse,
                matcher: Some("^Bash$".to_string()),
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
            PreToolUseRequest {
                session_id: ThreadId::new(),
                turn_id: "turn-1".to_string(),
                cwd: temp.path().to_path_buf(),
                transcript_path: None,
                model: "gpt-5".to_string(),
                permission_mode: "default".to_string(),
                tool_name: "Bash".to_string(),
                tool_use_id: "tool-1".to_string(),
                command: "echo hello".to_string(),
            },
        )
        .await;

        assert!(outcome.should_block);
        assert_eq!(outcome.block_reason, Some("blocked by policy".to_string()));
        assert!(!marker_path.exists());
        assert_eq!(outcome.hook_events.len(), 2);
        assert_eq!(outcome.hook_events[0].run.status, HookRunStatus::Blocked);
        assert_eq!(outcome.hook_events[1].run.status, HookRunStatus::Failed);
        assert_eq!(
            outcome.hook_events[1].run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "skipped because a higher-precedence PreToolUse hook blocked the command"
                    .to_string(),
            }]
        );

        Ok(())
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::PreToolUse,
            matcher: Some("^Bash$".to_string()),
            command: "echo hook".to_string(),
            timeout_sec: 5,
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
