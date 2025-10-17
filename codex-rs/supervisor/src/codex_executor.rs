// Codex Executor for SubAgents - Best Practices Implementation
use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InitialHistory;
use codex_core::protocol::Op;
use codex_core::protocol::SessionSource;
use codex_core::AuthManager;
use codex_core::Codex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::agent_prompts::get_agent_prompt;
use crate::subagent::AgentType;

/// Codex executor for real LLM calls
/// Implements AIエージェントベストプラクティス:
/// 1. 明確な要件定義と段階的な実装
/// 2. ハイブリッドアプローチ（ルールベース + ML）
/// 3. 環境認識能力の強化
/// 4. 継続的な学習と改善
/// 5. 適切なツールとフレームワーク
pub struct CodexExecutor {
    config: Arc<Config>,
    auth_manager: Arc<AuthManager>,
    metrics: ExecutionMetrics,
}

/// Execution metrics for continuous improvement
#[derive(Debug, Clone, Default)]
pub struct ExecutionMetrics {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub total_tokens: u64,
    pub average_latency_ms: u64,
}

impl CodexExecutor {
    /// Create a new CodexExecutor with configuration
    pub fn new(config: Config, auth_manager: Arc<AuthManager>) -> Self {
        Self {
            config: Arc::new(config),
            auth_manager,
            metrics: ExecutionMetrics::default(),
        }
    }

    /// Execute a task with the Codex LLM
    /// Best Practice: 段階的な実装 - シンプルな呼び出しから開始
    pub async fn execute_task(&mut self, agent_type: &AgentType, task: &str) -> Result<String> {
        let start_time = std::time::Instant::now();

        info!(
            "🚀 Starting Codex execution for {} agent: {}",
            agent_type,
            task.chars().take(50).collect::<String>()
        );

        // Best Practice: 環境認識能力 - 専門プロンプトを使用
        let specialized_prompt = get_agent_prompt(agent_type, task);

        debug!("Specialized prompt prepared for {}", agent_type);

        // Execute with Codex
        let result = self
            .execute_with_codex(&specialized_prompt)
            .await
            .context("Failed to execute with Codex")?;

        // Update metrics for continuous improvement
        let latency = start_time.elapsed().as_millis() as u64;
        self.update_metrics(true, latency);

        info!(
            "✅ Codex execution completed for {} in {}ms",
            agent_type, latency
        );

        Ok(result)
    }

    /// Execute with Codex instance
    /// Best Practice: ハイブリッドアプローチ - 実際のLLM呼び出し
    async fn execute_with_codex(&self, prompt: &str) -> Result<String> {
        // Spawn Codex instance
        let codex_spawn = Codex::spawn(
            (*self.config).clone(),
            Arc::clone(&self.auth_manager),
            InitialHistory::New,
            SessionSource::Exec,
        )
        .await
        .context("Failed to spawn Codex instance")?;

        let mut codex = codex_spawn.codex;
        let conversation_id = codex_spawn.conversation_id;

        debug!("Codex instance spawned: {:?}", conversation_id);

        // Submit user turn with specialized prompt
        let submission_id = codex
            .submit(Op::UserTurn {
                input: prompt.to_string(),
            })
            .await
            .context("Failed to submit user turn")?;

        debug!("User turn submitted: {}", submission_id);

        // Collect response with timeout
        let response = self
            .collect_response(&mut codex, Duration::from_secs(60))
            .await
            .context("Failed to collect response")?;

        // Shutdown Codex session
        codex
            .submit(Op::Shutdown)
            .await
            .context("Failed to shutdown Codex")?;

        debug!("Codex session shut down successfully");

        Ok(response)
    }

