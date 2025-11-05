use crate::subagent::AgentStatus;
use crate::subagent::AgentType;
use crate::AutonomousDispatcher;
use crate::SubAgentManager;
use crate::Supervisor;
use crate::TaskClassification;
use crate::TokenAllocationStrategy;
use crate::TokenTracker;
use crate::TokenUsage;
use crate::types::SupervisorConfig;
use crate::types::SupervisorResult;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;
use tokio::time::sleep;
use uuid::Uuid;

const PLANNING_CONFIDENCE_THRESHOLD: f32 = 0.6;
const DEFAULT_MAX_WAIT_ATTEMPTS: usize = 40;
const DEFAULT_WAIT_INTERVAL_MS: u64 = 50;
const EVENT_LOG_CAPACITY: usize = 200;

/// タスク状態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Waiting,
    Running,
    Completed,
    Failed,
}

/// タスクログエントリ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLogEntry {
    pub timestamp: String,
    pub task_id: String,
    pub agent: Option<AgentType>,
    pub message: String,
}

/// タスク記録（コンフリクト監視用途）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub description: String,
    pub assigned_agent: Option<AgentType>,
    pub status: TaskStatus,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub wait_attempts: usize,
    pub events: Vec<TaskLogEntry>,
}

impl TaskRecord {
    fn new(task_id: String, description: String) -> Self {
        Self {
            task_id,
            description,
            assigned_agent: None,
            status: TaskStatus::Pending,
            created_at: Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
            wait_attempts: 0,
            events: Vec::new(),
        }
    }

    fn push_event(&mut self, entry: TaskLogEntry) {
        self.events.push(entry);
    }

    fn set_status(&mut self, status: TaskStatus) {
        match status {
            TaskStatus::Running => {
                if self.started_at.is_none() {
                    self.started_at = Some(Utc::now().to_rfc3339());
                }
            }
            TaskStatus::Completed | TaskStatus::Failed => {
                if self.started_at.is_none() {
                    self.started_at = Some(Utc::now().to_rfc3339());
                }
                self.completed_at = Some(Utc::now().to_rfc3339());
            }
            _ => {}
        }
        self.status = status;
    }

    fn set_assigned_agent(&mut self, agent: AgentType) {
        self.assigned_agent = Some(agent);
    }

    fn set_wait_attempts(&mut self, attempts: usize) {
        self.wait_attempts = attempts;
    }

    fn is_active(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Pending | TaskStatus::Waiting | TaskStatus::Running
        )
    }
}

/// ClaudeCode 風の自律オーケストレーター
/// - タスク分類で適切なサブエージェントを選択
/// - アクティブエージェントを追跡しコンフリクトを回避
/// - 信頼度が低い場合は Supervisor で計画を生成
/// - トークン使用量と実行ログを追跡
pub struct AutonomousOrchestrator {
    supervisor: Supervisor,
    dispatcher: AutonomousDispatcher,
    agent_manager: SubAgentManager,
    token_tracker: TokenTracker,
    active_agents: HashSet<AgentType>,
    max_wait_attempts: usize,
    wait_interval: Duration,
    task_registry: HashMap<String, TaskRecord>,
    event_log: VecDeque<TaskLogEntry>,
}

/// 自律オーケストレーションの実行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousOrchestrationResult {
    pub task_id: String,
    pub task_description: String,
    pub assigned_agent: AgentType,
    pub used_fallback_agent: bool,
    pub classification: TaskClassification,
    pub output: String,
    pub token_usage: TokenUsage,
    pub queue_wait_attempts: usize,
    pub queue_wait_duration_ms: u128,
    pub supervisor_plan: Option<SupervisorResult>,
    pub task_status: TaskStatus,
    pub task_log: Vec<TaskLogEntry>,
    pub conflict_prevented: bool,
}

