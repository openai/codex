use std::sync::Arc;
use std::sync::Weak;

use codex_analytics::AnalyticsEventsClient;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadGoal;
use codex_app_server_protocol::ThreadGoalUpdatedNotification;
use codex_core::NewThread;
use codex_core::StartThreadOptions;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::AgentSpawnFuture;
use codex_extension_api::AgentSpawner;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_goal_extension::GoalService;
use codex_login::AuthManager;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadGoalUpdatedEvent;
use codex_rollout::state_db::StateDbHandle;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::ThreadStore;
use tokio::sync::mpsc;

use crate::outgoing_message::OutgoingMessageSender;
use crate::thread_state::ThreadListenerCommand;
use crate::thread_state::ThreadStateManager;

pub(crate) struct ThreadExtensionDependencies {
    pub(crate) event_sink: Arc<dyn ExtensionEventSink>,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) state_db: Option<StateDbHandle>,
    pub(crate) analytics_events_client: AnalyticsEventsClient,
    pub(crate) thread_manager: Weak<ThreadManager>,
    pub(crate) goal_service: Arc<GoalService>,
    pub(crate) environment_manager: Arc<EnvironmentManager>,
    pub(crate) executor_skill_provider: Arc<dyn codex_skills_extension::SkillProvider>,
    /// Process-scoped persistence backend for extensions that need stored thread history.
    pub(crate) thread_store: Arc<dyn ThreadStore>,
}

pub(crate) fn thread_extensions<S>(
    guardian_agent_spawner: S,
    dependencies: ThreadExtensionDependencies,
) -> Arc<ExtensionRegistry<Config>>
where
    S: AgentSpawner<StartThreadOptions, Spawned = NewThread, Error = CodexErr> + 'static,
{
    let ThreadExtensionDependencies {
        event_sink,
        auth_manager,
        state_db,
        analytics_events_client,
        thread_manager,
        goal_service,
        environment_manager,
        executor_skill_provider,
        thread_store: _thread_store,
    } = dependencies;
    let mut builder = ExtensionRegistryBuilder::<Config>::with_event_sink(event_sink);
    if let Some(state_db) = state_db {
        codex_goal_extension::install_with_backend(
            &mut builder,
            state_db,
            analytics_events_client,
            codex_otel::global(),
            thread_manager,
            goal_service,
            |config: &Config| config.features.enabled(codex_features::Feature::Goals),
        );
    }
    codex_guardian::install(&mut builder, guardian_agent_spawner);
    codex_memories_extension::install(&mut builder, codex_otel::global());
    codex_mcp_extension::install(&mut builder);
    codex_mcp_extension::install_executor_plugins(&mut builder, environment_manager);
    codex_web_search_extension::install(&mut builder, auth_manager.clone());
    codex_image_generation_extension::install(&mut builder, auth_manager, |config: &Config| {
        Some(config.codex_home.clone())
    });
    let skill_providers = codex_skills_extension::SkillProviders::new()
        .with_executor_provider(executor_skill_provider)
        .with_orchestrator_provider(Arc::new(
            codex_skills_extension::OrchestratorSkillProvider::new(),
        ));
    codex_skills_extension::install_with_providers(
        &mut builder,
        skill_providers,
        |config: &Config| codex_skills_extension::SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            bundled_skills_enabled: config.bundled_skills_enabled(),
            orchestrator_skills_enabled: config.orchestrator_skills_enabled,
        },
    );
    Arc::new(builder.build())
}

pub(crate) fn app_server_extension_event_sink(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
    thread_store: Arc<dyn ThreadStore>,
) -> Arc<dyn ExtensionEventSink> {
    let (fallback_goal_update_tx, fallback_goal_update_rx) = mpsc::unbounded_channel();
    tokio::spawn(run_fallback_goal_update_worker(
        Arc::downgrade(&outgoing),
        thread_store,
        fallback_goal_update_rx,
    ));
    Arc::new(AppServerExtensionEventSink {
        thread_state_manager,
        fallback_goal_update_tx,
    })
}

