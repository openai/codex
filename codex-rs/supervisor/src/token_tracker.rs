// トークン消費追跡と分担管理システム
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::subagent::AgentType;

/// トークン使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub timestamp: String,
}

impl TokenUsage {
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn add(&mut self, other: &TokenUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }
}

/// エージェント別トークン使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTokenUsage {
    pub agent_type: AgentType,
    pub agent_id: String,
    pub total_usage: TokenUsage,
    pub task_usages: Vec<TaskTokenUsage>,
}

impl AgentTokenUsage {
    pub fn new(agent_type: AgentType, agent_id: String) -> Self {
        Self {
            agent_type,
            agent_id,
            total_usage: TokenUsage::new(0, 0),
            task_usages: Vec::new(),
        }
    }

    pub fn add_task_usage(&mut self, task_usage: TaskTokenUsage) {
        self.total_usage.add(&task_usage.usage);
        self.task_usages.push(task_usage);
    }

    pub fn get_average_tokens_per_task(&self) -> f64 {
        if self.task_usages.is_empty() {
            return 0.0;
        }
        self.total_usage.total_tokens as f64 / self.task_usages.len() as f64
    }
}

/// タスク別トークン使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTokenUsage {
    pub task_id: String,
    pub task_description: String,
    pub usage: TokenUsage,
    pub start_time: String,
    pub end_time: String,
}

impl TaskTokenUsage {
    pub fn new(task_id: String, task_description: String, usage: TokenUsage) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            task_id,
            task_description,
            usage,
            start_time: now.clone(),
            end_time: now,
        }
    }
}

/// トークン分担戦略
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenAllocationStrategy {
    /// 均等分配
    Equal,
    /// 優先度ベース
    PriorityBased,
    /// 負荷ベース（軽いエージェントに多く割り当て）
    LoadBased,
    /// 動的調整
    Dynamic,
}

/// トークン制限
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLimit {
    pub max_tokens_per_task: u64,
    pub max_tokens_per_agent: u64,
    pub max_tokens_total: u64,
    pub warning_threshold: f64, // 0.0-1.0
}

impl Default for TokenLimit {
    fn default() -> Self {
        Self {
            max_tokens_per_task: 4096,
            max_tokens_per_agent: 100_000,
            max_tokens_total: 1_000_000,
            warning_threshold: 0.8, // 80%で警告
        }
    }
}

/// トークン追跡システム
#[derive(Debug)]
pub struct TokenTracker {
    agent_usages: Arc<RwLock<HashMap<String, AgentTokenUsage>>>,
    total_usage: Arc<RwLock<TokenUsage>>,
    limits: TokenLimit,
    strategy: TokenAllocationStrategy,
}

impl TokenTracker {
    pub fn new(limits: TokenLimit, strategy: TokenAllocationStrategy) -> Self {
        Self {
            agent_usages: Arc::new(RwLock::new(HashMap::new())),
            total_usage: Arc::new(RwLock::new(TokenUsage::new(0, 0))),
            limits,
            strategy,
        }
    }

    /// エージェントを登録
    pub async fn register_agent(&self, agent_type: AgentType, agent_id: String) {
        let mut usages = self.agent_usages.write().await;
        usages.insert(agent_id.clone(), AgentTokenUsage::new(agent_type, agent_id));
    }

    /// トークン使用量を記録
    pub async fn record_usage(
        &self,
        agent_id: &str,
        task_id: String,
        task_description: String,
        usage: TokenUsage,
    ) -> Result<()> {
        // エージェント別使用量を更新
        {
            let mut usages = self.agent_usages.write().await;
            if let Some(agent_usage) = usages.get_mut(agent_id) {
                let task_usage = TaskTokenUsage::new(task_id, task_description, usage.clone());
                agent_usage.add_task_usage(task_usage);
            }
        }

        // 全体使用量を更新
        {
            let mut total = self.total_usage.write().await;
            total.add(&usage);
        }

        // 制限チェック
        self.check_limits(agent_id).await?;

        Ok(())
    }

