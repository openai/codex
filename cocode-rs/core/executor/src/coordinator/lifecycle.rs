use serde::Deserialize;
use serde::Serialize;

/// Lifecycle status of a coordinated agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentLifecycleStatus {
    /// Agent is being set up (model, tools, context).
    Initializing,

    /// Agent is actively processing.
    Running,

    /// Agent is waiting for external input.
    Waiting,

    /// Agent finished successfully.
    Completed,

    /// Agent terminated with an error.
    Failed,
}

/// Unique thread identifier for an agent's execution context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadId(pub String);

impl ThreadId {
    /// Generate a new unique thread ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_status_variants() {
        let statuses = vec![
            AgentLifecycleStatus::Initializing,
            AgentLifecycleStatus::Running,
            AgentLifecycleStatus::Waiting,
            AgentLifecycleStatus::Completed,
            AgentLifecycleStatus::Failed,
        ];
        for status in &statuses {
            let _debug = format!("{status:?}");
            let _clone = status.clone();
        }
    }

    #[test]
    fn test_lifecycle_equality() {
        assert_eq!(AgentLifecycleStatus::Running, AgentLifecycleStatus::Running);
        assert_ne!(
            AgentLifecycleStatus::Running,
            AgentLifecycleStatus::Completed
        );
    }

    #[test]
    fn test_lifecycle_serde_roundtrip() {
        let status = AgentLifecycleStatus::Waiting;
        let json = serde_json::to_string(&status).expect("serialize");
        let back: AgentLifecycleStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, AgentLifecycleStatus::Waiting);
    }

    #[test]
    fn test_thread_id_unique() {
        let id1 = ThreadId::new();
        let id2 = ThreadId::new();
        assert_ne!(id1.0, id2.0);
    }

    #[test]
    fn test_thread_id_not_empty() {
        let id = ThreadId::new();
        assert!(!id.0.is_empty());
    }

    #[test]
    fn test_thread_id_default() {
        let id = ThreadId::default();
        assert!(!id.0.is_empty());
    }
}
