use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::protocol::HookEventName;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;
use futures::future::BoxFuture;
use serde::Deserialize;
use serde_json::json;
use tokio::time::timeout;

use super::ConfiguredHandler;
use super::ConfiguredHandlerKind;
use super::command_runner::CommandRunResult;
use crate::schema::hook_event_wire_name;

const PROMPT_ARGUMENTS_PLACEHOLDER: &str = "$ARGUMENTS";
const PROMPT_HOOK_INPUT_TOKEN_LIMIT: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptHookRequest {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelHookBehavior {
    Unsupported,
    Block,
    Noop,
    FeedbackOrStop,
}

pub(crate) fn model_hook_behavior(event_name: HookEventName) -> ModelHookBehavior {
    match event_name {
        // These events already use decision:block as the user-visible "try
        // again with this feedback" path: pre-action hooks block before the
        // action runs, while Stop/SubagentStop feed the reason into the next
        // model turn.
        HookEventName::PreToolUse
        | HookEventName::UserPromptSubmit
        | HookEventName::Stop
        | HookEventName::SubagentStop => ModelHookBehavior::Block,
        // Claude treats PermissionRequest ok:false as advisory only. Preserve
        // that parity: record the reason, but let normal approval flow continue.
        HookEventName::PermissionRequest => ModelHookBehavior::Noop,
        // PostToolUse runs after the tool succeeded, so ok:false is conditional:
        // continueOnBlock feeds the reason back to the model, otherwise it stops
        // the current turn.
        HookEventName::PostToolUse => ModelHookBehavior::FeedbackOrStop,
        // Claude does not support prompt hooks for these lifecycle events.
        // Keeping them explicit makes new events choose semantics deliberately.
        HookEventName::SessionStart
        | HookEventName::SubagentStart
        | HookEventName::PreCompact
        | HookEventName::PostCompact => ModelHookBehavior::Unsupported,
    }
}

#[derive(Deserialize)]
struct PromptHookOutput {
    ok: bool,
    #[serde(default)]
    reason: Option<String>,
}

/// Execute a model-backed prompt hook and adapt its response into the same
/// synthetic stdout shape that command hooks already parse. The hook prompt is
/// rendered with `$ARGUMENTS` replacement when present, otherwise the hook input
/// JSON is appended. The model must return `{ ok, reason }`; this function then
/// maps `ok:false` through the per-event behavior table so block/no-op/feedback
/// semantics stay centralized.
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
        return model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some("command handler cannot run as a prompt hook".to_string()),
        );
    };
    let Some(runner) = runner else {
        return model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some("prompt hook cannot run because no prompt runner is configured".to_string()),
        );
    };

    let request = PromptHookRequest {
        prompt: render_model_hook_prompt(prompt, input_json),
        model: model.clone().unwrap_or(default_model),
    };

    let run = timeout(Duration::from_secs(*timeout_sec), runner.run(request)).await;
    match run {
        Ok(Ok(output)) => {
            match model_hook_output_to_command_stdout(
                "prompt",
                handler.event_name,
                *continue_on_block,
                &output,
            ) {
                Ok(stdout) => {
                    model_hook_run_result(started_at, started, Some(0), stdout, /*error*/ None)
                }
                Err(error) => {
                    model_hook_run_result(
                        started_at,
                        started,
                        /*exit_code*/ None,
                        String::new(),
                        Some(error),
                    )
                }
            }
        }
        Ok(Err(error)) => model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some(error.to_string()),
        ),
        Err(_) => model_hook_run_result(
            started_at,
            started,
            /*exit_code*/ None,
            String::new(),
            Some(format!("prompt hook timed out after {timeout_sec}s")),
        ),
    }
}

pub(crate) fn render_model_hook_prompt(prompt: &str, input_json: &str) -> String {
    let rendered = if prompt.contains(PROMPT_ARGUMENTS_PLACEHOLDER) {
        prompt.replace(PROMPT_ARGUMENTS_PLACEHOLDER, input_json)
    } else {
        format!("{prompt}\n\n{input_json}")
    };
    let mut truncation_budget = PROMPT_HOOK_INPUT_TOKEN_LIMIT;
    loop {
        let candidate = truncate_text(&rendered, TruncationPolicy::Tokens(truncation_budget));
        let candidate_tokens = approx_token_count(&candidate);
        if candidate_tokens <= PROMPT_HOOK_INPUT_TOKEN_LIMIT {
            return candidate;
        }
        truncation_budget = truncation_budget.saturating_sub(
            candidate_tokens
                .saturating_sub(PROMPT_HOOK_INPUT_TOKEN_LIMIT)
                .max(1),
        );
    }
}

pub(crate) fn model_hook_output_to_command_stdout(
    hook_type: &str,
    event_name: HookEventName,
    continue_on_block: bool,
    output: &str,
) -> Result<String, String> {
    let output: PromptHookOutput = serde_json::from_str(output.trim())
        .map_err(|err| format!("{hook_type} hook returned invalid JSON output: {err}"))?;
    if output.ok {
        return Ok("{}".to_string());
    }

    let Some(reason) = output
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
    else {
        return Err(format!(
            "{hook_type} hook returned ok:false without a non-empty reason"
        ));
    };

    model_hook_block_output(hook_type, event_name, continue_on_block, reason.to_string())
}

fn model_hook_block_output(
    hook_type: &str,
    event_name: HookEventName,
    continue_on_block: bool,
    reason: String,
) -> Result<String, String> {
    let value = match model_hook_behavior(event_name) {
        ModelHookBehavior::Block => json!({
            "decision": "block",
            "reason": reason,
        }),
        ModelHookBehavior::Noop => json!({
            "systemMessage": reason,
        }),
        ModelHookBehavior::FeedbackOrStop => {
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
        ModelHookBehavior::Unsupported => {
            return Err(format!(
                "{hook_type} hooks are not supported for {}",
                hook_event_wire_name(event_name)
            ));
        }
    };
    serde_json::to_string(&value).map_err(|err| err.to_string())
}

pub(crate) fn model_hook_run_result(
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

#[cfg(test)]
#[path = "prompt_runner_tests.rs"]
mod tests;
