/// Async SubAgent Integration (Production Implementation)
///
/// Full-featured async subagent management system for parallel task execution.
use anyhow::Result;
/// Async SubAgent Integration (Production Implementation)
///
/// Full-featured async subagent management system for parallel task execution.
use anyhow::anyhow;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agents::AgentRuntime;

/// Async SubAgent Integration
pub struct AsyncSubAgentIntegration {
    /// Agent runtime for executing agents
    runtime: Arc<AgentRuntime>,
    /// Active agents (agent_id -> task handle)
    active_agents: Arc<Mutex<HashMap<String, JoinHandle<Result<String>>>>>,
    /// Agent states
    agent_states: Arc<Mutex<HashMap<String, AgentState>>>,
    /// Notification channel sender
    notification_tx: mpsc::UnboundedSender<AgentNotification>,
    /// Notification channel receiver
    notification_rx: Arc<Mutex<mpsc::UnboundedReceiver<AgentNotification>>>,
    /// Token usage tracker
    token_usage: Arc<Mutex<HashMap<String, TokenUsageStats>>>,
    /// Task results
    task_results: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Debug, Clone, Default)]
struct TokenUsageStats {
    total_tokens: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    last_task_id: Option<String>,
    last_description: Option<String>,
}

impl AsyncSubAgentIntegration {
    /// Create a new AsyncSubAgentIntegration instance
    pub fn new(runtime: Arc<AgentRuntime>) -> Self {
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();

        Self {
            runtime,
            active_agents: Arc::new(Mutex::new(HashMap::new())),
            agent_states: Arc::new(Mutex::new(HashMap::new())),
            notification_tx,
            notification_rx: Arc::new(Mutex::new(notification_rx)),
            token_usage: Arc::new(Mutex::new(HashMap::new())),
            task_results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start monitoring loop
    pub async fn start_monitoring_loop<T>(&self, _session: Arc<T>) -> Result<()> {
        let notification_rx = Arc::clone(&self.notification_rx);
        let agent_states = Arc::clone(&self.agent_states);

        tokio::spawn(async move {
            let mut rx = notification_rx.lock().await;
            while let Some(notification) = rx.recv().await {
                // Update agent state based on notification
                let mut states = agent_states.lock().await;
                if let Some(state) = states.get_mut(&notification.agent_id) {
                    match notification.notification_type {
                        NotificationType::TaskCompleted => {
                            state.status = "completed".to_string();
                            state.progress = 100.0;
                        }
                        NotificationType::TaskFailed => {
                            state.status = "failed".to_string();
                        }
                        NotificationType::ProgressUpdate => {
                            // Parse progress from message if available
                            state.status = "running".to_string();
                        }
                        _ => {}
                    }
                }

                tracing::info!(
                    "Agent notification: {} - {} - {}",
                    notification.agent_id,
                    notification.agent_type.as_str(),
                    notification.message
                );
            }
        });

        Ok(())
    }

    /// Start agent
    pub async fn start_agent(&self, agent_type: AgentType, task: &str) -> Result<String> {
        let agent_id = format!("{}-{}", agent_type.as_str(), uuid::Uuid::new_v4());

        // Create agent state
        let state = AgentState {
            agent_id: agent_id.clone(),
            agent_type,
            status: "initializing".to_string(),
            progress: 0.0,
        };

        self.agent_states
            .lock()
            .await
            .insert(agent_id.clone(), state);

        // Spawn agent task
        let runtime = Arc::clone(&self.runtime);
        let notification_tx = self.notification_tx.clone();
        let agent_id_clone = agent_id.clone();
        let task_clone = task.to_string();
        let token_usage = Arc::clone(&self.token_usage);
        let task_results = Arc::clone(&self.task_results);
        let agent_states = Arc::clone(&self.agent_states);

        let handle = tokio::spawn(async move {
            // Update state to running
            {
                let mut states = agent_states.lock().await;
                if let Some(state) = states.get_mut(&agent_id_clone) {
                    state.status = "running".to_string();
                }
            }

            // Send progress notification
            let _ = notification_tx.send(AgentNotification {
                agent_id: agent_id_clone.clone(),
                agent_type,
                notification_type: NotificationType::ProgressUpdate,
                message: "Agent started".to_string(),
                content: task_clone.clone(),
                timestamp: Utc::now().to_rfc3339(),
            });

            // Execute agent via runtime
            let result = runtime
                .delegate(agent_type.as_str(), &task_clone, HashMap::new(), None, None)
                .await;

            match result {
                Ok(agent_result) => {
                    let summary = if agent_result.artifacts.is_empty() {
                        format!(
                            "Agent {} completed task \"{}\".",
                            agent_result.agent_name, task_clone
                        )
                    } else {
                        format!(
                            "Agent {} completed task \"{}\" and produced artifacts:\n{}",
                            agent_result.agent_name,
                            task_clone,
                            agent_result.artifacts.join("\n"),
                        )
                    };

                    task_results
                        .lock()
                        .await
                        .insert(agent_id_clone.clone(), summary.clone());

                    {
                        let mut usage = token_usage.lock().await;
                        let stats = usage
                            .entry(agent_id_clone.clone())
                            .or_insert_with(TokenUsageStats::default);
                        stats.total_tokens = stats
                            .total_tokens
                            .saturating_add(agent_result.tokens_used as u64);
                        stats.last_task_id = Some(agent_id_clone.clone());
                        stats.last_description = Some(task_clone.clone());
                    }

                    let _ = notification_tx.send(AgentNotification {
                        agent_id: agent_id_clone.clone(),
                        agent_type,
                        notification_type: NotificationType::TaskCompleted,
                        message: "Task completed successfully".to_string(),
                        content: summary.clone(),
                        timestamp: Utc::now().to_rfc3339(),
                    });

                    Ok(summary)
                }
                Err(e) => {
                    // Send failure notification
                    let _ = notification_tx.send(AgentNotification {
                        agent_id: agent_id_clone.clone(),
                        agent_type,
                        notification_type: NotificationType::TaskFailed,
                        message: format!("Task failed: {}", e),
                        content: String::new(),
                        timestamp: Utc::now().to_rfc3339(),
                    });

                    Err(anyhow!("Agent task failed: {}", e))
                }
            }
        });

        self.active_agents
            .lock()
            .await
            .insert(agent_id.clone(), handle);

        Ok(agent_id)
    }

    /// Start subagent task
    pub async fn start_subagent_task(&self, agent_type: AgentType, task: &str) -> Result<String> {
        self.start_agent(agent_type, task).await
    }

    /// Start conversation with subagent
    pub async fn start_conversation_with_subagent(
        &self,
        agent_type: AgentType,
        message: &str,
    ) -> Result<()> {
        let _agent_id = self.start_agent(agent_type, message).await?;
        Ok(())
    }

    /// Terminate subagent
    pub async fn terminate_subagent(&self, agent_type: AgentType) -> Result<()> {
        let mut agents = self.active_agents.lock().await;
        let agent_prefix = agent_type.as_str();

        // Find and abort matching agents
        let to_remove: Vec<_> = agents
            .keys()
            .filter(|id| id.starts_with(agent_prefix))
            .cloned()
            .collect();

        for agent_id in to_remove {
            if let Some(handle) = agents.remove(&agent_id) {
                handle.abort();

                // Update state
                let mut states = self.agent_states.lock().await;
                if let Some(state) = states.get_mut(&agent_id) {
                    state.status = "terminated".to_string();
                }
            }
        }

        Ok(())
    }

    /// Get thinking summary
    pub async fn get_thinking_summary(&self, task_id: &str) -> Option<String> {
        self.task_results.lock().await.get(task_id).cloned()
    }

    /// Get all thinking summaries
    pub async fn get_all_thinking_summaries(&self) -> String {
        let results = self.task_results.lock().await;
        if results.is_empty() {
            "No thinking processes recorded.".to_string()
        } else {
            results
                .iter()
                .map(|(task_id, summary)| format!("{task_id}: {summary}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    /// Record token usage
    #[allow(clippy::too_many_arguments)]
    pub async fn record_token_usage(
        &self,
        agent_id: &str,
        task_id: &str,
        task_description: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> Result<()> {
        let mut usage = self.token_usage.lock().await;
        let stats = usage
            .entry(agent_id.to_string())
            .or_insert_with(TokenUsageStats::default);
        stats.prompt_tokens = stats.prompt_tokens.saturating_add(prompt_tokens);
        stats.completion_tokens = stats.completion_tokens.saturating_add(completion_tokens);
        stats.total_tokens = stats
            .total_tokens
            .saturating_add(prompt_tokens.saturating_add(completion_tokens));
        stats.last_task_id = Some(task_id.to_string());
        stats.last_description = Some(task_description.to_string());
        Ok(())
    }

    /// Check inbox
    pub async fn check_inbox(&self) -> Vec<AgentNotification> {
        let mut rx = self.notification_rx.lock().await;
        let mut notifications = Vec::new();

        while let Ok(notification) = rx.try_recv() {
            notifications.push(notification);
        }

        notifications
    }

    /// Cancel agent
    pub async fn cancel_agent(&self, agent_id: &str) -> Result<()> {
        let mut agents = self.active_agents.lock().await;

        if let Some(handle) = agents.remove(agent_id) {
            handle.abort();

            // Update state
            let mut states = self.agent_states.lock().await;
            if let Some(state) = states.get_mut(agent_id) {
                state.status = "cancelled".to_string();
            }

            Ok(())
        } else {
            Err(anyhow!("Agent not found: {}", agent_id))
        }
    }

    /// Get agent states
    pub async fn get_agent_states(&self) -> Vec<AgentState> {
        self.agent_states.lock().await.values().cloned().collect()
    }

    /// Auto dispatch task
    pub async fn auto_dispatch_task(&self, task: &str) -> Result<String> {
        // Simple auto-dispatch: analyze task and select appropriate agent
        let agent_type = self.select_agent_for_task(task);
        self.start_agent(agent_type, task).await
    }

    /// Get task summary
    pub async fn get_task_summary(&self, task_id: &str) -> Option<String> {
        self.task_results.lock().await.get(task_id).cloned()
    }

    /// Generate task summary
    pub async fn generate_task_summary(&self, agent_id: &str) -> String {
        let states = self.agent_states.lock().await;
        let results = self.task_results.lock().await;

        if let Some(state) = states.get(agent_id) {
            let result = results.get(agent_id).map(String::as_str).unwrap_or("N/A");
            format!(
                "Agent: {}\nType: {}\nStatus: {}\nProgress: {:.1}%\nResult: {}",
                agent_id,
                state.agent_type.as_str(),
                state.status,
                state.progress,
                result
            )
        } else {
            format!("Agent {agent_id} not found")
        }
    }
    pub async fn generate_token_report(&self) -> String {
        let usage = self.token_usage.lock().await;
        if usage.is_empty() {
            return "No token usage recorded.".to_string();
        }

        let total: u64 = usage.values().map(|stats| stats.total_tokens).sum();

        let mut report = format!("Total tokens used: {total}\n\nPer agent:\n");
        for (agent_id, stats) in usage.iter() {
            report.push_str(&format!(
                "  {agent_id}: {} tokens (prompt: {}, completion: {})\n",
                stats.total_tokens, stats.prompt_tokens, stats.completion_tokens
            ));
            if let Some(task_id) = &stats.last_task_id {
                let description = stats
                    .last_description
                    .as_deref()
                    .unwrap_or("(no description)");
                report.push_str(&format!("    last task {task_id}: {description}\n"));
            }
        }

        report
    }

    /// Force complete agent
    pub async fn force_complete_agent(&self, agent_id: &str) -> Result<()> {
        self.cancel_agent(agent_id).await?;

        // Mark as completed instead of cancelled
        let mut states = self.agent_states.lock().await;
        if let Some(state) = states.get_mut(agent_id) {
            state.status = "force_completed".to_string();
            state.progress = 100.0;
        }

        Ok(())
    }

    /// Select appropriate agent for task
    fn select_agent_for_task(&self, task: &str) -> AgentType {
        let task_lower = task.to_lowercase();

        if task_lower.contains("security") || task_lower.contains("vulnerability") {
            AgentType::SecurityExpert
        } else if task_lower.contains("test") || task_lower.contains("coverage") {
            AgentType::TestingExpert
        } else if task_lower.contains("document") || task_lower.contains("readme") {
            AgentType::DocsExpert
        } else if task_lower.contains("research") || task_lower.contains("investigate") {
            AgentType::DeepResearcher
        } else if task_lower.contains("debug") || task_lower.contains("error") {
            AgentType::DebugExpert
        } else if task_lower.contains("performance") || task_lower.contains("optimize") {
            AgentType::PerformanceExpert
        } else if task_lower.contains("code") || task_lower.contains("implement") {
            AgentType::CodeExpert
        } else {
            AgentType::General
        }
    }
}

impl Default for AsyncSubAgentIntegration {
    fn default() -> Self {
        // Note: Default implementation uses a placeholder runtime
        // In production, should be constructed with proper runtime
        panic!("AsyncSubAgentIntegration::default() should not be used. Use new() instead.");
    }
}

/// Agent type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentType {
    CodeExpert,
    SecurityExpert,
    TestingExpert,
    DocsExpert,
    DeepResearcher,
    DebugExpert,
    PerformanceExpert,
    General,
}

impl AgentType {
    /// Get agent type as string (matches .codex/agents/*.yaml)
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::CodeExpert => "code-reviewer",
            AgentType::SecurityExpert => "sec-audit",
            AgentType::TestingExpert => "test-gen",
            AgentType::DocsExpert => "docs-expert",
            AgentType::DeepResearcher => "researcher",
            AgentType::DebugExpert => "debug-expert",
            AgentType::PerformanceExpert => "perf-expert",
            AgentType::General => "general",
        }
    }
}

/// Agent notification
#[derive(Debug, Clone)]
pub struct AgentNotification {
    pub agent_id: String,
    pub agent_type: AgentType,
    pub notification_type: NotificationType,
    pub message: String,
    pub content: String,
    pub timestamp: String,
}

/// Notification type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    TaskCompleted,
    TaskFailed,
    ProgressUpdate,
    AgentMessage,
    Error,
    Info,
}

/// Agent state
#[derive(Debug, Clone)]
pub struct AgentState {
    pub agent_id: String,
    pub agent_type: AgentType,
    pub status: String,
    pub progress: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentDefinition;
    use crate::agents::budgeter::Budgeter;
    use crate::auth::AuthManager;
    use crate::config::Config;
    use crate::conversation_id::ConversationId;
    use crate::model_provider_info::ModelProviderInfo;
    use crate::otel_event_manager::OtelEventManager;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_agent_lifecycle() {
        // Create test runtime
        let config = Config::default();
        let provider = ModelProviderInfo::default();
        let auth = AuthManager::default();
        let otel = OtelEventManager::default();
        let conv_id = ConversationId::new();

        let budgeter = Arc::new(Budgeter::new());
        let agents_dir = PathBuf::from(".codex/agents");

        let runtime = Arc::new(AgentRuntime::new(
            budgeter, agents_dir, config, auth, otel, provider, conv_id,
        ));

        let integration = AsyncSubAgentIntegration::new(runtime);

        // Test agent creation
        let agent_id = integration
            .start_agent(AgentType::General, "Test task")
            .await
            .unwrap();

        assert!(agent_id.starts_with("general-"));

        // Check state
        let states = integration.get_agent_states().await;
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].agent_id, agent_id);
    }
}
