use std::sync::Arc;

use async_trait::async_trait;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadIdleTurnContributor;
use codex_extension_api::ThreadIdleTurnRequestFuture;
use codex_extension_api::ThreadIdleTurnStartFuture;
use codex_extension_api::ThreadIdleTurnStartInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TokenUsageContributor;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolContributionInput;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnErrorInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_otel::MetricsClient;
use codex_protocol::ThreadId;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_protocol::protocol::TokenUsageInfo;

use crate::accounting::BudgetLimitedGoalDisposition;
use crate::accounting::GoalAccountingState;
use crate::events::GoalEventEmitter;
use crate::events::GoalEventSink;
use crate::metrics::GoalMetrics;
use crate::runtime::GoalRuntimeHandle;
use crate::spec::UPDATE_GOAL_TOOL_NAME;
use crate::steering::budget_limit_steering_context;
use crate::tool::GoalToolExecutor;

#[derive(Clone)]
pub struct GoalExtension<C> {
    state_dbs: Arc<codex_state::StateRuntime>,
    event_emitter: GoalEventEmitter,
    metrics: GoalMetrics,
    goals_enabled: Arc<dyn Fn(&C) -> bool + Send + Sync>,
}

impl<C> std::fmt::Debug for GoalExtension<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoalExtension").finish_non_exhaustive()
    }
}

impl<C> GoalExtension<C> {
    pub(crate) fn new_with_host_capabilities(
        state_dbs: Arc<codex_state::StateRuntime>,
        event_sink: Arc<dyn GoalEventSink>,
        metrics_client: Option<MetricsClient>,
        goals_enabled: impl Fn(&C) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            state_dbs,
            event_emitter: GoalEventEmitter::new(event_sink),
            metrics: GoalMetrics::new(metrics_client),
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
        let enabled = (self.goals_enabled)(input.config);
        let accounting_state = input
            .thread_store
            .get_or_init::<GoalAccountingState>(GoalAccountingState::default);
        let Ok(thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
            return;
        };
        let runtime = input.thread_store.get_or_init::<GoalRuntimeHandle>(|| {
            GoalRuntimeHandle::new(
                thread_id,
                Arc::clone(&self.state_dbs),
                self.event_emitter.clone(),
                self.metrics.clone(),
                Arc::clone(&input.response_item_injector),
                accounting_state,
                enabled,
            )
        });
        runtime.set_enabled(enabled);
    }
}

impl<C> ThreadIdleTurnContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    fn request_thread_idle_turn<'a>(
        &'a self,
        input: ThreadIdleInput<'a>,
    ) -> ThreadIdleTurnRequestFuture<'a> {
        Box::pin(async move {
            let runtime = goal_runtime_handle(input.thread_store)?;
            match runtime.idle_continuation_request().await {
                Ok(request) => request,
                Err(err) => {
                    tracing::warn!("failed to request idle goal continuation: {err}");
                    None
                }
            }
        })
    }

    fn should_start_thread_idle_turn<'a>(
        &'a self,
        input: ThreadIdleTurnStartInput<'a>,
    ) -> ThreadIdleTurnStartFuture<'a> {
        Box::pin(async move {
            let Some(runtime) = goal_runtime_handle(input.thread_store) else {
                return false;
            };
            match runtime
                .idle_continuation_is_current(input.request.validation_key.as_deref())
                .await
            {
                Ok(should_start) => should_start,
                Err(err) => {
                    tracing::warn!("failed to validate idle goal continuation: {err}");
                    false
                }
            }
        })
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
        let enabled = (self.goals_enabled)(new_config);
        if let Some(runtime) = goal_runtime_handle(thread_store) {
            runtime.set_enabled(enabled);
        }
    }
}

