// サブエージェント通信プロトコル（gemini-cli風）
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// サブエージェントの種類
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    /// コード分析・生成エージェント
    CodeExpert,
    /// セキュリティ専門エージェント
    SecurityExpert,
    /// テスト専門エージェント
    TestingExpert,
    /// ドキュメント専門エージェント
    DocsExpert,
    /// 深層研究エージェント
    DeepResearcher,
    /// デバッグ専門エージェント
    DebugExpert,
    /// パフォーマンス最適化エージェント
    PerformanceExpert,
    /// 汎用エージェント
    General,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodeExpert => write!(f, "CodeExpert"),
            Self::SecurityExpert => write!(f, "SecurityExpert"),
            Self::TestingExpert => write!(f, "TestingExpert"),
            Self::DocsExpert => write!(f, "DocsExpert"),
            Self::DeepResearcher => write!(f, "DeepResearcher"),
            Self::DebugExpert => write!(f, "DebugExpert"),
            Self::PerformanceExpert => write!(f, "PerformanceExpert"),
            Self::General => write!(f, "General"),
        }
    }
}

/// サブエージェント間のメッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: AgentType,
    pub to: Option<AgentType>, // None = ブロードキャスト
    pub content: String,
    pub metadata: HashMap<String, String>,
}

/// サブエージェントの状態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub current_task: Option<String>,
    pub progress: f32, // 0.0-1.0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Working,
    Waiting,
    Completed,
    Failed,
}

/// サブエージェントの実装
pub struct SubAgent {
    agent_type: AgentType,
    state: AgentState,
    tx: mpsc::UnboundedSender<AgentMessage>,
    rx: mpsc::UnboundedReceiver<AgentMessage>,
}

impl SubAgent {
    pub fn new(agent_type: AgentType) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            agent_type: agent_type.clone(),
            state: AgentState {
                agent_type,
                status: AgentStatus::Idle,
                current_task: None,
                progress: 0.0,
            },
            tx,
            rx,
        }
    }

    pub fn get_sender(&self) -> mpsc::UnboundedSender<AgentMessage> {
        self.tx.clone()
    }

    pub async fn process_task(&mut self, task: String) -> Result<String> {
        self.state.status = AgentStatus::Working;
        self.state.current_task = Some(task.clone());
        self.state.progress = 0.0;

        // シミュレーション：実際にはLLM呼び出しやツール実行を行う
        let result = match self.agent_type {
            AgentType::CodeExpert => self.process_code_task(&task).await?,
            AgentType::SecurityExpert => self.process_security_task(&task).await?,
            AgentType::TestingExpert => self.process_testing_task(&task).await?,
            AgentType::DocsExpert => self.process_docs_task(&task).await?,
            AgentType::DeepResearcher => self.process_research_task(&task).await?,
            AgentType::DebugExpert => self.process_debug_task(&task).await?,
            AgentType::PerformanceExpert => self.process_performance_task(&task).await?,
            AgentType::General => self.process_general_task(&task).await?,
        };

        self.state.status = AgentStatus::Completed;
        self.state.progress = 1.0;

        Ok(result)
    }

    async fn process_code_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.3).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(0.7).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!(
            "[CodeExpert] Analyzed and generated code for: {task}"
        ))
    }

    async fn process_security_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.5).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!("[SecurityExpert] Reviewed security for: {task}"))
    }

    async fn process_testing_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.4).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(0.8).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!("[TestingExpert] Created tests for: {task}"))
    }

    async fn process_docs_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.5).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!("[DocsExpert] Generated documentation for: {task}"))
    }

    async fn process_research_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.2).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(0.5).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(0.8).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!(
            "[DeepResearcher] Conducted deep research on: {task}"
        ))
    }

    async fn process_debug_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.6).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!("[DebugExpert] Debugged: {task}"))
    }

    async fn process_performance_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.3).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(0.7).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!("[PerformanceExpert] Optimized: {task}"))
    }

    async fn process_general_task(&mut self, task: &str) -> Result<String> {
        self.update_progress(0.5).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_progress(1.0).await;
        Ok(format!("[General] Completed: {task}"))
    }

    async fn update_progress(&mut self, progress: f32) {
        self.state.progress = progress;
    }

    pub fn get_state(&self) -> AgentState {
        self.state.clone()
    }

    pub async fn send_message(&self, message: AgentMessage) -> Result<()> {
        self.tx
            .send(message)
            .context("Failed to send message to agent")?;
        Ok(())
    }

    pub async fn receive_message(&mut self) -> Option<AgentMessage> {
        self.rx.recv().await
    }
}

/// サブエージェント管理システム
pub struct SubAgentManager {
    agents: HashMap<AgentType, SubAgent>,
}

impl SubAgentManager {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub fn register_agent(&mut self, agent_type: AgentType) {
        let agent = SubAgent::new(agent_type.clone());
        self.agents.insert(agent_type, agent);
    }

    pub async fn dispatch_task(&mut self, agent_type: AgentType, task: String) -> Result<String> {
        let agent = self
            .agents
            .get_mut(&agent_type)
            .context("Agent not found")?;
        agent.process_task(task).await
    }

    pub fn get_agent_state(&self, agent_type: &AgentType) -> Option<AgentState> {
        self.agents.get(agent_type).map(|a| a.get_state())
    }

    pub fn get_all_states(&self) -> Vec<AgentState> {
        self.agents.values().map(|a| a.get_state()).collect()
    }

    pub async fn broadcast_message(&self, message: AgentMessage) -> Result<()> {
        for agent in self.agents.values() {
            agent.send_message(message.clone()).await?;
        }
        Ok(())
    }
}

impl Default for SubAgentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_subagent_process_task() {
        let mut agent = SubAgent::new(AgentType::CodeExpert);
        let result = agent
            .process_task("Build auth system".to_string())
            .await
            .unwrap();

        assert!(result.contains("CodeExpert"));
        assert!(result.contains("Build auth system"));
        assert_eq!(agent.get_state().status, AgentStatus::Completed);
        assert_eq!(agent.get_state().progress, 1.0);
    }

    #[tokio::test]
    async fn test_subagent_manager() {
        let mut manager = SubAgentManager::new();
        manager.register_agent(AgentType::CodeExpert);
        manager.register_agent(AgentType::SecurityExpert);

        let result = manager
            .dispatch_task(AgentType::CodeExpert, "Test task".to_string())
            .await
            .unwrap();

        assert!(result.contains("CodeExpert"));

        let states = manager.get_all_states();
        assert_eq!(states.len(), 2);
    }

    #[tokio::test]
    async fn test_different_agent_types() {
        let agent_types = vec![
            AgentType::CodeExpert,
            AgentType::SecurityExpert,
            AgentType::TestingExpert,
            AgentType::DocsExpert,
            AgentType::DeepResearcher,
            AgentType::DebugExpert,
            AgentType::PerformanceExpert,
            AgentType::General,
        ];

        for agent_type in agent_types {
            let mut agent = SubAgent::new(agent_type.clone());
            let result = agent.process_task("test".to_string()).await.unwrap();
            assert!(result.contains(&agent_type.to_string()));
        }
    }

    #[tokio::test]
    async fn test_agent_message() {
        let message = AgentMessage {
            from: AgentType::CodeExpert,
            to: Some(AgentType::SecurityExpert),
            content: "Review this code".to_string(),
            metadata: HashMap::new(),
        };

        let agent = SubAgent::new(AgentType::SecurityExpert);
        agent.send_message(message.clone()).await.unwrap();
    }
}
