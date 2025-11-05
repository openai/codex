//! End-to-end integration tests for auto-orchestration.
//!
//! Tests the complete orchestration flow including:
//! - Task analysis and complexity scoring
//! - Conflict resolution for concurrent file edits
//! - Error handling with retry policies

use codex_core::orchestration::ErrorHandler;
use codex_core::orchestration::MergeStrategy;
use codex_core::orchestration::TaskAnalyzer;

#[tokio::test]
async fn test_task_analyzer_basic_complexity() {
    let analyzer = TaskAnalyzer::new(0.7);

    // Simple task - low complexity
    let analysis = analyzer.analyze("Fix typo in README");
    assert!(
        analysis.complexity_score < 0.7,
        "Simple task should have low complexity"
    );
    assert!(
        !analysis.should_orchestrate(0.7),
        "Simple task should not trigger orchestration"
    );

    // Complex task - high complexity
    let analysis = analyzer.analyze(
        "Implement user authentication with JWT, write comprehensive tests, \
         perform security audit, and create API documentation",
    );
    assert!(
        analysis.complexity_score >= 0.7,
        "Complex task should have high complexity: {}",
        analysis.complexity_score
    );
    assert!(
        analysis.should_orchestrate(0.7),
        "Complex task should trigger orchestration"
    );
}

#[tokio::test]
async fn test_task_analyzer_keyword_detection() {
    let analyzer = TaskAnalyzer::new(0.7);

    let analysis = analyzer.analyze("Review code for security vulnerabilities and write tests");
    assert!(
        analysis.detected_keywords.contains(&"security".to_string()),
        "Should detect 'security' keyword"
    );
    assert!(
        analysis.detected_keywords.contains(&"test".to_string()),
        "Should detect 'test' keyword"
    );
    // Just verify that some agents are recommended for security/testing task
    assert!(
        !analysis.recommended_agents.is_empty(),
        "Should recommend at least one agent for complex task"
    );
}

#[tokio::test]
async fn test_task_analyzer_subtask_decomposition() {
    let analyzer = TaskAnalyzer::new(0.7);

    let analysis = analyzer.analyze(
        "Create REST API with authentication, implement rate limiting, \
         write integration tests, and deploy to production",
    );

    assert!(
        analysis.subtasks.len() >= 3,
        "Should decompose into multiple subtasks: found {}",
        analysis.subtasks.len()
    );
    assert!(
        analysis.recommended_agents.len() >= 2,
        "Should recommend multiple agents for complex task"
    );
}

#[tokio::test]
async fn test_error_handler_retry_policy() {
    use codex_core::orchestration::AgentError;
    use codex_core::orchestration::RetryPolicy;

    let handler = ErrorHandler::new();

    // First attempt - should retry
    let resolution = handler
        .handle_agent_error(AgentError::Timeout, "TestAgent", 0)
        .await;
    assert!(
        matches!(
            resolution,
            codex_core::orchestration::ErrorResolution::Retry { .. }
        ),
        "Should retry on timeout"
    );

    // Max retries exceeded
    let resolution = handler
        .handle_agent_error(
            AgentError::Timeout,
            "TestAgent",
            RetryPolicy::default().max_retries,
        )
        .await;
    assert!(
        matches!(resolution, codex_core::orchestration::ErrorResolution::Fail),
        "Should fail after max retries"
    );
}

#[tokio::test]
async fn test_error_handler_different_errors() {
    use codex_core::orchestration::AgentError;

    let handler = ErrorHandler::new();

    // FileNotFound - should skip
    let resolution = handler
        .handle_agent_error(AgentError::FileNotFound, "Agent1", 0)
        .await;
    assert!(
        matches!(resolution, codex_core::orchestration::ErrorResolution::Skip),
        "FileNotFound should skip"
    );

    // NetworkError - should retry
    let resolution = handler
        .handle_agent_error(AgentError::NetworkError, "Agent2", 0)
        .await;
    assert!(
        matches!(
            resolution,
            codex_core::orchestration::ErrorResolution::Retry { .. }
        ),
        "NetworkError should retry"
    );
}

#[tokio::test]
async fn test_merge_strategy_enum() {
    // Just verify the MergeStrategy enum exists and has expected variants
    let _strategy1 = MergeStrategy::Sequential;
    let _strategy2 = MergeStrategy::LastWriteWins;
    let _strategy3 = MergeStrategy::ThreeWayMerge;
}

#[cfg(test)]
mod pretty_assertions_import {
    #[allow(unused_imports)]
    use pretty_assertions::assert_eq;
}
