//! Background signal mechanism for mid-execution agent transitions.
//!
//! This module provides a mechanism to transition a foreground agent to
//! background execution mid-way through its run. This is used to implement
//! the Ctrl+B "background this agent" feature in the TUI.
//!
//! ## Lifecycle
//!
//! 1. When a foreground agent starts, call [`register_backgroundable_agent`]
//! 2. The agent execution uses `tokio::select!` to wait for both:
//!    - The agent completing normally
//!    - The background signal being triggered
//! 3. If user presses Ctrl+B, call [`trigger_background_transition`]
//! 4. The agent transitions to background mode
//! 5. On completion (either path), call [`unregister_backgroundable_agent`]

use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;
use tokio::sync::oneshot;

/// Global map of agent IDs to their background signal senders.
static BACKGROUND_SIGNAL_MAP: Lazy<RwLock<HashMap<String, oneshot::Sender<()>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Register an agent as backgroundable and get the receiver for the signal.
///
/// The returned receiver will fire when [`trigger_background_transition`] is
/// called for this agent ID.
///
/// # Arguments
///
/// * `agent_id` - Unique identifier for the agent
///
/// # Returns
///
/// A oneshot receiver that will receive a signal when backgrounding is requested.
pub fn register_backgroundable_agent(agent_id: String) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel();

    let mut map = BACKGROUND_SIGNAL_MAP.write().expect("lock poisoned");
    map.insert(agent_id, tx);

    rx
}

/// Trigger a background transition for the given agent.
///
/// If the agent is registered and the signal channel is still open, this will
/// send the background signal and return `true`. Otherwise returns `false`.
///
/// # Arguments
///
/// * `agent_id` - The agent ID to transition to background
///
/// # Returns
///
/// `true` if the signal was sent successfully, `false` if the agent is not
/// registered or the channel was already closed.
pub fn trigger_background_transition(agent_id: &str) -> bool {
    let mut map = BACKGROUND_SIGNAL_MAP.write().expect("lock poisoned");

    if let Some(tx) = map.remove(agent_id) {
        // Send the signal - if the receiver is already dropped, that's fine
        tx.send(()).is_ok()
    } else {
        false
    }
}

/// Unregister an agent from the backgroundable map.
///
/// This should be called when an agent completes (either normally or via
/// background transition) to clean up the signal sender.
///
/// # Arguments
///
/// * `agent_id` - The agent ID to unregister
pub fn unregister_backgroundable_agent(agent_id: &str) {
    let mut map = BACKGROUND_SIGNAL_MAP.write().expect("lock poisoned");
    map.remove(agent_id);
}

/// Check if an agent is currently registered as backgroundable.
///
/// # Arguments
///
/// * `agent_id` - The agent ID to check
///
/// # Returns
///
/// `true` if the agent is registered and can receive a background signal.
pub fn is_agent_backgroundable(agent_id: &str) -> bool {
    let map = BACKGROUND_SIGNAL_MAP.read().expect("lock poisoned");
    map.contains_key(agent_id)
}

/// Get the list of currently backgroundable agent IDs.
///
/// This is useful for UI elements that need to show which agents can be
/// sent to background.
pub fn backgroundable_agent_ids() -> Vec<String> {
    let map = BACKGROUND_SIGNAL_MAP.read().expect("lock poisoned");
    map.keys().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_unregister() {
        let agent_id = "test-agent-1".to_string();

        let _rx = register_backgroundable_agent(agent_id.clone());
        assert!(is_agent_backgroundable(&agent_id));

        unregister_backgroundable_agent(&agent_id);
        assert!(!is_agent_backgroundable(&agent_id));
    }

    #[test]
    fn test_trigger_removes_from_map() {
        let agent_id = "test-agent-2".to_string();

        let _rx = register_backgroundable_agent(agent_id.clone());
        assert!(is_agent_backgroundable(&agent_id));

        let triggered = trigger_background_transition(&agent_id);
        assert!(triggered);

        // Should be removed after trigger
        assert!(!is_agent_backgroundable(&agent_id));
    }

    #[test]
    fn test_trigger_nonexistent() {
        let triggered = trigger_background_transition("nonexistent");
        assert!(!triggered);
    }

    #[tokio::test]
    async fn test_signal_received() {
        let agent_id = "test-agent-3".to_string();

        let rx = register_backgroundable_agent(agent_id.clone());

        // Trigger in another task
        let agent_id_clone = agent_id.clone();
        tokio::spawn(async move {
            trigger_background_transition(&agent_id_clone);
        });

        // Wait for the signal
        let result = rx.await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_receiver_dropped_before_trigger() {
        let agent_id = "test-agent-4".to_string();

        let rx = register_backgroundable_agent(agent_id.clone());
        drop(rx); // Drop the receiver

        // Triggering should still work (returns false since receiver is closed)
        let triggered = trigger_background_transition(&agent_id);
        assert!(!triggered);
    }

    #[test]
    fn test_backgroundable_agent_ids() {
        let id1 = "bg-list-1".to_string();
        let id2 = "bg-list-2".to_string();

        let _rx1 = register_backgroundable_agent(id1.clone());
        let _rx2 = register_backgroundable_agent(id2.clone());

        let ids = backgroundable_agent_ids();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));

        unregister_backgroundable_agent(&id1);
        unregister_backgroundable_agent(&id2);
    }
}
