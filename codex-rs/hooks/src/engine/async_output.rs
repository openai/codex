use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use codex_protocol::protocol::HookEventName;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::formatted_truncate_text;
use codex_utils_output_truncation::truncate_text;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use super::command_runner::CommandRunResult;
use super::output_parser;

const ASYNC_HOOK_COMPLETION_TOKEN_LIMIT: usize = 500;
const ASYNC_HOOK_COMPLETION_TRUNCATION_TOKEN_LIMIT: usize = 450;
const ASYNC_HOOK_FLUSH_TOKEN_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AsyncHookCompletion {
    event_name: HookEventName,
    text: String,
}

/// Session-scoped FIFO for informational output from detached command hooks.
#[derive(Clone)]
pub struct AsyncHookOutputQueue {
    state: Arc<AsyncHookOutputState>,
}

struct AsyncHookOutputState {
    pending: Mutex<VecDeque<AsyncHookCompletion>>,
    cancellation: CancellationToken,
    tasks: TaskTracker,
}

/// A bounded prefix prepared for one accepted user turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncHookOutputBatch {
    completion_count: usize,
    text: String,
}

impl AsyncHookOutputBatch {
    /// Returns the merged developer context for this batch.
    pub fn into_text(self) -> String {
        self.text
    }
}

impl AsyncHookOutputQueue {
    /// Records one completed hook's bounded informational output.
    pub fn push(&self, event_name: HookEventName, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let text = formatted_truncate_text(
            &text,
            TruncationPolicy::Tokens(ASYNC_HOOK_COMPLETION_TRUNCATION_TOKEN_LIMIT),
        );
        let text = if approx_token_count(&text) > ASYNC_HOOK_COMPLETION_TOKEN_LIMIT {
            truncate_text(
                &text,
                TruncationPolicy::Tokens(ASYNC_HOOK_COMPLETION_TOKEN_LIMIT),
            )
        } else {
            text
        };
        self.lock_pending()
            .push_back(AsyncHookCompletion { event_name, text });
    }

    /// Prepares a bounded FIFO prefix without removing it from the queue.
    pub fn pending_batch(&self) -> Option<AsyncHookOutputBatch> {
        let pending = self.lock_pending();
        let mut selected = Vec::new();
        for completion in pending.iter() {
            let mut candidate = selected.clone();
            candidate.push(completion.clone());
            let text = render_batch(&candidate);
            if approx_token_count(&text) > ASYNC_HOOK_FLUSH_TOKEN_LIMIT {
                break;
            }
            selected = candidate;
        }
        (!selected.is_empty()).then(|| AsyncHookOutputBatch {
            completion_count: selected.len(),
            text: render_batch(&selected),
        })
    }

    /// Removes a previously prepared prefix after its user input is accepted.
    pub fn commit(&self, batch: &AsyncHookOutputBatch) {
        self.lock_pending().drain(..batch.completion_count);
    }

    /// Cancels detached hook commands and waits for their tasks to stop.
    pub async fn shutdown(&self) {
        self.state.cancellation.cancel();
        self.state.tasks.close();
        self.state.tasks.wait().await;
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.state.cancellation.clone()
    }

    pub(crate) fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        self.state.tasks.spawn(future);
    }

    fn lock_pending(&self) -> MutexGuard<'_, VecDeque<AsyncHookCompletion>> {
        self.state
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for AsyncHookOutputQueue {
    fn default() -> Self {
        Self {
            state: Arc::new(AsyncHookOutputState {
                pending: Mutex::new(VecDeque::new()),
                cancellation: CancellationToken::new(),
                tasks: TaskTracker::new(),
            }),
        }
    }
}

fn render_batch(completions: &[AsyncHookCompletion]) -> String {
    let outputs = completions
        .iter()
        .map(|completion| {
            format!(
                "<async_hook_output event=\"{:?}\">\n{}\n</async_hook_output>",
                completion.event_name, completion.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("<async_hook_outputs>\n{outputs}\n</async_hook_outputs>")
}

pub(crate) fn deliverable_output(
    event_name: HookEventName,
    run_result: &CommandRunResult,
) -> Option<String> {
    if let Some(error) = run_result.error.as_deref() {
        return Some(format!("Async hook failed to run: {error}"));
    }

    match run_result.exit_code {
        Some(0) => parse_stdout(event_name, &run_result.stdout),
        Some(exit_code) => Some(format!("Async hook exited with code {exit_code}")),
        None => Some("Async hook process terminated without an exit code".to_string()),
    }
}

fn parse_stdout(event_name: HookEventName, stdout: &str) -> Option<String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }

    let additional_context = match event_name {
        HookEventName::SessionStart => {
            output_parser::parse_session_start(trimmed).map(|output| output.additional_context)
        }
        HookEventName::SubagentStart => {
            output_parser::parse_subagent_start(trimmed).map(|output| output.additional_context)
        }
        HookEventName::PreToolUse => {
            output_parser::parse_pre_tool_use(trimmed).map(|output| output.additional_context)
        }
        HookEventName::PostToolUse => {
            output_parser::parse_post_tool_use(trimmed).map(|output| output.additional_context)
        }
        HookEventName::UserPromptSubmit => {
            output_parser::parse_user_prompt_submit(trimmed).map(|output| output.additional_context)
        }
        HookEventName::PermissionRequest => {
            output_parser::parse_permission_request(trimmed).map(|_| None)
        }
        HookEventName::PreCompact => output_parser::parse_pre_compact(trimmed).map(|_| None),
        HookEventName::PostCompact => output_parser::parse_post_compact(trimmed).map(|_| None),
        HookEventName::SubagentStop => output_parser::parse_subagent_stop(trimmed).map(|_| None),
        HookEventName::Stop => output_parser::parse_stop(trimmed).map(|_| None),
    };
    match additional_context {
        Some(additional_context) => additional_context.filter(|context| !context.trim().is_empty()),
        None => Some(format!(
            "Async {event_name:?} hook returned invalid JSON output"
        )),
    }
}

#[cfg(test)]
#[path = "async_output_tests.rs"]
mod tests;
