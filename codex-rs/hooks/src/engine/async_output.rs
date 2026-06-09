use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use codex_protocol::protocol::HookEventName;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::formatted_truncate_text;
use codex_utils_output_truncation::truncate_text;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use super::CommandShell;
use super::ConfiguredHandler;
use super::command_runner::CommandRunResult;
use super::command_runner::run_command;
use super::output_parser;

const ASYNC_HOOK_COMPLETION_TOKEN_LIMIT: usize = 500;
const ASYNC_HOOK_COMPLETION_TRUNCATION_TOKEN_LIMIT: usize = 450;
const ASYNC_HOOK_FLUSH_TOKEN_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AsyncCommandCompletion {
    event_name: HookEventName,
    text: String,
}

/// Session-scoped owner of detached command-hook tasks and their completed output.
///
/// Clones share cancellation, task tracking, and the FIFO completion queue. The
/// runtime is preserved across hook reconfiguration so already-running tasks and
/// completed output remain attached to the session that spawned them.
#[derive(Clone, Default)]
pub(crate) struct AsyncCommandRuntime {
    state: Arc<AsyncCommandState>,
}

#[derive(Default)]
struct AsyncCommandState {
    pending: Mutex<VecDeque<AsyncCommandCompletion>>,
    cancellation: CancellationToken,
    tasks: TaskTracker,
}

/// A rendered snapshot of the oldest deliverable completions in the queue.
///
/// Preparing a batch does not consume it. The caller commits it only after the
/// synchronous `UserPromptSubmit` lane accepts the turn, so a blocked prompt
/// cannot lose completed async output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsyncOutputBatch {
    completion_count: usize,
    text: String,
}

impl AsyncCommandRuntime {
    /// Spawns one detached task for one matched async command handler firing.
    ///
    /// Every firing gets its own task. Command failures and invalid output are
    /// converted to informational text and queued instead of being reported
    /// through hook lifecycle or control results.
    pub(crate) fn spawn_handler(
        &self,
        shell: CommandShell,
        handler: ConfiguredHandler,
        input_json: Result<String, String>,
        cwd: PathBuf,
    ) {
        let runtime = self.clone();
        let cancellation = self.state.cancellation.clone();
        self.state.tasks.spawn(async move {
            let run_result = match input_json {
                Ok(input_json) => {
                    tokio::select! {
                        _ = cancellation.cancelled() => return,
                        run_result = run_command(&shell, &handler, &input_json, &cwd) => run_result,
                    }
                }
                Err(error) => {
                    // Serialization failures use the same queued delivery path as
                    // command-runtime failures, unless the session is shutting down.
                    if cancellation.is_cancelled() {
                        return;
                    }
                    CommandRunResult::failed(error)
                }
            };
            if let Some(output) = deliverable_output(handler.event_name, &run_result) {
                runtime.push(handler.event_name, output);
            }
        });
    }

    /// Appends one completed firing to the FIFO queue without merging or deduplication.
    ///
    /// Each completion body is bounded before storage so one firing cannot later
    /// dominate a merged harness-generated context item.
    pub(crate) fn push(&self, event_name: HookEventName, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let text = formatted_truncate_text(
            &text,
            TruncationPolicy::Tokens(ASYNC_HOOK_COMPLETION_TRUNCATION_TOKEN_LIMIT),
        );
        // The formatted truncation marker can itself cross the requested budget.
        // Apply a second hard cap so the stored completion always respects the
        // explicit per-item limit.
        let text = if approx_token_count(&text) > ASYNC_HOOK_COMPLETION_TOKEN_LIMIT {
            truncate_text(
                &text,
                TruncationPolicy::Tokens(ASYNC_HOOK_COMPLETION_TOKEN_LIMIT),
            )
        } else {
            text
        };
        self.lock_pending()
            .push_back(AsyncCommandCompletion { event_name, text });
    }