struct AppServerExtensionEventSink {
    thread_state_manager: ThreadStateManager,
    fallback_goal_update_tx: mpsc::UnboundedSender<ThreadGoalUpdatedEvent>,
}

impl ExtensionEventSink for AppServerExtensionEventSink {
    fn emit(&self, event: Event) {
        match event.msg {
            EventMsg::ThreadGoalUpdated(thread_goal_event) => {
                let thread_id = thread_goal_event.thread_id;
                if let Some(listener_command_tx) = self
                    .thread_state_manager
                    .current_listener_command_tx(thread_id)
                {
                    let command = ThreadListenerCommand::PersistAndEmitThreadGoalUpdated(
                        thread_goal_event.clone(),
                    );
                    if listener_command_tx.send(command).is_ok() {
                        return;
                    }
                    tracing::warn!(
                        "failed to enqueue extension goal update for {thread_id}: listener command channel is closed"
                    );
                }
                if self
                    .fallback_goal_update_tx
                    .send(thread_goal_event)
                    .is_err()
                {
                    tracing::error!(
                        "failed to enqueue fallback extension goal update for {thread_id}: fallback worker is closed"
                    );
                }
            }
            msg => {
                tracing::debug!(event_id = %event.id, ?msg, "dropping unsupported extension event");
            }
        }
    }
}

async fn run_fallback_goal_update_worker(
    outgoing: Weak<OutgoingMessageSender>,
    thread_store: Arc<dyn ThreadStore>,
    mut goal_updates: mpsc::UnboundedReceiver<ThreadGoalUpdatedEvent>,
) {
    while let Some(event) = goal_updates.recv().await {
        let thread_id = event.thread_id;
        let notification = thread_goal_updated_notification(&event);
        let item = RolloutItem::EventMsg(EventMsg::ThreadGoalUpdated(event));
        if let Err(err) = thread_store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![item],
            })
            .await
        {
            tracing::error!(
                "failed to persist fallback extension goal update for {thread_id}: {err}"
            );
            continue;
        }
        let Some(outgoing) = outgoing.upgrade() else {
            return;
        };
        outgoing
            .send_server_notification(ServerNotification::ThreadGoalUpdated(notification))
            .await;
    }
}

/// Converts a core goal event without losing its persisted turn attribution.
pub(crate) fn thread_goal_updated_notification(
    event: &ThreadGoalUpdatedEvent,
) -> ThreadGoalUpdatedNotification {
    ThreadGoalUpdatedNotification {
        thread_id: event.thread_id.to_string(),
        turn_id: event.turn_id.clone(),
        goal: ThreadGoal::from(event.goal.clone()),
    }
}

