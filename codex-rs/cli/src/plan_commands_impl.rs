//! Plan command implementations
//!
//! Implementation functions for Plan commands

use anyhow::Context;
use anyhow::Result;
use codex_core::AuthManager;
use codex_core::agents::AgentRuntime;
use codex_core::Plan::PlanBlock;
use codex_core::Plan::PlanExecutor;
use codex_core::Plan::ExecutionLog;
use codex_core::Plan::ExecutorConfig;
use codex_core::Plan::ProgressEvent;
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

    // Create progress channel
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<ProgressEvent>();

    // Spawn progress listener
    let progress_handle = tokio::spawn(async move {
        while let Some(event) = progress_rx.recv().await {
            match event {
                ProgressEvent::ExecutionStarted { timestamp, .. } => {
                    println!("⏳ Execution started at {}", timestamp.format("%H:%M:%S"));
                }
                ProgressEvent::StepCompleted {
                    step_name,
                    progress,
                    ..
                } => {
                    println!(
                        "✅ Step completed: {} ({:.1}%)",
                        step_name,
                        progress * 100.0
                    );
                }
                ProgressEvent::FileChanged {
                    file_path,
                    change_type,
                    ..
                } => {
                    println!("📝 File {}: {}", change_type, file_path);
                }
                ProgressEvent::TestPassed { test_name, .. } => {
                    println!("🧪 Test passed: {}", test_name);
                }
                ProgressEvent::TestFailed {
                    test_name, error, ..
                } => {
                    println!("❌ Test failed: {} - {}", test_name, error);
                }
                ProgressEvent::Completed { duration_secs, .. } => {
                    println!();
                    println!("🎉 Execution completed in {:.2}s", duration_secs);
                }
                ProgressEvent::Failed { error, .. } => {
                    println!();
                    println!("💥 Execution failed: {}", error);
                }
            }
        }
    });

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
        config.model_reasoning_effort,
        config.model_reasoning_summary,
        config.model_verbosity.unwrap_or_default(),
    ));

    let executor_config = ExecutorConfig {
        workspace_dir: std::env::current_dir()?,
        log_dir: Plan_dir.parent().unwrap().join("executions"),
        ..Default::default()
    };

    let mut executor = PlanExecutor::new(runtime, executor_config)?;
    executor.set_progress_channel(progress_tx);

    // Execute Plan
    let execution_log = executor.execute(Plan).await?;

    // Wait for progress listener to finish
    progress_handle.await?;

    println!();
    println!("Execution ID: {}", execution_log.execution_id);
    println!("Status: {}", execution_log.final_state);
    println!("Summary: {}", execution_log.summary);

    if !execution_log.success {
        if let Some(error) = &execution_log.error {
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
        config.model_reasoning_effort,
        config.model_reasoning_summary,
        config.model_verbosity.unwrap_or_default(),
    ));

    let executor_config = ExecutorConfig {
        workspace_dir: std::env::current_dir()?,
        log_dir: Plan_dir.parent().unwrap().join("executions"),
        ..Default::default()
    };

    let executor = PlanExecutor::new(runtime, executor_config)?;

    executor.rollback(execution_id).await?;

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
        config.model_reasoning_effort,
        config.model_reasoning_summary,
        config.model_verbosity.unwrap_or_default(),
    ));

    let executor_config = ExecutorConfig {
        workspace_dir: std::env::current_dir()?,
        log_dir: Plan_dir.parent().unwrap().join("executions"),
        ..Default::default()
    };

    let executor = PlanExecutor::new(runtime, executor_config)?;

    let mut logs = executor.list_executions().await?;

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
            status_icon, log.execution_id, log.final_state, log.Plan_id
        );
        println!(
            "   Started: {} | Duration: {:.2}s",
            log.started_at.format("%Y-%m-%d %H:%M:%S"),
            log.ended_at
                .map(|end| (end - log.started_at).num_seconds() as f64)
                .unwrap_or(0.0)
        );

        if let Some(error) = &log.error {
            println!("   Error: {}", error);
        }

        println!();
    }

    Ok(())
}