    /// Prepares the oldest contiguous queue prefix that fits the per-turn flush budget.
    ///
    /// The selected completions remain queued until [`Self::commit`] is called.
    /// Later completions stay in FIFO order for a subsequent real user turn.
    pub(crate) fn prepare_batch(&self) -> Option<AsyncOutputBatch> {
        let pending = self.lock_pending();
        let mut selected = Vec::new();
        for completion in pending.iter() {
            selected.push(completion.clone());
            // Measure the fully rendered developer injection so wrapper overhead
            // counts toward the flush budget. The first item that does not fit,
            // and every item after it, remains queued.
            let text = render_batch(&selected);
            if approx_token_count(&text) > ASYNC_HOOK_FLUSH_TOKEN_LIMIT {
                selected.pop();
                break;
            }
        }
        (!selected.is_empty()).then(|| AsyncOutputBatch {
            completion_count: selected.len(),
            text: render_batch(&selected),
        })
    }

    /// Consumes a prepared prefix and returns its merged developer-context payload.
    pub(crate) fn commit(&self, batch: AsyncOutputBatch) -> String {
        // Producers only append, so completions arriving after preparation cannot
        // disturb the prefix identified by `completion_count`.
        self.lock_pending().drain(..batch.completion_count);
        batch.text
    }

    /// Cancels in-flight handlers, closes the tracker for waiting, and joins its tasks.
    pub(crate) async fn shutdown(&self) {
        self.state.cancellation.cancel();
        self.state.tasks.close();
        self.state.tasks.wait().await;
    }

    /// Locks the completion queue, recovering its contents if a producer panicked.
    fn lock_pending(&self) -> MutexGuard<'_, VecDeque<AsyncCommandCompletion>> {
        self.state
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Renders several completed firings as one ordered developer-context injection.
fn render_batch(completions: &[AsyncCommandCompletion]) -> String {
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

/// Converts a command result into informational text suitable for later delivery.
///
/// Successful output contributes only event-supported informational content.
/// Runtime, exit, and parse failures become text so detached failures are not
/// silently lost.
pub(crate) fn deliverable_output(
    event_name: HookEventName,
    run_result: &CommandRunResult,
) -> Option<String> {
    match (run_result.error.as_deref(), run_result.exit_code) {
        (Some(error), _) => Some(format!("Async hook failed to run: {error}")),
        (None, Some(0)) => parse_stdout(event_name, &run_result.stdout),
        (None, Some(exit_code)) => Some(format!("Async hook exited with code {exit_code}")),
        (None, None) => Some("Async hook process terminated without an exit code".to_string()),
    }
}

/// Minimal async-output envelope containing only model-deliverable information.
///
/// Unknown fields are intentionally ignored, including all hook control fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncCommandOutput {
    #[serde(default)]
    hook_specific_output: Option<AsyncHookSpecificOutput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncHookSpecificOutput {
    #[serde(default)]
    additional_context: Option<String>,
}

/// Extracts informational output according to the event's existing stdout convention.
///
/// JSON-shaped output is parsed through the minimal informational envelope;
/// malformed JSON or a malformed informational field is surfaced. Plain text is
/// delivered only for events where it already represents additional context and
/// cannot acquire control semantics in the async lane.
fn parse_stdout(event_name: HookEventName, stdout: &str) -> Option<String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }

    if output_parser::looks_like_json(trimmed) {
        return match serde_json::from_str::<AsyncCommandOutput>(trimmed) {
            Ok(output) => output
                .hook_specific_output
                .and_then(|output| output.additional_context)
                .filter(|context| !context.trim().is_empty()),
            Err(_) => Some(invalid_output_message(event_name)),
        };
    }

    match event_name {
        HookEventName::SessionStart
        | HookEventName::SubagentStart
        | HookEventName::UserPromptSubmit => Some(trimmed.to_string()),
        HookEventName::PreToolUse
        | HookEventName::PermissionRequest
        | HookEventName::PostToolUse
        | HookEventName::PreCompact
        | HookEventName::PostCompact => None,
        HookEventName::SubagentStop | HookEventName::Stop => {
            Some(invalid_output_message(event_name))
        }
    }
}

/// Builds user-visible text for malformed async hook output.
fn invalid_output_message(event_name: HookEventName) -> String {
    format!("Async {event_name:?} hook returned invalid JSON output")
}

#[cfg(test)]
#[path = "async_output_tests.rs"]
mod tests;
