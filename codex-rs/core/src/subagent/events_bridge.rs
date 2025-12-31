//! Event bridge for forwarding subagent events to parent session.

use super::events::SubagentActivityEvent;
use tokio::sync::mpsc;

/// Bridge for forwarding subagent events to parent.
#[derive(Debug)]
pub struct SubagentEventBridge {
    /// Sender for events to parent.
    tx: mpsc::Sender<SubagentActivityEvent>,
}

impl SubagentEventBridge {
    /// Create a new event bridge.
    pub fn new(tx: mpsc::Sender<SubagentActivityEvent>) -> Self {
        Self { tx }
    }

    /// Create a bridge with a new channel, returning the receiver.
    pub fn create() -> (Self, mpsc::Receiver<SubagentActivityEvent>) {
        let (tx, rx) = mpsc::channel(100);
        (Self { tx }, rx)
    }

    /// Send an event to the parent.
    pub async fn send(&self, event: SubagentActivityEvent) {
        // Best-effort send, don't block on full channel
        let _ = self.tx.send(event).await;
    }

    /// Try to send an event without waiting.
    pub fn try_send(&self, event: SubagentActivityEvent) {
        let _ = self.tx.try_send(event);
    }
}

impl Clone for SubagentEventBridge {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagent::events::SubagentEventType;

    #[tokio::test]
    async fn test_event_bridge() {
        let (bridge, mut rx) = SubagentEventBridge::create();

        let event = SubagentActivityEvent::new("agent-1", "Test", SubagentEventType::Started);
        bridge.send(event).await;

        let received = rx.recv().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().agent_id, "agent-1");
    }

    #[test]
    fn test_try_send() {
        let (bridge, _rx) = SubagentEventBridge::create();
        let event = SubagentActivityEvent::new("agent-1", "Test", SubagentEventType::Started);
        bridge.try_send(event);
        // Should not panic
    }
}
