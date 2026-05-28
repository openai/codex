use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;

use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_protocol::protocol::validate_thread_goal_objective;

use crate::runtime::GoalRuntimeHandle;
use crate::runtime::PreviousGoalSnapshot;
use crate::tool::fill_empty_thread_preview_if_possible;
use crate::tool::protocol_goal_from_state;
use crate::tool::state_status_from_protocol;
use crate::tool::validate_goal_budget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalServiceError {
    InvalidRequest(String),
    Internal(String),
}

impl fmt::Display for GoalServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) | Self::Internal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for GoalServiceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalObjectiveUpdate<'a> {
    Keep,
    Set(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalTokenBudgetUpdate {
    Keep,
    Set(Option<i64>),
}

#[derive(Clone, Copy, Debug)]
pub struct GoalSetRequest<'a> {
    pub thread_id: ThreadId,
    pub objective: GoalObjectiveUpdate<'a>,
    pub status: Option<ThreadGoalStatus>,
    pub token_budget: GoalTokenBudgetUpdate,
}

#[derive(Clone, Debug)]
pub struct GoalSetOutcome {
    pub goal: ThreadGoal,
    state_goal: codex_state::ThreadGoal,
    previous_goal: Option<PreviousGoalSnapshot>,
}

impl GoalSetOutcome {
    pub async fn apply_runtime_effects(&self, goal_service: &GoalService) {
        if let Some(runtime) = goal_service.runtime_for_thread(self.goal.thread_id)
            && let Err(err) = runtime
                .apply_external_goal_set(self.state_goal.clone(), self.previous_goal.clone())
                .await
        {
            tracing::warn!("failed to apply external goal status runtime effects: {err}");
        }
    }
}

#[derive(Debug, Default)]
pub struct GoalService {
    runtimes: Mutex<HashMap<String, Weak<GoalRuntimeHandle>>>,
}

impl GoalService {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_thread_goal(
        &self,
        state_db: &codex_state::StateRuntime,
        thread_id: ThreadId,
    ) -> Result<Option<ThreadGoal>, GoalServiceError> {
        state_db
            .thread_goals()
            .get_thread_goal(thread_id)
            .await
            .map(|goal| goal.map(protocol_goal_from_state))
            .map_err(|err| GoalServiceError::Internal(format!("failed to read thread goal: {err}")))
    }

