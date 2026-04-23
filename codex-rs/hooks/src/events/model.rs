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
use crate::events::compact::StatelessHookOutcome;
use crate::schema::PostModelResponseCommandInput;
use crate::schema::PreModelRequestCommandInput;

#[derive(Debug, Clone)]
pub struct PreModelRequestRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub input: Value,
    pub tools: Value,
    pub parallel_tool_calls: bool,
}

#[derive(Debug, Clone)]
pub struct PostModelResponseRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub status: String,
    pub error: Option<String>,
    pub output: Value,
    pub needs_follow_up: Option<bool>,
    pub last_assistant_message: Option<String>,
}

pub(crate) fn preview_pre(
    handlers: &[ConfiguredHandler],
    _request: &PreModelRequestRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::PreModelRequest,
        /*matcher_input*/ None,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run_pre(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: PreModelRequestRequest,
) -> StatelessHookOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::PreModelRequest,
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return StatelessHookOutcome {
            hook_events: Vec::new(),
        };
    }

    let input_json = match pre_command_input_json(&request) {
        Ok(input_json) => input_json,
        Err(error) => {
            return StatelessHookOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize pre model request hook input: {error}"),
                ),
            };
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id),
        parse_pre_completed,
    )
    .await;
    StatelessHookOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
    }
}

fn pre_command_input_json(request: &PreModelRequestRequest) -> Result<String, serde_json::Error> {
    serde_json::to_string(&PreModelRequestCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: crate::schema::NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PreModelRequest".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        input: request.input.clone(),
        tools: request.tools.clone(),
        parallel_tool_calls: request.parallel_tool_calls,
    })
}

pub(crate) fn preview_post(
    handlers: &[ConfiguredHandler],
    _request: &PostModelResponseRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::PostModelResponse,
        /*matcher_input*/ None,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run_post(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: PostModelResponseRequest,
) -> StatelessHookOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::PostModelResponse,
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return StatelessHookOutcome {
            hook_events: Vec::new(),
        };
    }

    let input_json = match post_command_input_json(&request) {
        Ok(input_json) => input_json,
        Err(error) => {
            return StatelessHookOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize post model response hook input: {error}"),
                ),
            };
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id),
        parse_post_completed,
    )
    .await;
    StatelessHookOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
    }
}

fn post_command_input_json(
    request: &PostModelResponseRequest,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&PostModelResponseCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: crate::schema::NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PostModelResponse".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        status: request.status.clone(),
        error: crate::schema::NullableString::from_string(request.error.clone()),
        output: request.output.clone(),
        needs_follow_up: request.needs_follow_up,
        last_assistant_message: crate::schema::NullableString::from_string(
            request.last_assistant_message.clone(),
        ),
    })
}

#[derive(Default)]
struct ModelHandlerData;

fn parse_pre_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<ModelHandlerData> {
    parse_completed(
        handler,
        run_result,
        turn_id,
        "PreModelRequest",
        output_parser::parse_pre_model_request,
    )
}

fn parse_post_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<ModelHandlerData> {
    parse_completed(
        handler,
        run_result,
        turn_id,
        "PostModelResponse",
        output_parser::parse_post_model_response,
    )
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
    event_label: &'static str,
    parse_output: fn(&str) -> Option<output_parser::StatelessHookOutput>,
) -> dispatcher::ParsedHandler<ModelHandlerData> {
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
                } else if let Some(parsed) = parse_output(&run_result.stdout) {
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
                    }
                } else {
                    status = HookRunStatus::Failed;
                    entries.push(HookOutputEntry {
                        kind: HookOutputEntryKind::Error,
                        text: format!("hook returned invalid {event_label} hook JSON output"),
                    });
                }
            }
            Some(code) => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: common::trimmed_non_empty(&run_result.stderr)
                        .unwrap_or_else(|| format!("hook exited with code {code}")),
                });
            }
            None => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: "hook process terminated without an exit code".to_string(),
                });
            }
        },
    }

    dispatcher::ParsedHandler {
        completed: HookCompletedEvent {
            turn_id,
            run: dispatcher::completed_summary(handler, &run_result, status, entries),
        },
        data: ModelHandlerData,
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

    use super::parse_post_completed;
    use super::post_command_input_json;
    use super::pre_command_input_json;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn pre_model_request_input_includes_model_payload() {
        let input_json = pre_command_input_json(&pre_request()).expect("serialize command input");
        let input: serde_json::Value =
            serde_json::from_str(&input_json).expect("parse command input");

        assert_eq!(
            input,
            json!({
                "session_id": pre_request().session_id.to_string(),
                "turn_id": "turn-1",
                "transcript_path": null,
                "cwd": test_path_buf("/tmp").display().to_string(),
                "hook_event_name": "PreModelRequest",
                "model": "gpt-test",
                "permission_mode": "default",
                "input": [{"type": "message", "content": "hello"}],
                "tools": [{"name": "shell", "type": "function"}],
                "parallel_tool_calls": true,
            })
        );
    }

    #[test]
    fn post_model_response_input_includes_response_metadata() {
        let input_json = post_command_input_json(&post_request()).expect("serialize command input");
        let input: serde_json::Value =
            serde_json::from_str(&input_json).expect("parse command input");

        assert_eq!(
            input,
            json!({
                "session_id": post_request().session_id.to_string(),
                "turn_id": "turn-1",
                "transcript_path": null,
                "cwd": test_path_buf("/tmp").display().to_string(),
                "hook_event_name": "PostModelResponse",
                "model": "gpt-test",
                "permission_mode": "default",
                "status": "completed",
                "error": null,
                "output": [{"type": "message", "content": "done"}],
                "needs_follow_up": false,
                "last_assistant_message": "done",
            })
        );
    }

    #[test]
    fn stateless_output_cannot_stop_model_response() {
        let parsed = parse_post_completed(
            &handler(HookEventName::PostModelResponse),
            run_result(Some(0), r#"{"continue":false,"stopReason":"nope"}"#, ""),
            Some("turn-1".to_string()),
        );

        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: "PostModelResponse hook returned unsupported continue:false".to_string(),
            }]
        );
    }

    fn pre_request() -> super::PreModelRequestRequest {
        super::PreModelRequestRequest {
            session_id: ThreadId::from_string("00000000-0000-4000-8000-000000000001")
                .expect("valid thread id"),
            turn_id: "turn-1".to_string(),
            cwd: test_path_buf("/tmp").abs(),
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            input: json!([{"type": "message", "content": "hello"}]),
            tools: json!([{"name": "shell", "type": "function"}]),
            parallel_tool_calls: true,
        }
    }

    fn post_request() -> super::PostModelResponseRequest {
        super::PostModelResponseRequest {
            session_id: ThreadId::from_string("00000000-0000-4000-8000-000000000002")
                .expect("valid thread id"),
            turn_id: "turn-1".to_string(),
            cwd: test_path_buf("/tmp").abs(),
            transcript_path: None,
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            status: "completed".to_string(),
            error: None,
            output: json!([{"type": "message", "content": "done"}]),
            needs_follow_up: Some(false),
            last_assistant_message: Some("done".to_string()),
        }
    }

    fn handler(event_name: HookEventName) -> ConfiguredHandler {
        ConfiguredHandler {
            event_name,
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

    fn run_result(
        exit_code: Option<i32>,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> CommandRunResult {
        CommandRunResult {
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_code,
            error: None,
            started_at: 10,
            completed_at: 11,
            duration_ms: 1,
        }
    }
}
