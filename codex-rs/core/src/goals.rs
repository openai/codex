//! Runtime orchestration for persisted thread goals.
//!
//! This module is the small bridge between the state-db representation of a
//! thread goal and the core turn lifecycle. It owns validation, protocol
//! conversion, active-work accounting, budget-abort behavior, and the hidden
//! continuation prompt that wakes an idle thread while a goal remains active.

use crate::StateDbHandle;
use crate::codex::Session;
use crate::codex::TurnContext;
use anyhow::Context;
use codex_features::Feature;
use codex_protocol::config_types::ModeKind;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_protocol::protocol::ThreadGoalUpdatedEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TurnAbortReason;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;

pub(crate) struct SetGoalRequest {
    pub(crate) objective: Option<String>,
    pub(crate) status: Option<ThreadGoalStatus>,
    pub(crate) token_budget: Option<Option<i64>>,
}

#[derive(Clone, Copy)]
pub(crate) enum GoalAccountingBoundary {
    Tool,
    Turn,
}

#[derive(Debug)]
pub(crate) struct GoalAccountingState {
    accounting_lock: Mutex<()>,
    last_accounted_token_usage: Mutex<TokenUsage>,
    completed_this_turn: AtomicBool,
    stopped_this_turn: AtomicBool,
    active_this_turn: AtomicBool,
    active_goal_created_at_ms: Mutex<Option<i64>>,
}

impl GoalAccountingState {
    pub(crate) fn new() -> Self {
        Self {
            accounting_lock: Mutex::new(()),
            last_accounted_token_usage: Mutex::new(TokenUsage::default()),
            completed_this_turn: AtomicBool::new(false),
            stopped_this_turn: AtomicBool::new(false),
            active_this_turn: AtomicBool::new(false),
            active_goal_created_at_ms: Mutex::new(None),
        }
    }

    pub(crate) async fn mark_turn_started(&self, token_usage: TokenUsage) {
        *self.last_accounted_token_usage.lock().await = token_usage;
        self.completed_this_turn.store(false, Ordering::SeqCst);
        self.stopped_this_turn.store(false, Ordering::SeqCst);
        self.active_this_turn.store(false, Ordering::SeqCst);
        *self.active_goal_created_at_ms.lock().await = None;
    }

    async fn mark_active_goal(&self, created_at_ms: i64) {
        self.active_this_turn.store(true, Ordering::SeqCst);
        *self.active_goal_created_at_ms.lock().await = Some(created_at_ms);
    }

    fn active_this_turn(&self) -> bool {
        self.active_this_turn.load(Ordering::SeqCst)
    }

    async fn active_goal_created_at_ms(&self) -> Option<i64> {
        *self.active_goal_created_at_ms.lock().await
    }

    async fn clear_active_goal(&self) {
        self.active_this_turn.store(false, Ordering::SeqCst);
        *self.active_goal_created_at_ms.lock().await = None;
    }

    async fn reset_baseline(&self, token_usage: TokenUsage) {
        *self.last_accounted_token_usage.lock().await = token_usage;
    }

    fn completed_this_turn(&self) -> bool {
        self.completed_this_turn.load(Ordering::SeqCst)
    }

    fn mark_completed_this_turn(&self) {
        self.completed_this_turn.store(true, Ordering::SeqCst);
    }

    fn clear_completed_this_turn(&self) {
        self.completed_this_turn.store(false, Ordering::SeqCst);
    }

    fn stopped_this_turn(&self) -> bool {
        self.stopped_this_turn.load(Ordering::SeqCst)
    }

    fn mark_stopped_this_turn(&self) {
        self.stopped_this_turn.store(true, Ordering::SeqCst);
    }

    fn clear_stopped_this_turn(&self) {
        self.stopped_this_turn.store(false, Ordering::SeqCst);
    }

