use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalUpdatedEvent;

/// Future returned when a host accepts a goal extension event.
pub type GoalEventFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Host capability for goal-extension events that should be delivered outside
/// the extension runtime.
pub trait GoalEventSink: Send + Sync {
    /// Queue a goal update for host-owned delivery.
    fn thread_goal_updated<'a>(&'a self, event: ThreadGoalUpdatedEvent) -> GoalEventFuture<'a>;
}

/// Goal event sink used when the host does not expose goal event delivery.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopGoalEventSink;

impl GoalEventSink for NoopGoalEventSink {
    fn thread_goal_updated<'a>(&'a self, _event: ThreadGoalUpdatedEvent) -> GoalEventFuture<'a> {
        Box::pin(std::future::ready(()))
    }
}

#[derive(Clone)]
pub(crate) struct GoalEventEmitter {
    sink: Arc<dyn GoalEventSink>,
}

impl GoalEventEmitter {
    pub(crate) fn new(sink: Arc<dyn GoalEventSink>) -> Self {
        Self { sink }
    }

    pub(crate) async fn thread_goal_updated(&self, turn_id: Option<String>, goal: ThreadGoal) {
        self.sink
            .thread_goal_updated(ThreadGoalUpdatedEvent {
                thread_id: goal.thread_id,
                turn_id,
                goal,
            })
            .await;
    }
}