    /// Collect response from Codex events
    /// Best Practice: エラーハンドリング - タイムアウトとエラー処理
    async fn collect_response(
        &self,
        codex: &mut Codex,
        timeout_duration: Duration,
    ) -> Result<String> {
        let mut response_text = String::new();
        let mut turn_completed = false;

        // Wait for events with timeout
        while !turn_completed {
            let event = timeout(timeout_duration, codex.next_event())
                .await
                .context("Timeout waiting for Codex response")?
                .context("Failed to receive event")?;

            match event.msg {
                EventMsg::SessionConfigured(_) => {
                    debug!("Session configured");
                }
                EventMsg::Text(text_event) => {
                    debug!(
                        "Received text: {}",
                        text_event.text.chars().take(50).collect::<String>()
                    );
                    response_text.push_str(&text_event.text);
                }
                EventMsg::TurnComplete(_) => {
                    info!("Turn completed");
                    turn_completed = true;
                }
                EventMsg::AgentMessage(msg) => {
                    debug!(
                        "Agent message: {}",
                        msg.content.chars().take(50).collect::<String>()
                    );
                    response_text.push_str(&msg.content);
                }
                EventMsg::Error(err) => {
                    error!("Codex error: {}", err.error);
                    return Err(anyhow::anyhow!("Codex error: {}", err.error));
                }
                _ => {
                    // Handle other event types
                    debug!("Other event received: {:?}", event.msg);
                }
            }
        }

        if response_text.is_empty() {
            warn!("Empty response from Codex");
            return Ok("[No response generated]".to_string());
        }

        Ok(response_text)
    }

    /// Update execution metrics
    /// Best Practice: 継続的な学習と改善 - メトリクス収集
    fn update_metrics(&mut self, success: bool, latency_ms: u64) {
        self.metrics.total_calls += 1;
        if success {
            self.metrics.successful_calls += 1;
        } else {
            self.metrics.failed_calls += 1;
        }

        // Update average latency
        let total_latency = self.metrics.average_latency_ms * (self.metrics.total_calls - 1);
        self.metrics.average_latency_ms = (total_latency + latency_ms) / self.metrics.total_calls;

        debug!("Metrics updated: {:?}", self.metrics);
    }

    /// Get current execution metrics
    /// Best Practice: 継続的な学習と改善 - パフォーマンス可視化
    pub fn get_metrics(&self) -> ExecutionMetrics {
        self.metrics.clone()
    }

    /// Generate metrics report
    pub fn generate_metrics_report(&self) -> String {
        let success_rate = if self.metrics.total_calls > 0 {
            (self.metrics.successful_calls as f64 / self.metrics.total_calls as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "📊 Codex Executor Metrics\n\n\
            Total Calls: {}\n\
            Successful: {}\n\
            Failed: {}\n\
            Success Rate: {:.2}%\n\
            Average Latency: {}ms\n\
            Total Tokens: {}",
            self.metrics.total_calls,
            self.metrics.successful_calls,
            self.metrics.failed_calls,
            success_rate,
            self.metrics.average_latency_ms,
            self.metrics.total_tokens
        )
    }
}

/// Create default CodexExecutor for testing
impl Default for CodexExecutor {
    fn default() -> Self {
        let config = Config {
            model_provider: codex_core::config_types::ModelProvider::OpenAI,
            model: "gpt-4o-mini".to_string(),
            cwd: PathBuf::from("."),
            codex_home: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".codex"),
            ..Default::default()
        };

        let auth_manager = AuthManager::shared(config.codex_home.clone(), false);

        Self::new(config, auth_manager)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_codex_executor_creation() {
        let executor = CodexExecutor::default();
        assert_eq!(executor.metrics.total_calls, 0);
        assert_eq!(executor.metrics.successful_calls, 0);
    }

    #[test]
    fn test_metrics_update() {
        let mut executor = CodexExecutor::default();
        executor.update_metrics(true, 100);

        assert_eq!(executor.metrics.total_calls, 1);
        assert_eq!(executor.metrics.successful_calls, 1);
        assert_eq!(executor.metrics.average_latency_ms, 100);
    }

    #[test]
    fn test_metrics_report_generation() {
        let mut executor = CodexExecutor::default();
        executor.update_metrics(true, 100);
        executor.update_metrics(true, 200);
        executor.update_metrics(false, 150);

        let report = executor.generate_metrics_report();
        assert!(report.contains("Total Calls: 3"));
        assert!(report.contains("Successful: 2"));
        assert!(report.contains("Failed: 1"));
    }
}
