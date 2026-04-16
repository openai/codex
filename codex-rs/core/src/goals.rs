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
}

impl GoalAccountingState {
    pub(crate) fn new() -> Self {
        Self {
            accounting_lock: Mutex::new(()),
            last_accounted_token_usage: Mutex::new(TokenUsage::default()),
            completed_this_turn: AtomicBool::new(false),
        }
    }

    pub(crate) async fn mark_turn_started(&self, token_usage: TokenUsage) {
        *self.last_accounted_token_usage.lock().await = token_usage;
        self.completed_this_turn.store(false, Ordering::SeqCst);
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

impl Session {
    pub(crate) async fn get_thread_goal(&self) -> anyhow::Result<Option<ThreadGoal>> {
        if !self.enabled(Feature::GoalMode) {
            return Ok(None);
        }

        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(None);
        };
        let goal = state_db
            .get_thread_goal(self.conversation_id)
            .await
            .map(|goal| goal.map(protocol_goal_from_state))?;
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

        let goal = protocol_goal_from_state(goal);
        self.thread_goal_may_exist.store(true, Ordering::SeqCst);
        *self.thread_goal_cache.lock().await = Some(goal.clone());
        if goal.status == ThreadGoalStatus::Complete {
            turn_context.goal_accounting.mark_completed_this_turn();
        }
        self.send_event(
            turn_context,
            EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: self.conversation_id,
                goal: goal.clone(),
            }),
        )
        .await;
        Ok(goal)
    }

    pub(crate) async fn account_thread_goal_progress(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        boundary: GoalAccountingBoundary,
    ) -> anyhow::Result<()> {
        let clear_completion_accounting = matches!(boundary, GoalAccountingBoundary::Turn)
            && turn_context.goal_accounting.completed_this_turn();
        let result = self
            .account_thread_goal_progress_inner(turn_context, boundary)
            .await;
        if clear_completion_accounting {
            turn_context.goal_accounting.clear_completed_this_turn();
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
        let current_token_usage = self.total_token_usage().await.unwrap_or_default();
        let token_delta = turn_context
            .goal_accounting
            .token_delta_since_last_accounting(current_token_usage.clone())
            .await;
        if token_delta <= 0 {
            return Ok(());
        }
        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(());
        };
        let account_terminal_goal = if turn_context.goal_accounting.completed_this_turn() {
            true
        } else if matches!(boundary, GoalAccountingBoundary::Turn) {
            match state_db.get_thread_goal(self.conversation_id).await? {
                Some(goal) if goal.status == codex_state::ThreadGoalStatus::Complete => {
                    *self.thread_goal_cache.lock().await = Some(protocol_goal_from_state(goal));
                    true
                }
                Some(_) | None => false,
            }
        } else {
            false
        };
        let mode = if account_terminal_goal {
            codex_state::ThreadGoalAccountingMode::ActiveOrComplete
        } else {
            codex_state::ThreadGoalAccountingMode::ActiveOnly
        };
        let outcome = state_db
            .account_thread_goal_usage(self.conversation_id, token_delta, mode)
            .await?;
        let goal = match outcome {
            codex_state::ThreadGoalAccountingOutcome::Updated(goal) => {
                turn_context
                    .goal_accounting
                    .mark_accounted(current_token_usage)
                    .await;
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
                goal,
            }),
        )
        .await;
        if status == codex_state::ThreadGoalStatus::BudgetLimited {
            let session = Arc::clone(self);
            tokio::spawn(async move {
                session
                    .abort_all_tasks_without_restart(TurnAbortReason::BudgetLimited)
                    .await;
            });
        }
        Ok(())
    }

    pub(crate) async fn pause_active_thread_goal_for_interrupt(&self) -> anyhow::Result<()> {
        self.pause_active_thread_goal_with_event_id(uuid::Uuid::new_v4().to_string())
            .await
    }

    async fn pause_active_thread_goal_with_event_id(&self, event_id: String) -> anyhow::Result<()> {
        if !self.enabled(Feature::GoalMode) {
            return Ok(());
        }

        let Some(state_db) = self.state_db_for_thread_goals().await? else {
            return Ok(());
        };
        let Some(goal) = state_db
            .pause_active_thread_goal(self.conversation_id)
            .await?
        else {
            return Ok(());
        };
        let goal = protocol_goal_from_state(goal);
        *self.thread_goal_cache.lock().await = Some(goal.clone());
        self.send_event_raw(Event {
            id: event_id,
            msg: EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: self.conversation_id,
                goal,
            }),
        })
        .await;
        Ok(())
    }

    pub(crate) async fn queue_goal_continuation_if_active(self: &Arc<Self>) {
        if !self.enabled(Feature::GoalMode) {
            return;
        }
        if should_ignore_goal_for_mode(self.collaboration_mode().await.mode) {
            tracing::debug!("skipping active goal continuation while plan mode is active");
            return;
        }
        if self.has_active_turn().await {
            tracing::debug!("skipping active goal continuation because a turn is already active");
            return;
        }
        if self.has_queued_response_items_for_next_turn().await {
            tracing::debug!(
                "skipping active goal continuation because pending next-turn input already exists"
            );
            return;
        }
        if self.has_trigger_turn_mailbox_items().await {
            tracing::debug!(
                "skipping active goal continuation because trigger-turn mailbox input is pending"
            );
            return;
        }
        let goal = match self.get_thread_goal().await {
            Ok(Some(goal)) => goal,
            Ok(None) => {
                tracing::debug!("skipping active goal continuation because no goal is set");
                return;
            }
            Err(err) => {
                tracing::warn!("failed to read thread goal for continuation: {err}");
                return;
            }
        };
        if goal.status != ThreadGoalStatus::Active {
            tracing::debug!(status = ?goal.status, "skipping inactive thread goal");
            return;
        }
        self.queue_response_items_for_next_turn(vec![ResponseInputItem::Message {
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: continuation_prompt(&goal),
            }],
        }])
        .await;
        tracing::info!("queued active goal continuation");
    }
}