    pub async fn set_thread_goal(
        &self,
        state_db: &codex_state::StateRuntime,
        request: GoalSetRequest<'_>,
    ) -> Result<GoalSetOutcome, GoalServiceError> {
        let GoalSetRequest {
            thread_id,
            objective,
            status,
            token_budget,
        } = request;
        let status = status.map(state_status_from_protocol);
        let objective = match objective {
            GoalObjectiveUpdate::Keep => None,
            GoalObjectiveUpdate::Set(objective) => Some(objective.trim()),
        };
        let token_budget = match token_budget {
            GoalTokenBudgetUpdate::Keep => None,
            GoalTokenBudgetUpdate::Set(token_budget) => Some(token_budget),
        };

        if let Some(objective) = objective {
            validate_thread_goal_objective(objective).map_err(GoalServiceError::InvalidRequest)?;
        }
        if objective.is_some() || token_budget.is_some() {
            validate_goal_budget(token_budget.flatten())
                .map_err(GoalServiceError::InvalidRequest)?;
        }

        if let Some(runtime) = self.runtime_for_thread(thread_id)
            && let Err(err) = runtime.prepare_external_goal_mutation().await
        {
            tracing::warn!("failed to prepare external goal mutation: {err}");
        }

        let (goal, previous_goal) = if let Some(objective) = objective {
            let existing_goal = state_db
                .thread_goals()
                .get_thread_goal(thread_id)
                .await
                .map_err(|err| {
                    GoalServiceError::Internal(format!("failed to read thread goal: {err}"))
                })?;
            if let Some(existing_goal) = existing_goal.as_ref() {
                let previous_goal = PreviousGoalSnapshot::from(existing_goal);
                state_db
                    .thread_goals()
                    .update_thread_goal(
                        thread_id,
                        codex_state::GoalUpdate {
                            objective: Some(objective.to_string()),
                            status,
                            token_budget,
                            expected_goal_id: Some(existing_goal.goal_id.clone()),
                        },
                    )
                    .await
                    .map_err(|err| {
                        GoalServiceError::Internal(format!("failed to update thread goal: {err}"))
                    })?
                    .ok_or_else(|| {
                        GoalServiceError::InvalidRequest(format!(
                            "cannot update goal for thread {thread_id}: no goal exists"
                        ))
                    })
                    .map(|goal| (goal, Some(previous_goal)))?
            } else {
                state_db
                    .thread_goals()
                    .replace_thread_goal(
                        thread_id,
                        objective,
                        status.unwrap_or(codex_state::ThreadGoalStatus::Active),
                        token_budget.flatten(),
                    )
                    .await
                    .map_err(|err| {
                        GoalServiceError::Internal(format!("failed to replace thread goal: {err}"))
                    })
                    .map(|goal| (goal, None))?
            }
        } else {
            let existing_goal = state_db
                .thread_goals()
                .get_thread_goal(thread_id)
                .await
                .map_err(|err| {
                    GoalServiceError::Internal(format!("failed to read thread goal: {err}"))
                })?
                .ok_or_else(|| {
                    GoalServiceError::InvalidRequest(format!(
                        "cannot update goal for thread {thread_id}: no goal exists"
                    ))
                })?;
            let previous_goal = PreviousGoalSnapshot::from(&existing_goal);
            let expected_goal_id = existing_goal.goal_id.clone();
            state_db
                .thread_goals()
                .update_thread_goal(
                    thread_id,
                    codex_state::GoalUpdate {
                        objective: None,
                        status,
                        token_budget,
                        expected_goal_id: Some(expected_goal_id),
                    },
                )
                .await
                .map_err(|err| {
                    GoalServiceError::Internal(format!("failed to update thread goal: {err}"))
                })?
                .ok_or_else(|| {
                    GoalServiceError::InvalidRequest(format!(
                        "cannot update goal for thread {thread_id}: no goal exists"
                    ))
                })
                .map(|goal| (goal, Some(previous_goal)))?
        };

        if objective.is_some() {
            fill_empty_thread_preview_if_possible(state_db, thread_id, &goal).await;
        }
        Ok(GoalSetOutcome {
            goal: protocol_goal_from_state(goal.clone()),
            state_goal: goal,
            previous_goal,
        })
    }

    pub async fn clear_thread_goal(
        &self,
        state_db: &codex_state::StateRuntime,
        thread_id: ThreadId,
    ) -> Result<bool, GoalServiceError> {
        if let Some(runtime) = self.runtime_for_thread(thread_id)
            && let Err(err) = runtime.prepare_external_goal_mutation().await
        {
            tracing::warn!("failed to prepare external goal mutation: {err}");
        }

        let cleared = state_db
            .thread_goals()
            .delete_thread_goal(thread_id)
            .await
            .map_err(|err| {
                GoalServiceError::Internal(format!("failed to clear thread goal: {err}"))
            })?
            .is_some();

        if cleared
            && let Some(runtime) = self.runtime_for_thread(thread_id)
            && let Err(err) = runtime.apply_external_goal_clear().await
        {
            tracing::warn!("failed to apply external goal clear runtime effects: {err}");
        }

        Ok(cleared)
    }

    pub(crate) fn register_runtime(&self, runtime: &Arc<GoalRuntimeHandle>) {
        self.runtimes()
            .insert(runtime.thread_id().to_string(), Arc::downgrade(runtime));
    }

    pub(crate) fn unregister_runtime(&self, runtime: &Arc<GoalRuntimeHandle>) {
        let key = runtime.thread_id().to_string();
        let runtime = Arc::downgrade(runtime);
        let mut runtimes = self.runtimes();
        if runtimes
            .get(&key)
            .is_some_and(|registered| registered.ptr_eq(&runtime))
        {
            runtimes.remove(&key);
        }
    }

    fn runtime_for_thread(&self, thread_id: ThreadId) -> Option<Arc<GoalRuntimeHandle>> {
        let key = thread_id.to_string();
        let mut runtimes = self.runtimes();
        let runtime = runtimes.get(&key).and_then(Weak::upgrade);
        if runtime.is_none() {
            runtimes.remove(&key);
        }
        runtime
    }

    fn runtimes(&self) -> std::sync::MutexGuard<'_, HashMap<String, Weak<GoalRuntimeHandle>>> {
        self.runtimes.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
