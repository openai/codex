use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_protocol::ThreadId;
use codex_utils_output_truncation::approx_token_count;
use tokio::task::JoinHandle;

use super::CommandShell;
use super::ConfiguredHandler;
use super::command_runner::run_command;
use super::output_parser;
use crate::output_spill::HookOutputSpiller;

const MAX_QUEUED_COMPLETIONS: usize = 64;
const MAX_IN_FLIGHT_COMMANDS: usize = 32;
const MAX_DELIVERED_COMPLETIONS_PER_TURN: usize = 8;
const MAX_DELIVERED_CONTEXT_TOKENS_PER_TURN: usize = 10_000;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AsyncHookDelivery {
    pub additional_contexts: Vec<String>,
    pub system_messages: Vec<String>,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct AsyncHookDeliveryCutoff {
    ready_sequence: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum AsyncDeliveryTiming {
    NextAcceptedTurn,
    AcceptedTurnAfterNext,
}

impl AsyncDeliveryTiming {
    fn generation_delay(self) -> u64 {
        match self {
            Self::NextAcceptedTurn => 1,
            Self::AcceptedTurnAfterNext => 2,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AsyncCommandRuntime {
    inner: Arc<AsyncCommandRuntimeInner>,
}

struct AsyncCommandRuntimeInner {
    accepted_turn_generation: AtomicU64,
    next_launch_sequence: AtomicU64,
    shutting_down: AtomicBool,
    state: Mutex<AsyncCommandRuntimeState>,
    output_spiller: HookOutputSpiller,
}

#[derive(Default)]
struct AsyncCommandRuntimeState {
    next_ready_sequence: u64,
    completions: BTreeMap<u64, AsyncHookCompletion>,
    tasks: Vec<JoinHandle<()>>,
}

struct AsyncHookCompletion {
    deliver_at_generation: u64,
    ready_sequence: u64,
    additional_context: Option<String>,
    system_message: Option<String>,
}

impl AsyncCommandRuntime {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(AsyncCommandRuntimeInner {
                accepted_turn_generation: AtomicU64::new(0),
                next_launch_sequence: AtomicU64::new(0),
                shutting_down: AtomicBool::new(false),
                state: Mutex::new(AsyncCommandRuntimeState::default()),
                output_spiller: HookOutputSpiller::new(),
            }),
        }
    }

    pub(crate) fn output_spiller(&self) -> &HookOutputSpiller {
        &self.inner.output_spiller
    }

    pub(crate) fn delivery_cutoff(&self) -> AsyncHookDeliveryCutoff {
        let ready_sequence = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_ready_sequence;
        AsyncHookDeliveryCutoff { ready_sequence }
    }

    pub(crate) fn spawn(
        &self,
        shell: CommandShell,
        handler: ConfiguredHandler,
        input_json: String,
        cwd: PathBuf,
        thread_id: ThreadId,
        delivery_timing: AsyncDeliveryTiming,
    ) {
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return;
        }

        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.tasks.retain(|task| !task.is_finished());
        if self.inner.shutting_down.load(Ordering::Acquire) {
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

        let launch_sequence = self
            .inner
            .next_launch_sequence
            .fetch_add(1, Ordering::AcqRel);
        let deliver_at_generation = self
            .inner
            .accepted_turn_generation
            .load(Ordering::Acquire)
            .saturating_add(delivery_timing.generation_delay());
        let inner = Arc::clone(&self.inner);
        let handle = tokio::spawn(async move {
            let result = run_command(&shell, &handler, &input_json, &cwd).await;
            tracing::debug!(
                event_name = ?handler.event_name,
                hook_source = ?handler.source,
                exit_code = result.exit_code,
                duration_ms = result.duration_ms,
                failed = result.error.is_some(),
                "async hook command completed"
            );
            let Some(mut output) =
                output_parser::parse_async_informational(handler.event_name, &result)
            else {
                return;
            };
            if let Some(additional_context) = output.additional_context.take() {
                output.additional_context = Some(
                    inner
                        .output_spiller
                        .maybe_spill_text(thread_id, additional_context)
                        .await,
                );
            }
            if inner.shutting_down.load(Ordering::Acquire) {
                return;
            }

            let mut state = inner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if inner.shutting_down.load(Ordering::Acquire) {
                return;
            }
            let ready_sequence = state.next_ready_sequence;
            state.next_ready_sequence = state.next_ready_sequence.saturating_add(1);
            state.completions.insert(
                launch_sequence,
                AsyncHookCompletion {
                    deliver_at_generation,
                    ready_sequence,
                    additional_context: output.additional_context,
                    system_message: output.system_message,
                },
            );
            while state.completions.len() > MAX_QUEUED_COMPLETIONS {
                let Some(oldest) = state.completions.first_key_value().map(|(key, _)| *key) else {
                    break;
                };
                state.completions.remove(&oldest);
                tracing::warn!(
                    launch_sequence = oldest,
                    "dropping queued async hook output after reaching the session limit"
                );
            }
        });
        state.tasks.push(handle);
    }

    pub(crate) fn commit_accepted_turn_and_drain(
        &self,
        cutoff: AsyncHookDeliveryCutoff,
    ) -> AsyncHookDelivery {
        let accepted_generation = self
            .inner
            .accepted_turn_generation
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut eligible = Vec::new();
        let mut context_tokens = 0usize;
        for (launch_sequence, completion) in &state.completions {
            if completion.ready_sequence >= cutoff.ready_sequence
                || completion.deliver_at_generation > accepted_generation
            {
                continue;
            }
            if eligible.len() >= MAX_DELIVERED_COMPLETIONS_PER_TURN {
                break;
            }
            let completion_context_tokens = completion
                .additional_context
                .as_deref()
                .map(approx_token_count)
                .unwrap_or_default();
            if context_tokens.saturating_add(completion_context_tokens)
                > MAX_DELIVERED_CONTEXT_TOKENS_PER_TURN
            {
                break;
            }
            eligible.push(*launch_sequence);
            context_tokens = context_tokens.saturating_add(completion_context_tokens);
        }
        let mut delivery = AsyncHookDelivery::default();
        for launch_sequence in eligible {
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

    pub(crate) async fn shutdown(&self) {
        self.inner.shutting_down.store(true, Ordering::Release);
        let tasks = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.completions.clear();
            std::mem::take(&mut state.tasks)
        };
        for task in &tasks {
            task.abort();
        }
        for task in tasks {
            let _ = task.await;
        }
    }
}

#[cfg(test)]
#[path = "async_command_tests.rs"]
mod tests;
