use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use futures::future::BoxFuture;
use serde::Deserialize;
use serde_json::json;
use tokio::time::timeout;

use super::ConfiguredHandler;
use super::ConfiguredHandlerKind;
use super::command_runner::CommandRunResult;
use codex_protocol::protocol::HookEventName;

const PROMPT_ARGUMENTS_PLACEHOLDER: &str = "$ARGUMENTS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptHookRequest {
    pub event_name: HookEventName,
    pub prompt: String,
    pub model: String,
}

#[derive(Clone)]
pub struct PromptHookRunner {
    run: Arc<dyn Fn(PromptHookRequest) -> BoxFuture<'static, anyhow::Result<String>> + Send + Sync>,
}

impl PromptHookRunner {
    pub fn new<F, Fut>(run: F) -> Self
    where
        F: Fn(PromptHookRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<String>> + Send + 'static,
    {
        Self {
            run: Arc::new(move |request| Box::pin(run(request))),
        }
    }

    async fn run(&self, request: PromptHookRequest) -> anyhow::Result<String> {
        (self.run)(request).await
    }
}

#[derive(Deserialize)]
struct PromptHookOutput {
    ok: bool,
    #[serde(default)]
    reason: Option<String>,
}

pub(crate) async fn run_prompt(
    runner: Option<&PromptHookRunner>,
    handler: &ConfiguredHandler,
    input_json: &str,
    default_model: String,
) -> CommandRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = Instant::now();

    let ConfiguredHandlerKind::Prompt {
        prompt,
        model,
        timeout_sec,
        continue_on_block,
    } = &handler.kind
    else {
        return prompt_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some("command handler cannot run as a prompt hook".to_string()),
        );
    };
    let Some(runner) = runner else {
        return prompt_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some("prompt hook cannot run because no prompt runner is configured".to_string()),
        );
    };

    let request = PromptHookRequest {
        event_name: handler.event_name,
        prompt: render_prompt(prompt, input_json),
        model: model.clone().unwrap_or(default_model),
    };

    let run = timeout(Duration::from_secs(*timeout_sec), runner.run(request)).await;
    match run {
        Ok(Ok(output)) => {
            match prompt_output_to_command_stdout(handler.event_name, *continue_on_block, &output) {
                Ok(stdout) => {
                    prompt_run_result(started_at, started, Some(0), stdout, /*error*/ None)
                }
                Err(error) => {
                    prompt_run_result(
                        started_at,
                        started,
                        /*exit_code*/ None,
                        String::new(),
                        Some(error),
                    )
                }
            }
        }
        Ok(Err(error)) => prompt_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some(error.to_string()),
        ),
        Err(_) => prompt_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some(format!("prompt hook timed out after {timeout_sec}s")),
        ),
    }
}

fn render_prompt(prompt: &str, input_json: &str) -> String {
    if prompt.contains(PROMPT_ARGUMENTS_PLACEHOLDER) {
        prompt.replace(PROMPT_ARGUMENTS_PLACEHOLDER, input_json)
    } else {
        format!("{prompt}\n\n{input_json}")
    }
}

fn prompt_output_to_command_stdout(
    event_name: HookEventName,
    continue_on_block: bool,
    output: &str,
) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(output.trim())
        .map_err(|err| format!("prompt hook returned invalid JSON output: {err}"))?;
    if !value.is_object() {
        return Err("prompt hook returned invalid JSON output: expected an object".to_string());
    }
    let output: PromptHookOutput = serde_json::from_value(value)
        .map_err(|err| format!("prompt hook returned invalid JSON output: {err}"))?;
    if output.ok {
        return Ok("{}".to_string());
    }

    let Some(reason) = output.reason.as_deref().and_then(trimmed_reason) else {
        return Err("prompt hook returned ok:false without a non-empty reason".to_string());
    };

    prompt_block_output(event_name, continue_on_block, reason)
}

fn prompt_block_output(
    event_name: HookEventName,
    continue_on_block: bool,
    reason: String,
) -> Result<String, String> {
    let value = match event_name {
        HookEventName::PreToolUse => json!({
            "hookSpecificOutput": {
                "hookEventName": event_label(event_name),
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            },
        }),
        HookEventName::PermissionRequest => json!({}),
        HookEventName::PostToolUse => {
            if continue_on_block {
                json!({
                    "decision": "block",
                    "reason": reason,
                })
            } else {
                json!({
                    "continue": false,
                    "stopReason": reason,
                    "decision": "block",
                    "reason": reason,
                })
            }
        }
        HookEventName::UserPromptSubmit | HookEventName::Stop => json!({
            "decision": "block",
            "reason": reason,
        }),
        HookEventName::SessionStart
        | HookEventName::SubagentStart
        | HookEventName::SubagentStop
        | HookEventName::PreCompact
        | HookEventName::PostCompact => {
            return Err(format!(
                "prompt hooks are not supported for {}",
                event_label(event_name)
            ));
        }
    };
    serde_json::to_string(&value).map_err(|err| err.to_string())
}

fn trimmed_reason(reason: &str) -> Option<String> {
    let trimmed = reason.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn prompt_run_result(
    started_at: i64,
    started: Instant,
    exit_code: Option<i32>,
    stdout: String,
    error: Option<String>,
) -> CommandRunResult {
    CommandRunResult {
        started_at,
        completed_at: chrono::Utc::now().timestamp(),
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
        exit_code,
        stdout,
        stderr: String::new(),
        error,
    }
}

fn event_label(event_name: HookEventName) -> &'static str {
    match event_name {
        HookEventName::PreToolUse => "PreToolUse",
        HookEventName::PermissionRequest => "PermissionRequest",
        HookEventName::PostToolUse => "PostToolUse",
        HookEventName::PreCompact => "PreCompact",
        HookEventName::PostCompact => "PostCompact",
        HookEventName::SessionStart => "SessionStart",
        HookEventName::UserPromptSubmit => "UserPromptSubmit",
        HookEventName::SubagentStart => "SubagentStart",
        HookEventName::SubagentStop => "SubagentStop",
        HookEventName::Stop => "Stop",
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn render_prompt_replaces_arguments_placeholder() {
        assert_eq!(
            render_prompt("Check: $ARGUMENTS", r#"{"event":"Stop"}"#),
            r#"Check: {"event":"Stop"}"#
        );
    }

    #[test]
    fn render_prompt_appends_arguments_without_placeholder() {
        assert_eq!(
            render_prompt("Check the turn.", r#"{"event":"Stop"}"#),
            "Check the turn.\n\n{\"event\":\"Stop\"}"
        );
    }

    #[test]
    fn stop_ok_false_becomes_block_decision() {
        assert_eq!(
            prompt_output_to_command_stdout(
                HookEventName::Stop,
                /*continue_on_block*/ false,
                r#"{"ok":false,"reason":"mention tests"}"#
            )
            .expect("prompt output"),
            r#"{"decision":"block","reason":"mention tests"}"#
        );
    }

    #[test]
    fn pre_tool_use_ok_false_becomes_permission_deny() {
        assert_eq!(
            prompt_output_to_command_stdout(
                HookEventName::PreToolUse,
                /*continue_on_block*/ false,
                r#"{"ok":false,"reason":"destructive command"}"#
            )
            .expect("prompt output"),
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"destructive command"}}"#
        );
    }
}