    async fn token_delta_since_last_accounting(&self, current: TokenUsage) -> i64 {
        let last = self.last_accounted_token_usage.lock().await;
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

    async fn mark_accounted(&self, current: TokenUsage) {
        *self.last_accounted_token_usage.lock().await = current;
    }
}

#[derive(Debug)]
pub(crate) struct GoalWallClockAccountingState {
    accounting_lock: Mutex<()>,
    last_accounted_at: Mutex<Instant>,
    active_goal_created_at_ms: Mutex<Option<i64>>,
}

impl GoalWallClockAccountingState {
    pub(crate) fn new() -> Self {
        Self {
            accounting_lock: Mutex::new(()),
            last_accounted_at: Mutex::new(Instant::now()),
            active_goal_created_at_ms: Mutex::new(None),
        }
    }

    async fn time_delta_since_last_accounting(&self) -> i64 {
        let last = self.last_accounted_at.lock().await;
        i64::try_from(last.elapsed().as_secs()).unwrap_or(i64::MAX)
    }

    async fn mark_accounted(&self, time_delta_seconds: i64) {
        if time_delta_seconds > 0 {
            let mut last_accounted_at = self.last_accounted_at.lock().await;
            let advance =
                Duration::from_secs(u64::try_from(time_delta_seconds).unwrap_or(u64::MAX));
            *last_accounted_at = last_accounted_at
                .checked_add(advance)
                .unwrap_or_else(Instant::now);
        }
    }

    async fn reset_baseline(&self) {
        *self.last_accounted_at.lock().await = Instant::now();
    }

    async fn mark_active_goal(&self, created_at_ms: i64) {
        let mut active_goal_created_at_ms = self.active_goal_created_at_ms.lock().await;
        if *active_goal_created_at_ms != Some(created_at_ms) {
            self.reset_baseline().await;
            *active_goal_created_at_ms = Some(created_at_ms);
        }
    }

    async fn clear_active_goal(&self) {
        *self.active_goal_created_at_ms.lock().await = None;
    }
}

impl Session {
    pub(crate) async fn get_thread_goal(&self) -> anyhow::Result<Option<ThreadGoal>> {
        if !self.enabled(Feature::GoalMode) {
            return Ok(None);
        }

        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(None);
        };
        let goal = self
            .account_thread_goal_wall_clock_usage(
                &state_db,
                codex_state::ThreadGoalAccountingMode::ActiveOnly,
            )
            .await?
            .or(state_db
                .get_thread_goal(self.conversation_id)
                .await?
                .map(protocol_goal_from_state));
        if goal.is_some() {
            self.thread_goal_may_exist.store(true, Ordering::SeqCst);
        }
        *self.thread_goal_cache.lock().await = goal.clone();
        Ok(goal)
    }

