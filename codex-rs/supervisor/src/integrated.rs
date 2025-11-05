// 統合API: supervisorとdeep-researchを組み合わせた高レベルAPI
use crate::subagent::AgentType;
use crate::subagent::SubAgentManager;
use crate::types::SupervisorConfig;
use crate::Supervisor;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;

/// 統合タスク実行システム（gemini-cli風）
pub struct IntegratedTaskRunner {
    supervisor: Supervisor,
    agent_manager: SubAgentManager,
}

/// タスクの種類
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    /// コード生成・分析タスク
    CodeGeneration { description: String },
    /// セキュリティレビュータスク
    SecurityReview { target: String },
    /// テスト生成タスク
    TestGeneration { module: String },
    /// ドキュメント生成タスク
    Documentation { topic: String },
    /// 深層研究タスク
    DeepResearch { query: String },
    /// デバッグタスク
    Debug { issue: String },
    /// パフォーマンス最適化タスク
    PerformanceOptimization { target: String },
    /// 複合タスク（複数のサブタスク）
    Composite {
        goal: String,
        subtasks: Vec<Box<TaskType>>,
    },
}

/// タスク実行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    pub task_type: String,
    pub success: bool,
    pub output: String,
    pub agents_used: Vec<String>,
    pub execution_time_ms: u128,
}

impl IntegratedTaskRunner {
    pub fn new(config: SupervisorConfig) -> Self {
        let supervisor = Supervisor::new(config);
        let mut agent_manager = SubAgentManager::new();

        // デフォルトエージェントを登録
        agent_manager.register_agent(AgentType::CodeExpert);
        agent_manager.register_agent(AgentType::SecurityExpert);
        agent_manager.register_agent(AgentType::TestingExpert);
        agent_manager.register_agent(AgentType::DocsExpert);
        agent_manager.register_agent(AgentType::DeepResearcher);
        agent_manager.register_agent(AgentType::DebugExpert);
        agent_manager.register_agent(AgentType::PerformanceExpert);
        agent_manager.register_agent(AgentType::General);

        Self {
            supervisor,
            agent_manager,
        }
    }

