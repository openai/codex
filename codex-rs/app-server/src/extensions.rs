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
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_goal_extension::GoalEventFuture;
use codex_goal_extension::GoalEventSink;
use codex_login::AuthManager;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::ThreadGoalUpdatedEvent;
use codex_rollout::state_db::StateDbHandle;

use crate::outgoing_message::OutgoingMessageSender;

pub(crate) fn thread_extensions<S>(
    guardian_agent_spawner: S,
    goal_event_sink: Arc<dyn GoalEventSink>,
    auth_manager: Arc<AuthManager>,
    state_db: Option<StateDbHandle>,
) -> Arc<ExtensionRegistry<Config>>
where
    S: AgentSpawner<StartThreadOptions, Spawned = NewThread, Error = CodexErr> + 'static,
{
    let mut builder = ExtensionRegistryBuilder::<Config>::new();
    codex_guardian::install(&mut builder, guardian_agent_spawner);
    if let Some(state_db) = state_db {
        codex_goal_extension::install_with_backend(
            &mut builder,
            state_db,
            goal_event_sink,
            codex_otel::global(),
            |config: &Config| config.features.enabled(Feature::Goals),
        );
    }
    codex_memories_extension::install(&mut builder, codex_otel::global());
    codex_web_search_extension::install(&mut builder, auth_manager);
    Arc::new(builder.build())
}

pub(crate) fn app_server_goal_event_sink(
    outgoing: Arc<OutgoingMessageSender>,
) -> Arc<dyn GoalEventSink> {
    Arc::new(AppServerGoalEventSink { outgoing })
}

struct AppServerGoalEventSink {
    outgoing: Arc<OutgoingMessageSender>,
}

impl GoalEventSink for AppServerGoalEventSink {
    fn thread_goal_updated<'a>(&'a self, event: ThreadGoalUpdatedEvent) -> GoalEventFuture<'a> {
        Box::pin(async move {
            let notification =
                ServerNotification::ThreadGoalUpdated(ThreadGoalUpdatedNotification {
                    thread_id: event.thread_id.to_string(),
                    turn_id: event.turn_id,
                    goal: event.goal.into(),
                });
            self.outgoing.send_server_notification(notification).await;
        })
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
    use codex_app_server_protocol::ThreadGoal as AppServerThreadGoal;
    use codex_app_server_protocol::ThreadGoalStatus as AppServerThreadGoalStatus;
    use codex_protocol::protocol::ThreadGoal;
    use codex_protocol::protocol::ThreadGoalStatus;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;

    #[tokio::test]
    async fn app_server_goal_event_sink_waits_for_outgoing_capacity() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx.clone(),
            AnalyticsEventsClient::disabled(),
        ));
        let thread_id = ThreadId::default();
        outgoing_tx
            .try_send(OutgoingEnvelope::Broadcast {
                message: OutgoingMessage::AppServerNotification(
                    ServerNotification::ThreadGoalUpdated(app_server_goal_update(
                        thread_id,
                        "prefill channel",
                        "prefill",
                    )),
                ),
            })
            .expect("prefill should fit in one-slot channel");
        let sink = app_server_goal_event_sink(outgoing);

        let emit = tokio::spawn(async move {
            sink.thread_goal_updated(thread_goal_update_event(
                thread_id,
                "wait for capacity",
                "turn-1",
            ))
            .await;
        });

        let _prefill = recv_goal_update(&mut outgoing_rx).await;
        emit.await.expect("event emission should complete");
        let notification = recv_goal_update(&mut outgoing_rx).await;

        assert_eq!(
            app_server_goal_update(thread_id, "wait for capacity", "turn-1"),
            notification
        );
    }

    #[tokio::test]
    async fn app_server_goal_event_sink_preserves_goal_update_order() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(2);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            AnalyticsEventsClient::disabled(),
        ));
        let thread_id = ThreadId::default();
        let sink = app_server_goal_event_sink(outgoing);

        sink.thread_goal_updated(thread_goal_update_event(thread_id, "first", "turn-1"))
            .await;
        sink.thread_goal_updated(thread_goal_update_event(thread_id, "second", "turn-2"))
            .await;

        assert_eq!(
            app_server_goal_update(thread_id, "first", "turn-1"),
            recv_goal_update(&mut outgoing_rx).await
        );
        assert_eq!(
            app_server_goal_update(thread_id, "second", "turn-2"),
            recv_goal_update(&mut outgoing_rx).await
        );
    }

    fn thread_goal_update_event(
        thread_id: ThreadId,
        objective: &str,
        turn_id: &str,
    ) -> ThreadGoalUpdatedEvent {
        ThreadGoalUpdatedEvent {
            thread_id,
            turn_id: Some(turn_id.to_string()),
            goal: ThreadGoal {
                thread_id,
                objective: objective.to_string(),
                status: ThreadGoalStatus::Active,
                token_budget: Some(123),
                tokens_used: 45,
                time_used_seconds: 6,
                created_at: 7,
                updated_at: 8,
            },
        }
    }

    fn app_server_goal_update(
        thread_id: ThreadId,
        objective: &str,
        turn_id: &str,
    ) -> ThreadGoalUpdatedNotification {
        ThreadGoalUpdatedNotification {
            thread_id: thread_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            goal: AppServerThreadGoal {
                thread_id: thread_id.to_string(),
                objective: objective.to_string(),
                status: AppServerThreadGoalStatus::Active,
                token_budget: Some(123),
                tokens_used: 45,
                time_used_seconds: 6,
                created_at: 7,
                updated_at: 8,
            },
        }
    }

    async fn recv_goal_update(
        outgoing_rx: &mut mpsc::Receiver<OutgoingEnvelope>,
    ) -> ThreadGoalUpdatedNotification {
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
        notification
    }
}