    pub(crate) async fn set_thread_goal(
        &self,
        turn_context: &TurnContext,
        request: SetGoalRequest,
    ) -> anyhow::Result<ThreadGoal> {
        if !self.enabled(Feature::GoalMode) {
            anyhow::bail!("goal_mode feature is disabled");
        }

        validate_goal_budget(request.token_budget.flatten())?;
        let state_db = self.require_state_db_for_thread_goals().await?;
        let replacing_goal = request.objective.is_some();
        if !replacing_goal {
            self.account_thread_goal_wall_clock_usage(
                &state_db,
                codex_state::ThreadGoalAccountingMode::ActiveOnly,
            )
            .await?;
        }
        let previous_status = if !replacing_goal {
            state_db
                .get_thread_goal(self.conversation_id)
                .await?
                .map(|goal| goal.status)
        } else {
            None
        };
        let goal = if let Some(objective) = request.objective {
            let objective = objective.trim();
            if objective.is_empty() {
                anyhow::bail!("goal objective must not be empty");
            }
            state_db
                .replace_thread_goal(
                    self.conversation_id,
                    objective,
                    request
                        .status
                        .map(state_goal_status_from_protocol)
                        .unwrap_or(codex_state::ThreadGoalStatus::Active),
                    request.token_budget.flatten(),
                )
                .await?
        } else {
            let status = request.status.map(state_goal_status_from_protocol);
            state_db
                .update_thread_goal(
                    self.conversation_id,
                    codex_state::ThreadGoalUpdate {
                        status,
                        token_budget: request.token_budget,
                    },
                )
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "cannot update goal for thread {}: no goal exists",
                        self.conversation_id
                    )
                })?
        };

        let goal_status = goal.status;
        let goal_created_at_ms = goal.created_at.timestamp_millis();
        let goal = protocol_goal_from_state(goal);
        self.thread_goal_may_exist.store(true, Ordering::SeqCst);
        *self.thread_goal_cache.lock().await = Some(goal.clone());
        if replacing_goal
            || (goal_status == codex_state::ThreadGoalStatus::Active
                && previous_status
                    .is_some_and(|status| status != codex_state::ThreadGoalStatus::Active))
        {
            let current_token_usage = self.total_token_usage().await.unwrap_or_default();
            turn_context
                .goal_accounting
                .reset_baseline(current_token_usage)
                .await;
            if goal_status == codex_state::ThreadGoalStatus::Active {
                self.thread_goal_wall_clock_accounting
                    .mark_active_goal(goal_created_at_ms)
                    .await;
                turn_context
                    .goal_accounting
                    .mark_active_goal(goal_created_at_ms)
                    .await;
            } else {
                self.thread_goal_wall_clock_accounting
                    .clear_active_goal()
                    .await;
                turn_context.goal_accounting.clear_active_goal().await;
            }
        }
        if goal.status == ThreadGoalStatus::Complete {
            turn_context.goal_accounting.mark_completed_this_turn();
        } else if matches!(
            goal.status,
            ThreadGoalStatus::Paused | ThreadGoalStatus::BudgetLimited
        ) && previous_status == Some(codex_state::ThreadGoalStatus::Active)
        {
            turn_context.goal_accounting.mark_stopped_this_turn();
        }
        self.send_event(
            turn_context,
            EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: self.conversation_id,
                turn_id: Some(turn_context.sub_id.clone()),
                goal: goal.clone(),
            }),
        )
        .await;
        Ok(goal)
    }

    pub(crate) async fn mark_thread_goal_turn_started(
        &self,
        turn_context: &TurnContext,
        token_usage: TokenUsage,
    ) {
        turn_context
            .goal_accounting
            .mark_turn_started(token_usage)
            .await;

        if !self.enabled(Feature::GoalMode) {
            return;
        }
        if should_ignore_goal_for_mode(turn_context.collaboration_mode.mode) {
            self.thread_goal_wall_clock_accounting
                .clear_active_goal()
                .await;
            turn_context.goal_accounting.clear_active_goal().await;
            return;
        }
        let state_db = match self.state_db_for_thread_goals().await {
            Ok(Some(state_db)) => state_db,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!("failed to open state db at turn start: {err}");
                return;
            }
        };
        match state_db.get_thread_goal(self.conversation_id).await {
            Ok(Some(goal)) if goal.status == codex_state::ThreadGoalStatus::Active => {
                self.thread_goal_may_exist.store(true, Ordering::SeqCst);
                *self.thread_goal_cache.lock().await = Some(protocol_goal_from_state(goal.clone()));
                turn_context
                    .goal_accounting
                    .mark_active_goal(goal.created_at.timestamp_millis())
                    .await;
                self.thread_goal_wall_clock_accounting
                    .mark_active_goal(goal.created_at.timestamp_millis())
                    .await;
            }
            Ok(Some(_)) | Ok(None) => {
                self.thread_goal_wall_clock_accounting
                    .clear_active_goal()
                    .await;
            }
            Err(err) => {
                tracing::warn!("failed to read thread goal at turn start: {err}");
            }
        }
    }

    pub(crate) async fn account_thread_goal_progress(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        boundary: GoalAccountingBoundary,
    ) -> anyhow::Result<()> {
        let clear_terminal_accounting = matches!(boundary, GoalAccountingBoundary::Turn)
            && (turn_context.goal_accounting.completed_this_turn()
                || turn_context.goal_accounting.stopped_this_turn());
        let result = self
            .account_thread_goal_progress_inner(turn_context, boundary)
            .await;
        if clear_terminal_accounting {
            turn_context.goal_accounting.clear_completed_this_turn();
            turn_context.goal_accounting.clear_stopped_this_turn();
        }
        result
    }

    async fn account_thread_goal_progress_inner(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        boundary: GoalAccountingBoundary,
    ) -> anyhow::Result<()> {
        if !self.enabled(Feature::GoalMode) {
            return Ok(());
        }
        if should_ignore_goal_for_mode(turn_context.collaboration_mode.mode) {
            return Ok(());
        }
        let _accounting_guard = turn_context.goal_accounting.accounting_lock.lock().await;
        let _wall_clock_guard = self
            .thread_goal_wall_clock_accounting
            .accounting_lock
            .lock()
            .await;
        let current_token_usage = self.total_token_usage().await.unwrap_or_default();
        let time_delta_seconds = self
            .thread_goal_wall_clock_accounting
            .time_delta_since_last_accounting()
            .await;
        let token_delta = turn_context
            .goal_accounting
            .token_delta_since_last_accounting(current_token_usage.clone())
            .await;
        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(());
        };
        if self
            .refresh_goal_accounting_baseline_if_needed(
                turn_context,
                &state_db,
                current_token_usage.clone(),
                boundary,
            )
            .await?
        {
            return Ok(());
        }
        if time_delta_seconds == 0 && token_delta <= 0 {
            if let Some(goal) = state_db.get_thread_goal(self.conversation_id).await? {
                let status = goal.status;
                *self.thread_goal_cache.lock().await = Some(protocol_goal_from_state(goal));
                if status == codex_state::ThreadGoalStatus::BudgetLimited {
                    self.abort_all_tasks_without_goal_accounting_from_current_turn(
                        TurnAbortReason::BudgetLimited,
                        &turn_context.sub_id,
                    )
                    .await;
                }
            }
            return Ok(());
        }
        let mode = match boundary {
            GoalAccountingBoundary::Tool => {
                if turn_context.goal_accounting.completed_this_turn() {
                    codex_state::ThreadGoalAccountingMode::ActiveOrComplete
                } else if turn_context.goal_accounting.stopped_this_turn() {
                    codex_state::ThreadGoalAccountingMode::ActiveOrStopped
                } else {
                    codex_state::ThreadGoalAccountingMode::ActiveOnly
                }
            }
            GoalAccountingBoundary::Turn => {
                let completed_this_turn = turn_context.goal_accounting.completed_this_turn();
                let stopped_this_turn = turn_context.goal_accounting.stopped_this_turn();
                match state_db.get_thread_goal(self.conversation_id).await? {
                    Some(goal) => {
                        let status = goal.status;
                        *self.thread_goal_cache.lock().await = Some(protocol_goal_from_state(goal));
                        if completed_this_turn
                            || (turn_context.goal_accounting.active_this_turn()
                                && status == codex_state::ThreadGoalStatus::Complete)
                        {
                            codex_state::ThreadGoalAccountingMode::ActiveOrComplete
                        } else if stopped_this_turn
                            || (turn_context.goal_accounting.active_this_turn()
                                && matches!(
                                    status,
                                    codex_state::ThreadGoalStatus::Paused
                                        | codex_state::ThreadGoalStatus::BudgetLimited
                                ))
                        {
                            codex_state::ThreadGoalAccountingMode::ActiveOrStopped
                        } else {
                            codex_state::ThreadGoalAccountingMode::ActiveOnly
                        }
                    }
                    None => codex_state::ThreadGoalAccountingMode::ActiveOnly,
                }
            }
        };
        let outcome = state_db
            .account_thread_goal_usage(self.conversation_id, time_delta_seconds, token_delta, mode)
            .await?;
        let goal = match outcome {
            codex_state::ThreadGoalAccountingOutcome::Updated(goal) => {
                let clear_active_goal = turn_context.goal_accounting.active_this_turn()
                    && matches!(
                        goal.status,
                        codex_state::ThreadGoalStatus::Paused
                            | codex_state::ThreadGoalStatus::BudgetLimited
                            | codex_state::ThreadGoalStatus::Complete
                    );
                turn_context
                    .goal_accounting
                    .mark_accounted(current_token_usage)
                    .await;
                self.thread_goal_wall_clock_accounting
                    .mark_accounted(time_delta_seconds)
                    .await;
                if clear_active_goal {
                    self.thread_goal_wall_clock_accounting
                        .clear_active_goal()
                        .await;
                    turn_context.goal_accounting.clear_active_goal().await;
                }
                goal
            }
            codex_state::ThreadGoalAccountingOutcome::Unchanged(goal) => {
                if let Some(goal) = goal {
                    *self.thread_goal_cache.lock().await = Some(protocol_goal_from_state(goal));
                }
                return Ok(());
            }
        };
        let status = goal.status;
        let goal = protocol_goal_from_state(goal);
        *self.thread_goal_cache.lock().await = Some(goal.clone());
        self.send_event(
            turn_context,
            EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: self.conversation_id,
                turn_id: Some(turn_context.sub_id.clone()),
                goal,
            }),
        )
        .await;
        if status == codex_state::ThreadGoalStatus::BudgetLimited {
            self.abort_all_tasks_without_goal_accounting_from_current_turn(
                TurnAbortReason::BudgetLimited,
                &turn_context.sub_id,
            )
            .await;
        }
        Ok(())
    }

    pub(crate) async fn account_active_thread_goal_progress(
        self: &Arc<Self>,
    ) -> anyhow::Result<()> {
        let turn_context = {
            let active = self.active_turn.lock().await;
            active.as_ref().and_then(|active_turn| {
                active_turn
                    .tasks
                    .first()
                    .map(|(_, task)| Arc::clone(&task.turn_context))
            })
        };
        let Some(turn_context) = turn_context else {
            return Ok(());
        };
        self.account_thread_goal_progress(turn_context.as_ref(), GoalAccountingBoundary::Tool)
            .await
    }

    async fn account_thread_goal_wall_clock_usage(
        &self,
        state_db: &StateDbHandle,
        mode: codex_state::ThreadGoalAccountingMode,
    ) -> anyhow::Result<Option<ThreadGoal>> {
        let _wall_clock_guard = self
            .thread_goal_wall_clock_accounting
            .accounting_lock
            .lock()
            .await;
        let time_delta_seconds = self
            .thread_goal_wall_clock_accounting
            .time_delta_since_last_accounting()
            .await;
        if time_delta_seconds == 0 {
            return Ok(None);
        }

        match state_db
            .account_thread_goal_usage(
                self.conversation_id,
                time_delta_seconds,
                /*token_delta*/ 0,
                mode,
            )
            .await?
        {
            codex_state::ThreadGoalAccountingOutcome::Updated(goal) => {
                self.thread_goal_wall_clock_accounting
                    .mark_accounted(time_delta_seconds)
                    .await;
                let goal = protocol_goal_from_state(goal);
                *self.thread_goal_cache.lock().await = Some(goal.clone());
                Ok(Some(goal))
            }
            codex_state::ThreadGoalAccountingOutcome::Unchanged(goal) => {
                self.thread_goal_wall_clock_accounting
                    .reset_baseline()
                    .await;
                self.thread_goal_wall_clock_accounting
                    .clear_active_goal()
                    .await;
                if let Some(goal) = goal {
                    let goal = protocol_goal_from_state(goal);
                    *self.thread_goal_cache.lock().await = Some(goal.clone());
                    return Ok(Some(goal));
                }
                Ok(None)
            }
        }
    }

    async fn refresh_goal_accounting_baseline_if_needed(
        &self,
        turn_context: &TurnContext,
        state_db: &StateDbHandle,
        current_token_usage: TokenUsage,
        _boundary: GoalAccountingBoundary,
    ) -> anyhow::Result<bool> {
        let Some(goal) = state_db.get_thread_goal(self.conversation_id).await? else {
            return Ok(false);
        };
        let created_at_ms = goal.created_at.timestamp_millis();
        let tracked_created_at_ms = turn_context
            .goal_accounting
            .active_goal_created_at_ms()
            .await;
        let goal_status = goal.status;
        *self.thread_goal_cache.lock().await = Some(protocol_goal_from_state(goal));

        if goal_status == codex_state::ThreadGoalStatus::Active {
            if tracked_created_at_ms == Some(created_at_ms) {
                return Ok(false);
            }
            turn_context
                .goal_accounting
                .reset_baseline(current_token_usage)
                .await;
            self.thread_goal_wall_clock_accounting
                .mark_active_goal(created_at_ms)
                .await;
            turn_context
                .goal_accounting
                .mark_active_goal(created_at_ms)
                .await;
            return Ok(true);
        }

        if turn_context.goal_accounting.active_this_turn()
            && tracked_created_at_ms.is_some_and(|tracked| tracked != created_at_ms)
        {
            turn_context
                .goal_accounting
                .reset_baseline(current_token_usage)
                .await;
            self.thread_goal_wall_clock_accounting
                .clear_active_goal()
                .await;
            turn_context.goal_accounting.clear_active_goal().await;
            return Ok(true);
        }

        Ok(false)
    }

    pub(crate) async fn pause_active_thread_goal_for_interrupt(&self) -> anyhow::Result<()> {
        self.pause_active_thread_goal_with_event_id(uuid::Uuid::new_v4().to_string())
            .await
    }

    async fn pause_active_thread_goal_with_event_id(&self, event_id: String) -> anyhow::Result<()> {
        if !self.enabled(Feature::GoalMode) {
            return Ok(());
        }

        let _continuation_guard = self.goal_continuation_lock.lock().await;
        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(());
        };
        let Some(goal) = state_db
            .pause_active_thread_goal(self.conversation_id)
            .await?
        else {
            return Ok(());
        };
        self.clear_queued_response_items_for_next_turn().await;
        let goal = protocol_goal_from_state(goal);
        *self.thread_goal_cache.lock().await = Some(goal.clone());
        self.thread_goal_wall_clock_accounting
            .clear_active_goal()
            .await;
        self.send_event_raw(Event {
            id: event_id,
            msg: EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: self.conversation_id,
                turn_id: None,
                goal,
            }),
        })
        .await;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn queue_goal_continuation_if_active(self: &Arc<Self>) {
        let _continuation_guard = self.goal_continuation_lock.lock().await;
        let Some(items) = self.goal_continuation_items_if_active().await else {
            return;
        };
        self.queue_response_items_for_next_turn(items).await;
        tracing::info!("queued active goal continuation");
    }

    pub(crate) async fn goal_continuation_items_if_active(
        self: &Arc<Self>,
    ) -> Option<Vec<ResponseInputItem>> {
        if !self.enabled(Feature::GoalMode) {
            return None;
        }
        if should_ignore_goal_for_mode(self.collaboration_mode().await.mode) {
            tracing::debug!("skipping active goal continuation while plan mode is active");
            return None;
        }
        if self.has_active_turn().await {
            tracing::debug!("skipping active goal continuation because a turn is already active");
            return None;
        }
        if self.has_queued_response_items_for_next_turn().await {
            tracing::debug!(
                "skipping active goal continuation because pending next-turn input already exists"
            );
            return None;
        }
        if self.has_trigger_turn_mailbox_items().await {
            tracing::debug!(
                "skipping active goal continuation because trigger-turn mailbox input is pending"
            );
            return None;
        }
        let goal = match self.get_thread_goal().await {
            Ok(Some(goal)) => goal,
            Ok(None) => {
                tracing::debug!("skipping active goal continuation because no goal is set");
                return None;
            }
            Err(err) => {
                tracing::warn!("failed to read thread goal for continuation: {err}");
                return None;
            }
        };
        if goal.status != ThreadGoalStatus::Active {
            tracing::debug!(status = ?goal.status, "skipping inactive thread goal");
            return None;
        }
        if self.has_active_turn().await
            || self.has_queued_response_items_for_next_turn().await
            || self.has_trigger_turn_mailbox_items().await
        {
            tracing::debug!("skipping active goal continuation because pending work appeared");
            return None;
        }
        let goal = match self.get_thread_goal().await {
            Ok(Some(goal)) if goal.status == ThreadGoalStatus::Active => goal,
            Ok(Some(goal)) => {
                tracing::debug!(
                    status = ?goal.status,
                    "skipping thread goal that changed before continuation queueing"
                );
                return None;
            }
            Ok(None) => {
                tracing::debug!(
                    "skipping thread goal that disappeared before continuation queueing"
                );
                return None;
            }
            Err(err) => {
                tracing::warn!("failed to re-read thread goal for continuation: {err}");
                return None;
            }
        };
        Some(vec![ResponseInputItem::Message {
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: continuation_prompt(&goal),
            }],
        }])
    }
}

