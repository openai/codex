use std::sync::Arc;

use codex_extension_api::ExtensionEvent;
use codex_extension_api::ExtensionEventSink;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalUpdatedEvent;

#[derive(Clone)]
pub(crate) struct GoalEventEmitter {
    sink: Arc<dyn ExtensionEventSink>,
}

impl GoalEventEmitter {
    pub(crate) fn new(sink: Arc<dyn ExtensionEventSink>) -> Self {
        Self { sink }
    }

    pub(crate) async fn thread_goal_updated(&self, turn_id: Option<String>, goal: ThreadGoal) {
        self.sink
            .emit(ExtensionEvent::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: goal.thread_id,
                turn_id,
                goal,
            }))
            .await;
    }
}
