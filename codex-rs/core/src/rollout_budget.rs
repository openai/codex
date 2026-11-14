use crate::config::RolloutBudgetConfig;
use crate::config::rollout_budget_threshold_reached;
use codex_protocol::BudgetUnitsSum;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TokenUsage;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;

#[derive(Debug, Default)]
struct RolloutWorkGate(OnceLock<()>);

impl RolloutWorkGate {
    fn stop_budget(&self) {
        let _ = self.0.set(());
    }

    fn is_stopped(&self) -> bool {
        self.0.get().is_some()
    }

    fn check_start(&self) -> CodexResult<()> {
        if self.is_stopped() {
            return Err(CodexErr::SessionBudgetExceeded);
        }
        Ok(())
    }
}

/// Budget reminder returned by a controller and acknowledged after history insertion.
pub struct RolloutBudgetReminder {
    pub remaining_units: f64,
    /// Backend-defined reminder position, returned unchanged on acknowledgement.
    pub reminder_index: i64,
}

/// Native spending and reminders, plus the stop published by the tree's selected
/// accounting owner (native or host). In-flight work can still report its bill.
#[derive(Default)]
pub struct RolloutBudget {
    state: OnceLock<Mutex<RolloutBudgetState>>,
    gate: Arc<RolloutWorkGate>,
}

/// A native account owns this thread's bills. A host-owned thread only shares its work gate.
pub(crate) enum ThreadBudgetAccount {
    Native(Arc<RolloutBudget>),
    Shared(Arc<RolloutBudget>),
}

/// Every thread consults the shared work gate before starting new work.
#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadBudgetPolicy {
    rollout: Arc<RolloutBudget>,
}

impl ThreadBudgetPolicy {
    pub(crate) fn new(account: ThreadBudgetAccount) -> CodexResult<Self> {
        let (ThreadBudgetAccount::Native(rollout) | ThreadBudgetAccount::Shared(rollout)) = account;
        Ok(Self { rollout })
    }

    pub(crate) fn check_start(&self) -> CodexResult<()> {
        self.rollout.gate.check_start()
    }
}

impl std::fmt::Debug for RolloutBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RolloutBudget")
            .field("state", &self.state)
            .field("stopped", &self.is_exhausted())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct RolloutBudgetState {
    config: RolloutBudgetConfig,
    posted: BudgetUnitsSum,
    /// Last reminder delivered to each thread, so every thread observes crossed thresholds.
    deliveries: HashMap<ThreadId, ThreadBudgetDelivery>,
}

#[derive(Debug)]
struct ThreadBudgetDelivery {
    window_id: String,
    reminder_index: i64,
}

impl RolloutBudget {
    pub(crate) fn configure(&self, config: RolloutBudgetConfig) {
        self.state.get_or_init(|| {
            Mutex::new(RolloutBudgetState {
                config,
                posted: BudgetUnitsSum::default(),
                deliveries: HashMap::new(),
            })
        });
    }

    /// Post the response cost and enforce the allowance under the same lock.
    pub(crate) fn record_usage(
        &self,
        usage: Option<&TokenUsage>,
        hook_charge: codex_hooks::HookCharge,
    ) -> CodexResult<()> {
        let Some(mut state) = self.lock() else {
            return Ok(());
        };
        if !hook_charge.units.is_finite() || hook_charge.units < 0.0 {
            return Err(CodexErr::Fatal("invalid additional sampling charge".into()));
        }
        if hook_charge.overflowed && state.posted.total().is_finite() {
            // Stop new work before validating the rest of an overflowing receipt.
            state.posted.add(f64::INFINITY);
            self.enforce_posted_limit(&state)?;
        }
        let provider_units = if let Some(units) =
            usage.and_then(|usage| usage.codex_rollout_budget_units.as_ref())
        {
            let units = units.as_f64().unwrap_or(f64::NAN);
            if !units.is_finite() || units < 0.0 {
                return Err(CodexErr::Fatal(
                    "response.completed usage.codex_rollout_budget_units must be finite and non-negative"
                        .to_string(),
                ));
            }
            units
        } else if let Some(usage) = usage {
            usage.output_tokens.max(0) as f64 * state.config.sampling_token_weight
                + usage.non_cached_input() as f64 * state.config.prefill_token_weight
        } else {
            0.0
        };
        if state.posted.total().is_infinite() {
            // Later valid receipts cannot restore precision. The original
            // overflow was reported; it must not veto every advisory turn.
            return Ok(());
        }
        // Keep the full bill even if enforcing the new allowance fails.
        state.posted.add(provider_units);
        state.posted.add(hook_charge.units);
        // Enforce the allowance before reporting an overflowing sum.
        self.enforce_posted_limit(&state)?;
        if !state.posted.total().is_finite() {
            return Err(CodexErr::Fatal(
                "rollout sampling cost exceeds f64 range".into(),
            ));
        }
        Ok(())
    }

    fn enforce_posted_limit(&self, state: &RolloutBudgetState) -> CodexResult<()> {
        let posted = state.posted.total();
        if state.config.enforce_limit
            && (!posted.is_finite() || posted > state.config.limit_budget.next_up())
        {
            self.stop_new_work();
        }
        Ok(())
    }

    pub(crate) fn pending_reminder(
        &self,
        thread_id: ThreadId,
        window_id: &str,
    ) -> Option<RolloutBudgetReminder> {
        let state = self.lock()?;
        let remaining_units = self.remaining_units(&state).ok()?;
        let reminder_index = state
            .config
            .reminder_at_remaining_tokens
            .iter()
            .filter(|&&threshold| rollout_budget_threshold_reached(remaining_units, threshold))
            .count() as i64;
        let previous = state.deliveries.get(&thread_id);
        let crossed_threshold = reminder_index > previous.map_or(0, |d| d.reminder_index);
        let new_window = state.config.reminder_at_context_start
            && previous.is_none_or(|d| d.window_id != window_id);
        if !crossed_threshold && !new_window {
            return None;
        }
        Some(RolloutBudgetReminder {
            remaining_units,
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

    fn lock(&self) -> Option<MutexGuard<'_, RolloutBudgetState>> {
        self.state.get().map(|state| {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
    }

    pub(crate) fn stop_new_work(&self) {
        self.gate.stop_budget();
    }

    pub(crate) fn can_receive_turn_input(&self, thread: ThreadId, turn: &str) -> bool {
        let _ = (thread, turn);
        !self.is_exhausted()
    }

    pub(crate) fn can_force_root_final(&self) -> bool {
        false
    }

    pub(crate) fn is_exhausted(&self) -> bool {
        self.gate.is_stopped()
    }

    fn remaining_units(&self, state: &RolloutBudgetState) -> CodexResult<f64> {
        if state.posted.total().is_infinite() {
            return Ok(0.0);
        }
        let used = state.posted.total();
        Ok((state.config.limit_budget - used).max(0.0))
    }
}
