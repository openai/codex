//! Token budget tracking and monitoring for AI agent usage.
//!
//! Provides thread-safe token usage tracking with configurable budgets,
//! warning thresholds, and per-agent limits.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Helper to convert lock poisoned errors
fn lock_poisoned_err<T>(_: T) -> anyhow::Error {
    anyhow!(
        "Token budget tracker lock is poisoned, indicating a panic occurred while holding the lock. This typically requires restarting the process."
    )
}

/// Token usage entry for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Agent identifier
    pub agent_id: String,
    /// Model used
    pub model: String,
    /// Prompt tokens consumed
    pub prompt_tokens: u64,
    /// Completion tokens generated
    pub completion_tokens: u64,
    /// Total tokens (prompt + completion)
    pub total_tokens: u64,
    /// Timestamp of usage
    pub timestamp: u64,
}

/// Budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Total token budget (0 = unlimited)
    pub total_budget: u64,
    /// Warning threshold (percentage: 0-100)
    pub warning_threshold: u8,
    /// Per-agent token limits
    pub per_agent_limits: HashMap<String, u64>,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            total_budget: 1_000_000, // 1M tokens default
            warning_threshold: 80,   // 80% threshold
            per_agent_limits: HashMap::new(),
        }
    }
}

/// Token budget tracker (thread-safe)
#[derive(Debug, Clone)]
pub struct TokenBudgetTracker {
    inner: Arc<RwLock<TrackerInner>>,
}

#[derive(Debug)]
struct TrackerInner {
    config: BudgetConfig,
    usage_history: Vec<TokenUsage>,
    total_used: u64,
    agent_totals: HashMap<String, u64>,
    warning_emitted: bool,
}

impl TokenBudgetTracker {
    /// Create a new tracker with the given configuration
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(TrackerInner {
                config,
                usage_history: Vec::new(),
                total_used: 0,
                agent_totals: HashMap::new(),
                warning_emitted: false,
            })),
        }
    }

    /// Create a tracker with default configuration
    pub fn with_defaults() -> Self {
        Self::new(BudgetConfig::default())
    }

    /// Report token usage for an agent
    pub fn report_usage(
        &self,
        agent_id: String,
        model: String,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> Result<()> {
        let total_tokens = prompt_tokens + completion_tokens;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut inner = self.inner.write().map_err(lock_poisoned_err)?;

        // Check per-agent limit
        if let Some(&limit) = inner.config.per_agent_limits.get(&agent_id) {
            let agent_used = inner.agent_totals.get(&agent_id).copied().unwrap_or(0);
            if agent_used + total_tokens > limit {
                return Err(anyhow!(
                    "Agent {} would exceed limit of {} tokens (current: {}, requested: {})",
                    agent_id,
                    limit,
                    agent_used,
                    total_tokens
                ));
            }
        }

        // Check total budget
        if inner.config.total_budget > 0 {
            if inner.total_used + total_tokens > inner.config.total_budget {
                return Err(anyhow!(
                    "Total budget of {} tokens would be exceeded (current: {}, requested: {})",
                    inner.config.total_budget,
                    inner.total_used,
                    total_tokens
                ));
            }
        }

        // Record usage
        let usage = TokenUsage {
            agent_id: agent_id.clone(),
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            timestamp,
        };

        inner.usage_history.push(usage);
        inner.total_used += total_tokens;
        *inner.agent_totals.entry(agent_id.clone()).or_insert(0) += total_tokens;

        // Check for warning threshold
        if !inner.warning_emitted && inner.config.total_budget > 0 {
            let usage_pct = (inner.total_used as f64 / inner.config.total_budget as f64) * 100.0;
            if usage_pct >= inner.config.warning_threshold as f64 {
                inner.warning_emitted = true;
                tracing::warn!(
                    "Token budget warning: {:.1}% used ({} / {})",
                    usage_pct,
                    inner.total_used,
                    inner.config.total_budget
                );
            }
        }

        Ok(())
    }

    /// Get total tokens used
    pub fn get_total_used(&self) -> Result<u64> {
        let inner = self.inner.read().map_err(lock_poisoned_err)?;
        Ok(inner.total_used)
    }

    /// Get usage by agent
    pub fn get_agent_usage(&self, agent_id: &str) -> Result<u64> {
        let inner = self.inner.read().map_err(lock_poisoned_err)?;
        Ok(inner.agent_totals.get(agent_id).copied().unwrap_or(0))
    }

    /// Get all agent totals
    pub fn get_all_agent_totals(&self) -> Result<HashMap<String, u64>> {
        let inner = self.inner.read().map_err(lock_poisoned_err)?;
        Ok(inner.agent_totals.clone())
    }

    /// Get remaining budget
    pub fn get_remaining_budget(&self) -> Result<Option<u64>> {
        let inner = self.inner.read().map_err(lock_poisoned_err)?;
        if inner.config.total_budget == 0 {
            Ok(None) // Unlimited
        } else {
            Ok(Some(
                inner.config.total_budget.saturating_sub(inner.total_used),
            ))
        }
    }

    /// Get budget status summary
    pub fn get_status(&self) -> Result<BudgetStatus> {
        let inner = self.inner.read().map_err(lock_poisoned_err)?;

        let usage_percentage = if inner.config.total_budget > 0 {
            Some((inner.total_used as f64 / inner.config.total_budget as f64) * 100.0)
        } else {
            None
        };

        Ok(BudgetStatus {
            total_budget: inner.config.total_budget,
            total_used: inner.total_used,
            usage_percentage,
            warning_threshold: inner.config.warning_threshold,
            agent_totals: inner.agent_totals.clone(),
        })
    }

    /// Reset usage tracking
    pub fn reset(&self) -> Result<()> {
        let mut inner = self.inner.write().map_err(lock_poisoned_err)?;
        inner.usage_history.clear();
        inner.total_used = 0;
        inner.agent_totals.clear();
        inner.warning_emitted = false;
        Ok(())
    }

    /// Update configuration
    pub fn update_config(&self, config: BudgetConfig) -> Result<()> {
        let mut inner = self.inner.write().map_err(lock_poisoned_err)?;
        inner.config = config;
        inner.warning_emitted = false; // Reset warning flag
        Ok(())
    }
}

