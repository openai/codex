//! Core support for persisted thread goals.
//!
//! This module bridges core sessions and the state-db goal table. It validates
//! goal mutations, converts between state and protocol shapes, emits goal-update
//! events, and owns helper hooks used by goal lifecycle behavior.

use crate::StateDbHandle;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
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
use codex_rollout::state_db::reconcile_rollout;
use codex_utils_template::Template;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

pub(crate) struct SetGoalRequest {
    pub(crate) objective: Option<String>,
    pub(crate) status: Option<ThreadGoalStatus>,
    pub(crate) token_budget: Option<Option<i64>>,
}

pub(crate) struct CreateGoalRequest {
    pub(crate) objective: String,
    pub(crate) token_budget: Option<i64>,
}

static CONTINUATION_PROMPT_TEMPLATE: LazyLock<Template> =
    LazyLock::new(
        || match Template::parse(include_str!("../templates/goals/continuation.md")) {
            Ok(template) => template,
            Err(err) => panic!("embedded goals/continuation.md template is invalid: {err}"),
        },
    );

static BUDGET_LIMIT_PROMPT_TEMPLATE: LazyLock<Template> =
    LazyLock::new(
        || match Template::parse(include_str!("../templates/goals/budget_limit.md")) {
            Ok(template) => template,
            Err(err) => panic!("embedded goals/budget_limit.md template is invalid: {err}"),
        },
    );

#[derive(Clone, Copy)]
pub(crate) enum BudgetLimitSteering {
    Allowed,
    Suppressed,
}

pub(crate) struct GoalRuntimeState {
    pub(crate) state_db: Mutex<Option<StateDbHandle>>,
    pub(crate) budget_limit_reported_goal_id: Mutex<Option<String>>,
    continuation_turn_ids: Mutex<HashSet<String>>,
    pub(crate) wall_clock_accounting: GoalWallClockAccountingState,
    pub(crate) continuation_lock: Semaphore,
    pub(crate) continuation_suppressed: AtomicBool,
}

impl GoalRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            state_db: Mutex::new(None),
            budget_limit_reported_goal_id: Mutex::new(None),
            continuation_turn_ids: Mutex::new(HashSet::new()),
            wall_clock_accounting: GoalWallClockAccountingState::new(),
            continuation_lock: Semaphore::new(/*permits*/ 1),
            continuation_suppressed: AtomicBool::new(false),
        }
    }
}

#[derive(Debug)]
pub(crate) struct GoalTurnAccountingState {
    accounting_lock: Semaphore,
    inner: Mutex<GoalTurnAccountingSnapshot>,
}

#[derive(Debug)]
struct GoalTurnAccountingSnapshot {
    last_accounted_token_usage: TokenUsage,
    active_goal_id: Option<String>,
}

impl GoalTurnAccountingState {
    pub(crate) fn new() -> Self {
        Self {
            accounting_lock: Semaphore::new(/*permits*/ 1),
            inner: Mutex::new(GoalTurnAccountingSnapshot {
                last_accounted_token_usage: TokenUsage::default(),
                active_goal_id: None,
            }),
        }
    }

    pub(crate) async fn mark_turn_started(&self, token_usage: TokenUsage) {
        let mut inner = self.inner.lock().await;
        inner.last_accounted_token_usage = token_usage;
        inner.active_goal_id = None;
    }

    async fn lock(&self) -> tokio::sync::MutexGuard<'_, GoalTurnAccountingSnapshot> {
        self.inner.lock().await
    }

    async fn accounting_permit(&self) -> anyhow::Result<SemaphorePermit<'_>> {
        self.accounting_lock
            .acquire()
            .await
            .context("goal turn accounting semaphore closed")
    }
}

impl GoalTurnAccountingSnapshot {
    fn mark_active_goal(&mut self, goal_id: impl Into<String>) {
        self.active_goal_id = Some(goal_id.into());
    }

    fn active_this_turn(&self) -> bool {
        self.active_goal_id.is_some()
    }

