use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_core::ThreadManager;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ThreadGoal;

use crate::accounting::BudgetLimitedGoalDisposition;
use crate::accounting::GoalAccountingState;
use crate::events::GoalEventEmitter;
use crate::metrics::GoalMetrics;
use crate::steering::objective_updated_steering_item;
use crate::tool::protocol_goal_from_state;

#[derive(Clone)]
pub struct GoalRuntimeHandle {
    inner: Arc<GoalRuntimeInner>,
}

pub(crate) struct GoalRuntimeConfig {
    pub(crate) enabled: bool,
    pub(crate) tools_available_for_thread: bool,
}

struct GoalRuntimeInner {
    thread_id: ThreadId,
    state_dbs: Arc<codex_state::StateRuntime>,
    event_emitter: GoalEventEmitter,
    metrics: GoalMetrics,
    thread_manager: Weak<ThreadManager>,
    accounting_state: Arc<GoalAccountingState>,
    enabled: AtomicBool,
    tools_available_for_thread: bool,
}

pub(crate) struct AccountedGoalProgress {
    pub(crate) goal: ThreadGoal,
    pub(crate) goal_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviousGoalSnapshot {
    pub goal_id: String,
    pub status: codex_state::ThreadGoalStatus,
    pub objective: String,
}

impl From<&codex_state::ThreadGoal> for PreviousGoalSnapshot {
    fn from(goal: &codex_state::ThreadGoal) -> Self {
        Self {
            goal_id: goal.goal_id.clone(),
            status: goal.status,
            objective: goal.objective.clone(),
        }
    }
}

impl std::fmt::Debug for GoalRuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoalRuntimeHandle").finish_non_exhaustive()
    }
}

impl GoalRuntimeHandle {
    pub(crate) fn new(
        thread_id: ThreadId,
        state_dbs: Arc<codex_state::StateRuntime>,
        event_emitter: GoalEventEmitter,
        metrics: GoalMetrics,
        thread_manager: Weak<ThreadManager>,
        accounting_state: Arc<GoalAccountingState>,
        config: GoalRuntimeConfig,
    ) -> Self {
        Self {
            inner: Arc::new(GoalRuntimeInner {
                thread_id,
                state_dbs,
                event_emitter,
                metrics,
                thread_manager,
                accounting_state,
                enabled: AtomicBool::new(config.enabled),
                tools_available_for_thread: config.tools_available_for_thread,
            }),
        }
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.inner.enabled.store(enabled, Ordering::Relaxed);
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.enabled.load(Ordering::Relaxed)
    }

    pub(crate) fn tools_visible(&self) -> bool {
        self.is_enabled() && self.inner.tools_available_for_thread
    }

    pub(crate) fn thread_id(&self) -> ThreadId {
        self.inner.thread_id
    }

    pub(crate) fn accounting_state(&self) -> Arc<GoalAccountingState> {
        Arc::clone(&self.inner.accounting_state)
    }