    /// 制限をチェック
    async fn check_limits(&self, agent_id: &str) -> Result<()> {
        let usages = self.agent_usages.read().await;
        let total = self.total_usage.read().await;

        // エージェント別制限チェック
        if let Some(agent_usage) = usages.get(agent_id) {
            let usage_ratio = agent_usage.total_usage.total_tokens as f64
                / self.limits.max_tokens_per_agent as f64;

            if usage_ratio >= 1.0 {
                anyhow::bail!(
                    "Agent {} exceeded token limit: {} / {}",
                    agent_id,
                    agent_usage.total_usage.total_tokens,
                    self.limits.max_tokens_per_agent
                );
            }

            if usage_ratio >= self.limits.warning_threshold {
                eprintln!(
                    "Warning: Agent {} is at {:.1}% of token limit",
                    agent_id,
                    usage_ratio * 100.0
                );
            }
        }

        // 全体制限チェック
        let total_ratio = total.total_tokens as f64 / self.limits.max_tokens_total as f64;

        if total_ratio >= 1.0 {
            anyhow::bail!(
                "Total token limit exceeded: {} / {}",
                total.total_tokens,
                self.limits.max_tokens_total
            );
        }

        if total_ratio >= self.limits.warning_threshold {
            eprintln!(
                "Warning: Total token usage is at {:.1}% of limit",
                total_ratio * 100.0
            );
        }

        Ok(())
    }

    /// エージェントの使用量を取得
    pub async fn get_agent_usage(&self, agent_id: &str) -> Option<AgentTokenUsage> {
        let usages = self.agent_usages.read().await;
        usages.get(agent_id).cloned()
    }

    /// 全体の使用量を取得
    pub async fn get_total_usage(&self) -> TokenUsage {
        let total = self.total_usage.read().await;
        total.clone()
    }

    /// 全エージェントの使用量を取得
    pub async fn get_all_agent_usages(&self) -> Vec<AgentTokenUsage> {
        let usages = self.agent_usages.read().await;
        usages.values().cloned().collect()
    }

    /// サマリーレポートを生成
    pub async fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Token Usage Report ===\n\n");

        // 全体使用量
        let total = self.get_total_usage().await;
        report.push_str(&format!("Total Usage:\n"));
        report.push_str(&format!("  Prompt Tokens: {}\n", total.prompt_tokens));
        report.push_str(&format!(
            "  Completion Tokens: {}\n",
            total.completion_tokens
        ));
        report.push_str(&format!("  Total Tokens: {}\n", total.total_tokens));
        report.push_str(&format!(
            "  Percentage: {:.1}%\n\n",
            (total.total_tokens as f64 / self.limits.max_tokens_total as f64) * 100.0
        ));

        // エージェント別使用量
        report.push_str("Agent Usage:\n");
        let agent_usages = self.get_all_agent_usages().await;
        for agent_usage in agent_usages {
            report.push_str(&format!(
                "  {} ({}):\n",
                agent_usage.agent_type, agent_usage.agent_id
            ));
            report.push_str(&format!(
                "    Total: {} tokens\n",
                agent_usage.total_usage.total_tokens
            ));
            report.push_str(&format!("    Tasks: {}\n", agent_usage.task_usages.len()));
            report.push_str(&format!(
                "    Avg per Task: {:.1} tokens\n",
                agent_usage.get_average_tokens_per_task()
            ));
            report.push_str(&format!(
                "    Percentage: {:.1}%\n\n",
                (agent_usage.total_usage.total_tokens as f64
                    / self.limits.max_tokens_per_agent as f64)
                    * 100.0
            ));
        }