    fn active_goal_id(&self) -> Option<String> {
        self.active_goal_id.clone()
    }

    fn clear_active_goal(&mut self) {
        self.active_goal_id = None;
    }

    fn reset_baseline(&mut self, token_usage: TokenUsage) {
        self.last_accounted_token_usage = token_usage;
    }

    fn token_delta_since_last_accounting(&self, current: &TokenUsage) -> i64 {
        let last = &self.last_accounted_token_usage;
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

    fn mark_accounted(&mut self, current: TokenUsage) {
        self.last_accounted_token_usage = current;
    }
}

#[derive(Debug)]
pub(crate) struct GoalWallClockAccountingState {
    accounting_lock: Semaphore,
    inner: Mutex<GoalWallClockAccountingSnapshot>,
}

#[derive(Debug)]
struct GoalWallClockAccountingSnapshot {
    last_accounted_at: Instant,
    active_goal_id: Option<String>,
}

impl GoalWallClockAccountingState {
    pub(crate) fn new() -> Self {
        Self {
            accounting_lock: Semaphore::new(/*permits*/ 1),
            inner: Mutex::new(GoalWallClockAccountingSnapshot {
                last_accounted_at: Instant::now(),
                active_goal_id: None,
            }),
        }
    }

    async fn lock(&self) -> tokio::sync::MutexGuard<'_, GoalWallClockAccountingSnapshot> {
        self.inner.lock().await
    }

    async fn accounting_permit(&self) -> anyhow::Result<SemaphorePermit<'_>> {
        self.accounting_lock
            .acquire()
            .await
            .context("goal wall-clock accounting semaphore closed")
    }
}

impl GoalWallClockAccountingSnapshot {
    fn time_delta_since_last_accounting(&self) -> i64 {
        let last = self.last_accounted_at;
        i64::try_from(last.elapsed().as_secs()).unwrap_or(i64::MAX)
    }

    fn mark_accounted(&mut self) {
        self.reset_baseline();
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
    }

    fn active_goal_id(&self) -> Option<String> {
        self.active_goal_id.clone()
    }
}

