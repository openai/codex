use codex_protocol::protocol::TokenUsage;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::PoisonError;

#[derive(Debug, Default)]
pub(crate) struct GoalAccountingState {
    inner: Mutex<GoalAccountingInner>,
}

#[derive(Debug, Default)]
struct GoalAccountingInner {
    turns: HashMap<String, GoalTurnAccounting>,
    unflushed_token_delta: i64,
}

#[derive(Debug, Default)]
struct GoalTurnAccounting {
    token_delta: i64,
    stopped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordedTokenDelta {
    pub(crate) turn_delta: i64,
    pub(crate) thread_unflushed_delta: i64,
}

impl GoalAccountingState {
    pub(crate) fn start_turn(&self, turn_id: impl Into<String>) {
        let turn_id = turn_id.into();
        self.inner().turns.entry(turn_id).or_default().stopped = false;
    }

    pub(crate) fn record_token_usage(
        &self,
        turn_id: impl Into<String>,
        usage: &TokenUsage,
    ) -> Option<RecordedTokenDelta> {
        let delta = goal_token_delta_for_usage(usage);
        if delta <= 0 {
            return None;
        }

        let turn_id = turn_id.into();
        let mut inner = self.inner();
        let turn = inner.turns.entry(turn_id).or_default();
        turn.token_delta = turn.token_delta.saturating_add(delta);
        let turn_delta = turn.token_delta;
        inner.unflushed_token_delta = inner.unflushed_token_delta.saturating_add(delta);
        Some(RecordedTokenDelta {
            turn_delta,
            thread_unflushed_delta: inner.unflushed_token_delta,
        })
    }

    pub(crate) fn stop_turn(&self, turn_id: &str) {
        if let Some(turn) = self.inner().turns.get_mut(turn_id) {
            turn.stopped = true;
        }
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, GoalAccountingInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub(crate) fn goal_token_delta_for_usage(usage: &TokenUsage) -> i64 {
    usage
        .input_tokens
        .saturating_sub(usage.cached_input_tokens)
        .saturating_add(usage.output_tokens.max(0))
}