impl AutonomousOrchestrator {
    /// コンフリクト回避付きオーケストレーターを構築
    pub async fn new(config: SupervisorConfig) -> Result<Self> {
        let supervisor = Supervisor::new(config);
        let mut dispatcher = AutonomousDispatcher::new();
        // キャッシュ初期化のために明示的にクリア
        dispatcher.clear_cache();

        let mut agent_manager = SubAgentManager::new();
        let agent_types = Self::default_agent_types();
        for agent_type in &agent_types {
            agent_manager.register_agent(agent_type.clone());
        }

        let token_tracker =
            TokenTracker::new(Default::default(), TokenAllocationStrategy::Dynamic);
        for agent_type in &agent_types {
            token_tracker
                .register_agent(agent_type.clone(), agent_type.to_string())
                .await;
        }

        Ok(Self {
            supervisor,
            dispatcher,
            agent_manager,
            token_tracker,
            active_agents: HashSet::new(),
            max_wait_attempts: DEFAULT_MAX_WAIT_ATTEMPTS,
            wait_interval: Duration::from_millis(DEFAULT_WAIT_INTERVAL_MS),
            task_registry: HashMap::new(),
            event_log: VecDeque::with_capacity(EVENT_LOG_CAPACITY),
        })
    }

    /// タスクを自律実行
    pub async fn execute_task(
        &mut self,
        description: &str,
    ) -> Result<AutonomousOrchestrationResult> {
        let task_id = Uuid::new_v4().to_string();
        self.register_task(&task_id, description);
        self.append_event(&task_id, None, "Queued task for autonomous orchestration run");

        let classification = self.dispatcher.classify_task(description);
        self.append_event(
            &task_id,
            Some(&classification.recommended_agent),
            format!(
                "Dispatcher classified task (confidence {:.2}) — {}",
                classification.confidence, classification.reasoning
            ),
        );

        let selection = self.select_agent(&task_id, &classification).await;
        self.update_wait_attempts(&task_id, selection.wait_attempts);

        if selection.used_fallback {
            self.append_event(
                &task_id,
                Some(&selection.agent),
                format!(
                    "Fallback agent {} engaged due to conflict on recommended agent",
                    selection.agent
                ),
            );
        } else {
            self.append_event(
                &task_id,
                Some(&selection.agent),
                format!("Selected agent {} for execution", selection.agent),
            );
        }

        self.set_assigned_agent(&task_id, &selection.agent);
        self.active_agents.insert(selection.agent.clone());
        self.update_task_status(&task_id, TaskStatus::Running);
        self.append_event(
            &task_id,
            Some(&selection.agent),
            "Dispatching task to agent",
        );

        let output_result = self
            .agent_manager
            .dispatch_task(selection.agent.clone(), description.to_string())
            .await;
        self.active_agents.remove(&selection.agent);

        let output = match output_result {
            Ok(output) => {
                self.append_event(
                    &task_id,
                    Some(&selection.agent),
                    "Agent completed task successfully",
                );
                self.update_task_status(&task_id, TaskStatus::Completed);
                output
            }
            Err(error) => {
                self.update_task_status(&task_id, TaskStatus::Failed);
                self.append_event(
                    &task_id,
                    Some(&selection.agent),
                    format!("Agent execution failed: {error}"),
                );
                return Err(error);
            }
        };

        let token_usage = self.estimate_token_usage(description, &output);
        if let Err(error) = self
            .token_tracker
            .record_usage(
                &selection.agent.to_string(),
                task_id.clone(),
                description.to_string(),
                token_usage.clone(),
            )
            .await
        {
            self.append_event(
                &task_id,
                Some(&selection.agent),
                format!("Failed to record token usage: {error}"),
            );
            self.update_task_status(&task_id, TaskStatus::Failed);
            return Err(error);
        } else {
            self.append_event(
                &task_id,
                Some(&selection.agent),
                format!(
                    "Recorded token usage (prompt: {}, completion: {})",
                    token_usage.prompt_tokens, token_usage.completion_tokens
                ),
            );
        }

        let supervisor_plan = if classification.confidence < PLANNING_CONFIDENCE_THRESHOLD {
            self.append_event(
                &task_id,
                None,
                "Confidence below threshold; invoking Supervisor planning",
            );

            match self.supervisor.coordinate_goal(description, None).await {
                Ok(plan) => {
                    self.append_event(
                        &task_id,
                        None,
                        "Supervisor produced follow-up plan",
                    );
                    Some(plan)
                }
                Err(error) => {
                    self.append_event(
                        &task_id,
                        None,
                        format!("Supervisor planning failed: {error}"),
                    );
                    None
                }
            }
        } else {
            None
        };

        let task_log = self
            .task_registry
            .get(&task_id)
            .map(|record| record.events.clone())
            .unwrap_or_default();
        let task_status = self
            .task_registry
            .get(&task_id)
            .map(|record| record.status.clone())
            .unwrap_or(TaskStatus::Completed);

        let conflict_prevented =
            selection.used_fallback || selection.wait_attempts > 0;

        Ok(AutonomousOrchestrationResult {
            task_id,
            task_description: description.to_string(),
            assigned_agent: selection.agent,
            used_fallback_agent: selection.used_fallback,
            classification,
            output,
            token_usage,
            queue_wait_attempts: selection.wait_attempts,
            queue_wait_duration_ms: selection.wait_duration.as_millis(),
            supervisor_plan,
            task_status,
            task_log,
            conflict_prevented,
        })
    }

