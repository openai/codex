//! Execution event bus for streaming agent events.
//!
//! Mirrors `a2a-js/src/server/events/execution_event_bus.ts`.

use crate::types::{Message, Task, TaskArtifactUpdateEvent, TaskStatusUpdateEvent};
use tokio::sync::broadcast;

/// Agent execution event — union of all event types.
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    /// A complete message result.
    Message(Message),
    /// A complete or updated task.
    Task(Task),
    /// A task status update (for streaming).
    StatusUpdate(TaskStatusUpdateEvent),
    /// A task artifact update (for streaming).
    ArtifactUpdate(TaskArtifactUpdateEvent),
}

/// Event bus for publishing and subscribing to agent execution events.
///
/// Uses `tokio::sync::broadcast` for multi-consumer support.
pub struct EventBus {
    tx: broadcast::Sender<ExecutionEvent>,
}

impl EventBus {
    /// Create a new event bus with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: ExecutionEvent) {
        // Ignore error if no receivers.
        let _ = self.tx.send(event);
    }

    /// Subscribe to events. Returns a receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.tx.subscribe()
    }

    /// Create a lightweight clone of this EventBus that shares the same
    /// broadcast sender. Useful for spawning forwarder tasks.
    pub fn clone_sender(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }

    /// Publish a completed status for the given task.
    pub fn publish_task_completed(&self, task: Task) {
        self.publish(ExecutionEvent::Task(task));
    }

    /// Publish a status update event.
    pub fn publish_status_update(&self, event: TaskStatusUpdateEvent) {
        self.publish(ExecutionEvent::StatusUpdate(event));
    }

    /// Publish an artifact update event.
    pub fn publish_artifact_update(&self, event: TaskArtifactUpdateEvent) {
        self.publish(ExecutionEvent::ArtifactUpdate(event));
    }

    /// Publish a message result (no task created).
    pub fn publish_message(&self, message: Message) {
        self.publish(ExecutionEvent::Message(message));
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(64)
    }
}
