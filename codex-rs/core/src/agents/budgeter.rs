use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::debug;
use tracing::warn;

/// トークン予算管理
#[derive(Debug, Clone)]
pub struct TokenBudgeter {
    /// 全体の予算
    total_budget: usize,
    /// 使用済みトークン
    used: Arc<Mutex<usize>>,
    /// エージェント別の使用量
    agent_usage: Arc<Mutex<HashMap<String, usize>>>,
    /// エージェント別の予算上限
    agent_limits: Arc<Mutex<HashMap<String, usize>>>,
}

impl TokenBudgeter {
    /// 新しいBudgeterを作成
    pub fn new(total_budget: usize) -> Self {
        Self {
            total_budget,
            used: Arc::new(Mutex::new(0)),
            agent_usage: Arc::new(Mutex::new(HashMap::new())),
            agent_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// エージェントの予算上限を設定
    pub fn set_agent_limit(&self, agent_name: &str, limit: usize) -> Result<()> {
        let mut limits = self.agent_limits.lock().unwrap();
        limits.insert(agent_name.to_string(), limit);
        debug!("Set token limit for agent '{}': {}", agent_name, limit);
        Ok(())
    }

    /// トークン使用を試行
    pub fn try_consume(&self, agent_name: &str, tokens: usize) -> Result<bool> {
        let mut used = self.used.lock().unwrap();
        let mut agent_usage = self.agent_usage.lock().unwrap();
        let agent_limits = self.agent_limits.lock().unwrap();

        // 全体予算チェック
        if *used + tokens > self.total_budget {
            warn!(
                "Total budget exceeded: {} + {} > {}",
                *used, tokens, self.total_budget
            );
            return Ok(false);
        }

        // エージェント別予算チェック
        let agent_used = agent_usage.get(agent_name).copied().unwrap_or(0);
        if let Some(&limit) = agent_limits.get(agent_name) {
            if agent_used + tokens > limit {
                warn!(
                    "Agent '{}' budget exceeded: {} + {} > {}",
                    agent_name, agent_used, tokens, limit
                );
                return Ok(false);
            }
        }

        // 使用量を更新
        *used += tokens;
        *agent_usage.entry(agent_name.to_string()).or_insert(0) += tokens;

        debug!(
            "Agent '{}' consumed {} tokens (total: {}/{})",
            agent_name, tokens, *used, self.total_budget
        );

        Ok(true)
    }

    /// 強制的にトークンを消費（予算チェックなし）
    pub fn force_consume(&self, agent_name: &str, tokens: usize) {
        let mut used = self.used.lock().unwrap();
        let mut agent_usage = self.agent_usage.lock().unwrap();

        *used += tokens;
        *agent_usage.entry(agent_name.to_string()).or_insert(0) += tokens;

        debug!("Agent '{}' force consumed {} tokens", agent_name, tokens);
    }

    /// 使用済みトークン数を取得
    pub fn get_used(&self) -> usize {
        *self.used.lock().unwrap()
    }

    /// 残りトークン数を取得
    pub fn get_remaining(&self) -> usize {
        let used = *self.used.lock().unwrap();
        self.total_budget.saturating_sub(used)
    }

    /// エージェント別の使用量を取得
    pub fn get_agent_usage(&self, agent_name: &str) -> usize {
        self.agent_usage
            .lock()
            .unwrap()
            .get(agent_name)
            .copied()
            .unwrap_or(0)
    }

    /// すべてのエージェントの使用量を取得
    pub fn get_all_usage(&self) -> HashMap<String, usize> {
        self.agent_usage.lock().unwrap().clone()
    }

    /// 予算を動的に再配分
    pub fn rebalance(&self, redistributions: HashMap<String, usize>) -> Result<()> {
        let mut agent_limits = self.agent_limits.lock().unwrap();

        for (agent_name, new_limit) in redistributions {
            agent_limits.insert(agent_name.clone(), new_limit);
            debug!("Rebalanced budget for '{}': {}", agent_name, new_limit);
        }

        Ok(())
    }

    /// 使用率を取得（0.0 ~ 1.0）
    pub fn get_utilization(&self) -> f64 {
        let used = *self.used.lock().unwrap();
        if self.total_budget == 0 {
            0.0
        } else {
            used as f64 / self.total_budget as f64
        }
    }

    /// 軽量版フォールバックが必要かチェック
    pub fn should_fallback_lightweight(&self, threshold: f64) -> bool {
        self.get_utilization() >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_token_budgeter_basic() {
        let budgeter = TokenBudgeter::new(1000);

        budgeter.set_agent_limit("agent1", 500).unwrap();

        assert!(budgeter.try_consume("agent1", 300).unwrap());
        assert_eq!(budgeter.get_used(), 300);
        assert_eq!(budgeter.get_remaining(), 700);
        assert_eq!(budgeter.get_agent_usage("agent1"), 300);
    }

    #[test]
    fn test_total_budget_exceeded() {
        let budgeter = TokenBudgeter::new(1000);

        assert!(budgeter.try_consume("agent1", 600).unwrap());
        assert!(!budgeter.try_consume("agent2", 500).unwrap());
        assert_eq!(budgeter.get_used(), 600);
    }

    #[test]
    fn test_agent_limit_exceeded() {
        let budgeter = TokenBudgeter::new(1000);
        budgeter.set_agent_limit("agent1", 300).unwrap();

        assert!(budgeter.try_consume("agent1", 200).unwrap());
        assert!(!budgeter.try_consume("agent1", 200).unwrap());
        assert_eq!(budgeter.get_agent_usage("agent1"), 200);
    }

    #[test]
    fn test_utilization() {
        let budgeter = TokenBudgeter::new(1000);

        budgeter.force_consume("agent1", 500);
        assert_eq!(budgeter.get_utilization(), 0.5);

        budgeter.force_consume("agent2", 300);
        assert_eq!(budgeter.get_utilization(), 0.8);
    }

    #[test]
    fn test_should_fallback() {
        let budgeter = TokenBudgeter::new(1000);

        budgeter.force_consume("agent1", 850);
        assert!(budgeter.should_fallback_lightweight(0.8));
        assert!(!budgeter.should_fallback_lightweight(0.9));
    }

    #[test]
    fn test_rebalance() {
        let budgeter = TokenBudgeter::new(1000);

        budgeter.set_agent_limit("agent1", 400).unwrap();
        budgeter.set_agent_limit("agent2", 400).unwrap();

        let mut redistributions = HashMap::new();
        redistributions.insert("agent1".to_string(), 600);
        redistributions.insert("agent2".to_string(), 200);

        budgeter.rebalance(redistributions).unwrap();

        assert!(budgeter.try_consume("agent1", 500).unwrap());
        assert!(!budgeter.try_consume("agent2", 300).unwrap());
    }
}