impl Session {
    pub(crate) async fn get_thread_goal(&self) -> anyhow::Result<Option<ThreadGoal>> {
        if !self.enabled(Feature::Goals) {
            return Ok(None);
        }

        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(None);
        };
        state_db
            .get_thread_goal(self.conversation_id)
            .await
            .map(|goal| goal.map(protocol_goal_from_state))
    }

    pub(crate) async fn set_thread_goal(
        &self,
        turn_context: &TurnContext,
        request: SetGoalRequest,
    ) -> anyhow::Result<ThreadGoal> {
        if !self.enabled(Feature::Goals) {
            anyhow::bail!("goals feature is disabled");
        }

        let SetGoalRequest {
            objective,
            status,
            token_budget,
        } = request;
        validate_goal_budget(token_budget.flatten())?;
        let state_db = self.require_state_db_for_thread_goals().await?;
        let objective = objective.map(|objective| objective.trim().to_string());
        if let Some(objective) = objective.as_deref()
            && objective.is_empty()
        {
            anyhow::bail!("goal objective must not be empty");
        }

        self.account_thread_goal_wall_clock_usage(
            &state_db,
            codex_state::ThreadGoalAccountingMode::ActiveOnly,
        )
        .await?;
        let mut replacing_goal = objective.is_some();
        let previous_status;
        let goal = if let Some(objective) = objective.as_deref() {
            let existing_goal = state_db.get_thread_goal(self.conversation_id).await?;
            previous_status = existing_goal.as_ref().map(|goal| goal.status);
            let same_nonterminal_goal = existing_goal.as_ref().is_some_and(|goal| {
                goal.objective == objective
                    && goal.status != codex_state::ThreadGoalStatus::Complete
            });
            if same_nonterminal_goal {
                replacing_goal = false;
                state_db
                    .update_thread_goal(
                        self.conversation_id,
                        codex_state::ThreadGoalUpdate {
                            status: status
                                .map(state_goal_status_from_protocol)
                                .or(Some(codex_state::ThreadGoalStatus::Active)),
                            token_budget,
                            expected_goal_id: existing_goal
                                .as_ref()
                                .map(|goal| goal.goal_id.clone()),
                        },
                    )
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "cannot update goal for thread {}: no goal exists",
                            self.conversation_id
                        )
                    })?
            } else {
                state_db
                    .replace_thread_goal(
                        self.conversation_id,
                        objective,
                        status
                            .map(state_goal_status_from_protocol)
                            .unwrap_or(codex_state::ThreadGoalStatus::Active),
                        token_budget.flatten(),
                    )
                    .await?
            }
        } else {
            let existing_goal = state_db.get_thread_goal(self.conversation_id).await?;
            previous_status = existing_goal.as_ref().map(|goal| goal.status);
            let expected_goal_id = existing_goal.map(|goal| goal.goal_id);
            let status = status.map(state_goal_status_from_protocol);
            state_db
                .update_thread_goal(
                    self.conversation_id,
                    codex_state::ThreadGoalUpdate {
                        status,
                        token_budget,
                        expected_goal_id,
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
        let goal_id = goal.goal_id.clone();
        let goal = protocol_goal_from_state(goal);
        self.reset_thread_goal_continuation_suppression();
        *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;
        let newly_active_goal = goal_status == codex_state::ThreadGoalStatus::Active
            && (replacing_goal
                || previous_status
                    .is_some_and(|status| status != codex_state::ThreadGoalStatus::Active));
        if newly_active_goal {
            let current_token_usage = self.total_token_usage().await.unwrap_or_default();
            {
                let mut turn_accounting = turn_context.goal_accounting.lock().await;
                turn_accounting.reset_baseline(current_token_usage);
                turn_accounting.mark_active_goal(goal_id.clone());
            }
            self.goal_runtime
                .wall_clock_accounting
                .lock()
                .await
                .mark_active_goal(goal_id);
        } else if goal_status != codex_state::ThreadGoalStatus::Active {
            self.clear_active_goal_accounting(turn_context).await;
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

    pub(crate) async fn create_thread_goal(
        &self,
        turn_context: &TurnContext,
        request: CreateGoalRequest,
    ) -> anyhow::Result<ThreadGoal> {
        if !self.enabled(Feature::Goals) {
            anyhow::bail!("goals feature is disabled");
        }

        let CreateGoalRequest {
            objective,
            token_budget,
        } = request;
        validate_goal_budget(token_budget)?;
        let objective = objective.trim();
        if objective.is_empty() {
            anyhow::bail!("goal objective must not be empty");
        }

        let state_db = self.require_state_db_for_thread_goals().await?;
        self.account_thread_goal_wall_clock_usage(
            &state_db,
            codex_state::ThreadGoalAccountingMode::ActiveOnly,
        )
        .await?;
        let goal = state_db
            .insert_thread_goal(
                self.conversation_id,
                objective,
                codex_state::ThreadGoalStatus::Active,
                token_budget,
            )
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot create a new goal because thread {} already has a goal",
                    self.conversation_id
                )
            })?;

        let goal_id = goal.goal_id.clone();
        let goal = protocol_goal_from_state(goal);
        self.reset_thread_goal_continuation_suppression();
        *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;

        let current_token_usage = self.total_token_usage().await.unwrap_or_default();
        {
            let mut turn_accounting = turn_context.goal_accounting.lock().await;
            turn_accounting.reset_baseline(current_token_usage);
            turn_accounting.mark_active_goal(goal_id.clone());
        }
        self.goal_runtime
            .wall_clock_accounting
            .lock()
            .await
            .mark_active_goal(goal_id);

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

    pub(crate) async fn clear_cached_thread_goal_after_delete(&self) {
        self.clear_stopped_thread_goal_runtime_state().await;
    }

    pub(crate) async fn clear_stopped_thread_goal_runtime_state(&self) {
        self.reset_thread_goal_continuation_suppression();
        *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;
        for turn_context in self.active_turn_contexts().await {
            turn_context
                .goal_accounting
                .lock()
                .await
                .clear_active_goal();
        }
        self.goal_runtime
            .wall_clock_accounting
            .lock()
            .await
            .clear_active_goal();
    }

    async fn clear_active_goal_accounting(&self, turn_context: &TurnContext) {
        turn_context
            .goal_accounting
            .lock()
            .await
            .clear_active_goal();
        self.goal_runtime
            .wall_clock_accounting
            .lock()
            .await
            .clear_active_goal();
    }

    async fn active_turn_context(&self) -> Option<Arc<TurnContext>> {
        self.active_turn_contexts().await.into_iter().next()
    }

    async fn active_turn_contexts(&self) -> Vec<Arc<TurnContext>> {
        let active = self.active_turn.lock().await;
        active
            .as_ref()
            .map(|active_turn| {
                active_turn
                    .tasks
                    .values()
                    .map(|task| Arc::clone(&task.turn_context))
                    .collect()
            })
            .unwrap_or_default()
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

        if !self.enabled(Feature::Goals) {
            return;
        }
        if should_ignore_goal_for_mode(turn_context.collaboration_mode.mode) {
            self.clear_active_goal_accounting(turn_context).await;
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
            Ok(Some(goal))
                if matches!(
                    goal.status,
                    codex_state::ThreadGoalStatus::Active
                        | codex_state::ThreadGoalStatus::BudgetLimited
                ) =>
            {
                turn_context
                    .goal_accounting
                    .lock()
                    .await
                    .mark_active_goal(goal.goal_id.clone());
                self.goal_runtime
                    .wall_clock_accounting
                    .lock()
                    .await
                    .mark_active_goal(goal.goal_id);
            }
            Ok(Some(_)) | Ok(None) => {
                self.goal_runtime
                    .wall_clock_accounting
                    .lock()
                    .await
                    .clear_active_goal();
            }
            Err(err) => {
                tracing::warn!("failed to read thread goal at turn start: {err}");
            }
        }
    }

    pub(crate) fn reset_thread_goal_continuation_suppression(&self) {
        self.goal_runtime
            .continuation_suppressed
            .store(false, Ordering::SeqCst);
    }

    pub(crate) async fn mark_thread_goal_continuation_turn_started(&self, turn_id: String) {
        self.goal_runtime
            .continuation_turn_ids
            .lock()
            .await
            .insert(turn_id);
    }

    async fn take_thread_goal_continuation_turn(&self, turn_id: &str) -> bool {
        self.goal_runtime
            .continuation_turn_ids
            .lock()
            .await
            .remove(turn_id)
    }

    pub(crate) async fn finish_thread_goal_turn(
        &self,
        turn_context: &TurnContext,
        turn_completed: bool,
        turn_tool_calls: u64,
    ) {
        if turn_completed
            && let Err(err) = self
                .account_thread_goal_progress(turn_context, BudgetLimitSteering::Suppressed)
                .await
        {
            tracing::warn!("failed to account thread goal progress at turn end: {err}");
        }

        if self
            .take_thread_goal_continuation_turn(&turn_context.sub_id)
            .await
            && turn_tool_calls == 0
        {
            self.goal_runtime
                .continuation_suppressed
                .store(true, Ordering::SeqCst);
        }
    }

    pub(crate) async fn handle_thread_goal_task_abort(
        &self,
        turn_context: Option<&TurnContext>,
        reason: TurnAbortReason,
    ) {
        if let Some(turn_context) = turn_context {
            self.take_thread_goal_continuation_turn(&turn_context.sub_id)
                .await;
            if let Err(err) = self
                .account_thread_goal_progress(turn_context, BudgetLimitSteering::Suppressed)
                .await
            {
                tracing::warn!("failed to account thread goal progress after abort: {err}");
            }
        }

        if reason == TurnAbortReason::Interrupted
            && let Err(err) = self.pause_active_thread_goal_for_interrupt().await
        {
            tracing::warn!("failed to pause active thread goal after interrupt: {err}");
        }
    }

    pub(crate) async fn account_thread_goal_progress(
        &self,
        turn_context: &TurnContext,
        budget_limit_steering: BudgetLimitSteering,
    ) -> anyhow::Result<()> {
        if !self.enabled(Feature::Goals) {
            return Ok(());
        }
        if should_ignore_goal_for_mode(turn_context.collaboration_mode.mode) {
            return Ok(());
        }
        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(());
        };
        let _turn_accounting_permit = turn_context.goal_accounting.accounting_permit().await?;
        let _wall_clock_accounting_permit = self
            .goal_runtime
            .wall_clock_accounting
            .accounting_permit()
            .await?;
        let current_token_usage = self.total_token_usage().await.unwrap_or_default();
        let (token_delta, expected_goal_id) = {
            let turn_accounting = turn_context.goal_accounting.lock().await;
            if !turn_accounting.active_this_turn() {
                return Ok(());
            }
            (
                turn_accounting.token_delta_since_last_accounting(&current_token_usage),
                turn_accounting.active_goal_id(),
            )
        };
        let time_delta_seconds = {
            self.goal_runtime
                .wall_clock_accounting
                .lock()
                .await
                .time_delta_since_last_accounting()
        };
        if time_delta_seconds == 0 && token_delta <= 0 {
            return Ok(());
        }
        let outcome = state_db
            .account_thread_goal_usage(
                self.conversation_id,
                time_delta_seconds,
                token_delta,
                codex_state::ThreadGoalAccountingMode::ActiveOnly,
                expected_goal_id.as_deref(),
            )
            .await?;
        let budget_limit_was_already_reported = {
            let reported_goal_id = self.goal_runtime.budget_limit_reported_goal_id.lock().await;
            expected_goal_id
                .as_deref()
                .is_some_and(|goal_id| reported_goal_id.as_deref() == Some(goal_id))
        };
        let goal = match outcome {
            codex_state::ThreadGoalAccountingOutcome::Updated(goal) => {
                let clear_active_goal = match goal.status {
                    codex_state::ThreadGoalStatus::Active => false,
                    codex_state::ThreadGoalStatus::BudgetLimited => {
                        matches!(budget_limit_steering, BudgetLimitSteering::Suppressed)
                    }
                    codex_state::ThreadGoalStatus::Paused
                    | codex_state::ThreadGoalStatus::Complete => true,
                };
                {
                    let mut turn_accounting = turn_context.goal_accounting.lock().await;
                    turn_accounting.mark_accounted(current_token_usage);
                    if clear_active_goal {
                        turn_accounting.clear_active_goal();
                    }
                }
                {
                    let mut wall_clock_accounting =
                        self.goal_runtime.wall_clock_accounting.lock().await;
                    wall_clock_accounting.mark_accounted();
                    if clear_active_goal {
                        wall_clock_accounting.clear_active_goal();
                    }
                }
                goal
            }
            codex_state::ThreadGoalAccountingOutcome::Unchanged(_) => return Ok(()),
        };
        let should_steer_budget_limit =
            matches!(budget_limit_steering, BudgetLimitSteering::Allowed)
                && goal.status == codex_state::ThreadGoalStatus::BudgetLimited
                && !budget_limit_was_already_reported;
        let goal_status = goal.status;
        let goal_id = goal.goal_id.clone();
        if goal_status != codex_state::ThreadGoalStatus::BudgetLimited {
            *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;
        }
        let goal = protocol_goal_from_state(goal);
        self.send_event(
            turn_context,
            EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: self.conversation_id,
                turn_id: Some(turn_context.sub_id.clone()),
                goal: goal.clone(),
            }),
        )
        .await;
        if should_steer_budget_limit {
            let item = budget_limit_steering_item(&goal);
            if self.inject_response_items(vec![item]).await.is_err() {
                tracing::debug!("skipping budget-limit goal steering because no turn is active");
            }
            *self.goal_runtime.budget_limit_reported_goal_id.lock().await = Some(goal_id);
        }
        Ok(())
    }

    pub(crate) async fn account_thread_goal_before_external_mutation(&self) -> anyhow::Result<()> {
        if let Some(turn_context) = self.active_turn_context().await {
            return self
                .account_thread_goal_progress(
                    turn_context.as_ref(),
                    BudgetLimitSteering::Suppressed,
                )
                .await;
        }

        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(());
        };
        self.account_thread_goal_wall_clock_usage(
            &state_db,
            codex_state::ThreadGoalAccountingMode::ActiveOnly,
        )
        .await?;
        Ok(())
    }

    async fn account_thread_goal_wall_clock_usage(
        &self,
        state_db: &StateDbHandle,
        mode: codex_state::ThreadGoalAccountingMode,
    ) -> anyhow::Result<Option<ThreadGoal>> {
        let _accounting_permit = self
            .goal_runtime
            .wall_clock_accounting
            .accounting_permit()
            .await?;
        let (time_delta_seconds, expected_goal_id) = {
            let wall_clock_accounting = self.goal_runtime.wall_clock_accounting.lock().await;
            (
                wall_clock_accounting.time_delta_since_last_accounting(),
                wall_clock_accounting.active_goal_id(),
            )
        };
        if time_delta_seconds == 0 {
            return Ok(None);
        }

        match state_db
            .account_thread_goal_usage(
                self.conversation_id,
                time_delta_seconds,
                /*token_delta*/ 0,
                mode,
                expected_goal_id.as_deref(),
            )
            .await?
        {
            codex_state::ThreadGoalAccountingOutcome::Updated(goal) => {
                self.goal_runtime
                    .wall_clock_accounting
                    .lock()
                    .await
                    .mark_accounted();
                let goal = protocol_goal_from_state(goal);
                Ok(Some(goal))
            }
            codex_state::ThreadGoalAccountingOutcome::Unchanged(goal) => {
                {
                    let mut wall_clock_accounting =
                        self.goal_runtime.wall_clock_accounting.lock().await;
                    wall_clock_accounting.reset_baseline();
                    wall_clock_accounting.clear_active_goal();
                }
                if let Some(goal) = goal {
                    let goal = protocol_goal_from_state(goal);
                    return Ok(Some(goal));
                }
                Ok(None)
            }
        }
    }

    pub(crate) async fn pause_active_thread_goal_for_interrupt(&self) -> anyhow::Result<()> {
        self.pause_active_thread_goal_with_event_id(uuid::Uuid::new_v4().to_string())
            .await
    }

    async fn pause_active_thread_goal_with_event_id(&self, event_id: String) -> anyhow::Result<()> {
        if !self.enabled(Feature::Goals) {
            return Ok(());
        }

        let _continuation_guard = self
            .goal_runtime
            .continuation_lock
            .acquire()
            .await
            .context("goal continuation semaphore closed")?;
        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(());
        };
        self.account_thread_goal_wall_clock_usage(
            &state_db,
            codex_state::ThreadGoalAccountingMode::ActiveStatusOnly,
        )
        .await?;
        let Some(goal) = state_db
            .pause_active_thread_goal(self.conversation_id)
            .await?
        else {
            return Ok(());
        };
        let goal = protocol_goal_from_state(goal);
        *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;
        self.goal_runtime
            .wall_clock_accounting
            .lock()
            .await
            .clear_active_goal();
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

    pub(crate) async fn activate_paused_thread_goal_after_resume(&self) -> anyhow::Result<bool> {
        if !self.enabled(Feature::Goals) {
            return Ok(false);
        }
        if should_ignore_goal_for_mode(self.collaboration_mode().await.mode) {
            tracing::debug!(
                "skipping paused goal auto-resume while current collaboration mode ignores goals"
            );
            return Ok(false);
        }

        let _continuation_guard = self
            .goal_runtime
            .continuation_lock
            .acquire()
            .await
            .context("goal continuation semaphore closed")?;
        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(false);
        };
        let Some(goal) = state_db.get_thread_goal(self.conversation_id).await? else {
            *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;
            self.goal_runtime
                .wall_clock_accounting
                .lock()
                .await
                .clear_active_goal();
            return Ok(false);
        };
        if goal.status != codex_state::ThreadGoalStatus::Paused {
            let goal_id = goal.goal_id.clone();
            let is_active = goal.status == codex_state::ThreadGoalStatus::Active;
            if is_active {
                self.goal_runtime
                    .wall_clock_accounting
                    .lock()
                    .await
                    .mark_active_goal(goal_id);
            } else {
                self.goal_runtime
                    .wall_clock_accounting
                    .lock()
                    .await
                    .clear_active_goal();
            }
            return Ok(false);
        }

        let Some(goal) = state_db
            .update_thread_goal(
                self.conversation_id,
                codex_state::ThreadGoalUpdate {
                    status: Some(codex_state::ThreadGoalStatus::Active),
                    token_budget: None,
                    expected_goal_id: Some(goal.goal_id.clone()),
                },
            )
            .await?
        else {
            *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;
            self.goal_runtime
                .wall_clock_accounting
                .lock()
                .await
                .clear_active_goal();
            return Ok(false);
        };
        let goal_id = goal.goal_id.clone();
        let goal = protocol_goal_from_state(goal);
        self.reset_thread_goal_continuation_suppression();
        *self.goal_runtime.budget_limit_reported_goal_id.lock().await = None;
        self.goal_runtime
            .wall_clock_accounting
            .lock()
            .await
            .mark_active_goal(goal_id);
        self.send_event_raw(Event {
            id: uuid::Uuid::new_v4().to_string(),
            msg: EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: self.conversation_id,
                turn_id: None,
                goal,
            }),
        })
        .await;
        Ok(true)
    }

    pub(crate) async fn goal_continuation_items_if_active(
        self: &Arc<Self>,
    ) -> Option<Vec<ResponseInputItem>> {
        if !self.enabled(Feature::Goals) {
            return None;
        }
        if should_ignore_goal_for_mode(self.collaboration_mode().await.mode) {
            tracing::debug!("skipping active goal continuation while plan mode is active");
            return None;
        }
        if self.active_turn.lock().await.is_some() {
            tracing::debug!("skipping active goal continuation because a turn is already active");
            return None;
        }
        if self.has_queued_response_items_for_next_turn().await {
            tracing::debug!("skipping active goal continuation because queued input exists");
            return None;
        }
        if self.has_trigger_turn_mailbox_items().await {
            tracing::debug!(
                "skipping active goal continuation because trigger-turn mailbox input is pending"
            );
            return None;
        }
        if self
            .goal_runtime
            .continuation_suppressed
            .load(Ordering::SeqCst)
        {
            tracing::debug!(
                "skipping active goal continuation because the last continuation made no tool calls"
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
        if self.active_turn.lock().await.is_some()
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

        let state_db = if let Some(state_db) = self.state_db() {
            state_db
        } else if let Some(state_db) = self.goal_runtime.state_db.lock().await.clone() {
            state_db
        } else {
            codex_state::StateRuntime::init(
                config.sqlite_home.clone(),
                config.model_provider_id.clone(),
            )
            .await
            .context("failed to initialize sqlite state db for thread goals")?
        };

        if let Some(rollout_path) = self.current_rollout_path().await {
            reconcile_rollout(
                Some(&state_db),
                rollout_path.as_path(),
                config.model_provider_id.as_str(),
                /*builder*/ None,
                &[],
                /*archived_only*/ None,
                /*new_thread_memory_mode*/ None,
            )
            .await;
        }

        *self.goal_runtime.state_db.lock().await = Some(state_db.clone());
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

// Builds the hidden developer prompt used to continue an active goal after the
// previous turn completes. Runtime-owned state such as budget exhaustion is
// reported as context, but the model is only asked to mark goals active,
// paused, or complete.
fn continuation_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let remaining_tokens = goal
        .token_budget
        .map(|budget| (budget - goal.tokens_used).max(0).to_string())
        .unwrap_or_else(|| "unbounded".to_string());
    let tokens_used = goal.tokens_used.to_string();
    let time_used_seconds = goal.time_used_seconds.to_string();

    match CONTINUATION_PROMPT_TEMPLATE.render([
        ("objective", goal.objective.as_str()),
        ("tokens_used", tokens_used.as_str()),
        ("time_used_seconds", time_used_seconds.as_str()),
        ("token_budget", token_budget.as_str()),
        ("remaining_tokens", remaining_tokens.as_str()),
    ]) {
        Ok(prompt) => prompt,
        Err(err) => panic!("embedded goals/continuation.md template failed to render: {err}"),
    }
}

fn budget_limit_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let tokens_used = goal.tokens_used.to_string();
    let time_used_seconds = goal.time_used_seconds.to_string();

    match BUDGET_LIMIT_PROMPT_TEMPLATE.render([
        ("objective", goal.objective.as_str()),
        ("tokens_used", tokens_used.as_str()),
        ("time_used_seconds", time_used_seconds.as_str()),
        ("token_budget", token_budget.as_str()),
    ]) {
        Ok(prompt) => prompt,
        Err(err) => panic!("embedded goals/budget_limit.md template failed to render: {err}"),
    }
}

fn budget_limit_steering_item(goal: &ThreadGoal) -> ResponseInputItem {
    ResponseInputItem::Message {
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: budget_limit_prompt(goal),
        }],
    }
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
    use super::budget_limit_prompt;
    use super::continuation_prompt;
    use super::should_ignore_goal_for_mode;
    use codex_protocol::ThreadId;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::protocol::ThreadGoal;
    use codex_protocol::protocol::ThreadGoalStatus;

    #[test]
    fn goal_continuation_is_ignored_only_in_plan_mode() {
        assert!(should_ignore_goal_for_mode(ModeKind::Plan));
        assert!(!should_ignore_goal_for_mode(ModeKind::Default));
        assert!(!should_ignore_goal_for_mode(ModeKind::PairProgramming));
        assert!(!should_ignore_goal_for_mode(ModeKind::Execute));
    }

    #[test]
    fn continuation_prompt_only_tells_model_to_update_goal_when_complete() {
        let prompt = continuation_prompt(&ThreadGoal {
            thread_id: ThreadId::new(),
            objective: "finish the stack".to_string(),
            status: ThreadGoalStatus::Active,
            token_budget: Some(10_000),
            tokens_used: 1_234,
            time_used_seconds: 56,
            created_at: 1,
            updated_at: 2,
        });

        assert!(prompt.contains("finish the stack"));
        assert!(prompt.contains("Token budget: 10000"));
        assert!(prompt.contains("call update_goal with status \"complete\""));
        assert!(prompt.contains(
            "explain the blocker or next required input to the user and wait for new input"
        ));
        assert!(!prompt.contains("budgetLimited"));
        assert!(!prompt.contains("status \"paused\""));
    }

    #[test]
    fn budget_limit_prompt_steers_model_to_wrap_up_without_pausing() {
        let prompt = budget_limit_prompt(&ThreadGoal {
            thread_id: ThreadId::new(),
            objective: "finish the stack".to_string(),
            status: ThreadGoalStatus::BudgetLimited,
            token_budget: Some(10_000),
            tokens_used: 10_100,
            time_used_seconds: 56,
            created_at: 1,
            updated_at: 2,
        });

        assert!(prompt.contains("finish the stack"));
        assert!(prompt.contains("Token budget: 10000"));
        assert!(prompt.contains("Tokens used: 10100"));
        assert!(prompt.to_lowercase().contains("wrap up this turn soon"));
        assert!(!prompt.contains("status \"paused\""));
    }
}
