//! Session-scoped runtime for detached command hooks.
//!
//! Async hooks cannot affect the operation that launched them. Successful
//! informational output is queued until a later model request accepts user
//! input. Delivery uses two independent gates:
//!
//! - An accepted-input generation prevents output from being delivered before
//!   its eligible model request. Most events target the next generation;
//!   session and subagent start events target the generation after that so
//!   their output always skips the model request that runs the start hook.
//! - Core snapshots the ready completions before synchronous prompt hooks run,
//!   so output that completes during that work cannot race into the same model
//!   request.
//!
//! The runtime survives hook configuration refreshes and bounds concurrent
//! commands, queued completions, and the amount delivered to one request.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_utils_output_truncation::approx_token_count;
use tokio::task::JoinSet;

use super::CommandShell;
use super::ConfiguredHandler;
use super::command_runner::run_command;
use super::output_parser;
use crate::output_spill::HookOutputSpiller;

const MAX_QUEUED_COMPLETIONS: usize = 64;
const MAX_IN_FLIGHT_COMMANDS: usize = 32;
const MAX_DELIVERED_COMPLETIONS_PER_REQUEST: usize = 8;
const MAX_DELIVERED_OUTPUT_TOKENS_PER_REQUEST: usize = 10_000;

/// Informational async hook output ready to be recorded for a model request.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AsyncHookDelivery {
    /// Context fragments to inject into model-visible conversation history.
    pub additional_contexts: Vec<String>,
    /// User-visible messages to emit without adding them to model context.
    pub system_messages: Vec<String>,
}

/// Async output that was ready before prompt submission began.
///
/// Dropping this value leaves the output queued. Calling [`Self::accept_input`]
/// advances the accepted-input generation and drains output for that request.
pub struct PendingAsyncHookDelivery {
    runtime: AsyncCommandRuntime,
    ready: Vec<u64>,
}

impl PendingAsyncHookDelivery {
    pub fn accept_input(self) -> AsyncHookDelivery {
        self.runtime.drain_for_accepted_input(self.ready)
    }
}

/// Shared runtime state for async commands launched during one Codex session.
///
/// Clones refer to the same in-flight tasks, queued output, and delivery
/// generations. This lets hook configuration refresh without orphaning work.
#[derive(Clone)]
pub(crate) struct AsyncCommandRuntime {
    inner: Arc<AsyncCommandRuntimeInner>,
}

struct AsyncCommandRuntimeInner {
    state: Mutex<AsyncCommandRuntimeState>,
    output_spiller: HookOutputSpiller,
}

