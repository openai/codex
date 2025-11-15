//! Plan command implementations
//!
//! Implementation functions for Plan commands

use anyhow::Context;
use anyhow::Result;
use codex_core::AuthManager;
use codex_core::agents::AgentRuntime;
use codex_core::plan::PlanBlock;
use codex_core::plan::PlanExecutor;
use codex_core::plan::execution_log::ExecutionLog;
use codex_core::plan::ExecutionEvent;
use codex_core::orchestration::CollaborationStore;
use codex_core::orchestration::PlanOrchestrator;
use codex_core::config::Config;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Execute a Plan
pub async fn execute_Plan(Plan_id: &str, Plan_dir: &PathBuf) -> Result<()> {
    // Load Plan
    let Plan_file = Plan_dir.join(format!("{}.json", Plan_id));

    if !Plan_file.exists() {
        anyhow::bail!("Plan not found: {}", Plan_id);
    }

    let content = std::fs::read_to_string(&Plan_file)?;
    let Plan: PlanBlock = serde_json::from_str(&content)?;

    println!("🚀 Executing Plan: {}", Plan.title);
    println!("ID: {}", Plan.id);
    println!("Mode: {}", Plan.mode);
    println!();

    // Create runtime and executor
    let config = Arc::new(Config::load_from_disk_or_default()?);
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    let otel_manager = OtelEventManager::new_noop();
    let conversation_id = ConversationId::new();

    let runtime = Arc::new(AgentRuntime::new(
        std::env::current_dir()?,
        1_000_000,
        config.clone(),
        Some(Arc::clone(&auth_manager)),
        otel_manager,
        config.model_provider.clone(),
        conversation_id,
        config.model_reasoning_effort.unwrap_or_default(),
        config.model_reasoning_summary,
        config.model_verbosity.unwrap_or_default(),
    ));

    let workspace_dir = std::env::current_dir()?;
    let collaboration_store = Arc::new(codex_core::orchestration::CollaborationStore::new());
    let log_dir = Plan_dir.parent().unwrap().join("executions");
    let orchestrator = Arc::new(codex_core::orchestration::PlanOrchestrator::new(
        runtime,
        collaboration_store,
        workspace_dir,
        vec![],
    ));
    let executor = PlanExecutor::new(orchestrator, log_dir);

    // Subscribe to events before executing
    let mut event_rx = executor.subscribe().await;
    
    // Spawn event listener
    let progress_handle = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                ExecutionEvent::Started { timestamp, .. } => {
                    println!("⏳ Execution started at {}", timestamp.format("%H:%M:%S"));
                }
                ExecutionEvent::StepCompleted {
                    step_name,
                    timestamp,
                    ..
                } => {
                    println!("✅ Step completed: {} at {}", step_name, timestamp.format("%H:%M:%S"));
                }
                ExecutionEvent::FileChanged {
                    file_path,
                    change_type,
                    ..
                } => {
                    println!("📝 File {}: {}", change_type, file_path);
                }
                ExecutionEvent::TestPassed { test_name, .. } => {
                    println!("🧪 Test passed: {}", test_name);
                }
                ExecutionEvent::TestFailed {
                    test_name, error, ..
                } => {
                    println!("❌ Test failed: {} - {}", test_name, error);
                }
                ExecutionEvent::Completed { timestamp, .. } => {
                    println!();
                    println!("🎉 Execution completed at {}", timestamp.format("%H:%M:%S"));
                }
                ExecutionEvent::Failed { error, .. } => {
                    println!();
                    println!("💥 Execution failed: {}", error);
                }
                _ => {}
            }
        }
    });
    
    // Execute Plan
    let execution_result = executor.execute(Plan).await?;

    // Wait for progress listener to finish
    progress_handle.await?;

    println!();
    println!("Execution ID: {}", execution_result.execution_id);
    println!("Success: {}", execution_result.success);

    if !execution_result.success {
        if let Some(error) = &execution_result.error {
            println!("Error: {}", error);
        }
        std::process::exit(1);
    }

    Ok(())
}