    /// タスクを実行
    pub fn execute_task(
        &mut self,
        task: TaskType,
    ) -> Pin<Box<dyn Future<Output = Result<TaskExecutionResult>> + '_>> {
        Box::pin(async move {
            let start = std::time::Instant::now();

            let result = match task {
                TaskType::CodeGeneration { description } => {
                    self.execute_code_generation(&description).await?
                }
                TaskType::SecurityReview { target } => {
                    self.execute_security_review(&target).await?
                }
                TaskType::TestGeneration { module } => {
                    self.execute_test_generation(&module).await?
                }
                TaskType::Documentation { topic } => self.execute_documentation(&topic).await?,
                TaskType::DeepResearch { query } => self.execute_deep_research(&query).await?,
                TaskType::Debug { issue } => self.execute_debug(&issue).await?,
                TaskType::PerformanceOptimization { target } => {
                    self.execute_performance_optimization(&target).await?
                }
                TaskType::Composite { goal, subtasks } => {
                    self.execute_composite(&goal, subtasks).await?
                }
            };

            let execution_time_ms = start.elapsed().as_millis();

            Ok(TaskExecutionResult {
                task_type: result.0,
                success: result.1,
                output: result.2,
                agents_used: result.3,
                execution_time_ms,
            })
        })
    }

    async fn execute_code_generation(
        &mut self,
        description: &str,
    ) -> Result<(String, bool, String, Vec<String>)> {
        let output = self
            .agent_manager
            .dispatch_task(AgentType::CodeExpert, description.to_string())
            .await?;

        Ok((
            "CodeGeneration".to_string(),
            true,
            output,
            vec!["CodeExpert".to_string()],
        ))
    }

    async fn execute_security_review(
        &mut self,
        target: &str,
    ) -> Result<(String, bool, String, Vec<String>)> {
        let output = self
            .agent_manager
            .dispatch_task(AgentType::SecurityExpert, target.to_string())
            .await?;

        Ok((
            "SecurityReview".to_string(),
            true,
            output,
            vec!["SecurityExpert".to_string()],
        ))
    }

    async fn execute_test_generation(
        &mut self,
        module: &str,
    ) -> Result<(String, bool, String, Vec<String>)> {
        let output = self
            .agent_manager
            .dispatch_task(AgentType::TestingExpert, module.to_string())
            .await?;

        Ok((
            "TestGeneration".to_string(),
            true,
            output,
            vec!["TestingExpert".to_string()],
        ))
    }

    async fn execute_documentation(
        &mut self,
        topic: &str,
    ) -> Result<(String, bool, String, Vec<String>)> {
        let output = self
            .agent_manager
            .dispatch_task(AgentType::DocsExpert, topic.to_string())
            .await?;

        Ok((
            "Documentation".to_string(),
            true,
            output,
            vec!["DocsExpert".to_string()],
        ))
    }

    async fn execute_deep_research(
        &mut self,
        query: &str,
    ) -> Result<(String, bool, String, Vec<String>)> {
        let output = self
            .agent_manager
            .dispatch_task(AgentType::DeepResearcher, query.to_string())
            .await?;

        Ok((
            "DeepResearch".to_string(),
            true,
            output,
            vec!["DeepResearcher".to_string()],
        ))
    }

    async fn execute_debug(&mut self, issue: &str) -> Result<(String, bool, String, Vec<String>)> {
        let output = self
            .agent_manager
            .dispatch_task(AgentType::DebugExpert, issue.to_string())
            .await?;

        Ok((
            "Debug".to_string(),
            true,
            output,
            vec!["DebugExpert".to_string()],
        ))
    }

    async fn execute_performance_optimization(
        &mut self,
        target: &str,
    ) -> Result<(String, bool, String, Vec<String>)> {
        let output = self
            .agent_manager
            .dispatch_task(AgentType::PerformanceExpert, target.to_string())
            .await?;

        Ok((
            "PerformanceOptimization".to_string(),
            true,
            output,
            vec!["PerformanceExpert".to_string()],
        ))
    }

    async fn execute_composite(
        &mut self,
        goal: &str,
        subtasks: Vec<Box<TaskType>>,
    ) -> Result<(String, bool, String, Vec<String>)> {
        // supervisorを使って複合タスクを調整
        let result = self
            .supervisor
            .coordinate_goal(
                goal,
                Some(vec!["CodeExpert".to_string(), "SecurityExpert".to_string()]),
            )
            .await?;

        // サブタスクを実行
        let mut all_outputs = Vec::new();
        let mut all_agents = Vec::new();

        for subtask in subtasks {
            let sub_result = self.execute_task(*subtask).await?;
            all_outputs.push(sub_result.output);
            all_agents.extend(sub_result.agents_used);
        }

        let combined_output = format!(
            "Supervisor Result:\n{}\n\nSubtask Results:\n{}",
            result.results.summary,
            all_outputs.join("\n\n")
        );

        Ok(("Composite".to_string(), true, combined_output, all_agents))
    }

    /// エージェントの状態を取得
    pub fn get_agent_states(&self) -> Vec<crate::subagent::AgentState> {
        self.agent_manager.get_all_states()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_integrated_task_runner_code_generation() {
        let config = SupervisorConfig::default();
        let mut runner = IntegratedTaskRunner::new(config);

        let result = runner
            .execute_task(TaskType::CodeGeneration {
                description: "Build auth system".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(result.task_type, "CodeGeneration");
        assert!(result.success);
        assert!(result.output.contains("CodeExpert"));
        assert_eq!(result.agents_used.len(), 1);
    }

    #[tokio::test]
    async fn test_integrated_task_runner_security_review() {
        let config = SupervisorConfig::default();
        let mut runner = IntegratedTaskRunner::new(config);

        let result = runner
            .execute_task(TaskType::SecurityReview {
                target: "auth module".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(result.task_type, "SecurityReview");
        assert!(result.success);
        assert!(result.output.contains("SecurityExpert"));
    }

    #[tokio::test]
    async fn test_integrated_task_runner_deep_research() {
        let config = SupervisorConfig::default();
        let mut runner = IntegratedTaskRunner::new(config);

        let result = runner
            .execute_task(TaskType::DeepResearch {
                query: "Rust async patterns".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(result.task_type, "DeepResearch");
        assert!(result.success);
        assert!(result.output.contains("DeepResearcher"));
    }

    #[tokio::test]
    async fn test_integrated_task_runner_composite() {
        let config = SupervisorConfig::default();
        let mut runner = IntegratedTaskRunner::new(config);

        let result = runner
            .execute_task(TaskType::Composite {
                goal: "Build secure authentication".to_string(),
                subtasks: vec![
                    Box::new(TaskType::CodeGeneration {
                        description: "Auth logic".to_string(),
                    }),
                    Box::new(TaskType::SecurityReview {
                        target: "Auth system".to_string(),
                    }),
                    Box::new(TaskType::TestGeneration {
                        module: "Auth".to_string(),
                    }),
                ],
            })
            .await
            .unwrap();

        assert_eq!(result.task_type, "Composite");
        assert!(result.success);
        assert!(result.agents_used.len() >= 3);
    }

    #[tokio::test]
    async fn test_all_task_types() {
        let config = SupervisorConfig::default();
        let mut runner = IntegratedTaskRunner::new(config);

        let tasks = vec![
            TaskType::CodeGeneration {
                description: "test".to_string(),
            },
            TaskType::SecurityReview {
                target: "test".to_string(),
            },
            TaskType::TestGeneration {
                module: "test".to_string(),
            },
            TaskType::Documentation {
                topic: "test".to_string(),
            },
            TaskType::DeepResearch {
                query: "test".to_string(),
            },
            TaskType::Debug {
                issue: "test".to_string(),
            },
            TaskType::PerformanceOptimization {
                target: "test".to_string(),
            },
        ];

        for task in tasks {
            let result = runner.execute_task(task).await.unwrap();
            assert!(result.success);
            assert!(!result.output.is_empty());
        }
    }
}
