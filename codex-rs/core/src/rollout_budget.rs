use crate::config::RolloutBudgetConfig;
use codex_protocol::ThreadId;
use codex_protocol::protocol::TokenUsage;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;

pub(crate) struct RolloutBudgetReminder {
    pub(crate) remaining_tokens: i64,
    reminder_index: i64,
}

/// Shared accounting and reminder state for one root-thread session tree.
#[derive(Default)]
pub(crate) struct RolloutBudget {
    state: OnceLock<Mutex<RolloutBudgetState>>,
}

struct RolloutBudgetState {
    config: RolloutBudgetConfig,
    weighted_tokens_used: f64,
    /// Last reminder delivered to each thread, so every thread observes crossed thresholds.
    deliveries: HashMap<ThreadId, ThreadBudgetDelivery>,
}

struct ThreadBudgetDelivery {
    window_id: String,
    reminder_index: i64,
}

impl RolloutBudget {
    pub(crate) fn configure(&self, config: RolloutBudgetConfig) {
        self.state.get_or_init(|| {
            Mutex::new(RolloutBudgetState {
                config,
                weighted_tokens_used: 0.0,
                deliveries: HashMap::new(),
            })
        });
    }

    /// Returns true once the configured budget is exhausted, including on later calls.
    pub(crate) fn record_usage(&self, usage: &TokenUsage) -> bool {
        let Some(mut state) = self.lock() else {
            return false;
        };
        state.weighted_tokens_used += usage.output_tokens.max(0) as f64
            * state.config.sampling_token_weight
            + usage.non_cached_input() as f64 * state.config.prefill_token_weight;
        state.weighted_tokens_used >= state.config.limit_tokens as f64
    }

    pub(crate) fn pending_reminder(
        &self,
        thread_id: ThreadId,
        window_id: &str,
    ) -> Option<RolloutBudgetReminder> {
        let state = self.lock()?;
        let reminder_index = (state.weighted_tokens_used
            / state.config.reminder_interval_tokens as f64)
            .floor() as i64;
        if state.deliveries.get(&thread_id).is_some_and(|delivery| {
            delivery.window_id.as_str() == window_id && delivery.reminder_index >= reminder_index
        }) {
            return None;
        }
        Some(RolloutBudgetReminder {
            remaining_tokens: (state.config.limit_tokens as f64 - state.weighted_tokens_used)
                .max(0.0)
                .floor() as i64,
            reminder_index,
        })
    }

    pub(crate) fn mark_reminder_delivered(
        &self,
        thread_id: ThreadId,
        window_id: &str,
        reminder: RolloutBudgetReminder,
    ) {
        // Mark delivery only after history insertion; cancellation before then should retry it.
        let Some(mut state) = self.lock() else {
            return;
        };
        state.deliveries.insert(
            thread_id,
            ThreadBudgetDelivery {
                window_id: window_id.to_string(),
                reminder_index: reminder.reminder_index,
            },
        );
    }

    /// Forces the next sampling request for `thread_id` to restate the current remainder.
    pub(crate) fn rearm_reminder(&self, thread_id: ThreadId) {
        let Some(mut state) = self.lock() else {
            return;
        };
        state.deliveries.remove(&thread_id);
    }

    fn lock(&self) -> Option<MutexGuard<'_, RolloutBudgetState>> {
        self.state.get().map(|state| {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
    }
}
