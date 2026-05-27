use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::TokenUsage;
use codex_state::ThreadGoalStatus;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Default)]
pub(crate) struct GoalAccountingState {
    inner: Mutex<GoalAccountingInner>,
}

#[derive(Debug)]
struct GoalAccountingInner {
    current_turn_id: Option<String>,
    turns: HashMap<String, GoalTurnAccounting>,
    wall_clock: GoalWallClockAccounting,
    budget_limit_reported_goal_id: Option<String>,
}

#[derive(Debug)]
struct GoalTurnAccounting {
    current_token_usage: TokenUsage,
    last_accounted_token_usage: TokenUsage,
    active_goal_id: Option<String>,
    account_tokens: bool,
}

#[derive(Debug)]
struct GoalWallClockAccounting {
    last_accounted_at: Instant,
    active_goal_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GoalProgressSnapshot {
    pub(crate) current_token_usage: TokenUsage,
    pub(crate) expected_goal_id: String,
    pub(crate) time_delta_seconds: i64,
    pub(crate) token_delta: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct IdleGoalProgressSnapshot {
    pub(crate) expected_goal_id: String,
    pub(crate) time_delta_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetLimitedGoalDisposition {
    KeepActive,
    ClearActive,
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
        let mut inner = self.inner();
        inner.current_turn_id = Some(turn_id.clone());
        inner.turns.insert(
            turn_id,
            GoalTurnAccounting::new(
                token_usage_at_turn_start.clone(),
                !matches!(collaboration_mode, ModeKind::Plan),
            ),
        );
    }

    pub(crate) fn current_turn_id(&self) -> Option<String> {
        self.inner().current_turn_id.clone()
    }

    pub(crate) fn turn_is_current_active_goal(&self, turn_id: &str) -> bool {
        let inner = self.inner();
        if inner.current_turn_id.as_deref() != Some(turn_id) {
            return false;
        }
        let Some(turn) = inner.turns.get(turn_id) else {
            return false;
        };
        turn.account_tokens && turn.active_goal_id.is_some()
    }

    pub(crate) fn record_token_usage(
        &self,
        turn_id: impl Into<String>,
        total_usage: &TokenUsage,
    ) -> Option<RecordedTokenDelta> {
        let turn_id = turn_id.into();
        let mut inner = self.inner();
        let turn = inner.turns.get_mut(&turn_id)?;
        turn.current_token_usage = total_usage.clone();
        if !turn.account_tokens {
            return None;
        }

        let delta = turn.token_delta_since_last_accounting();
        if delta <= 0 {
            return None;
        }
        Some(RecordedTokenDelta {
            turn_delta: delta,
            thread_unflushed_delta: inner.thread_unflushed_token_delta(),
        })
    }

    pub(crate) fn mark_turn_goal_active(&self, turn_id: &str, goal_id: impl Into<String>) {
        let mut inner = self.inner();
        let goal_id = goal_id.into();
        if inner.budget_limit_reported_goal_id.as_deref() != Some(goal_id.as_str()) {
            inner.budget_limit_reported_goal_id = None;
        }
        if let Some(turn) = inner.turns.get_mut(turn_id) {
            turn.active_goal_id = Some(goal_id.clone());
            if inner.current_turn_id.as_deref() == Some(turn_id) {
                inner.wall_clock.mark_active_goal(goal_id);
            }
        }
    }

    pub(crate) fn mark_current_turn_goal_active(
        &self,
        goal_id: impl Into<String>,
    ) -> Option<String> {
        let mut inner = self.inner();
        let turn_id = inner.current_turn_id.clone()?;
        let goal_id = goal_id.into();
        if inner.budget_limit_reported_goal_id.as_deref() != Some(goal_id.as_str()) {
            inner.budget_limit_reported_goal_id = None;
        }
        let turn = inner.turns.get_mut(turn_id.as_str())?;
        turn.active_goal_id = Some(goal_id.clone());
        turn.reset_baseline_to_current();
        inner.wall_clock.mark_active_goal(goal_id);
        Some(turn_id)
    }

    pub(crate) fn mark_idle_goal_active(&self, goal_id: impl Into<String>) {
        let mut inner = self.inner();
        let goal_id = goal_id.into();
        if inner.budget_limit_reported_goal_id.as_deref() != Some(goal_id.as_str()) {
            inner.budget_limit_reported_goal_id = None;
        }
        inner.wall_clock.mark_active_goal(goal_id);
    }

    pub(crate) fn clear_current_turn_goal(&self) -> Option<String> {
        let mut inner = self.inner();
        let turn_id = inner.current_turn_id.clone()?;
        if let Some(turn) = inner.turns.get_mut(turn_id.as_str()) {
            turn.active_goal_id = None;
        }
        inner.wall_clock.clear_active_goal();
        inner.budget_limit_reported_goal_id = None;
        Some(turn_id)
    }

    pub(crate) fn clear_active_goal(&self) {
        let mut inner = self.inner();
        if let Some(turn_id) = inner.current_turn_id.clone()
            && let Some(turn) = inner.turns.get_mut(turn_id.as_str())
        {
            turn.active_goal_id = None;
        }
        inner.wall_clock.clear_active_goal();
        inner.budget_limit_reported_goal_id = None;
    }

    pub(crate) fn progress_snapshot(&self, turn_id: &str) -> Option<GoalProgressSnapshot> {
        let inner = self.inner();
        let turn = inner.turns.get(turn_id)?;
        if !turn.account_tokens {
            return None;
        }
        let expected_goal_id = turn.active_goal_id()?;
        let token_delta = turn.token_delta_since_last_accounting();
        let time_delta_seconds =
            if inner.wall_clock.active_goal_id.as_deref() == Some(expected_goal_id.as_str()) {
                inner.wall_clock.time_delta_since_last_accounting()
            } else {
                0
            };
        if time_delta_seconds == 0 && token_delta <= 0 {
            return None;
        }
        Some(GoalProgressSnapshot {
            current_token_usage: turn.current_token_usage.clone(),
            expected_goal_id,
            time_delta_seconds,
            token_delta,
        })
    }

    pub(crate) fn idle_progress_snapshot(&self) -> Option<IdleGoalProgressSnapshot> {
        let inner = self.inner();
        let expected_goal_id = inner.wall_clock.active_goal_id.clone()?;
        let time_delta_seconds = inner.wall_clock.time_delta_since_last_accounting();
        if time_delta_seconds == 0 {
            return None;
        }
        Some(IdleGoalProgressSnapshot {
            expected_goal_id,
            time_delta_seconds,
        })
    }

    pub(crate) fn mark_progress_accounted_for_status(
        &self,
        turn_id: &str,
        snapshot: &GoalProgressSnapshot,
        status: ThreadGoalStatus,
        budget_limited_goal_disposition: BudgetLimitedGoalDisposition,
    ) {
        let clear_active_goal = should_clear_active_goal(status, budget_limited_goal_disposition);
        let mut inner = self.inner();
        if let Some(turn) = inner.turns.get_mut(turn_id) {
            turn.last_accounted_token_usage = snapshot.current_token_usage.clone();
            if clear_active_goal {
                turn.active_goal_id = None;
            }
        }
        inner.wall_clock.mark_accounted(snapshot.time_delta_seconds);
        if clear_active_goal {
            inner.wall_clock.clear_active_goal();
        }
        if status != ThreadGoalStatus::BudgetLimited {
            inner.budget_limit_reported_goal_id = None;
        }
    }

    pub(crate) fn finish_turn(&self, turn_id: &str) {
        let mut inner = self.inner();
        inner.turns.remove(turn_id);
        if inner.current_turn_id.as_deref() == Some(turn_id) {
            inner.current_turn_id = None;
        }
    }

    pub(crate) fn mark_idle_progress_accounted_for_status(
        &self,
        snapshot: &IdleGoalProgressSnapshot,
        status: ThreadGoalStatus,
        budget_limited_goal_disposition: BudgetLimitedGoalDisposition,
    ) {
        let clear_active_goal = should_clear_active_goal(status, budget_limited_goal_disposition);
        let mut inner = self.inner();
        inner.wall_clock.mark_accounted(snapshot.time_delta_seconds);
        if clear_active_goal {
            inner.wall_clock.clear_active_goal();
        }
        if status != ThreadGoalStatus::BudgetLimited {
            inner.budget_limit_reported_goal_id = None;
        }
    }

    pub(crate) fn reset_idle_progress_baseline_and_clear_active_goal(&self) {
        let mut inner = self.inner();
        inner.wall_clock.reset_baseline();
        inner.wall_clock.clear_active_goal();
        inner.budget_limit_reported_goal_id = None;
    }

    pub(crate) fn mark_budget_limit_reported_if_new(&self, goal_id: &str) -> bool {
        let mut inner = self.inner();
        if inner.budget_limit_reported_goal_id.as_deref() == Some(goal_id) {
            return false;
        }
        inner.budget_limit_reported_goal_id = Some(goal_id.to_string());
        true
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

impl Default for GoalAccountingInner {
    fn default() -> Self {
        Self {
            current_turn_id: None,
            turns: HashMap::new(),
            wall_clock: GoalWallClockAccounting::new(),
            budget_limit_reported_goal_id: None,
        }
    }
}

impl GoalAccountingInner {
    fn thread_unflushed_token_delta(&self) -> i64 {
        self.turns
            .values()
            .filter(|turn| turn.account_tokens)
            .fold(0_i64, |total, turn| {
                total.saturating_add(turn.token_delta_since_last_accounting().max(0))
            })
    }
}

impl GoalTurnAccounting {
    fn new(current_token_usage: TokenUsage, account_tokens: bool) -> Self {
        Self {
            last_accounted_token_usage: current_token_usage.clone(),
            current_token_usage,
            active_goal_id: None,
            account_tokens,
        }
    }

    fn active_goal_id(&self) -> Option<String> {
        self.active_goal_id.clone()
    }

    fn reset_baseline_to_current(&mut self) {
        self.last_accounted_token_usage = self.current_token_usage.clone();
    }

    fn token_delta_since_last_accounting(&self) -> i64 {
        token_delta_since_last_accounting(
            &self.last_accounted_token_usage,
            &self.current_token_usage,
        )
    }
}

impl GoalWallClockAccounting {
    fn new() -> Self {
        Self {
            last_accounted_at: Instant::now(),
            active_goal_id: None,
        }
    }

    fn time_delta_since_last_accounting(&self) -> i64 {
        i64::try_from(self.last_accounted_at.elapsed().as_secs()).unwrap_or(i64::MAX)
    }

    fn mark_accounted(&mut self, accounted_seconds: i64) {
        if accounted_seconds <= 0 {
            return;
        }
        let advance = Duration::from_secs(u64::try_from(accounted_seconds).unwrap_or(u64::MAX));
        self.last_accounted_at = self
            .last_accounted_at
            .checked_add(advance)
            .unwrap_or_else(Instant::now);
    }

    fn reset_baseline(&mut self) {
        self.last_accounted_at = Instant::now();
    }

    fn mark_active_goal(&mut self, goal_id: impl Into<String>) {
        let goal_id = goal_id.into();
        if self.active_goal_id.as_deref() != Some(goal_id.as_str()) {
            self.reset_baseline();
            self.active_goal_id = Some(goal_id);
        }
    }

    fn clear_active_goal(&mut self) {
        self.active_goal_id = None;
        self.reset_baseline();
    }
}

fn should_clear_active_goal(
    status: ThreadGoalStatus,
    budget_limited_goal_disposition: BudgetLimitedGoalDisposition,
) -> bool {
    match status {
        ThreadGoalStatus::Active => false,
        ThreadGoalStatus::BudgetLimited => matches!(
            budget_limited_goal_disposition,
            BudgetLimitedGoalDisposition::ClearActive
        ),
        ThreadGoalStatus::Paused
        | ThreadGoalStatus::Blocked
        | ThreadGoalStatus::UsageLimited
        | ThreadGoalStatus::Complete => true,
    }
}
