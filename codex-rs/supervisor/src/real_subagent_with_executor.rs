// Real SubAgent with CodexExecutor Integration
// Best Practices Implementation for AI Agent Development

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::codex_executor::CodexExecutor;
use crate::subagent::AgentMessage;
use crate::subagent::AgentState;
use crate::subagent::AgentStatus;
use crate::subagent::AgentType;

/// Real SubAgent with full Codex integration
/// Implements AIエージェントベストプラクティス
pub struct RealSubAgentWithExecutor {
    agent_type: AgentType,
    state: Arc<Mutex<AgentState>>,
    tx: mpsc::UnboundedSender<AgentMessage>,
    rx: Arc<Mutex<mpsc::UnboundedReceiver<AgentMessage>>>,
    executor: Arc<Mutex<CodexExecutor>>,
}

impl RealSubAgentWithExecutor {
    pub fn new(agent_type: AgentType, executor: Arc<Mutex<CodexExecutor>>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            agent_type: agent_type.clone(),
            state: Arc::new(Mutex::new(AgentState {
                agent_type,
                status: AgentStatus::Idle,
                current_task: None,
                progress: 0.0,
            })),
            tx,
            rx: Arc::new(Mutex::new(rx)),
            executor,
        }
    }

    pub fn get_sender(&self) -> mpsc::UnboundedSender<AgentMessage> {
        self.tx.clone()
    }

    /// Process task with actual Codex LLM call
    /// Best Practice: 段階的な実装 + 実際のLLM統合
    pub async fn process_task(&self, task: String) -> Result<String> {
        info!(
            "🤖 {} agent starting task: {}",
            self.agent_type,
            task.chars().take(50).collect::<String>()
        );

        // Update state
        {
            let mut state = self.state.lock().await;
            state.status = AgentStatus::Working;
            state.current_task = Some(task.clone());
            state.progress = 0.0;
        }

        // Execute with CodexExecutor
        let mut executor = self.executor.lock().await;
        let result = executor
            .execute_task(&self.agent_type, &task)
            .await
            .context("Failed to execute task with CodexExecutor")?;

        // Update state
        {
            let mut state = self.state.lock().await;
            state.status = AgentStatus::Completed;
            state.progress = 1.0;
            state.current_task = None;
        }

        info!("✅ {} agent completed task successfully", self.agent_type);

        Ok(result)
    }

    pub async fn get_state(&self) -> AgentState {
        self.state.lock().await.clone()
    }

    pub async fn send_message(&self, message: AgentMessage) -> Result<()> {
        self.tx
            .send(message)
            .context("Failed to send message to agent")?;
        Ok(())
    }

    pub async fn receive_message(&self) -> Option<AgentMessage> {
        self.rx.lock().await.recv().await
    }
}

/// Manager for RealSubAgentWithExecutor
/// Best Practice: 適切なツールとフレームワークの選定
pub struct RealSubAgentManagerWithExecutor {
    agents: Arc<Mutex<std::collections::HashMap<AgentType, RealSubAgentWithExecutor>>>,
    executor: Arc<Mutex<CodexExecutor>>,
}

impl RealSubAgentManagerWithExecutor {
    pub fn new(executor: CodexExecutor) -> Self {
        Self {
            agents: Arc::new(Mutex::new(std::collections::HashMap::new())),
            executor: Arc::new(Mutex::new(executor)),
        }
    }

    pub async fn register_agent(&self, agent_type: AgentType) {
        let agent = RealSubAgentWithExecutor::new(agent_type.clone(), Arc::clone(&self.executor));
        self.agents.lock().await.insert(agent_type, agent);
    }

    pub async fn dispatch_task(&self, agent_type: AgentType, task: String) -> Result<String> {
        let agents = self.agents.lock().await;
        let agent = agents
            .get(&agent_type)
            .context(format!("Agent {:?} not found", agent_type))?;

        agent.process_task(task).await
    }

    pub async fn get_agent_state(&self, agent_type: &AgentType) -> Option<AgentState> {
        let agents = self.agents.lock().await;
        if let Some(agent) = agents.get(agent_type) {
            Some(agent.get_state().await)
        } else {
            None
        }
    }

    pub async fn get_all_states(&self) -> Vec<AgentState> {
        let agents = self.agents.lock().await;
        let mut states = Vec::new();
        for agent in agents.values() {
            states.push(agent.get_state().await);
        }
        states
    }

    /// Register all default agents
    pub async fn register_all_agents(&self) {
        self.register_agent(AgentType::CodeExpert).await;
        self.register_agent(AgentType::SecurityExpert).await;
        self.register_agent(AgentType::TestingExpert).await;
        self.register_agent(AgentType::DocsExpert).await;
        self.register_agent(AgentType::DeepResearcher).await;
        self.register_agent(AgentType::DebugExpert).await;
        self.register_agent(AgentType::PerformanceExpert).await;
        self.register_agent(AgentType::General).await;
    }

    /// Get execution metrics
    /// Best Practice: 継続的な学習と改善
    pub async fn get_metrics_report(&self) -> String {
        let executor = self.executor.lock().await;
        executor.generate_metrics_report()
    }
}

impl Default for RealSubAgentManagerWithExecutor {
    fn default() -> Self {
        let manager = Self::new(CodexExecutor::default());
        // Note: register_all_agents is async, so it can't be called here
        // Callers should call it manually after construction
        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_real_subagent_with_executor_creation() {
        let executor = CodexExecutor::default();
        let agent =
            RealSubAgentWithExecutor::new(AgentType::CodeExpert, Arc::new(Mutex::new(executor)));

        let state = agent.get_state().await;
        assert_eq!(state.agent_type, AgentType::CodeExpert);
        assert_eq!(state.status, AgentStatus::Idle);
    }

    #[tokio::test]
    async fn test_real_subagent_manager_with_executor() {
        let manager = RealSubAgentManagerWithExecutor::default();
        manager.register_all_agents().await;

        let states = manager.get_all_states().await;
        assert_eq!(states.len(), 8); // All 8 agent types

        let report = manager.get_metrics_report().await;
        assert!(report.contains("Codex Executor Metrics"));
    }
}