impl Session {
    async fn state_db_for_thread_goals(&self) -> anyhow::Result<Option<StateDbHandle>> {
        let config = self.get_config().await;
        if config.ephemeral {
            return Ok(None);
        }

        self.try_ensure_rollout_materialized()
            .await
            .context("failed to materialize rollout before opening state db for thread goals")?;

        if let Some(state_db) = self.state_db() {
            return Ok(Some(state_db));
        }
        if let Some(state_db) = self.thread_goal_state_db.lock().await.clone() {
            return Ok(Some(state_db));
        }

        let state_db = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .context("failed to initialize sqlite state db for thread goals")?;
        *self.thread_goal_state_db.lock().await = Some(state_db.clone());
        Ok(Some(state_db))
    }

    async fn require_state_db_for_thread_goals(&self) -> anyhow::Result<StateDbHandle> {
        self.state_db_for_thread_goals().await?.ok_or_else(|| {
            anyhow::anyhow!("thread goals require a persisted thread; this thread is ephemeral")
        })
    }
}

fn should_ignore_goal_for_mode(mode: ModeKind) -> bool {
    mode == ModeKind::Plan
}

fn continuation_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let remaining_tokens = goal
        .token_budget
        .map(|budget| (budget - goal.tokens_used).max(0).to_string())
        .unwrap_or_else(|| "unbounded".to_string());

    format!(
        r#"Continue working toward the active thread goal.

Objective:
{objective}

Budget:
- Time spent pursuing goal: {time_used_seconds} seconds
- Tokens used: {tokens_used}
- Token budget: {token_budget}
- Tokens remaining: {remaining_tokens}

Avoid repeating work that is already done. Choose the next concrete action toward the objective.

Before deciding that the goal is achieved, perform a completion audit against the actual current state:
- Restate the objective as concrete deliverables or success criteria.
- Inspect the relevant files, command output, test results, PR state, or other real evidence.
- Compare each deliverable to that evidence and identify any missing, incomplete, or unverified part.
- Treat uncertainty as not achieved; do more verification or continue the work.

Do not rely on intent, partial progress, elapsed effort, memory of earlier work, or a plausible final answer as proof of completion. Only mark the goal achieved when the audit shows that the objective has actually been achieved and no required work remains. If any requirement is missing, incomplete, or unverified, keep working instead of marking the goal complete. If the objective is achieved, call set_goal with status "complete" and omit objective so usage accounting is preserved. Report the final elapsed time, and if the achieved goal has a token budget, report the final consumed token budget to the user after set_goal succeeds.

If the goal has not been achieved and cannot be achieved within the remaining budget, or the remaining budget is too small for productive continuation, call set_goal with status "budgetLimited" and omit objective. Do not mark a goal complete merely because the budget is nearly exhausted or because you are stopping work. If the goal is otherwise blocked and cannot continue productively for a non-budget reason, call set_goal with status "paused" and omit objective."#,
        objective = goal.objective,
        tokens_used = goal.tokens_used,
        time_used_seconds = goal.time_used_seconds,
    )
}

