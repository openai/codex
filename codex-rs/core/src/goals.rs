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
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_protocol::protocol::ThreadGoalUpdatedEvent;

pub(crate) struct SetGoalRequest {
    pub(crate) objective: Option<String>,
    pub(crate) status: Option<ThreadGoalStatus>,
    pub(crate) token_budget: Option<Option<i64>>,
}

impl Session {
    pub(crate) async fn get_thread_goal(&self) -> anyhow::Result<Option<ThreadGoal>> {
        if !self.enabled(Feature::GoalMode) {
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

        codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .map(Some)
        .context("failed to initialize sqlite state db for thread goals")
    }

    async fn require_state_db_for_thread_goals(&self) -> anyhow::Result<StateDbHandle> {
        self.state_db_for_thread_goals().await?.ok_or_else(|| {
            anyhow::anyhow!("thread goals require a persisted thread; this thread is ephemeral")
        })
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