    fn is_agent_available(&self, agent_type: &AgentType) -> bool {
        if self.active_agents.contains(agent_type) {
            return false;
        }

        match self.agent_manager.get_agent_state(agent_type) {
            Some(state) => matches!(
                state.status,
                AgentStatus::Idle | AgentStatus::Completed | AgentStatus::Failed
            ),
            None => false,
        }
    }

    async fn select_agent(
        &mut self,
        task_id: &str,
        classification: &TaskClassification,
    ) -> AgentSelection {
        let candidates = self.build_candidate_list(classification);
        let start = Instant::now();
        let mut attempts = 0;
        let mut noted_busy: HashSet<AgentType> = HashSet::new();
        let mut has_waited = false;

        loop {
            for agent in &candidates {
                if self.is_agent_available(agent) {
                    if has_waited {
                        self.append_event(
                            task_id,
                            Some(agent),
                            format!("Agent {} is now available", agent),
                        );
                    }
                    let used_fallback = agent != &classification.recommended_agent;
                    return AgentSelection {
                        agent: agent.clone(),
                        used_fallback,
                        wait_attempts: attempts,
                        wait_duration: start.elapsed(),
                    };
                }

                if noted_busy.insert(agent.clone()) {
                    self.append_event(
                        task_id,
                        Some(agent),
                        format!("Agent {} busy; monitoring availability", agent),
                    );
                }
            }

            attempts += 1;
            has_waited = true;

            if attempts == 1 {
                self.update_task_status(task_id, TaskStatus::Waiting);
            }

            self.append_event(
                task_id,
                None,
                format!("Waiting for agent availability (attempt {attempts})"),
            );

            if attempts >= self.max_wait_attempts {
                self.append_event(
                    task_id,
                    Some(&classification.recommended_agent),
                    "Wait limit reached; returning recommended agent despite conflict",
                );
                return AgentSelection {
                    agent: classification.recommended_agent.clone(),
                    used_fallback: false,
                    wait_attempts: attempts,
                    wait_duration: start.elapsed(),
                };
            }

            sleep(self.wait_interval).await;
        }
    }

    fn build_candidate_list(&self, classification: &TaskClassification) -> Vec<AgentType> {
        let mut candidates = Vec::new();
        candidates.push(classification.recommended_agent.clone());

        for alt in &classification.alternative_agents {
            if !candidates.contains(alt) {
                candidates.push(alt.clone());
            }
        }

        if !candidates.contains(&AgentType::General) {
            candidates.push(AgentType::General);
        }

        candidates
    }

    fn estimate_token_usage(&self, description: &str, output: &str) -> TokenUsage {
        let prompt_tokens = ((description.chars().count() / 4) + 1) as u64;
        let completion_tokens = ((output.chars().count() / 4) + 1) as u64;
        TokenUsage::new(prompt_tokens, completion_tokens)
    }

    fn register_task(&mut self, task_id: &str, description: &str) {
        self.task_registry.insert(
            task_id.to_string(),
            TaskRecord::new(task_id.to_string(), description.to_string()),
        );
    }

    fn append_event(
        &mut self,
        task_id: &str,
        agent: Option<&AgentType>,
        message: impl Into<String>,
    ) {
        let entry = TaskLogEntry {
            timestamp: Utc::now().to_rfc3339(),
            task_id: task_id.to_string(),
            agent: agent.cloned(),
            message: message.into(),
        };

        if let Some(record) = self.task_registry.get_mut(task_id) {
            record.push_event(entry.clone());
        }

        if self.event_log.len() == EVENT_LOG_CAPACITY {
            self.event_log.pop_front();
        }
        self.event_log.push_back(entry);
    }

