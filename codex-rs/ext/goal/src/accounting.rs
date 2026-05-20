use codex_protocol::config_types::ModeKind;
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
    last_accounted_token_usage: TokenUsage,
    account_tokens: bool,
    stopped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordedTokenDelta {
    pub(crate) turn_delta: i64,
    pub(crate) thread_unflushed_delta: i64,
}

impl GoalAccountingState {
    pub(crate) fn start_turn(
        &self,
        turn_id: impl Into<String>,
        collaboration_mode: ModeKind,
        token_usage_at_turn_start: &TokenUsage,
    ) {
        let turn_id = turn_id.into();
        self.inner().turns.insert(
            turn_id,
            GoalTurnAccounting {
                token_delta: 0,
                last_accounted_token_usage: token_usage_at_turn_start.clone(),
                account_tokens: !matches!(collaboration_mode, ModeKind::Plan),
                stopped: false,
            },
        );
    }

    pub(crate) fn record_token_usage(
        &self,
        turn_id: impl Into<String>,
        total_usage: &TokenUsage,
    ) -> Option<RecordedTokenDelta> {
        let turn_id = turn_id.into();
        let mut inner = self.inner();
        let turn = inner.turns.get_mut(&turn_id)?;
        if turn.stopped || !turn.account_tokens {
            return None;
        }

        let delta =
            token_delta_since_last_accounting(&turn.last_accounted_token_usage, total_usage);
        turn.last_accounted_token_usage = total_usage.clone();
        if delta <= 0 {
            return None;
        }
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

fn token_delta_since_last_accounting(last: &TokenUsage, current: &TokenUsage) -> i64 {
    let delta = TokenUsage {
        input_tokens: current.input_tokens.saturating_sub(last.input_tokens),
        cached_input_tokens: current
            .cached_input_tokens
            .saturating_sub(last.cached_input_tokens),
        output_tokens: current.output_tokens.saturating_sub(last.output_tokens),
        reasoning_output_tokens: current
            .reasoning_output_tokens
            .saturating_sub(last.reasoning_output_tokens),
        total_tokens: current.total_tokens.saturating_sub(last.total_tokens),
    };
    goal_token_delta_for_usage(&delta)
}

pub(crate) fn goal_token_delta_for_usage(usage: &TokenUsage) -> i64 {
    usage
        .input_tokens
        .saturating_sub(usage.cached_input_tokens)
        .saturating_add(usage.output_tokens.max(0))
}
