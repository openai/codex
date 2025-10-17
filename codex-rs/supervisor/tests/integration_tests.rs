// Integration tests for the complete SubAgent system
use anyhow::Result;
use codex_supervisor::AgentType;
use codex_supervisor::AutonomousDispatcher;
use codex_supervisor::CodexExecutor;
use codex_supervisor::RealSubAgentManagerWithExecutor;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_complete_subagent_system_initialization() {
    let manager = RealSubAgentManagerWithExecutor::default();
    manager.register_all_agents().await;

    // Verify all agents are registered
    let states = manager.get_all_states().await;
    assert_eq!(states.len(), 8);

    // Verify initial state
    for state in states {
        assert_eq!(state.status, codex_supervisor::subagent::AgentStatus::Idle);
        assert_eq!(state.progress, 0.0);
        assert!(state.current_task.is_none());
    }
}

#[tokio::test]
async fn test_autonomous_dispatcher_classification() {
    let dispatcher = AutonomousDispatcher::new();

    // Test security-related task
    let classification = dispatcher.classify_task("Review code for security vulnerabilities");
    assert_eq!(classification.recommended_agent, AgentType::SecurityExpert);
    assert!(classification.confidence > 0.5);

    // Test code-related task
    let classification = dispatcher.classify_task("Analyze this code for bugs");
    assert_eq!(classification.recommended_agent, AgentType::CodeExpert);

    // Test research-related task
    let classification = dispatcher.classify_task("Research best practices for Rust async");
    assert_eq!(classification.recommended_agent, AgentType::DeepResearcher);

    // Test performance-related task
    let classification = dispatcher.classify_task("Optimize database query performance");
    assert_eq!(
        classification.recommended_agent,
        AgentType::PerformanceExpert
    );
}

#[tokio::test]
async fn test_agent_prompts_for_all_types() {
    use codex_supervisor::agent_prompts::get_agent_prompt;

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
        let prompt = get_agent_prompt(&agent_type, "Test task");
        assert!(!prompt.is_empty());
        assert!(prompt.contains("Test task"));
        assert!(prompt.contains("Role:"));
    }
}

#[tokio::test]
async fn test_metrics_collection_system() {
    let executor = CodexExecutor::default();
    let metrics = executor.get_metrics();

    assert_eq!(metrics.total_calls, 0);
    assert_eq!(metrics.successful_calls, 0);
    assert_eq!(metrics.failed_calls, 0);

    let report = executor.generate_metrics_report();
    assert!(report.contains("Codex Executor Metrics"));
    assert!(report.contains("Success Rate: 0.00%"));
}

#[tokio::test]
async fn test_agent_state_management() {
    let manager = RealSubAgentManagerWithExecutor::default();
    manager.register_all_agents().await;

    // Test get_agent_state for each agent type
    let agent_types = vec![
        AgentType::CodeExpert,
        AgentType::SecurityExpert,
        AgentType::TestingExpert,
    ];

    for agent_type in agent_types {
        let state = manager.get_agent_state(&agent_type).await;
        assert!(state.is_some());

        let state = state.unwrap();
        assert_eq!(state.agent_type, agent_type);
        assert_eq!(state.status, codex_supervisor::subagent::AgentStatus::Idle);
    }
}

// Integration tests requiring actual Codex setup
#[cfg(feature = "integration_tests")]
mod full_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_task_execution() -> Result<()> {
        let manager = RealSubAgentManagerWithExecutor::default();
        manager.register_all_agents().await;

        // Execute a task
        let result = manager
            .dispatch_task(AgentType::General, "Test task execution".to_string())
            .await?;

        assert!(!result.is_empty());

        // Verify metrics updated
        let report = manager.get_metrics_report().await;
        assert!(report.contains("Total Calls: 1"));

        Ok(())
    }

    #[tokio::test]
    async fn test_auto_dispatch_integration() -> Result<()> {
        let manager = RealSubAgentManagerWithExecutor::default();
        manager.register_all_agents().await;

        let dispatcher = AutonomousDispatcher::new();

        // Classify and execute
        let task = "Review code for security issues";
        let classification = dispatcher.classify_task(task);

        let result = manager
            .dispatch_task(classification.recommended_agent.clone(), task.to_string())
            .await?;

        assert!(!result.is_empty());
        assert_eq!(classification.recommended_agent, AgentType::SecurityExpert);

        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_concurrent_tasks() -> Result<()> {
        let manager = RealSubAgentManagerWithExecutor::default();
        manager.register_all_agents().await;

        let tasks = vec![
            (AgentType::CodeExpert, "Analyze code"),
            (AgentType::SecurityExpert, "Check security"),
            (AgentType::TestingExpert, "Generate tests"),
        ];

        let mut handles = vec![];

        for (agent_type, task) in tasks {
            let manager_clone = &manager;
            let handle = tokio::spawn(async move {
                manager_clone
                    .dispatch_task(agent_type, task.to_string())
                    .await
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await??;
            assert!(!result.is_empty());
        }

        // Verify metrics
        let report = manager.get_metrics_report().await;
        assert!(report.contains("Total Calls: 3"));

        Ok(())
    }
}