impl AsyncCommandRuntimeInner {
    fn lock_state(&self) -> MutexGuard<'_, AsyncCommandRuntimeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Default)]
struct AsyncCommandRuntimeState {
    accepted_input_generation: u64,
    next_launch_sequence: u64,
    shutting_down: bool,
    completions: BTreeMap<u64, AsyncHookCompletion>,
    tasks: JoinSet<()>,
}

struct AsyncHookCompletion {
    deliver_at_generation: u64,
    output: output_parser::AsyncInformationalOutput,
}

impl AsyncCommandRuntime {
    /// Creates an empty runtime for a newly started Codex session.
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(AsyncCommandRuntimeInner {
                state: Mutex::new(AsyncCommandRuntimeState::default()),
                output_spiller: HookOutputSpiller::new(),
            }),
        }
    }

    /// Returns the spiller shared by synchronous and asynchronous hook output.
    ///
    /// Keeping it with the refresh-stable runtime lets detached commands spill
    /// output even if hook configuration changes before they finish.
    pub(crate) fn output_spiller(&self) -> &HookOutputSpiller {
        &self.inner.output_spiller
    }

    /// Captures which async completions were ready before prompt hooks run.
    ///
    /// A completion registered after this snapshot is ineligible for the
    /// accepted model request associated with the snapshot, even if the command
    /// finishes before synchronous prompt hooks return.
    pub(crate) fn pending_delivery(&self) -> PendingAsyncHookDelivery {
        let state = self.inner.lock_state();
        PendingAsyncHookDelivery {
            runtime: self.clone(),
            ready: state.completions.keys().copied().collect(),
        }
    }

    /// Launches one command without waiting for it or emitting hook lifecycle events.
    ///
    /// Only successful informational output is queued. Control decisions are
    /// discarded by the async parser. The event determines the earliest
    /// accepted-input generation at which the output may be delivered.
    pub(crate) fn spawn(
        &self,
        shell: CommandShell,
        handler: ConfiguredHandler,
        input_json: String,
        cwd: PathBuf,
        thread_id: ThreadId,
    ) {
        let mut state = self.inner.lock_state();
        while state.tasks.try_join_next().is_some() {}
        if state.shutting_down {
            return;
        }
        if state.tasks.len() >= MAX_IN_FLIGHT_COMMANDS {
            tracing::warn!(
                event_name = ?handler.event_name,
                hook_source = ?handler.source,
                limit = MAX_IN_FLIGHT_COMMANDS,
                "skipping async hook command after reaching the session concurrency limit"
            );
            return;
        }

        let launch_sequence = state.next_launch_sequence;
        state.next_launch_sequence = state.next_launch_sequence.saturating_add(1);
        let generation_delay = match handler.event_name {
            HookEventName::SessionStart | HookEventName::SubagentStart => 2,
            HookEventName::PreToolUse
            | HookEventName::PermissionRequest
            | HookEventName::PostToolUse
            | HookEventName::PreCompact
            | HookEventName::PostCompact
            | HookEventName::UserPromptSubmit
            | HookEventName::SubagentStop
            | HookEventName::Stop => 1,
        };
        let deliver_at_generation = state
            .accepted_input_generation
            .saturating_add(generation_delay);
        let inner = Arc::clone(&self.inner);
        state.tasks.spawn(async move {
            let result = run_command(&shell, &handler, &input_json, &cwd).await;
            tracing::debug!(
                event_name = ?handler.event_name,
                hook_source = ?handler.source,
                exit_code = result.exit_code,
                duration_ms = result.duration_ms,
                failed = result.error.is_some(),
                "async hook command completed"
            );
            if result.error.is_some() || result.exit_code != Some(0) {
                return;
            }
            let Some(mut output) =
                output_parser::parse_async_informational(handler.event_name, &result.stdout)
            else {
                return;
            };
            output.additional_context = inner
                .output_spiller
                .maybe_spill_optional_text(thread_id, output.additional_context)
                .await;
            output.system_message = inner
                .output_spiller
                .maybe_spill_optional_text(thread_id, output.system_message)
                .await;
            let mut state = inner.lock_state();
            if state.shutting_down {
                return;
            }
            state.completions.insert(
                launch_sequence,
                AsyncHookCompletion {
                    deliver_at_generation,
                    output,
                },
            );
            if state.completions.len() > MAX_QUEUED_COMPLETIONS
                && let Some((oldest, _)) = state.completions.pop_first()
            {
                tracing::warn!(
                    launch_sequence = oldest,
                    "dropping queued async hook output after reaching the session limit"
                );
            }
        });
    }

    /// Advances the accepted-input generation and drains eligible output.
    ///
    /// Output must both target the new generation and have been ready when the
    /// pending delivery was created. Delivery is bounded; remaining eligible
    /// output stays queued for later accepted model requests.
    fn drain_for_accepted_input(&self, ready: Vec<u64>) -> AsyncHookDelivery {
        let mut state = self.inner.lock_state();
        state.accepted_input_generation = state.accepted_input_generation.saturating_add(1);
        let accepted_generation = state.accepted_input_generation;
        let mut eligible = Vec::new();
        let mut output_tokens = 0usize;
        for launch_sequence in ready {
            let Some(completion) = state.completions.get(&launch_sequence) else {
                continue;
            };
            if completion.deliver_at_generation > accepted_generation {
                continue;
            }
            if eligible.len() >= MAX_DELIVERED_COMPLETIONS_PER_REQUEST {
                break;
            }
            let completion_output_tokens = completion
                .output
                .additional_context
                .as_deref()
                .into_iter()
                .chain(completion.output.system_message.as_deref())
                .map(approx_token_count)
                .fold(0, usize::saturating_add);
            if output_tokens.saturating_add(completion_output_tokens)
                > MAX_DELIVERED_OUTPUT_TOKENS_PER_REQUEST
            {
                break;
            }
            eligible.push(launch_sequence);
            output_tokens = output_tokens.saturating_add(completion_output_tokens);
        }
        let mut delivery = AsyncHookDelivery::default();
        for launch_sequence in eligible {
            let Some(completion) = state.completions.remove(&launch_sequence) else {
                continue;
            };
            if let Some(additional_context) = completion.output.additional_context {
                delivery.additional_contexts.push(additional_context);
            }
            if let Some(system_message) = completion.output.system_message {
                delivery.system_messages.push(system_message);
            }
        }
        delivery
    }

    /// Stops accepting output, clears queued completions, and aborts all tasks.
    ///
    /// Shutdown waits for every aborted Tokio task so no detached hook command
    /// remains owned by the session after this method returns.
    pub(crate) async fn shutdown(&self) {
        let mut tasks = {
            let mut state = self.inner.lock_state();
            state.shutting_down = true;
            state.completions.clear();
            std::mem::take(&mut state.tasks)
        };
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
}

#[cfg(test)]
#[path = "async_command_tests.rs"]
mod tests;
