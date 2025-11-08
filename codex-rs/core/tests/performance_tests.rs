use codex_core::AuthManager;
/// Performance tests for Sub-Agent system and Deep Research
///
/// Benchmarks execution time, memory usage, and throughput
use codex_core::agents::AgentRuntime;
use codex_core::agents::TokenBudgeter;
use codex_core::config::Config;
use codex_deep_research::DeepResearcher;
use codex_deep_research::DeepResearcherConfig;
use codex_deep_research::MockProvider;
use codex_deep_research::ResearchPlanner;
use codex_deep_research::ResearchStrategy;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

// Test helper
fn create_test_runtime(workspace_dir: PathBuf, budget: usize) -> AgentRuntime {
    let config = Arc::new(Config::load_from_disk_or_default().unwrap());
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    let otel_manager = OtelEventManager::new_noop();
    let conversation_id = ConversationId::new();

    AgentRuntime::new(
        workspace_dir,
        budget,
        config.clone(),
        Some(Arc::clone(&auth_manager)),
        otel_manager,
        config.model_provider.clone(),
        conversation_id,
        ReasoningEffort::default(),
        ReasoningSummary::default(),
        Verbosity::default(),
    )
}

#[tokio::test]
async fn test_perf_agent_delegation_latency() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".codex/agents");
    fs::create_dir_all(&agents_dir).unwrap();

    let agent_yaml = r#"
name: "Perf Test"
goal: "Performance test"
tools: {}
policies:
  context: {}
success_criteria: []
artifacts:
  - "artifacts/output.md"
"#;

    fs::write(agents_dir.join("perf-test.yaml"), agent_yaml).unwrap();

    let runtime = create_test_runtime(temp_dir.path().to_path_buf(), 10000);

    let start = Instant::now();

    let result = runtime
        .delegate("perf-test", "Test task", HashMap::new(), None, None)
        .await
        .unwrap();

    let elapsed = start.elapsed();

    println!("⏱️  Agent delegation latency: {:?}", elapsed);
    assert!(
        elapsed.as_millis() < 5000,
        "Should complete within 5 seconds"
    );
    assert!(result.duration_secs < 5.0);
}

#[tokio::test]
async fn test_perf_parallel_agent_throughput() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".codex/agents");
    fs::create_dir_all(&agents_dir).unwrap();

    let agent_yaml = r#"
name: "Throughput Test"
goal: "Throughput test"
tools: {}
policies:
  context: {}
success_criteria: []
artifacts:
  - "artifacts/output.md"
"#;

    fs::write(agents_dir.join("throughput-test.yaml"), agent_yaml).unwrap();

    let runtime = create_test_runtime(temp_dir.path().to_path_buf(), 50000);

    let start = Instant::now();

    // Execute 10 agents in parallel
    let mut tasks = Vec::new();
    for i in 0..10 {
        let task = runtime.delegate(
            "throughput-test",
            &format!("Task {}", i),
            HashMap::new(),
            Some(3000),
            None,
        );
        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;

    let elapsed = start.elapsed();

    println!("⏱️  10 parallel agents completed in: {:?}", elapsed);
    println!(
        "   Throughput: {:.2} agents/sec",
        10.0 / elapsed.as_secs_f64()
    );

    let successful = results.iter().filter(|r| r.is_ok()).count();
    assert!(successful >= 8, "At least 80% should succeed");
    assert!(elapsed.as_secs() < 30, "Should complete within 30 seconds");
}

#[tokio::test]
async fn test_perf_token_budgeter_overhead() {
    let budgeter = TokenBudgeter::new(100000);

    let start = Instant::now();

    // Simulate high-frequency token consumption
    for i in 0..1000 {
        let agent = format!("agent{}", i % 10);
        budgeter.set_agent_limit(&agent, 5000).unwrap();
        budgeter.try_consume(&agent, 100).unwrap();
    }

    let elapsed = start.elapsed();

    println!("⏱️  1000 token operations: {:?}", elapsed);
    assert!(elapsed.as_micros() < 100_000, "Should be under 100ms");
}

#[tokio::test]
async fn test_perf_research_plan_generation() {
    let start = Instant::now();

    let plan = ResearchPlanner::generate_plan("Test topic", 3, 8).unwrap();

    let elapsed = start.elapsed();

    println!("⏱️  Research plan generation: {:?}", elapsed);
    assert_eq!(plan.sub_queries.len(), 8);
    assert!(elapsed.as_millis() < 100, "Should be under 100ms");
}

#[tokio::test]
async fn test_perf_deep_research_execution() {
    let provider = Arc::new(MockProvider);
    let config = DeepResearcherConfig {
        max_depth: 2,
        max_sources: 5,
        strategy: ResearchStrategy::Focused,
    };

    let researcher = DeepResearcher::new(config, provider);

    let start = Instant::now();

    let report = researcher.research("Performance test query").await.unwrap();

    let elapsed = start.elapsed();

    println!("⏱️  Deep research execution: {:?}", elapsed);
    println!("   Sources found: {}", report.sources.len());
    println!("   Depth reached: {}", report.depth_reached);

    assert!(!report.sources.is_empty());
    assert!(elapsed.as_secs() < 10, "Should complete within 10 seconds");
}

#[test]
fn test_perf_agent_definition_loading() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".codex/agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create 50 agent definitions
    for i in 0..50 {
        let yaml = format!(
            r#"
name: "Agent {}"
goal: "Goal {}"
tools: {{}}
policies:
  context: {{}}
success_criteria: []
artifacts: []
"#,
            i, i
        );
        fs::write(agents_dir.join(format!("agent{}.yaml", i)), yaml).unwrap();
    }

    let mut loader = codex_core::agents::AgentLoader::new(temp_dir.path());

    let start = Instant::now();

    let agents = loader.load_all().unwrap();

    let elapsed = start.elapsed();

    println!("⏱️  Loaded 50 agents in: {:?}", elapsed);
    assert_eq!(agents.len(), 50);
    assert!(elapsed.as_millis() < 500, "Should load within 500ms");
}

#[tokio::test]
async fn test_perf_memory_usage_baseline() {
    // Baseline memory measurement
    let runtime = create_test_runtime(std::env::current_dir().unwrap(), 100000);

    // Get baseline stats
    let (used, remaining, utilization) = runtime.get_budget_status();

    println!("💾 Memory baseline:");
    println!("   Used: {}", used);
    println!("   Remaining: {}", remaining);
    println!("   Utilization: {:.2}%", utilization * 100.0);

    assert_eq!(used, 0);
    assert_eq!(remaining, 100000);
    assert_eq!(utilization, 0.0);
}
