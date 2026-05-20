use std::sync::Arc;

use async_trait::async_trait;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TokenUsageContributor;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::TokenUsageInfo;

use crate::accounting::BudgetLimitedGoalDisposition;
use crate::accounting::GoalAccountingState;
use crate::events::GoalEventEmitter;
use crate::spec::UPDATE_GOAL_TOOL_NAME;
use crate::tool::GoalToolExecutor;
use crate::tool::protocol_goal_from_state;

#[derive(Clone, Debug)]
pub struct GoalExtensionConfig {
    pub enabled: bool,
}

impl GoalExtensionConfig {
    fn from_enabled(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[derive(Clone)]
pub struct GoalExtension<C> {
    state_dbs: Arc<codex_state::StateRuntime>,
    event_emitter: GoalEventEmitter,
    goals_enabled: Arc<dyn Fn(&C) -> bool + Send + Sync>,
}

impl<C> std::fmt::Debug for GoalExtension<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoalExtension").finish_non_exhaustive()
    }
}

impl<C> GoalExtension<C> {
    pub(crate) fn new_with_event_sink(
        state_dbs: Arc<codex_state::StateRuntime>,
        event_sink: Arc<dyn ExtensionEventSink>,
        goals_enabled: impl Fn(&C) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            state_dbs,
            event_emitter: GoalEventEmitter::new(event_sink),
            goals_enabled: Arc::new(goals_enabled),
        }
    }
}

#[async_trait]
impl<C> ThreadLifecycleContributor<C> for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    async fn on_thread_start(&self, input: ThreadStartInput<'_, C>) {
        input
            .thread_store
            .insert(GoalExtensionConfig::from_enabled((self.goals_enabled)(
                input.config,
            )));
        input
            .thread_store
            .get_or_init::<GoalAccountingState>(GoalAccountingState::default);
    }
}

impl<C> ConfigContributor<C> for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &C,
        new_config: &C,
    ) {
        thread_store.insert(GoalExtensionConfig::from_enabled((self.goals_enabled)(
            new_config,
        )));
    }
}

#[async_trait]
impl<C> TurnLifecycleContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    async fn on_turn_start(&self, input: TurnStartInput<'_>) {
        if !goal_enabled(input.thread_store) {
            return;
        }

        let accounting = accounting_state(input.thread_store);
        accounting.start_turn(
            input.turn_id,
            input.collaboration_mode.mode,
            input.token_usage_at_turn_start,
        );
        if matches!(
            input.collaboration_mode.mode,
            codex_protocol::config_types::ModeKind::Plan
        ) {
            accounting.clear_current_turn_goal();
            return;
        }
        let Ok(thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
            return;
        };
        let Ok(goal) = self
            .state_dbs
            .thread_goals()
            .get_thread_goal(thread_id)
            .await
        else {
            return;
        };
        if let Some(goal) = goal
            && matches!(
                goal.status,
                codex_state::ThreadGoalStatus::Active
                    | codex_state::ThreadGoalStatus::BudgetLimited
            )
        {
            accounting.mark_turn_goal_active(input.turn_id, goal.goal_id);
        }
    }

    async fn on_turn_stop(&self, input: TurnStopInput<'_>) {
        if !goal_enabled(input.thread_store) {
            return;
        }

        let turn_id = input.turn_store.level_id();
        if let Err(err) = self
            .account_active_goal_progress(
                input.thread_store,
                turn_id,
                &format!("{turn_id}:turn-stop"),
                codex_state::GoalAccountingMode::ActiveOnly,
                BudgetLimitedGoalDisposition::ClearActive,
            )
            .await
        {
            tracing::warn!(
                "failed to account active goal progress at turn stop for {turn_id}: {err}"
            );
            return;
        }
        accounting_state(input.thread_store).finish_turn(turn_id);
    }

    async fn on_turn_abort(&self, input: TurnAbortInput<'_>) {
        if !goal_enabled(input.thread_store) {
            return;
        }

        let turn_id = input.turn_store.level_id();
        if let Err(err) = self
            .account_active_goal_progress(
                input.thread_store,
                turn_id,
                &format!("{turn_id}:turn-abort"),
                codex_state::GoalAccountingMode::ActiveOnly,
                BudgetLimitedGoalDisposition::ClearActive,
            )
            .await
        {
            tracing::warn!(
                "failed to account active goal progress after turn abort for {turn_id}: {err}"
            );
            return;
        }
        accounting_state(input.thread_store).finish_turn(turn_id);
    }
}

#[async_trait]
impl<C> TokenUsageContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    async fn on_token_usage(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        turn_store: &ExtensionData,
        token_usage: &TokenUsageInfo,
    ) {
        if !goal_enabled(thread_store) {
            return;
        }

        let Some(_recorded) = accounting_state(thread_store)
            .record_token_usage(turn_store.level_id(), &token_usage.total_token_usage)
        else {
            return;
        };
    }
}

