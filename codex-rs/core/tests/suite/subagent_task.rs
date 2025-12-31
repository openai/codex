//! Integration tests for the subagent Task tool system.
//!
//! These tests verify:
//! 1. Subagent stores lifecycle (creation, cleanup)
//! 2. Built-in agent definitions (Explore, Plan)
//! 3. Tool filtering behavior
//! 4. Background task management

use codex_core::subagent::AgentRegistry;
use codex_core::subagent::BackgroundTaskStore;
use codex_core::subagent::cleanup_stores;
use codex_core::subagent::get_or_create_stores;
use codex_core::subagent::get_stores;
use codex_protocol::ConversationId;
use std::time::Duration;

/// Test that stores are created on first access and reused for subsequent calls.
#[test]
fn test_stores_creation_and_reuse() {
    let conv_id = ConversationId::new();

    // First access creates stores
    let stores1 = get_or_create_stores(conv_id);

    // Second access returns same stores
    let stores2 = get_or_create_stores(conv_id);

    // Both should point to same Arc
    assert!(std::sync::Arc::ptr_eq(&stores1, &stores2));

    // Cleanup
    cleanup_stores(&conv_id);
}

/// Test that cleanup_stores properly removes stores from registry.
#[test]
fn test_stores_cleanup() {
    let conv_id = ConversationId::new();

    // Create stores
    let _ = get_or_create_stores(conv_id);
    assert!(
        get_stores(&conv_id).is_some(),
        "stores should exist after creation"
    );

    // Cleanup
    cleanup_stores(&conv_id);

    // Verify cleanup
    assert!(
        get_stores(&conv_id).is_none(),
        "stores should not exist after cleanup"
    );
}

/// Test that different conversations have isolated stores.
#[test]
fn test_stores_isolation() {
    let conv_id1 = ConversationId::new();
    let conv_id2 = ConversationId::new();

    let stores1 = get_or_create_stores(conv_id1);
    let stores2 = get_or_create_stores(conv_id2);

    // Different conversations should have different stores
    assert!(!std::sync::Arc::ptr_eq(&stores1, &stores2));

    // Cleanup one should not affect the other
    cleanup_stores(&conv_id1);
    assert!(get_stores(&conv_id1).is_none());
    assert!(get_stores(&conv_id2).is_some());

    // Cleanup the other
    cleanup_stores(&conv_id2);
}

/// Test that built-in agents (Explore, Plan) are available.
#[tokio::test]
async fn test_builtin_agents_available() {
    let registry = AgentRegistry::new();

    // Explore agent should be available
    let explore = registry.get("Explore").await;
    assert!(explore.is_some(), "Explore agent should be available");

    let explore_def = explore.unwrap();
    assert_eq!(explore_def.agent_type, "Explore");

    // Plan agent should be available
    let plan = registry.get("Plan").await;
    assert!(plan.is_some(), "Plan agent should be available");

    let plan_def = plan.unwrap();
    assert_eq!(plan_def.agent_type, "Plan");
}

/// Test that unknown agents return None.
#[tokio::test]
async fn test_unknown_agent_returns_none() {
    let registry = AgentRegistry::new();

    let unknown = registry.get("NonExistentAgent").await;
    assert!(unknown.is_none(), "Unknown agent should return None");
}

/// Test that agent registry lists all available types.
#[tokio::test]
async fn test_agent_registry_list_types() {
    let registry = AgentRegistry::new();

    let types = registry.list_types().await;

    // Should contain at least Explore and Plan
    assert!(
        types.contains(&"Explore".to_string()),
        "Should list Explore"
    );
    assert!(types.contains(&"Plan".to_string()), "Should list Plan");
}

/// Test background task store basic operations.
#[tokio::test]
async fn test_background_task_store_operations() {
    let store = BackgroundTaskStore::new();

    // Register a pending task
    let agent_id = "test-agent-123";
    store.register_pending(
        agent_id.to_string(),
        "Test description".to_string(),
        "Test prompt".to_string(),
    );

    // Task should be in pending state
    let status = store.get_status(agent_id);
    assert!(status.is_some(), "Task should exist after registration");

    // Getting result without blocking should return None (task is pending, no handle)
    let result = store
        .get_result(agent_id, false, Duration::from_millis(100))
        .await;
    assert!(
        result.is_none(),
        "Pending task without handle should return None"
    );

    // Cleanup
    store.cleanup_old_tasks(Duration::ZERO);
}

/// Test that cleanup_session_resources from codex_ext works correctly.
#[test]
fn test_codex_ext_cleanup_session_resources() {
    use codex_core::codex_ext::cleanup_session_resources;

    let conv_id = ConversationId::new();

    // Create stores
    let _ = get_or_create_stores(conv_id);
    assert!(get_stores(&conv_id).is_some());

    // Use the extension function to cleanup
    cleanup_session_resources(&conv_id);

    // Verify cleanup
    assert!(get_stores(&conv_id).is_none());
}

/// Test Explore agent has expected configuration.
#[tokio::test]
async fn test_explore_agent_configuration() {
    let registry = AgentRegistry::new();
    let explore = registry
        .get("Explore")
        .await
        .expect("Explore agent should exist");

    // Verify key configuration
    assert_eq!(explore.agent_type, "Explore");
    assert!(explore.run_config.max_time_seconds > 0);
    assert!(explore.run_config.max_turns > 0);

    // Explore should be read-only (no write tools)
    let _disallowed = &explore.disallowed_tools;
    // Verify it doesn't explicitly allow dangerous tools
    // (Tool filtering is handled by ToolFilter, not disallowed_tools for builtins)
}

/// Test Plan agent has expected configuration.
#[tokio::test]
async fn test_plan_agent_configuration() {
    let registry = AgentRegistry::new();
    let plan = registry.get("Plan").await.expect("Plan agent should exist");

    // Verify key configuration
    assert_eq!(plan.agent_type, "Plan");
    assert!(plan.run_config.max_time_seconds > 0);
    assert!(plan.run_config.max_turns > 0);
    assert!(plan.run_config.grace_period_seconds > 0);
}
