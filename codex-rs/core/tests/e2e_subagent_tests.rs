/// End-to-End tests for Sub-Agent system
///
/// Tests the complete flow: delegate → execute → artifacts → report
use codex_core::agents::AgentRuntime;
/// End-to-End tests for Sub-Agent system
///
/// Tests the complete flow: delegate → execute → artifacts → report
use codex_core::agents::AgentStatus;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_e2e_delegate_test_gen_agent() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".codex/agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create test-gen agent definition
    let agent_yaml = r#"
name: "Test Generator"
goal: "Generate unit tests"
tools:
  mcp: [code_indexer]
  fs:
    read: true
    write:
      - "./artifacts"
  net:
    allow: []
  shell:
    exec: [cargo, npm]
policies:
  context:
    max_tokens: 16000
    retention: "job"
  secrets:
    redact: false
success_criteria:
  - "CI green"
  - "coverage_delta >= 10%"
artifacts:
  - "artifacts/test-report.md"
"#;

    fs::write(agents_dir.join("test-gen.yaml"), agent_yaml).unwrap();

    // Create runtime with budget
    let runtime = AgentRuntime::new(temp_dir.path().to_path_buf(), 20000);

    // Delegate task
    let mut inputs = HashMap::new();
    inputs.insert("scope".to_string(), "./src".to_string());

    let result = runtime
        .delegate(
            "test-gen",
            "Generate tests for core module",
            inputs,
            Some(16000),
            None,
        )
        .await
        .unwrap();

    // Verify result
    assert_eq!(result.agent_name, "test-gen");
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(!result.artifacts.is_empty());
    assert!(result.tokens_used > 0);
    assert!(result.duration_secs > 0.0);

    // Verify artifacts exist
    for artifact in &result.artifacts {
        let artifact_path = temp_dir.path().join(artifact);
        assert!(
            artifact_path.exists(),
            "Artifact should exist: {}",
            artifact
        );
    }
}

#[tokio::test]
async fn test_e2e_delegate_researcher_agent() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".codex/agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create researcher agent definition
    let agent_yaml = r#"
name: "Deep Researcher"
goal: "Conduct deep research"
tools:
  mcp: [search, crawler]
  fs:
    read: true
    write:
      - "./artifacts"
  net:
    allow: ["https://*"]
  shell: []
policies:
  context:
    max_tokens: 24000
    retention: "job"
  secrets:
    redact: false
success_criteria:
  - "Multiple domain citations"
artifacts:
  - "artifacts/report.md"
  - "artifacts/evidence/data.json"
"#;

    fs::write(agents_dir.join("researcher.yaml"), agent_yaml).unwrap();

    let runtime = AgentRuntime::new(temp_dir.path().to_path_buf(), 30000);

    let result = runtime
        .delegate(
            "researcher",
            "Research Rust async patterns",
            HashMap::new(),
            Some(24000),
            None,
        )
        .await
        .unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert!(result.artifacts.len() >= 1);
}

#[tokio::test]
async fn test_e2e_multiple_agents_parallel() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".codex/agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create multiple agent definitions
    let agent1_yaml = r#"
name: "Agent1"
goal: "Task 1"
tools: {}
policies:
  context:
    max_tokens: 5000
    retention: "job"
success_criteria: []
artifacts:
  - "artifacts/agent1-output.md"
"#;

    let agent2_yaml = r#"
name: "Agent2"
goal: "Task 2"
tools: {}
policies:
  context:
    max_tokens: 5000
    retention: "job"
success_criteria: []
artifacts:
  - "artifacts/agent2-output.md"
"#;

    fs::write(agents_dir.join("agent1.yaml"), agent1_yaml).unwrap();
    fs::write(agents_dir.join("agent2.yaml"), agent2_yaml).unwrap();

    let runtime = AgentRuntime::new(temp_dir.path().to_path_buf(), 15000);

    // Execute agents in parallel
    let task1 = runtime.delegate("agent1", "Task 1", HashMap::new(), Some(5000), None);
    let task2 = runtime.delegate("agent2", "Task 2", HashMap::new(), Some(5000), None);

    let (result1, result2) = tokio::join!(task1, task2);

    assert_eq!(result1.unwrap().status, AgentStatus::Completed);
    assert_eq!(result2.unwrap().status, AgentStatus::Completed);

    // Verify both agents consumed tokens
    let (used, _, _) = runtime.get_budget_status();
    assert!(used > 0);
    assert!(used <= 15000);
}

#[tokio::test]
async fn test_e2e_budget_exceeded() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".codex/agents");
    fs::create_dir_all(&agents_dir).unwrap();

    let agent_yaml = r#"
name: "Budget Test"
goal: "Test budget limits"
tools: {}
policies:
  context:
    max_tokens: 5000
    retention: "job"
success_criteria: []
artifacts:
  - "artifacts/output.md"
"#;

    fs::write(agents_dir.join("budget-test.yaml"), agent_yaml).unwrap();

    // Very low total budget
    let runtime = AgentRuntime::new(temp_dir.path().to_path_buf(), 500);

    let result = runtime
        .delegate("budget-test", "Test task", HashMap::new(), Some(5000), None)
        .await
        .unwrap();

    // Should fail due to budget constraints
    assert_eq!(result.status, AgentStatus::Failed);
    assert!(result.error.is_some());
}
