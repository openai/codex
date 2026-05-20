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
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnAbortReason;

use crate::accounting::GoalAccountingState;
use crate::events::GoalEventEmitter;
use crate::spec::UPDATE_GOAL_TOOL_NAME;
use crate::tool::GoalToolExecutor;

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

        accounting_state(input.thread_store).start_turn(
            input.turn_id,
            input.collaboration_mode.mode,
            input.token_usage_at_turn_start,
        );
    }

    async fn on_turn_stop(&self, input: TurnStopInput<'_>) {
        if !goal_enabled(input.thread_store) {
            return;
        }

        // TODO: this should flush wall-clock and any unflushed token usage to
        // persisted goal storage, emit ThreadGoalUpdated, and optionally inject
        // budget-limit steering through a host event/input capability.
        // TODO: the host also needs an idle/next-turn wake capability so an
        // active goal can enqueue continuation context after the turn is fully
        // cleared, only when there is no pending user or mailbox work.
        accounting_state(input.thread_store).stop_turn(input.turn_store.level_id());
    }

    async fn on_turn_abort(&self, input: TurnAbortInput<'_>) {
        if !goal_enabled(input.thread_store) {
            return;
        }

        accounting_state(input.thread_store).stop_turn(input.turn_store.level_id());
        if input.reason == TurnAbortReason::Interrupted {
            // TODO: interrupted turns should pause the active goal via persisted
            // goal storage and emit ThreadGoalUpdated with turn_id None.
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
        if !goal_enabled(thread_store) {
            return;
        }

        let Some(_recorded) = accounting_state(thread_store)
            .record_token_usage(turn_store.level_id(), &token_usage.total_token_usage)
        else {
            return;
        };

        // TODO: TokenUsageContributor needs a host goal storage capability so
        // this recorded delta can be committed to the active persisted goal.
        // It also needs an event/input capability to emit ThreadGoalUpdated and
        // inject budget-limit steering when accounting changes goal status.
    }
}

impl<C> ToolLifecycleContributor for GoalExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let _should_count_for_goal_progress = goal_enabled(input.thread_store)
                && tool_attempt_counts_for_goal_progress(input.outcome)
                && !(input.tool_name.namespace.is_none()
                    && input.tool_name.name == UPDATE_GOAL_TOOL_NAME);

            // TODO: commit active goal progress through host goal storage and emit
            // ThreadGoalUpdated when the persisted goal changes. This replaces
            // GoalRuntimeEvent::ToolCompleted once the goal extension owns runtime
            // accounting.
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
                self.event_emitter.clone(),
            )),
            Arc::new(GoalToolExecutor::create(
                thread_id,
                Arc::clone(&self.state_dbs),
                self.event_emitter.clone(),
            )),
            Arc::new(GoalToolExecutor::update(
                thread_id,
                Arc::clone(&self.state_dbs),
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