impl Session {
    async fn state_db_for_thread_goals(&self) -> anyhow::Result<Option<StateDbHandle>> {
        if let Some(state_db) = self.state_db() {
            return Ok(Some(state_db));
        }
        if let Some(state_db) = self.thread_goal_state_db.lock().await.clone() {
            return Ok(Some(state_db));
        }

        let config = self.get_config().await;
        if config.ephemeral {
            return Ok(None);
        }

        self.try_ensure_rollout_materialized()
            .await
            .context("failed to materialize rollout before opening state db for thread goals")?;

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
- Tokens used: {tokens_used}
- Token budget: {token_budget}
- Tokens remaining: {remaining_tokens}

Avoid repeating work that is already done. Choose the next concrete action toward the objective. Only mark the goal achieved when the objective has actually been achieved and no required work remains. If the objective is achieved, call set_goal with status "complete" and omit objective so usage accounting is preserved. If the achieved goal has a token budget, report the final consumed budget to the user after set_goal succeeds.

If the goal has not been achieved and cannot be achieved within the remaining budget, or the remaining budget is too small for productive continuation, call set_goal with status "budgetLimited" and omit objective. Do not mark a goal complete merely because the budget is nearly exhausted or because you are stopping work. If the goal is otherwise blocked and cannot continue productively for a non-budget reason, call set_goal with status "paused" and omit objective."#,
        objective = goal.objective,
        tokens_used = goal.tokens_used,
    )
}

pub(crate) fn protocol_goal_from_state(goal: codex_state::ThreadGoal) -> ThreadGoal {
    ThreadGoal {
        thread_id: goal.thread_id,
        objective: goal.objective,
        status: protocol_goal_status_from_state(goal.status),
        token_budget: goal.token_budget,
        tokens_used: goal.tokens_used,
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
    use super::should_ignore_goal_for_mode;
    use codex_protocol::config_types::ModeKind;

    #[test]
    fn goal_continuation_is_ignored_only_in_plan_mode() {
        assert!(should_ignore_goal_for_mode(ModeKind::Plan));
        assert!(!should_ignore_goal_for_mode(ModeKind::Default));
        assert!(!should_ignore_goal_for_mode(ModeKind::PairProgramming));
        assert!(!should_ignore_goal_for_mode(ModeKind::Execute));
    }
}