impl<C> ToolLifecycleContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let should_count_for_goal_progress = goal_enabled(input.thread_store)
                && tool_attempt_counts_for_goal_progress(input.outcome)
                && !(input.tool_name.namespace.is_none()
                    && input.tool_name.name == UPDATE_GOAL_TOOL_NAME);
            if !should_count_for_goal_progress {
                return;
            }
            let turn_id = input.turn_id;
            if let Err(err) = self
                .account_active_goal_progress(
                    input.thread_store,
                    turn_id,
                    input.call_id,
                    codex_state::GoalAccountingMode::ActiveOnly,
                    BudgetLimitedGoalDisposition::KeepActive,
                )
                .await
            {
                tracing::warn!(
                    "failed to account active goal progress after tool finish for {turn_id}: {err}"
                );
            }
        })
    }
}

// TODO: app-server initiated goal set/clear operations need a contributor or
// backend callback here. They currently happen outside thread/turn/token
// lifecycle, but the goal extension must observe them to account before
// mutation, refresh active-goal accounting, emit objective-update steering, and
// clear runtime state when a goal is removed.

impl<C> ToolContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn codex_extension_api::ToolExecutor<codex_extension_api::ToolCall>>> {
        if !goal_enabled(thread_store) {
            return Vec::new();
        }

        let Ok(thread_id) = ThreadId::from_string(thread_store.level_id()) else {
            return Vec::new();
        };
        vec![
            Arc::new(GoalToolExecutor::get(
                thread_id,
                Arc::clone(&self.state_dbs),
                accounting_state(thread_store),
                self.event_emitter.clone(),
            )),
            Arc::new(GoalToolExecutor::create(
                thread_id,
                Arc::clone(&self.state_dbs),
                accounting_state(thread_store),
                self.event_emitter.clone(),
            )),
            Arc::new(GoalToolExecutor::update(
                thread_id,
                Arc::clone(&self.state_dbs),
                accounting_state(thread_store),
                self.event_emitter.clone(),
            )),
        ]
    }
}

pub fn install_with_backend<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    state_dbs: Arc<codex_state::StateRuntime>,
    goals_enabled: impl Fn(&C) -> bool + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    let extension = Arc::new(GoalExtension::new_with_event_sink(
        state_dbs,
        registry.event_sink(),
        goals_enabled,
    ));
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.turn_lifecycle_contributor(extension.clone());
    registry.token_usage_contributor(extension.clone());
    registry.tool_lifecycle_contributor(extension.clone());
    registry.tool_contributor(extension);
}

fn goal_enabled(thread_store: &ExtensionData) -> bool {
    thread_store
        .get::<GoalExtensionConfig>()
        .is_some_and(|config| config.enabled)
}

fn accounting_state(thread_store: &ExtensionData) -> Arc<GoalAccountingState> {
    thread_store.get_or_init::<GoalAccountingState>(GoalAccountingState::default)
}

fn tool_attempt_counts_for_goal_progress(outcome: ToolCallOutcome) -> bool {
    match outcome {
        ToolCallOutcome::Completed { .. } => true,
        ToolCallOutcome::Failed {
            handler_executed: true,
        } => true,
        ToolCallOutcome::Blocked
        | ToolCallOutcome::Failed {
            handler_executed: false,
        }
        | ToolCallOutcome::Aborted => false,
    }
}

impl<C> GoalExtension<C> {
    async fn account_active_goal_progress(
        &self,
        thread_store: &ExtensionData,
        turn_id: &str,
        event_id: &str,
        mode: codex_state::GoalAccountingMode,
        budget_limited_goal_disposition: BudgetLimitedGoalDisposition,
    ) -> Result<Option<ThreadGoal>, String> {
        let Ok(thread_id) = ThreadId::from_string(thread_store.level_id()) else {
            return Ok(None);
        };
        let accounting = accounting_state(thread_store);
        let Some(snapshot) = accounting.progress_snapshot(turn_id) else {
            return Ok(None);
        };
        let outcome = self
            .state_dbs
            .thread_goals()
            .account_thread_goal_usage(
                thread_id,
                snapshot.time_delta_seconds,
                snapshot.token_delta,
                mode,
                Some(snapshot.expected_goal_id.as_str()),
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(match outcome {
            codex_state::GoalAccountingOutcome::Updated(goal) => {
                accounting.mark_progress_accounted_for_status(
                    turn_id,
                    &snapshot,
                    goal.status,
                    budget_limited_goal_disposition,
                );
                let goal = protocol_goal_from_state(goal);
                self.event_emitter.thread_goal_updated(
                    event_id.to_string(),
                    Some(turn_id.to_string()),
                    goal.clone(),
                );
                Some(goal)
            }
            codex_state::GoalAccountingOutcome::Unchanged(_) => None,
        })
    }
}