pub(crate) fn is_goal_continuation_item(item: &ResponseInputItem) -> bool {
    const GOAL_CONTINUATION_PROMPT_PREFIX: &str = "Continue working toward the active thread goal.";

    let ResponseInputItem::Message { role, content } = item else {
        return false;
    };
    role == "developer"
        && content.iter().any(|item| {
            matches!(
                item,
                ContentItem::InputText { text }
                    if text.starts_with(GOAL_CONTINUATION_PROMPT_PREFIX)
            )
        })
}

pub(crate) fn protocol_goal_from_state(goal: codex_state::ThreadGoal) -> ThreadGoal {
    ThreadGoal {
        thread_id: goal.thread_id,
        objective: goal.objective,
        status: protocol_goal_status_from_state(goal.status),
        token_budget: goal.token_budget,
        tokens_used: goal.tokens_used,
        time_used_seconds: goal.time_used_seconds,
        created_at: goal.created_at.timestamp(),
        updated_at: goal.updated_at.timestamp(),
    }
}

pub(crate) fn protocol_goal_status_from_state(
    status: codex_state::ThreadGoalStatus,
) -> ThreadGoalStatus {
    match status {
        codex_state::ThreadGoalStatus::Active => ThreadGoalStatus::Active,
        codex_state::ThreadGoalStatus::Paused => ThreadGoalStatus::Paused,
        codex_state::ThreadGoalStatus::BudgetLimited => ThreadGoalStatus::BudgetLimited,
        codex_state::ThreadGoalStatus::Complete => ThreadGoalStatus::Complete,
    }
}