pub(crate) fn guardian_agent_spawner(
    thread_manager: Weak<ThreadManager>,
) -> impl AgentSpawner<StartThreadOptions, Spawned = NewThread, Error = CodexErr> {
    move |forked_from_thread_id: ThreadId,
          options: StartThreadOptions|
          -> AgentSpawnFuture<'static, NewThread, CodexErr> {
        let thread_manager = thread_manager.clone();
        Box::pin(async move {
            let thread_manager = thread_manager.upgrade().ok_or_else(|| {
                CodexErr::UnsupportedOperation("thread manager dropped".to_string())
            })?;
            thread_manager
                .spawn_subagent(forked_from_thread_id, options)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use codex_protocol::protocol::ThreadGoal as CoreThreadGoal;
    use codex_protocol::protocol::ThreadGoalStatus;
    use codex_protocol::protocol::ThreadGoalUpdatedEvent;
    use codex_thread_store::InMemoryThreadStore;
    use codex_thread_store::LoadThreadHistoryParams;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;

    #[tokio::test]
    async fn app_server_event_sink_uses_listener_fifo_for_goal_updates_and_clears() {
        let (outgoing_tx, _outgoing_rx) = mpsc::channel(4);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            AnalyticsEventsClient::disabled(),
        ));
        let thread_state_manager = ThreadStateManager::new();
        let thread_id = ThreadId::default();
        let (listener_command_tx, mut listener_command_rx) = mpsc::unbounded_channel();
        thread_state_manager.register_listener_command_tx(thread_id, listener_command_tx.clone());
        let sink = app_server_extension_event_sink(
            outgoing,
            thread_state_manager,
            Arc::new(InMemoryThreadStore::default()),
        );

        for turn_id in ["turn-1", "turn-2"] {
            sink.emit(thread_goal_updated_event(thread_id, turn_id));
        }
        listener_command_tx
            .send(ThreadListenerCommand::EmitThreadGoalCleared)
            .expect("listener command channel should be open");

        let mut observed = Vec::new();
        for _ in 0..3 {
            let command = timeout(Duration::from_secs(1), listener_command_rx.recv())
                .await
                .expect("timed out waiting for listener command")
                .expect("listener command channel closed unexpectedly");
            match command {
                ThreadListenerCommand::PersistAndEmitThreadGoalUpdated(event) => {
                    observed.push(
                        event
                            .turn_id
                            .expect("extension goal updates should include turn ids"),
                    );
                }
                ThreadListenerCommand::EmitThreadGoalCleared => {
                    observed.push("cleared".to_string())
                }
                _ => panic!("unexpected listener command"),
            }
        }

        assert_eq!(
            vec![
                "turn-1".to_string(),
                "turn-2".to_string(),
                "cleared".to_string()
            ],
            observed
        );
    }

    #[tokio::test]
    async fn app_server_event_sink_fallback_persists_goal_updates_before_broadcast_in_order() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(4);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            AnalyticsEventsClient::disabled(),
        ));
        let thread_store = Arc::new(InMemoryThreadStore::default());
        let thread_id = ThreadId::default();
        let sink = app_server_extension_event_sink(
            outgoing.clone(),
            ThreadStateManager::new(),
            thread_store.clone(),
        );

        for turn_id in ["turn-1", "turn-2"] {
            sink.emit(thread_goal_updated_event(thread_id, turn_id));
        }

        let mut notified_turn_ids = Vec::new();
        for _ in 0..2 {
            let envelope = timeout(Duration::from_secs(1), outgoing_rx.recv())
                .await
                .expect("timed out waiting for fallback goal update")
                .expect("outgoing channel closed unexpectedly");
            let OutgoingEnvelope::Broadcast { message } = envelope else {
                panic!("expected broadcast goal update");
            };
            let OutgoingMessage::AppServerNotification(ServerNotification::ThreadGoalUpdated(
                notification,
            )) = message
            else {
                panic!("expected thread goal update notification");
            };
            notified_turn_ids.push(
                notification
                    .turn_id
                    .expect("extension goal updates should include turn ids"),
            );
        }

        let history = thread_store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                include_archived: true,
            })
            .await
            .expect("fallback goal updates should be persisted");
        let persisted_turn_ids = history
            .items
            .into_iter()
            .filter_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::ThreadGoalUpdated(event)) => event.turn_id,
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(notified_turn_ids, vec!["turn-1", "turn-2"]);
        assert_eq!(persisted_turn_ids, notified_turn_ids);
        assert_eq!(thread_store.calls().await.append_items, 2);
    }

    #[tokio::test]
    async fn app_server_event_sink_fallback_worker_does_not_own_outgoing_lifetime() {
        let (outgoing_tx, _outgoing_rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            AnalyticsEventsClient::disabled(),
        ));
        let weak_outgoing = Arc::downgrade(&outgoing);

        let _sink = app_server_extension_event_sink(
            outgoing,
            ThreadStateManager::new(),
            Arc::new(InMemoryThreadStore::default()),
        );

        assert!(weak_outgoing.upgrade().is_none());
    }

    fn thread_goal_updated_event(thread_id: ThreadId, turn_id: &str) -> Event {
        Event {
            id: turn_id.to_string(),
            msg: EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id,
                turn_id: Some(turn_id.to_string()),
                goal: CoreThreadGoal {
                    thread_id,
                    objective: "wire extension events".to_string(),
                    status: ThreadGoalStatus::Active,
                    token_budget: Some(123),
                    tokens_used: 45,
                    time_used_seconds: 6,
                    created_at: 7,
                    updated_at: 8,
                },
            }),
        }
    }
}
