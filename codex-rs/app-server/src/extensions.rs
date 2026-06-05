use std::sync::Arc;
use std::sync::Weak;

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadGoalUpdatedNotification;
use codex_core::NewThread;
use codex_core::StartThreadOptions;
use codex_core::ThreadManager;
use codex_core::config::Config;
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
use codex_rollout::state_db::StateDbHandle;
use tokio::sync::mpsc::UnboundedSender;

use crate::outgoing_message::OutgoingMessageSender;

pub(crate) fn thread_extensions<S>(
    guardian_agent_spawner: S,
    event_sink: Arc<dyn ExtensionEventSink>,
    auth_manager: Arc<AuthManager>,
    state_db: Option<StateDbHandle>,
    thread_manager: Weak<ThreadManager>,
    goal_service: Arc<GoalService>,
) -> Arc<ExtensionRegistry<Config>>
where
    S: AgentSpawner<StartThreadOptions, Spawned = NewThread, Error = CodexErr> + 'static,
{
    let mut builder = ExtensionRegistryBuilder::<Config>::with_event_sink(event_sink);
    if let Some(state_db) = state_db {
        codex_goal_extension::install_with_backend(
            &mut builder,
            state_db,
            codex_otel::global(),
            thread_manager,
            goal_service,
            |config: &Config| config.features.enabled(codex_features::Feature::Goals),
        );
    }
    codex_guardian::install(&mut builder, guardian_agent_spawner);
    codex_memories_extension::install(&mut builder, codex_otel::global());
    codex_web_search_extension::install(&mut builder, auth_manager.clone());
    codex_image_generation_extension::install(&mut builder, auth_manager);
    Arc::new(builder.build())
}

pub(crate) fn app_server_extension_event_sink(
    outgoing: Arc<OutgoingMessageSender>,
) -> Arc<dyn ExtensionEventSink> {
    let (notification_tx, mut notification_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(notification) = notification_rx.recv().await {
            outgoing.send_server_notification(notification).await;
        }
    });
    Arc::new(AppServerExtensionEventSink { notification_tx })
}

struct AppServerExtensionEventSink {
    notification_tx: UnboundedSender<ServerNotification>,
}

impl ExtensionEventSink for AppServerExtensionEventSink {
    fn emit(&self, event: Event) {
        match event.msg {
            EventMsg::ThreadGoalUpdated(thread_goal_event) => {
                let notification =
                    ServerNotification::ThreadGoalUpdated(ThreadGoalUpdatedNotification {
                        thread_id: thread_goal_event.thread_id.to_string(),
                        turn_id: thread_goal_event.turn_id,
                        goal: thread_goal_event.goal.into(),
                    });
                if let Err(err) = self.notification_tx.send(notification) {
                    tracing::warn!("failed to enqueue extension event notification: {err:?}");
                }
            }
            msg => {
                tracing::debug!(event_id = %event.id, ?msg, "dropping unsupported extension event");
            }
        }
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

    use codex_analytics::AnalyticsEventsClient;
    use codex_app_server_protocol::ServerNotification;
    use codex_protocol::protocol::ThreadGoal;
    use codex_protocol::protocol::ThreadGoalStatus;
    use codex_protocol::protocol::ThreadGoalUpdatedEvent;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;

    #[tokio::test]
    async fn app_server_event_sink_forwards_thread_goal_updates_in_order() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(4);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            AnalyticsEventsClient::disabled(),
        ));
        let sink = app_server_extension_event_sink(outgoing);
        let thread_id = ThreadId::default();

        for turn_id in ["turn-1", "turn-2"] {
            sink.emit(Event {
                id: turn_id.to_string(),
                msg: EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                    thread_id,
                    turn_id: Some(turn_id.to_string()),
                    goal: ThreadGoal {
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
            });
        }

        let mut turn_ids = Vec::new();
        for _ in 0..2 {
            let envelope = timeout(Duration::from_secs(1), outgoing_rx.recv())
                .await
                .expect("timed out waiting for forwarded extension event")
                .expect("outgoing channel closed unexpectedly");
            let OutgoingEnvelope::Broadcast { message } = envelope else {
                panic!("expected broadcast notification");
            };
            let OutgoingMessage::AppServerNotification(ServerNotification::ThreadGoalUpdated(
                notification,
            )) = message
            else {
                panic!("expected thread goal updated notification");
            };
            turn_ids.push(notification.turn_id);
        }

        assert_eq!(
            vec![Some("turn-1".to_string()), Some("turn-2".to_string())],
            turn_ids
        );
    }
}