pub(crate) fn state_goal_status_from_protocol(
    status: ThreadGoalStatus,
) -> codex_state::ThreadGoalStatus {
    match status {
        ThreadGoalStatus::Active => codex_state::ThreadGoalStatus::Active,
        ThreadGoalStatus::Paused => codex_state::ThreadGoalStatus::Paused,
        ThreadGoalStatus::BudgetLimited => codex_state::ThreadGoalStatus::BudgetLimited,
        ThreadGoalStatus::Complete => codex_state::ThreadGoalStatus::Complete,
    }
}

pub(crate) fn validate_goal_budget(value: Option<i64>) -> anyhow::Result<()> {
    if let Some(value) = value
        && value <= 0
    {
        anyhow::bail!("goal budgets must be positive when provided");
    }
    Ok(())
}

pub(crate) fn goal_token_delta_for_usage(usage: &TokenUsage) -> i64 {
    usage
        .non_cached_input()
        .saturating_add(usage.output_tokens.max(0))
}

#[cfg(test)]
mod tests {
    use super::GoalWallClockAccountingState;
    use super::should_ignore_goal_for_mode;
    use codex_protocol::config_types::ModeKind;
    use std::time::Duration;
    use std::time::Instant;

    #[test]
    fn goal_continuation_is_ignored_only_in_plan_mode() {
        assert!(should_ignore_goal_for_mode(ModeKind::Plan));
        assert!(!should_ignore_goal_for_mode(ModeKind::Default));
        assert!(!should_ignore_goal_for_mode(ModeKind::PairProgramming));
        assert!(!should_ignore_goal_for_mode(ModeKind::Execute));
    }

    #[tokio::test]
    async fn goal_accounting_preserves_fractional_seconds_between_boundaries() {
        let accounting = GoalWallClockAccountingState::new();
        *accounting.last_accounted_at.lock().await = Instant::now() - Duration::from_millis(2500);

        let delta = accounting.time_delta_since_last_accounting().await;
        assert_eq!(2, delta);

        accounting.mark_accounted(delta).await;
        let elapsed = accounting.last_accounted_at.lock().await.elapsed();
        assert!(
            elapsed >= Duration::from_millis(400),
            "expected subsecond remainder to be preserved, got {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1500),
            "expected only subsecond-ish remainder after accounting, got {elapsed:?}"
        );
    }
}
