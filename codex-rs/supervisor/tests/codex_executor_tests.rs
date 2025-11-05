// Comprehensive tests for CodexExecutor
use anyhow::Result;
use codex_supervisor::AgentType;
use codex_supervisor::CodexExecutor;
use codex_supervisor::ExecutionMetrics;
use pretty_assertions::assert_eq;

#[test]
fn test_codex_executor_creation() {
    let executor = CodexExecutor::default();
    let metrics = executor.get_metrics();

    assert_eq!(metrics.total_calls, 0);
    assert_eq!(metrics.successful_calls, 0);
    assert_eq!(metrics.failed_calls, 0);
    assert_eq!(metrics.average_latency_ms, 0);
}

#[test]
fn test_codex_executor_with_custom_config() {
    use codex_core::config::Config;
    use codex_core::AuthManager;
    use std::path::PathBuf;
    use std::sync::Arc;

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
    let executor = CodexExecutor::new(config, auth_manager);

    let metrics = executor.get_metrics();
    assert_eq!(metrics.total_calls, 0);
}

#[test]
fn test_execution_metrics_default() {
    let metrics = ExecutionMetrics::default();

    assert_eq!(metrics.total_calls, 0);
    assert_eq!(metrics.successful_calls, 0);
    assert_eq!(metrics.failed_calls, 0);
    assert_eq!(metrics.total_tokens, 0);
    assert_eq!(metrics.average_latency_ms, 0);
}

#[test]
fn test_metrics_report_generation_empty() {
    let executor = CodexExecutor::default();
    let report = executor.generate_metrics_report();

    assert!(report.contains("Codex Executor Metrics"));
    assert!(report.contains("Total Calls: 0"));
    assert!(report.contains("Successful: 0"));
    assert!(report.contains("Failed: 0"));
    assert!(report.contains("Success Rate: 0.00%"));
}

#[test]
fn test_all_agent_types_supported() {
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

    // All agent types should be creatable
    for agent_type in agent_types {
        let type_str = format!("{:?}", agent_type);
        assert!(!type_str.is_empty());
    }
}

#[test]
fn test_agent_type_display() {
    assert_eq!(format!("{}", AgentType::CodeExpert), "CodeExpert");
    assert_eq!(format!("{}", AgentType::SecurityExpert), "SecurityExpert");
    assert_eq!(format!("{}", AgentType::TestingExpert), "TestingExpert");
}

// Integration test markers (these require actual Codex setup)
#[cfg(feature = "integration_tests")]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_codex_executor_execute_task() -> Result<()> {
        let mut executor = CodexExecutor::default();

        let result = executor
            .execute_task(&AgentType::General, "Hello, test!")
            .await?;

        assert!(!result.is_empty());

        let metrics = executor.get_metrics();
        assert_eq!(metrics.total_calls, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_codex_executor_multiple_calls() -> Result<()> {
        let mut executor = CodexExecutor::default();

        for i in 0..3 {
            let task = format!("Test task {}", i);
            let _result = executor.execute_task(&AgentType::General, &task).await?;
        }

        let metrics = executor.get_metrics();
        assert_eq!(metrics.total_calls, 3);

        Ok(())
    }
}