#[async_trait]
impl<C> TurnLifecycleContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    async fn on_turn_start(&self, input: TurnStartInput<'_>) {
        let Some(runtime) = goal_runtime_handle(input.thread_store) else {
            return;
        };
        if !runtime.is_enabled() {
            return;
        }

        let accounting = runtime.accounting_state();
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
        let Ok(goal) = self
            .state_dbs
            .thread_goals()
            .get_thread_goal(runtime.thread_id())
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
        let Some(runtime) = goal_runtime_handle(input.thread_store) else {
            return;
        };
        if !runtime.is_enabled() {
            return;
        }

        let turn_id = input.turn_store.level_id();
        if let Err(err) = runtime
            .account_active_goal_progress(
                turn_id,
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
        runtime.accounting_state().finish_turn(turn_id);
    }

    async fn on_turn_abort(&self, input: TurnAbortInput<'_>) {
        let Some(runtime) = goal_runtime_handle(input.thread_store) else {
            return;
        };
        if !runtime.is_enabled() {
            return;
        }

        let turn_id = input.turn_store.level_id();
        if let Err(err) = runtime
            .account_active_goal_progress(
                turn_id,
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
        runtime.accounting_state().finish_turn(turn_id);
    }

    async fn on_turn_error(&self, input: TurnErrorInput<'_>) {
        if input.error != CodexErrorInfo::UsageLimitExceeded {
            return;
        }
        let Some(runtime) = goal_runtime_handle(input.thread_store) else {
            return;
        };
        if !runtime.is_enabled() {
            return;
        }

        if let Err(err) = runtime
            .usage_limit_active_goal_for_turn(input.turn_id)
            .await
        {
            tracing::warn!(
                "failed to usage-limit active goal after usage-limit error for {}: {err}",
                input.turn_id
            );
        }
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
        let Some(runtime) = goal_runtime_handle(thread_store) else {
            return;
        };
        if !runtime.is_enabled() {
            return;
        }

        let Some(_recorded) = runtime
            .accounting_state()
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
            let Some(runtime) = goal_runtime_handle(input.thread_store) else {
                return;
            };
            let should_count_for_goal_progress = runtime.is_enabled()
                && tool_attempt_counts_for_goal_progress(input.outcome)
                && !(input.tool_name.namespace.is_none()
                    && input.tool_name.name == UPDATE_GOAL_TOOL_NAME);
            if !should_count_for_goal_progress {
                return;
            }
            let turn_id = input.turn_id;
            let progress = match runtime
                .account_active_goal_progress(
                    turn_id,
                    codex_state::GoalAccountingMode::ActiveOnly,
                    BudgetLimitedGoalDisposition::KeepActive,
                )
                .await
            {
                Ok(Some(progress)) => progress,
                Ok(None) => return,
                Err(err) => {
                    tracing::warn!(
                        "failed to account active goal progress after tool finish for {turn_id}: {err}"
                    );
                    return;
                }
            };
            let goal = progress.goal;
            if goal.status != ThreadGoalStatus::BudgetLimited {
                return;
            }
            if !runtime
                .accounting_state()
                .mark_budget_limit_reported_if_new(progress.goal_id.as_str())
            {
                return;
            }
            let context = budget_limit_steering_context(&goal);
            runtime.inject_active_turn_steering(context).await;
        })
    }
}

impl<C> ToolContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn codex_extension_api::ToolExecutor<codex_extension_api::ToolCall>>> {
        self.goal_tools_for_thread(thread_store)
    }

    fn tools_for_turn(
        &self,
        input: ToolContributionInput<'_>,
    ) -> Vec<Arc<dyn codex_extension_api::ToolExecutor<codex_extension_api::ToolCall>>> {
        if !input.persistent_thread_state_available
            || matches!(
                input.session_source,
                SessionSource::SubAgent(SubAgentSource::Review)
            )
        {
            return Vec::new();
        }
        self.goal_tools_for_thread(input.thread_store)
    }
}

impl<C> GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    fn goal_tools_for_thread(
        &self,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn codex_extension_api::ToolExecutor<codex_extension_api::ToolCall>>> {
        let Some(runtime) = goal_runtime_handle(thread_store) else {
            return Vec::new();
        };
        if !runtime.is_enabled() {
            return Vec::new();
        }

        vec![
            Arc::new(GoalToolExecutor::get(
                runtime.thread_id(),
                Arc::clone(&self.state_dbs),
                runtime.accounting_state(),
                self.event_emitter.clone(),
                self.metrics.clone(),
            )),
            Arc::new(GoalToolExecutor::create(
                runtime.thread_id(),
                Arc::clone(&self.state_dbs),
                runtime.accounting_state(),
                self.event_emitter.clone(),
                self.metrics.clone(),
            )),
            Arc::new(GoalToolExecutor::update(
                runtime.thread_id(),
                Arc::clone(&self.state_dbs),
                runtime.accounting_state(),
                self.event_emitter.clone(),
                self.metrics.clone(),
            )),
        ]
    }
}

pub fn install_with_backend<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    state_dbs: Arc<codex_state::StateRuntime>,
    event_sink: Arc<dyn GoalEventSink>,
    metrics_client: Option<MetricsClient>,
    goals_enabled: impl Fn(&C) -> bool + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    let extension = Arc::new(GoalExtension::new_with_host_capabilities(
        state_dbs,
        event_sink,
        metrics_client,
        goals_enabled,
    ));
    registry.thread_lifecycle_contributor(extension.clone());
    registry.thread_idle_turn_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.turn_lifecycle_contributor(extension.clone());
    registry.token_usage_contributor(extension.clone());
    registry.tool_lifecycle_contributor(extension.clone());
    registry.tool_contributor(extension);
}

fn goal_runtime_handle(thread_store: &ExtensionData) -> Option<Arc<GoalRuntimeHandle>> {
    thread_store.get::<GoalRuntimeHandle>()
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