    pub async fn prepare_external_goal_mutation(&self) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }

        if let Some(turn_id) = self.inner.accounting_state.current_turn_id() {
            self.account_active_goal_progress(
                turn_id.as_str(),
                &format!("{turn_id}:external-goal-mutation"),
                codex_state::GoalAccountingMode::ActiveOnly,
                BudgetLimitedGoalDisposition::ClearActive,
            )
            .await?;
            return Ok(());
        }

        self.account_idle_goal_progress(
            &format!("{}:external-goal-mutation", self.inner.thread_id),
            codex_state::GoalAccountingMode::ActiveOnly,
            BudgetLimitedGoalDisposition::ClearActive,
        )
        .await?;
        Ok(())
    }

    pub async fn apply_external_goal_set(
        &self,
        goal: codex_state::ThreadGoal,
        previous_goal: Option<PreviousGoalSnapshot>,
    ) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }

        let replaced_existing_goal = previous_goal
            .as_ref()
            .is_some_and(|previous_goal| previous_goal.goal_id != goal.goal_id);
        if previous_goal.is_none() || replaced_existing_goal {
            self.inner.metrics.record_created();
        }
        let previous_status = previous_goal
            .as_ref()
            .and_then(|previous_goal| (!replaced_existing_goal).then_some(previous_goal.status));
        self.inner
            .metrics
            .record_resumed_if_status_changed(previous_status, goal.status);
        self.inner
            .metrics
            .record_terminal_if_status_changed(previous_status, &goal);
        let should_steer_active_turn = previous_goal.as_ref().is_none_or(|previous_goal| {
            previous_goal.goal_id != goal.goal_id
                || previous_goal.status != codex_state::ThreadGoalStatus::Active
                || previous_goal.objective != goal.objective
        });
        match goal.status {
            codex_state::ThreadGoalStatus::Active => {
                if self.inner.accounting_state.current_turn_id().is_some() {
                    let _ = self
                        .inner
                        .accounting_state
                        .mark_current_turn_goal_active(goal.goal_id.clone());
                } else {
                    self.inner
                        .accounting_state
                        .mark_idle_goal_active(goal.goal_id.clone());
                }
                if should_steer_active_turn {
                    let item = objective_updated_steering_item(&protocol_goal_from_state(goal));
                    self.inject_active_turn_steering(item).await;
                }
            }
            codex_state::ThreadGoalStatus::BudgetLimited => {
                if self.inner.accounting_state.current_turn_id().is_none() {
                    self.inner.accounting_state.clear_active_goal();
                }
            }
            codex_state::ThreadGoalStatus::Paused
            | codex_state::ThreadGoalStatus::Blocked
            | codex_state::ThreadGoalStatus::UsageLimited
            | codex_state::ThreadGoalStatus::Complete => {
                self.inner.accounting_state.clear_active_goal();
            }
        }
        Ok(())
    }

    pub async fn apply_external_goal_clear(&self) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }

        self.inner.accounting_state.clear_active_goal();
        Ok(())
    }

    pub async fn usage_limit_active_goal_for_turn(&self, turn_id: &str) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }

        if !self
            .inner
            .accounting_state
            .turn_is_current_active_goal(turn_id)
        {
            return Ok(());
        }

        let progress_event_id = format!("{turn_id}:usage-limit-progress");
        self.account_active_goal_progress(
            turn_id,
            progress_event_id.as_str(),
            codex_state::GoalAccountingMode::ActiveOnly,
            BudgetLimitedGoalDisposition::ClearActive,
        )
        .await?;

        let previous_status = self
            .current_goal_status_for_metrics(/*expected_goal_id*/ None)
            .await?;
        let Some(goal) = self
            .inner
            .state_dbs
            .thread_goals()
            .usage_limit_active_thread_goal(self.thread_id())
            .await
            .map_err(|err| err.to_string())?
        else {
            return Ok(());
        };
        self.inner
            .metrics
            .record_terminal_if_status_changed(previous_status, &goal);
        self.inner.accounting_state.clear_active_goal();
        let goal = protocol_goal_from_state(goal);
        self.inner.event_emitter.thread_goal_updated(
            format!("{turn_id}:usage-limit"),
            Some(turn_id.to_string()),
            goal,
        );
        Ok(())
    }

    pub async fn restore_after_resume(&self) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }

        let goal = self
            .inner
            .state_dbs
            .thread_goals()
            .get_thread_goal(self.thread_id())
            .await
            .map_err(|err| err.to_string())?;
        match goal {
            Some(goal) if goal.status == codex_state::ThreadGoalStatus::Active => {
                self.inner
                    .accounting_state
                    .mark_idle_goal_active(goal.goal_id);
                self.inner.metrics.record_resumed();
            }
            Some(_) | None => self.inner.accounting_state.clear_active_goal(),
        }
        Ok(())
    }

    pub(crate) async fn inject_active_turn_steering(&self, item: ResponseItem) {
        let Some(thread_manager) = self.inner.thread_manager.upgrade() else {
            tracing::debug!("skipping goal steering because thread manager is unavailable");
            return;
        };
        let Ok(thread) = thread_manager.get_thread(self.inner.thread_id).await else {
            tracing::debug!("skipping goal steering because live thread is unavailable");
            return;
        };
        if thread.inject_if_running(vec![item]).await.is_err() {
            tracing::debug!("skipping goal steering because no turn is active");
        }
    }

    pub(crate) async fn account_active_goal_progress(
        &self,
        turn_id: &str,
        event_id: &str,
        mode: codex_state::GoalAccountingMode,
        budget_limited_goal_disposition: BudgetLimitedGoalDisposition,
    ) -> Result<Option<AccountedGoalProgress>, String> {
        let accounting = self.accounting_state();
        let Some(snapshot) = accounting.progress_snapshot(turn_id) else {
            return Ok(None);
        };
        let previous_status = self
            .current_goal_status_for_metrics(Some(snapshot.expected_goal_id.as_str()))
            .await?;
        let outcome = self
            .inner
            .state_dbs
            .thread_goals()
            .account_thread_goal_usage(
                self.thread_id(),
                snapshot.time_delta_seconds,
                snapshot.token_delta,
                mode,
                Some(snapshot.expected_goal_id.as_str()),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(match outcome {
            codex_state::GoalAccountingOutcome::Updated(goal) => {
                let goal_id = goal.goal_id.clone();
                self.inner
                    .metrics
                    .record_terminal_if_status_changed(previous_status, &goal);
                accounting.mark_progress_accounted_for_status(
                    turn_id,
                    &snapshot,
                    goal.status,
                    budget_limited_goal_disposition,
                );
                let goal = protocol_goal_from_state(goal);
                self.inner.event_emitter.thread_goal_updated(
                    event_id.to_string(),
                    Some(turn_id.to_string()),
                    goal.clone(),
                );
                Some(AccountedGoalProgress { goal, goal_id })
            }
            codex_state::GoalAccountingOutcome::Unchanged(_) => None,
        })
    }

    async fn account_idle_goal_progress(
        &self,
        event_id: &str,
        mode: codex_state::GoalAccountingMode,
        budget_limited_goal_disposition: BudgetLimitedGoalDisposition,
    ) -> Result<Option<AccountedGoalProgress>, String> {
        let accounting = self.accounting_state();
        let Some(snapshot) = accounting.idle_progress_snapshot() else {
            return Ok(None);
        };
        let previous_status = self
            .current_goal_status_for_metrics(Some(snapshot.expected_goal_id.as_str()))
            .await?;
        let outcome = self
            .inner
            .state_dbs
            .thread_goals()
            .account_thread_goal_usage(
                self.thread_id(),
                snapshot.time_delta_seconds,
                /*token_delta*/ 0,
                mode,
                Some(snapshot.expected_goal_id.as_str()),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(match outcome {
            codex_state::GoalAccountingOutcome::Updated(goal) => {
                let goal_id = goal.goal_id.clone();
                self.inner
                    .metrics
                    .record_terminal_if_status_changed(previous_status, &goal);
                accounting.mark_idle_progress_accounted_for_status(
                    &snapshot,
                    goal.status,
                    budget_limited_goal_disposition,
                );
                let goal = protocol_goal_from_state(goal);
                self.inner.event_emitter.thread_goal_updated(
                    event_id.to_string(),
                    /*turn_id*/ None,
                    goal.clone(),
                );
                Some(AccountedGoalProgress { goal, goal_id })
            }
            codex_state::GoalAccountingOutcome::Unchanged(_) => {
                accounting.reset_idle_progress_baseline_and_clear_active_goal();
                None
            }
        })
    }

    async fn current_goal_status_for_metrics(
        &self,
        expected_goal_id: Option<&str>,
    ) -> Result<Option<codex_state::ThreadGoalStatus>, String> {
        let goal = self
            .inner
            .state_dbs
            .thread_goals()
            .get_thread_goal(self.thread_id())
            .await
            .map_err(|err| err.to_string())?;
        Ok(goal.and_then(|goal| {
            expected_goal_id
                .is_none_or(|expected_goal_id| goal.goal_id == expected_goal_id)
                .then_some(goal.status)
        }))
    }
}