    fn update_task_status(&mut self, task_id: &str, status: TaskStatus) {
        if let Some(record) = self.task_registry.get_mut(task_id) {
            record.set_status(status);
        }
    }

    fn set_assigned_agent(&mut self, task_id: &str, agent: &AgentType) {
        if let Some(record) = self.task_registry.get_mut(task_id) {
            record.set_assigned_agent(agent.clone());
        }
    }

    fn update_wait_attempts(&mut self, task_id: &str, attempts: usize) {
        if let Some(record) = self.task_registry.get_mut(task_id) {
            record.set_wait_attempts(attempts);
        }
    }

    /// アクティブタスクを返す
    pub fn active_tasks(&self) -> Vec<TaskRecord> {
        self.task_registry
            .values()
            .filter(|record| record.is_active())
            .cloned()
            .collect()
    }

    /// タスク記録を取得
    pub fn task_record(&self, task_id: &str) -> Option<&TaskRecord> {
        self.task_registry.get(task_id)
    }

    /// 最近のイベントを取得
    pub fn recent_events(&self) -> Vec<TaskLogEntry> {
        self.event_log.iter().cloned().collect()
    }

    fn default_agent_types() -> Vec<AgentType> {
        vec![
            AgentType::CodeExpert,
            AgentType::SecurityExpert,
            AgentType::TestingExpert,
            AgentType::DocsExpert,
            AgentType::DeepResearcher,
            AgentType::DebugExpert,
            AgentType::PerformanceExpert,
            AgentType::General,
        ]
    }
}

struct AgentSelection {
    agent: AgentType,
    used_fallback: bool,
    wait_attempts: usize,
    wait_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SupervisorConfig;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_execute_task_basic_assignment() {
        let mut orchestrator = AutonomousOrchestrator::new(SupervisorConfig::default())
            .await
            .unwrap();

        let result = orchestrator
            .execute_task("Create comprehensive integration tests for the payment module")
            .await
            .unwrap();

        assert_eq!(result.assigned_agent, AgentType::TestingExpert);
        assert!(result.output.contains("TestingExpert"));
        assert!(result.token_usage.total_tokens > 0);
        assert_eq!(result.task_status, TaskStatus::Completed);
        assert!(!result.task_log.is_empty());
        assert!(!result.conflict_prevented);
        assert!(orchestrator
            .recent_events()
            .iter()
            .any(|entry| entry.message.contains("completed task successfully")));
    }

    #[tokio::test]
    async fn test_execute_task_fallback_when_agent_busy() {
        let mut orchestrator = AutonomousOrchestrator::new(SupervisorConfig::default())
            .await
            .unwrap();

        orchestrator
            .active_agents
            .insert(AgentType::CodeExpert);

        let result = orchestrator
            .execute_task("Implement and test the new authentication feature")
            .await
            .unwrap();

        assert_eq!(result.assigned_agent, AgentType::TestingExpert);
        assert!(result.used_fallback_agent);
        assert!(result.conflict_prevented);
        assert!(result
            .task_log
            .iter()
            .any(|entry| entry.message.contains("Fallback agent")));
    }

    #[tokio::test]
    async fn test_execute_task_triggers_supervisor_when_low_confidence() {
        let mut orchestrator = AutonomousOrchestrator::new(SupervisorConfig::default())
            .await
            .unwrap();

        let result = orchestrator
            .execute_task("Hello world")
            .await
            .unwrap();

        assert!(result.supervisor_plan.is_some());
        assert!(result
            .task_log
            .iter()
            .any(|entry| entry.message.contains("Supervisor")));
    }

    #[tokio::test]
    async fn test_task_registry_and_recent_events() {
        let mut orchestrator = AutonomousOrchestrator::new(SupervisorConfig::default())
            .await
            .unwrap();

        let result = orchestrator
            .execute_task("Document the configuration flow")
            .await
            .unwrap();

        let record = orchestrator.task_record(&result.task_id);
        assert!(record.is_some());
        let record = record.unwrap();
        assert_eq!(record.status, TaskStatus::Completed);
        assert!(record
            .events
            .iter()
            .any(|entry| entry.message.contains("Recorded token usage")));

        let events = orchestrator.recent_events();
        assert!(!events.is_empty());
        assert!(events
            .iter()
            .any(|entry| entry.task_id == result.task_id));
    }
}
