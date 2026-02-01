use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
use tokio::sync::oneshot;

use crate::coordinator::lifecycle::AgentLifecycleStatus;
use crate::coordinator::lifecycle::ThreadId;

/// Callback type for executing an agent.
///
/// The callback receives:
/// - `model`: The model to use
/// - `prompt`: The initial prompt
/// - `tools`: List of available tool names
///
/// Returns the agent output as a string.
pub type CoordinatorExecuteFn = Arc<
    dyn Fn(
            String,      // model
            String,      // prompt
            Vec<String>, // tools
        ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>
        + Send
        + Sync,
>;

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

    /// Channel for receiving completion signal.
    completion_rx: Option<oneshot::Receiver<String>>,
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
    /// Optional execution callback for spawning agents.
    execute_fn: Option<CoordinatorExecuteFn>,
}

impl AgentCoordinator {
    /// Create a new empty coordinator.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            execute_fn: None,
        }
    }

    /// Set the execution callback for spawning agents.
    pub fn with_execute_fn(mut self, f: CoordinatorExecuteFn) -> Self {
        self.execute_fn = Some(f);
        self
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

        // Create completion channel
        let (completion_tx, completion_rx) = oneshot::channel::<String>();

        let agent = CoordinatedAgent {
            agent_id: agent_id.clone(),
            thread_id: thread_id.0,
            status: AgentLifecycleStatus::Initializing,
            output: None,
            completion_rx: Some(completion_rx),
        };

        self.agents.insert(agent_id.clone(), agent);

        // Transition to Running and start execution if callback available.
        if let Some(a) = self.agents.get_mut(&agent_id) {
            a.status = AgentLifecycleStatus::Running;
        }

        // Start agent execution in background if we have an execute function
        if let Some(execute_fn) = &self.execute_fn {
            let execute_fn = execute_fn.clone();
            let model = config.model;
            let prompt = config.prompt;
            let tools = config.tools;
            let agent_id_clone = agent_id.clone();

            tokio::spawn(async move {
                let result = execute_fn(model, prompt, tools).await;
                let output = match result {
                    Ok(o) => o,
                    Err(e) => {
                        tracing::error!(agent_id = %agent_id_clone, error = %e, "Agent execution failed");
                        format!("Agent failed: {e}")
                    }
                };
                // Send completion signal
                let _ = completion_tx.send(output);
            });
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

        // Note: Full input routing would require a more complex channel system.
        // This is a placeholder for future multi-turn coordination.
        Ok(())
    }

    /// Wait for an agent to complete and return its output.
    pub async fn wait_for(&mut self, agent_id: &str) -> anyhow::Result<String> {
        // First check if we already have output
        if let Some(agent) = self.agents.get(agent_id) {
            if let Some(output) = &agent.output {
                return Ok(output.clone());
            }
        }

        // Try to await the completion channel
        let agent = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {agent_id}"))?;

        tracing::debug!(agent_id, status = ?agent.status, "Waiting for agent");

        if let Some(rx) = agent.completion_rx.take() {
            match rx.await {
                Ok(output) => {
                    agent.output = Some(output.clone());
                    agent.status = AgentLifecycleStatus::Completed;
                    Ok(output)
                }
                Err(_) => {
                    agent.status = AgentLifecycleStatus::Failed;
                    anyhow::bail!("Agent completion channel closed unexpectedly")
                }
            }
        } else {
            // No completion channel - return any existing output
            Ok(agent.output.clone().unwrap_or_default())
        }
    }

    /// Close and clean up an agent.
    pub async fn close_agent(&mut self, agent_id: &str) -> anyhow::Result<()> {
        let agent = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {agent_id}"))?;

        tracing::info!(agent_id, "Closing agent");
        agent.status = AgentLifecycleStatus::Completed;
        // Drop the completion receiver
        agent.completion_rx = None;

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
    async fn test_wait_for_no_callback() {
        // Without an execute_fn, wait_for returns immediately with empty output
        let mut coord = AgentCoordinator::new();
        let config = SpawnConfig {
            model: "claude-3".to_string(),
            prompt: "test".to_string(),
            tools: vec![],
        };
        let id = coord.spawn_agent(config).await.expect("spawn");
        // Close the agent first since there's no callback to produce output
        coord.close_agent(&id).await.expect("close");
        let output = coord.wait_for(&id).await.expect("wait");
        assert!(output.is_empty());
    }

    #[tokio::test]
    async fn test_spawn_with_execute_fn() {
        // Create a coordinator with an execution callback
        let mut coord =
            AgentCoordinator::new().with_execute_fn(Arc::new(|_model, prompt, _tools| {
                Box::pin(async move { Ok(format!("Executed: {prompt}")) })
            }));

        let config = SpawnConfig {
            model: "claude-3".to_string(),
            prompt: "test task".to_string(),
            tools: vec![],
        };
        let id = coord.spawn_agent(config).await.expect("spawn");

        // Wait for completion
        let output = coord.wait_for(&id).await.expect("wait");
        assert!(output.contains("Executed: test task"));
        assert_eq!(
            coord.get_status(&id),
            Some(&AgentLifecycleStatus::Completed)
        );
    }

    #[test]
    fn test_get_status_missing() {
        let coord = AgentCoordinator::new();
        assert!(coord.get_status("nonexistent").is_none());
    }
}
