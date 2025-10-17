// Comprehensive tests for RealSubAgentWithExecutor
use anyhow::Result;
use codex_supervisor::AgentType;
use codex_supervisor::CodexExecutor;
use codex_supervisor::RealSubAgentManagerWithExecutor;
use codex_supervisor::RealSubAgentWithExecutor;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_real_subagent_with_executor_creation() {
    let executor = CodexExecutor::default();
    let agent =
        RealSubAgentWithExecutor::new(AgentType::CodeExpert, Arc::new(Mutex::new(executor)));

    let state = agent.get_state().await;
    assert_eq!(state.agent_type, AgentType::CodeExpert);
    assert_eq!(state.status, codex_supervisor::subagent::AgentStatus::Idle);
    assert_eq!(state.progress, 0.0);
    assert!(state.current_task.is_none());
}

#[tokio::test]
async fn test_real_subagent_manager_with_executor_creation() {
    let manager = RealSubAgentManagerWithExecutor::default();
    manager.register_all_agents().await;

    let states = manager.get_all_states().await;
    assert_eq!(states.len(), 8); // All 8 agent types

    // Check all agent types are registered
    let agent_types: Vec<AgentType> = states.iter().map(|s| s.agent_type.clone()).collect();
    assert!(agent_types.contains(&AgentType::CodeExpert));
    assert!(agent_types.contains(&AgentType::SecurityExpert));
    assert!(agent_types.contains(&AgentType::TestingExpert));
    assert!(agent_types.contains(&AgentType::DocsExpert));
    assert!(agent_types.contains(&AgentType::DeepResearcher));
    assert!(agent_types.contains(&AgentType::DebugExpert));
    assert!(agent_types.contains(&AgentType::PerformanceExpert));
    assert!(agent_types.contains(&AgentType::General));
}

#[tokio::test]
async fn test_real_subagent_manager_metrics_report() {
    let manager = RealSubAgentManagerWithExecutor::default();
    manager.register_all_agents().await;

    let report = manager.get_metrics_report().await;
    assert!(report.contains("Codex Executor Metrics"));
    assert!(report.contains("Total Calls: 0"));
}

#[tokio::test]
async fn test_real_subagent_get_agent_state() {
    let manager = RealSubAgentManagerWithExecutor::default();
    manager.register_all_agents().await;

    let state = manager.get_agent_state(&AgentType::CodeExpert).await;
    assert!(state.is_some());

    let state = state.unwrap();
    assert_eq!(state.agent_type, AgentType::CodeExpert);
    assert_eq!(state.status, codex_supervisor::subagent::AgentStatus::Idle);
}

#[tokio::test]
async fn test_real_subagent_message_sender() {
    let executor = CodexExecutor::default();
    let agent =
        RealSubAgentWithExecutor::new(AgentType::CodeExpert, Arc::new(Mutex::new(executor)));

    let sender = agent.get_sender();
    let message = codex_supervisor::subagent::AgentMessage {
        content: "Test message".to_string(),
        metadata: std::collections::HashMap::new(),
    };

    // Should not panic
    sender.send(message).unwrap();
}

#[tokio::test]
async fn test_all_agent_types_can_be_created() {
    let executor = Arc::new(Mutex::new(CodexExecutor::default()));
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
        let agent = RealSubAgentWithExecutor::new(agent_type.clone(), Arc::clone(&executor));
        let state = agent.get_state().await;
        assert_eq!(state.agent_type, agent_type);
    }
}

// Integration tests (require actual Codex setup)
#[cfg(feature = "integration_tests")]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_real_subagent_process_task() -> Result<()> {
        let executor = CodexExecutor::default();
        let agent =
            RealSubAgentWithExecutor::new(AgentType::General, Arc::new(Mutex::new(executor)));

        let result = agent.process_task("Test task".to_string()).await?;
        assert!(!result.is_empty());

        let state = agent.get_state().await;
        assert_eq!(
            state.status,
            codex_supervisor::subagent::AgentStatus::Completed
        );
        assert_eq!(state.progress, 1.0);

        Ok(())
    }

    #[tokio::test]
    async fn test_real_subagent_manager_dispatch_task() -> Result<()> {
        let manager = RealSubAgentManagerWithExecutor::default();
        manager.register_all_agents().await;

        let result = manager
            .dispatch_task(AgentType::General, "Test task".to_string())
            .await?;

        assert!(!result.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_real_subagent_manager_all_agent_types() -> Result<()> {
        let manager = RealSubAgentManagerWithExecutor::default();
        manager.register_all_agents().await;

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
            let result = manager
                .dispatch_task(agent_type.clone(), format!("Test for {:?}", agent_type))
                .await?;
            assert!(!result.is_empty());
        }

        // Check metrics after multiple calls
        let report = manager.get_metrics_report().await;
        assert!(report.contains("Total Calls: 8"));

        Ok(())
    }
}
