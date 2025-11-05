//! Integration tests for Blueprint Mode
//!
//! Tests full lifecycle: create → approve → execute

use codex_core::agents::{AgentRuntime, CompetitionConfig, CompetitionRunner};
use codex_core::blueprint::{
    ApprovalRole, BlueprintBlock, BlueprintManager, BlueprintState, Budget, ExecutionMode, WorkItem,
};
use codex_core::execution::ExecutionEngine;
use codex_core::orchestration::{BlueprintOrchestrator, CollaborationStore};
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_blueprint_full_lifecycle() {
    // Create manager
    let manager = BlueprintManager::default();

    // 1. Create blueprint
    let bp_id = manager
        .create_blueprint(
            "Test integration".to_string(),
            "test-integration".to_string(),
            Some("test-user".to_string()),
        )
        .unwrap();

    // 2. Get blueprint
    let bp = manager.get_blueprint(&bp_id).unwrap();
    assert_eq!(bp.goal, "Test integration");
    assert!(matches!(bp.state, BlueprintState::Drafting));

    // 3. Add work item
    let work_item = WorkItem {
        name: "Task 1".to_string(),
        files_touched: vec!["test.rs".to_string()],
        diff_contract: "patch".to_string(),
        tests: vec!["test::test_task1".to_string()],
    };
    manager.add_work_item(&bp_id, work_item).unwrap();

    // 4. Submit for approval
    manager.submit_for_approval(&bp_id).unwrap();
    let bp = manager.get_blueprint(&bp_id).unwrap();
    assert!(matches!(bp.state, BlueprintState::Pending { .. }));

    // 5. Approve
    manager
        .approve_blueprint(&bp_id, "reviewer".to_string(), ApprovalRole::Maintainer)
        .unwrap();
    let bp = manager.get_blueprint(&bp_id).unwrap();
    assert!(matches!(bp.state, BlueprintState::Approved { .. }));
    assert!(bp.can_execute());

    // 6. Export
    let (md_path, json_path) = manager.export_blueprint(&bp_id).unwrap();
    assert!(md_path.exists());
    assert!(json_path.exists());
}

#[test]
fn test_blueprint_rejection_flow() {
    let manager = BlueprintManager::default();

    let bp_id = manager
        .create_blueprint("Test".to_string(), "test".to_string(), None)
        .unwrap();

    manager.submit_for_approval(&bp_id).unwrap();

    // Reject
    manager
        .reject_blueprint(
            &bp_id,
            "Not ready".to_string(),
            Some("reviewer".to_string()),
        )
        .unwrap();

    let bp = manager.get_blueprint(&bp_id).unwrap();
    assert!(matches!(bp.state, BlueprintState::Rejected { .. }));
    assert!(!bp.can_execute());
}

#[test]
fn test_execution_mode_switching() {
    let runtime = Arc::new(AgentRuntime::default());
    let mut engine = ExecutionEngine::new(ExecutionMode::Single, runtime);

    assert_eq!(engine.mode(), ExecutionMode::Single);

    engine.set_mode(ExecutionMode::Orchestrated);
    assert_eq!(engine.mode(), ExecutionMode::Orchestrated);

    engine.set_mode(ExecutionMode::Competition);
    assert_eq!(engine.mode(), ExecutionMode::Competition);
}

#[tokio::test]
async fn test_telemetry_recording() {
    use codex_core::telemetry::{
        self, CollectorConfig, EventType, TelemetryEvent, TelemetryStorage,
    };

    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(TelemetryStorage::new(temp_dir.path().to_path_buf()).unwrap());

    let config = CollectorConfig {
        enabled: true,
        buffer_size: 10,
        flush_interval_secs: 1,
    };

    telemetry::init_with_config(config, temp_dir.path().to_path_buf()).unwrap();

    // Record event
    let event = TelemetryEvent::new(EventType::BlueprintStart)
        .with_blueprint_id("test-bp")
        .with_metadata("test", "value");

    telemetry::record(event).await.unwrap();

    // Give time for flush
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Check storage
    let logs = storage.list_logs().unwrap();
    assert!(!logs.is_empty());
}

#[tokio::test]
async fn test_webhook_delivery() {
    use codex_core::webhooks::{WebhookConfig, WebhookPayload, WebhookService};

    // This would require a mock server, so we just test config
    let config = WebhookConfig {
        service: WebhookService::Http,
        url: "https://httpbin.org/post".to_string(),
        secret: "test-secret".to_string(),
        max_retries: 3,
        timeout_secs: 10,
    };

    assert_eq!(config.service, WebhookService::Http);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_competition_config() {
    let config = CompetitionConfig::default();

    assert_eq!(config.num_variants, 2);
    assert_eq!(config.weights.tests, 0.5);
    assert_eq!(config.weights.performance, 0.3);
    assert_eq!(config.weights.simplicity, 0.2);
}

#[test]
fn test_budget_enforcement() {
    use codex_core::blueprint::{Budget, BudgetTracker};

    let budget = Budget {
        max_step: Some(1000),
        session_cap: Some(5000),
        estimate_min: Some(10),
        cap_min: Some(20),
    };

    let tracker = BudgetTracker::new(budget);

    // Record tokens
    tracker.record_tokens(500).unwrap();
    assert_eq!(tracker.tokens_used(), 500);

    tracker.record_tokens(300).unwrap();
    assert_eq!(tracker.tokens_used(), 800);

    // Check usage
    let usage = tracker.usage();
    assert!(!usage.tokens_exceeded);
    assert!(!usage.time_exceeded);
}

#[test]
fn test_policy_enforcement() {
    use codex_core::blueprint::{
        ApprovalRole, BlueprintPolicy, PolicyEnforcer, PrivilegedOperation,
    };

    let policy = BlueprintPolicy::default();
    let enforcer = PolicyEnforcer::new(policy);

    // Network requires approval
    assert!(enforcer.requires_approval(PrivilegedOperation::Network));

    // Maintainer can approve
    assert!(enforcer.can_approve(ApprovalRole::Maintainer));

    // User cannot approve (default policy requires Maintainer)
    assert!(!enforcer.can_approve(ApprovalRole::User));
}
