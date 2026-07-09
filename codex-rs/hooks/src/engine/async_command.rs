//! Session-scoped runtime for detached command hooks.
//!
//! Async hooks cannot affect the operation that launched them. Successful
//! informational output is queued until a later user turn accepts input. Core
//! snapshots the ready completions at turn entry, before any hooks for that
//! turn run, so output that completes during the turn cannot race into it.
//!
//! The runtime survives hook configuration refreshes and delivers all output
//! that was ready when an accepted user turn began.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use codex_protocol::ThreadId;
use tokio::task::JoinSet;

use super::CommandShell;
use super::ConfiguredHandler;
use super::command_runner::run_command;
use super::output_parser;
use crate::output_spill::HookOutputSpiller;

/// Informational async hook output ready to be recorded for a model request.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AsyncHookDelivery {
    /// Context fragments to inject into model-visible conversation history.
    pub additional_contexts: Vec<String>,
    /// User-visible messages to emit without adding them to model context.
    pub system_messages: Vec<String>,
}

/// Async output that was ready when a user turn began.
///
/// Dropping this value leaves the output queued. Calling [`Self::accept_turn`]
/// drains the snapshot for an accepted user turn.
pub struct PendingAsyncHookDelivery {
    runtime: AsyncCommandRuntime,
    ready: Vec<u64>,
}

impl PendingAsyncHookDelivery {
    pub fn accept_turn(self) -> AsyncHookDelivery {
        self.runtime.drain_for_accepted_turn(self.ready)
    }
}

/// Shared runtime state for async commands launched during one Codex session.
///
/// Clones refer to the same in-flight tasks and queued output. This lets hook
/// configuration refresh without orphaning work.
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
    next_launch_sequence: u64,
    shutting_down: bool,
    completions: BTreeMap<u64, output_parser::AsyncInformationalOutput>,
    tasks: JoinSet<()>,
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

    /// Captures which async completions were ready when a user turn began.
    ///
    /// A completion registered after this snapshot is ineligible for the
    /// accepted user turn associated with the snapshot, even if the command
    /// finishes before that turn reaches the model.
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
    /// discarded by the async parser.
    pub(crate) fn spawn(
        &self,
        shell: CommandShell,
        handler: ConfiguredHandler,
        configured_order: usize,
        input_json: String,
        cwd: PathBuf,
        thread_id: ThreadId,
    ) {
        let mut state = self.inner.lock_state();
        while state.tasks.try_join_next().is_some() {}
        if state.shutting_down {
            return;
        }

        let launch_sequence = state.next_launch_sequence;
        state.next_launch_sequence = state.next_launch_sequence.saturating_add(1);
        let inner = Arc::clone(&self.inner);
        state.tasks.spawn(async move {
            let result = run_command(&shell, &handler, configured_order, &input_json, &cwd).await;
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
            state.completions.insert(launch_sequence, output);
        });
    }

    /// Drains output that was ready when an accepted user turn began.
    fn drain_for_accepted_turn(&self, ready: Vec<u64>) -> AsyncHookDelivery {
        let mut state = self.inner.lock_state();
        let mut delivery = AsyncHookDelivery::default();
        for launch_sequence in ready {
            let Some(completion) = state.completions.remove(&launch_sequence) else {
                continue;
            };
            if let Some(additional_context) = completion.additional_context {
                delivery.additional_contexts.push(additional_context);
            }
            if let Some(system_message) = completion.system_message {
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
