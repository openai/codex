use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
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

#[derive(Clone)]
pub(crate) struct AsyncCommandRuntime {
    state: Arc<AsyncCommandState>,
}

struct AsyncCommandState {
    pending: Mutex<VecDeque<AsyncCommandCompletion>>,
    cancellation: CancellationToken,
    tasks: TaskTracker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsyncOutputBatch {
    completion_count: usize,
    text: String,
}

impl AsyncOutputBatch {
    pub(crate) fn into_text(self) -> String {
        self.text
    }
}

impl AsyncCommandRuntime {
    pub(crate) fn spawn_handler(
        &self,
        shell: CommandShell,
        handler: ConfiguredHandler,
        input_json: Result<String, String>,
        cwd: PathBuf,
    ) {
        let runtime = self.clone();
        let cancellation = self.state.cancellation.clone();
        self.spawn(async move {
            let run_result = match input_json {
                Ok(input_json) => {
                    tokio::select! {
                        _ = cancellation.cancelled() => return,
                        run_result = run_command(&shell, &handler, &input_json, &cwd) => run_result,
                    }
                }
                Err(error) => {
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

    pub(crate) fn push(&self, event_name: HookEventName, text: String) {
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
            .push_back(AsyncCommandCompletion { event_name, text });
    }

    pub(crate) fn prepare_batch(&self) -> Option<AsyncOutputBatch> {
        let pending = self.lock_pending();
        let mut selected = Vec::new();
        for completion in pending.iter() {
            selected.push(completion.clone());
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

    pub(crate) fn commit(&self, batch: &AsyncOutputBatch) {
        self.lock_pending().drain(..batch.completion_count);
    }

    pub(crate) async fn shutdown(&self) {
        self.state.cancellation.cancel();
        self.state.tasks.close();
        self.state.tasks.wait().await;
    }

    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        self.state.tasks.spawn(future);
    }

    fn lock_pending(&self) -> MutexGuard<'_, VecDeque<AsyncCommandCompletion>> {
        self.state
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for AsyncCommandRuntime {
    fn default() -> Self {
        Self {
            state: Arc::new(AsyncCommandState {
                pending: Mutex::new(VecDeque::new()),
                cancellation: CancellationToken::new(),
                tasks: TaskTracker::new(),
            }),
        }
    }
}

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

    let (parsed, plain_text_output) = match event_name {
        HookEventName::SessionStart => (
            output_parser::parse_session_start(trimmed).map(|output| output.additional_context),
            PlainTextOutput::Deliver,
        ),
        HookEventName::SubagentStart => (
            output_parser::parse_subagent_start(trimmed).map(|output| output.additional_context),
            PlainTextOutput::Deliver,
        ),
        HookEventName::PreToolUse => (
            output_parser::parse_pre_tool_use(trimmed).map(|output| output.additional_context),
            PlainTextOutput::Ignore,
        ),
        HookEventName::PostToolUse => (
            output_parser::parse_post_tool_use(trimmed).map(|output| output.additional_context),
            PlainTextOutput::Ignore,
        ),
        HookEventName::UserPromptSubmit => (
            output_parser::parse_user_prompt_submit(trimmed)
                .map(|output| output.additional_context),
            PlainTextOutput::Deliver,
        ),
        HookEventName::PermissionRequest => (
            output_parser::parse_permission_request(trimmed).map(|_| None),
            PlainTextOutput::Ignore,
        ),
        HookEventName::PreCompact => (
            output_parser::parse_pre_compact(trimmed).map(|_| None),
            PlainTextOutput::Ignore,
        ),
        HookEventName::PostCompact => (
            output_parser::parse_post_compact(trimmed).map(|_| None),
            PlainTextOutput::Ignore,
        ),
        HookEventName::SubagentStop => (
            output_parser::parse_subagent_stop(trimmed).map(|_| None),
            PlainTextOutput::Invalid,
        ),
        HookEventName::Stop => (
            output_parser::parse_stop(trimmed).map(|_| None),
            PlainTextOutput::Invalid,
        ),
    };
    parsed_context(event_name, trimmed, parsed, plain_text_output)
}

#[derive(Clone, Copy)]
enum PlainTextOutput {
    Deliver,
    Ignore,
    Invalid,
}

fn parsed_context(
    event_name: HookEventName,
    stdout: &str,
    parsed: Option<Option<String>>,
    plain_text_output: PlainTextOutput,
) -> Option<String> {
    match parsed {
        Some(context) => context.filter(|context| !context.trim().is_empty()),
        None if output_parser::looks_like_json(stdout) => Some(invalid_output_message(event_name)),
        None => match plain_text_output {
            PlainTextOutput::Deliver => Some(stdout.to_string()),
            PlainTextOutput::Ignore => None,
            PlainTextOutput::Invalid => Some(invalid_output_message(event_name)),
        },
    }
}

fn invalid_output_message(event_name: HookEventName) -> String {
    format!("Async {event_name:?} hook returned invalid JSON output")
}

#[cfg(test)]
#[path = "async_output_tests.rs"]
mod tests;
