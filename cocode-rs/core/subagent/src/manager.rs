use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::definition::AgentDefinition;

/// Runtime status of a subagent instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Running,
    Completed,
    Failed,
    Backgrounded,
}

/// A live subagent instance.
pub struct AgentInstance {
    /// Unique identifier for this instance.
    pub id: String,

    /// The agent type this instance was spawned from.
    pub agent_type: String,

    /// Current execution status.
    pub status: AgentStatus,

    /// Final output text (populated on completion).
    pub output: Option<String>,
}

/// Manages subagent registration, spawning, and lifecycle tracking.
pub struct SubagentManager {
    agents: HashMap<String, AgentInstance>,
    definitions: Vec<AgentDefinition>,
}

impl SubagentManager {
    /// Create a new empty subagent manager.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            definitions: Vec::new(),
        }
    }

    /// Register a new agent type definition.
    pub fn register_agent_type(&mut self, definition: AgentDefinition) {
        tracing::info!(agent_type = %definition.agent_type, "Registering agent type");
        self.definitions.push(definition);
    }

    /// Spawn a new subagent instance of the given type.
    ///
    /// Returns the unique agent ID on success.
    pub async fn spawn(&mut self, agent_type: &str, prompt: &str) -> anyhow::Result<String> {
        let _definition = self
            .definitions
            .iter()
            .find(|d| d.agent_type == agent_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown agent type: {agent_type}"))?
            .clone();

        let agent_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(agent_id = %agent_id, agent_type, prompt_len = prompt.len(), "Spawning subagent");

        let instance = AgentInstance {
            id: agent_id.clone(),
            agent_type: agent_type.to_string(),
            status: AgentStatus::Running,
            output: None,
        };
        self.agents.insert(agent_id.clone(), instance);

        Ok(agent_id)
    }

    /// Resume a previously backgrounded agent.
    pub async fn resume(&mut self, agent_id: &str) -> anyhow::Result<String> {
        let instance = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {agent_id}"))?;

        if instance.status != AgentStatus::Backgrounded {
            anyhow::bail!(
                "Agent {agent_id} is not backgrounded (status: {:?})",
                instance.status
            );
        }

        tracing::info!(agent_id, "Resuming backgrounded agent");
        instance.status = AgentStatus::Running;
        Ok(agent_id.to_string())
    }

    /// Get the output of a completed agent.
    pub async fn get_output(&self, agent_id: &str) -> Option<String> {
        self.agents.get(agent_id).and_then(|a| a.output.clone())
    }

    /// Get the current status of an agent.
    pub fn get_status(&self, agent_id: &str) -> Option<AgentStatus> {
        self.agents.get(agent_id).map(|a| a.status.clone())
    }
}

impl Default for SubagentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_definition(name: &str) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            description: format!("{name} agent"),
            agent_type: name.to_string(),
            tools: vec![],
            disallowed_tools: vec![],
            model: None,
            max_turns: None,
        }
    }

    #[test]
    fn test_new_manager() {
        let mgr = SubagentManager::new();
        assert!(mgr.agents.is_empty());
        assert!(mgr.definitions.is_empty());
    }

    #[test]
    fn test_register_agent_type() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));
        assert_eq!(mgr.definitions.len(), 1);
        assert_eq!(mgr.definitions[0].agent_type, "bash");
    }

    #[tokio::test]
    async fn test_spawn_agent() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));

        let id = mgr.spawn("bash", "run ls").await.expect("spawn");
        assert!(!id.is_empty());
        assert_eq!(mgr.get_status(&id), Some(AgentStatus::Running));
    }

    #[tokio::test]
    async fn test_spawn_unknown_type() {
        let mut mgr = SubagentManager::new();
        let result = mgr.spawn("nonexistent", "test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_output_none() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));
        let id = mgr.spawn("bash", "test").await.expect("spawn");
        assert!(mgr.get_output(&id).await.is_none());
    }

    #[tokio::test]
    async fn test_resume_non_backgrounded() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));
        let id = mgr.spawn("bash", "test").await.expect("spawn");
        let result = mgr.resume(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resume_backgrounded() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));
        let id = mgr.spawn("bash", "test").await.expect("spawn");

        // Manually set to backgrounded for test.
        mgr.agents.get_mut(&id).expect("agent").status = AgentStatus::Backgrounded;

        let resumed_id = mgr.resume(&id).await.expect("resume");
        assert_eq!(resumed_id, id);
        assert_eq!(mgr.get_status(&id), Some(AgentStatus::Running));
    }

    #[test]
    fn test_get_status_missing() {
        let mgr = SubagentManager::new();
        assert!(mgr.get_status("nonexistent").is_none());
    }
}
