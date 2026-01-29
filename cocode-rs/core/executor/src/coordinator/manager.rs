use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::coordinator::lifecycle::{AgentLifecycleStatus, ThreadId};

/// A single agent managed by the coordinator.
pub struct CoordinatedAgent {
    /// Unique agent identifier.
    pub agent_id: String,

    /// Thread identifier for the agent's execution context.
    pub thread_id: String,

    /// Current lifecycle status.
    pub status: AgentLifecycleStatus,

    /// Output produced by the agent (populated on completion).
    output: Option<String>,
}

/// Configuration for spawning a new coordinated agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnConfig {
    /// Model to use for the agent.
    pub model: String,

    /// Initial prompt for the agent.
    pub prompt: String,

    /// Tools available to the agent.
    #[serde(default)]
    pub tools: Vec<String>,
}

/// Coordinates multiple concurrent agent instances.
///
/// The coordinator manages the lifecycle of agents, routes input/output between
/// them, and provides waiting semantics for agent completion.
pub struct AgentCoordinator {
    agents: HashMap<String, CoordinatedAgent>,
}

impl AgentCoordinator {
    /// Create a new empty coordinator.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Spawn a new coordinated agent.
    ///
    /// Returns the unique agent ID.
    pub async fn spawn_agent(&mut self, config: SpawnConfig) -> anyhow::Result<String> {
        let agent_id = uuid::Uuid::new_v4().to_string();
        let thread_id = ThreadId::new();

        tracing::info!(
            agent_id = %agent_id,
            model = %config.model,
            tools_count = config.tools.len(),
            "Spawning coordinated agent"
        );

        let agent = CoordinatedAgent {
            agent_id: agent_id.clone(),
            thread_id: thread_id.0,
            status: AgentLifecycleStatus::Initializing,
            output: None,
        };

        self.agents.insert(agent_id.clone(), agent);

        // Transition to Running.
        if let Some(a) = self.agents.get_mut(&agent_id) {
            a.status = AgentLifecycleStatus::Running;
        }

        Ok(agent_id)
    }

    /// Send input to a running agent.
    pub async fn send_input(&self, agent_id: &str, input: &str) -> anyhow::Result<()> {
        let agent = self
            .agents
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {agent_id}"))?;

        if agent.status != AgentLifecycleStatus::Running
            && agent.status != AgentLifecycleStatus::Waiting
        {
            anyhow::bail!(
                "Agent {agent_id} is not in a state that accepts input (status: {:?})",
                agent.status
            );
        }

        tracing::debug!(agent_id, input_len = input.len(), "Sending input to agent");

        // TODO: Route input to the agent's execution context.
        Ok(())
    }

    /// Wait for an agent to complete and return its output.
    pub async fn wait_for(&self, agent_id: &str) -> anyhow::Result<String> {
        let agent = self
            .agents
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {agent_id}"))?;

        tracing::debug!(agent_id, status = ?agent.status, "Waiting for agent");

        // TODO: Actually await the agent's completion signal.
        // For now, return any available output or an empty string.
        Ok(agent.output.clone().unwrap_or_default())
    }

    /// Close and clean up an agent.
    pub async fn close_agent(&mut self, agent_id: &str) -> anyhow::Result<()> {
        let agent = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {agent_id}"))?;

        tracing::info!(agent_id, "Closing agent");
        agent.status = AgentLifecycleStatus::Completed;

        Ok(())
    }

    /// Get the status of an agent, if it exists.
    pub fn get_status(&self, agent_id: &str) -> Option<&AgentLifecycleStatus> {
        self.agents.get(agent_id).map(|a| &a.status)
    }

    /// Get the number of managed agents.
    pub fn agent_count(&self) -> i32 {
        self.agents.len() as i32
    }
}

impl Default for AgentCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_coordinator() {
        let coord = AgentCoordinator::new();
        assert_eq!(coord.agent_count(), 0);
    }

    #[tokio::test]
    async fn test_spawn_agent() {
        let mut coord = AgentCoordinator::new();
        let config = SpawnConfig {
            model: "claude-3".to_string(),
            prompt: "test".to_string(),
            tools: vec!["Bash".to_string()],
        };
        let id = coord.spawn_agent(config).await.expect("spawn");
        assert!(!id.is_empty());
        assert_eq!(coord.agent_count(), 1);
        assert_eq!(coord.get_status(&id), Some(&AgentLifecycleStatus::Running));
    }

    #[tokio::test]
    async fn test_send_input_to_running() {
        let mut coord = AgentCoordinator::new();
        let config = SpawnConfig {
            model: "claude-3".to_string(),
            prompt: "test".to_string(),
            tools: vec![],
        };
        let id = coord.spawn_agent(config).await.expect("spawn");
        let result = coord.send_input(&id, "hello").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_input_to_completed() {
        let mut coord = AgentCoordinator::new();
        let config = SpawnConfig {
            model: "claude-3".to_string(),
            prompt: "test".to_string(),
            tools: vec![],
        };
        let id = coord.spawn_agent(config).await.expect("spawn");
        coord.close_agent(&id).await.expect("close");
        let result = coord.send_input(&id, "hello").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_input_nonexistent() {
        let coord = AgentCoordinator::new();
        let result = coord.send_input("nonexistent", "hello").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_close_agent() {
        let mut coord = AgentCoordinator::new();
        let config = SpawnConfig {
            model: "claude-3".to_string(),
            prompt: "test".to_string(),
            tools: vec![],
        };
        let id = coord.spawn_agent(config).await.expect("spawn");
        coord.close_agent(&id).await.expect("close");
        assert_eq!(
            coord.get_status(&id),
            Some(&AgentLifecycleStatus::Completed)
        );
    }

    #[tokio::test]
    async fn test_close_nonexistent() {
        let mut coord = AgentCoordinator::new();
        let result = coord.close_agent("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wait_for_agent() {
        let mut coord = AgentCoordinator::new();
        let config = SpawnConfig {
            model: "claude-3".to_string(),
            prompt: "test".to_string(),
            tools: vec![],
        };
        let id = coord.spawn_agent(config).await.expect("spawn");
        let output = coord.wait_for(&id).await.expect("wait");
        assert!(output.is_empty()); // No output yet in skeleton.
    }

    #[test]
    fn test_get_status_missing() {
        let coord = AgentCoordinator::new();
        assert!(coord.get_status("nonexistent").is_none());
    }
}