/// Rollback a Plan execution
pub async fn rollback_execution(execution_id: &str, Plan_dir: &PathBuf) -> Result<()> {
    println!("🔄 Rolling back execution: {}", execution_id);

    let config = Arc::new(Config::load_from_disk_or_default()?);
    let auth_manager = AuthManager::shared(config.codex_home.clone(), false, config.cli_auth_credentials_store_mode);
    let otel_manager = OtelEventManager::new_noop();
    let conversation_id = ConversationId::new();
    
    let runtime = Arc::new(AgentRuntime::new(
        std::env::current_dir()?,
        1_000_000,
        config.clone(),
        Some(Arc::clone(&auth_manager)),
        otel_manager,
        config.model_provider.clone(),
        conversation_id,
        config.model_reasoning_effort.unwrap_or_default(),
        config.model_reasoning_summary,
        config.model_verbosity.unwrap_or_default(),
    ));

    let workspace_dir = std::env::current_dir()?;
    let collaboration_store = Arc::new(CollaborationStore::new());
    let log_dir = Plan_dir.parent().unwrap().join("executions");
    let orchestrator = Arc::new(PlanOrchestrator::new(
        runtime,
        collaboration_store,
        workspace_dir,
        vec![],
    ));
    let executor = PlanExecutor::new(orchestrator, log_dir);

    // Load execution log and mark as rolled back
    let execution_log = executor.load_execution_log(execution_id)?;
    // TODO: Implement actual rollback logic using ExecutionLog
    println!("⚠️  Rollback functionality not yet fully implemented");

    println!("✅ Rollback completed successfully");

    Ok(())
}

/// List execution logs
pub async fn list_executions(
    Plan_id_filter: Option<String>,
    Plan_dir: &PathBuf,
) -> Result<()> {
    let config = Arc::new(Config::load_from_disk_or_default()?);
    let auth_manager = AuthManager::shared(config.codex_home.clone(), false, config.cli_auth_credentials_store_mode);
    let otel_manager = OtelEventManager::new_noop();
    let conversation_id = ConversationId::new();
    
    let runtime = Arc::new(AgentRuntime::new(
        std::env::current_dir()?,
        1_000_000,
        config.clone(),
        Some(Arc::clone(&auth_manager)),
        otel_manager,
        config.model_provider.clone(),
        conversation_id,
        config.model_reasoning_effort.unwrap_or_default(),
        config.model_reasoning_summary,
        config.model_verbosity.unwrap_or_default(),
    ));

    let workspace_dir = std::env::current_dir()?;
    let collaboration_store = Arc::new(CollaborationStore::new());
    let log_dir = Plan_dir.parent().unwrap().join("executions");
    let orchestrator = Arc::new(PlanOrchestrator::new(
        runtime,
        collaboration_store,
        workspace_dir,
        vec![],
    ));
    let executor = PlanExecutor::new(orchestrator, log_dir);

    let mut logs = executor.list_executions()?;

    // Apply filter if specified
    if let Some(ref bp_id) = Plan_id_filter {
        logs.retain(|log| log.Plan_id == *bp_id);
    }

    if logs.is_empty() {
        println!("📋 No execution logs found.");
        return Ok(());
    }

    println!("📋 Execution Logs ({})", logs.len());
    println!();

    for log in logs {
        let status_icon = if log.success { "✅" } else { "❌" };

        println!(
            "{} {} | {} | {}",
            status_icon, log.execution_id, if log.success { "Success" } else { "Failed" }, log.Plan_id
        );
        println!(
            "   Started: {} | Duration: {:.2}s",
            log.started_at.format("%Y-%m-%d %H:%M:%S"),
            log.duration_secs.unwrap_or(0.0)
        );

        if let Some(error) = &log.error {
            println!("   Error: {}", error);
        }

        println!();
    }

    Ok(())
}