        report
    }

    /// トークン割り当てを計算
    pub async fn calculate_allocation(&self, required_tokens: u64) -> HashMap<String, u64> {
        let mut allocation = HashMap::new();
        let usages = self.agent_usages.read().await;

        match self.strategy {
            TokenAllocationStrategy::Equal => {
                // 均等分配
                let per_agent = required_tokens / usages.len() as u64;
                for (agent_id, _) in usages.iter() {
                    allocation.insert(agent_id.clone(), per_agent);
                }
            }
            TokenAllocationStrategy::LoadBased => {
                // 負荷ベース分配（使用量が少ないエージェントに多く割り当て）
                let total_used: u64 = usages.values().map(|u| u.total_usage.total_tokens).sum();

                for (agent_id, usage) in usages.iter() {
                    let usage_ratio = if total_used > 0 {
                        usage.total_usage.total_tokens as f64 / total_used as f64
                    } else {
                        1.0 / usages.len() as f64
                    };

                    // 逆比例で割り当て
                    let allocated = (required_tokens as f64 * (1.0 - usage_ratio)) as u64;
                    allocation.insert(agent_id.clone(), allocated);
                }
            }
            TokenAllocationStrategy::PriorityBased | TokenAllocationStrategy::Dynamic => {
                // 簡易実装: 均等分配
                let per_agent = required_tokens / usages.len() as u64;
                for (agent_id, _) in usages.iter() {
                    allocation.insert(agent_id.clone(), per_agent);
                }
            }
        }

        allocation
    }

    /// リセット
    pub async fn reset(&self) {
        let mut usages = self.agent_usages.write().await;
        let mut total = self.total_usage.write().await;

        usages.clear();
        *total = TokenUsage::new(0, 0);
    }
}

impl Default for TokenTracker {
    fn default() -> Self {
        Self::new(TokenLimit::default(), TokenAllocationStrategy::Dynamic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_token_usage() {
        let mut usage1 = TokenUsage::new(100, 50);
        let usage2 = TokenUsage::new(200, 100);

        assert_eq!(usage1.total_tokens, 150);

        usage1.add(&usage2);
        assert_eq!(usage1.total_tokens, 450); // 150 + 300
    }

    #[tokio::test]
    async fn test_token_tracker() {
        let tracker = TokenTracker::default();

        // エージェントを登録
        tracker
            .register_agent(AgentType::CodeExpert, "agent-1".to_string())
            .await;

        // 使用量を記録
        let usage = TokenUsage::new(100, 50);
        tracker
            .record_usage(
                "agent-1",
                "task-1".to_string(),
                "Test task".to_string(),
                usage,
            )
            .await
            .unwrap();

        // 使用量を取得
        let agent_usage = tracker.get_agent_usage("agent-1").await.unwrap();
        assert_eq!(agent_usage.total_usage.total_tokens, 150);
        assert_eq!(agent_usage.task_usages.len(), 1);

        let total_usage = tracker.get_total_usage().await;
        assert_eq!(total_usage.total_tokens, 150);
    }

    #[tokio::test]
    async fn test_token_limit_check() {
        let limits = TokenLimit {
            max_tokens_per_task: 100,
            max_tokens_per_agent: 500,
            max_tokens_total: 1000,
            warning_threshold: 0.8,
        };

        let tracker = TokenTracker::new(limits, TokenAllocationStrategy::Equal);

        tracker
            .register_agent(AgentType::CodeExpert, "agent-1".to_string())
            .await;

        // 制限以下
        let usage1 = TokenUsage::new(200, 100);
        assert!(tracker
            .record_usage(
                "agent-1",
                "task-1".to_string(),
                "Test 1".to_string(),
                usage1
            )
            .await
            .is_ok());

        // 制限超過
        let usage2 = TokenUsage::new(300, 200);
        assert!(tracker
            .record_usage(
                "agent-1",
                "task-2".to_string(),
                "Test 2".to_string(),
                usage2
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_token_allocation() {
        let tracker = TokenTracker::new(TokenLimit::default(), TokenAllocationStrategy::Equal);

        tracker
            .register_agent(AgentType::CodeExpert, "agent-1".to_string())
            .await;
        tracker
            .register_agent(AgentType::SecurityExpert, "agent-2".to_string())
            .await;

        let allocation = tracker.calculate_allocation(1000).await;
        assert_eq!(allocation.len(), 2);
        assert_eq!(*allocation.get("agent-1").unwrap(), 500);
        assert_eq!(*allocation.get("agent-2").unwrap(), 500);
    }
}