/// Budget status snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub total_budget: u64,
    pub total_used: u64,
    pub usage_percentage: Option<f64>,
    pub warning_threshold: u8,
    pub agent_totals: HashMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tracking() {
        let tracker = TokenBudgetTracker::with_defaults();

        tracker
            .report_usage("agent1".to_string(), "gpt-4".to_string(), 100, 50)
            .unwrap();

        assert_eq!(tracker.get_total_used().unwrap(), 150);
        assert_eq!(tracker.get_agent_usage("agent1").unwrap(), 150);
    }

    #[test]
    fn test_budget_limit() {
        let config = BudgetConfig {
            total_budget: 100,
            warning_threshold: 80,
            per_agent_limits: HashMap::new(),
        };
        let tracker = TokenBudgetTracker::new(config);

        // Should succeed
        tracker
            .report_usage("agent1".to_string(), "gpt-4".to_string(), 40, 10)
            .unwrap();

        // Should fail (would exceed budget)
        let result = tracker.report_usage("agent2".to_string(), "gpt-4".to_string(), 60, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_per_agent_limit() {
        let mut per_agent_limits = HashMap::new();
        per_agent_limits.insert("agent1".to_string(), 100);

        let config = BudgetConfig {
            total_budget: 1000,
            warning_threshold: 80,
            per_agent_limits,
        };
        let tracker = TokenBudgetTracker::new(config);

        // Should succeed
        tracker
            .report_usage("agent1".to_string(), "gpt-4".to_string(), 40, 10)
            .unwrap();

        // Should fail (would exceed agent limit)
        let result = tracker.report_usage("agent1".to_string(), "gpt-4".to_string(), 60, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_reset() {
        let tracker = TokenBudgetTracker::with_defaults();

        tracker
            .report_usage("agent1".to_string(), "gpt-4".to_string(), 100, 50)
            .unwrap();

        tracker.reset().unwrap();
        assert_eq!(tracker.get_total_used().unwrap(), 0);
    }
}
