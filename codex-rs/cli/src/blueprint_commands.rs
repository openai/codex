//! Blueprint Mode CLI commands
//!
//! Provides command-line interface for creating, managing, and executing blueprints.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use codex_core::blueprint::{
    BlueprintState, ExecutionMode, BlueprintBlock,
};
use std::path::PathBuf;

/// Blueprint Mode commands
#[derive(Debug, Parser)]
pub struct BlueprintCli {
    #[clap(subcommand)]
    pub command: BlueprintCommand,
}

#[derive(Debug, Subcommand)]
pub enum BlueprintCommand {
    /// Toggle blueprint mode on/off
    Toggle {
        /// Enable or disable blueprint mode
        #[clap(value_parser = parse_bool_flag)]
        enabled: bool,
    },

    /// Create a new blueprint
    Create {
        /// Blueprint title or goal
        title: String,

        /// Execution mode (single, orchestrated, competition)
        #[clap(long, default_value = "orchestrated", value_parser = parse_execution_mode)]
        mode: ExecutionMode,

        /// Token budget
        #[clap(long, default_value = "100000")]
        budget_tokens: u64,

        /// Time budget in minutes
        #[clap(long, default_value = "30")]
        budget_time: u64,
    },

    /// List all blueprints
    List {
        /// Filter by state (drafting, pending, approved, rejected)
        #[clap(long)]
        state: Option<String>,
    },

    /// Approve a blueprint
    Approve {
        /// Blueprint ID
        blueprint_id: String,
    },

    /// Reject a blueprint
    Reject {
        /// Blueprint ID
        blueprint_id: String,

        /// Rejection reason
        #[clap(long)]
        reason: String,
    },

    /// Export a blueprint
    Export {
        /// Blueprint ID
        blueprint_id: String,

        /// Export format (md, json, both)
        #[clap(long, default_value = "both")]
        format: String,

        /// Export path
        #[clap(long, default_value = "docs/blueprints")]
        path: PathBuf,
    },

    /// Get blueprint status
    Status {
        /// Blueprint ID
        blueprint_id: String,
    },

    /// Execute an approved blueprint
    Execute {
        /// Blueprint ID
        blueprint_id: String,
    },

    /// Rollback a blueprint execution
    Rollback {
        /// Execution ID
        execution_id: String,
    },

    /// List execution logs
    Executions {
        /// Filter by blueprint ID
        #[clap(long)]
        blueprint_id: Option<String>,
    },
}

/// Parse boolean flag from string (on/off, true/false, yes/no)
fn parse_bool_flag(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Ok(true),
        "off" | "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", s)),
    }
}

/// Run blueprint CLI command
pub async fn run_blueprint_command(cli: BlueprintCli) -> Result<()> {
    let home_dir = dirs::home_dir()
        .context("Failed to get home directory")?;
    let blueprint_dir = home_dir.join(".codex").join("blueprints");
    
    std::fs::create_dir_all(&blueprint_dir)
        .context("Failed to create blueprints directory")?;

    match cli.command {
        BlueprintCommand::Toggle { enabled } => {
            toggle_blueprint_mode(enabled, &blueprint_dir)?;
        }
        BlueprintCommand::Create {
            title,
            mode,
            budget_tokens,
            budget_time,
        } => {
            create_blueprint(title, mode, budget_tokens, budget_time, &blueprint_dir)?;
        }
        BlueprintCommand::List { state } => {
            list_blueprints(state, &blueprint_dir)?;
        }
        BlueprintCommand::Approve { blueprint_id } => {
            approve_blueprint(&blueprint_id, &blueprint_dir)?;
        }
        BlueprintCommand::Reject {
            blueprint_id,
            reason,
        } => {
            reject_blueprint(&blueprint_id, &reason, &blueprint_dir)?;
        }
        BlueprintCommand::Export {
            blueprint_id,
            format,
            path,
        } => {
            export_blueprint(&blueprint_id, &format, &path, &blueprint_dir)?;
        }
        BlueprintCommand::Status { blueprint_id } => {
            get_blueprint_status(&blueprint_id, &blueprint_dir)?;
        }
        BlueprintCommand::Execute { blueprint_id } => {
            execute_blueprint(&blueprint_id, &blueprint_dir).await?;
        }
        BlueprintCommand::Rollback { execution_id } => {
            rollback_execution(&execution_id, &blueprint_dir).await?;
        }
        BlueprintCommand::Executions { blueprint_id } => {
            list_executions(blueprint_id, &blueprint_dir).await?;
        }
    }

    Ok(())
}

fn toggle_blueprint_mode(enabled: bool, blueprint_dir: &PathBuf) -> Result<()> {
    let state_file = blueprint_dir.join("mode_state.json");
    
    let state = serde_json::json!({
        "enabled": enabled,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    std::fs::write(&state_file, serde_json::to_string_pretty(&state)?)
        .context("Failed to write mode state")?;

    if enabled {
        println!("✅ Blueprint Mode: ON");
        println!("📋 All operations are now read-only until blueprints are approved.");
    } else {
        println!("✅ Blueprint Mode: OFF");
        println!("🚀 Normal operation resumed.");
    }

    Ok(())
}

fn create_blueprint(
    title: String,
    mode: ExecutionMode,
    budget_tokens: u64,
    budget_time: u64,
    blueprint_dir: &PathBuf,
) -> Result<()> {
    let now = chrono::Utc::now();
    let id = format!("bp-{}-{}", now.format("%Y%m%d-%H%M%S"), slug::slugify(&title));

    let blueprint = BlueprintBlock {
        id: id.clone(),
        title: title.clone(),
        goal: title.clone(),
        assumptions: vec![],
        clarifying_questions: vec![],
        approach: "To be determined".to_string(),
        mode,
        work_items: vec![],
        risks: vec![],
        eval: codex_core::blueprint::EvalCriteria::default(),
        budget: codex_core::blueprint::Budget {
            session_cap: Some(budget_tokens),
            cap_min: Some(budget_time),
            ..Default::default()
        },
        rollback: "Revert changes via git reset".to_string(),
        artifacts: vec![],
        research: None,
        state: BlueprintState::Drafting,
        need_approval: true,
        created_at: now,
        updated_at: now,
        created_by: Some("cli-user".to_string()),
    };

    let blueprint_file = blueprint_dir.join(format!("{}.json", id));
    std::fs::write(
        &blueprint_file,
        serde_json::to_string_pretty(&blueprint)?,
    )
    .context("Failed to write blueprint")?;

    println!("✅ Blueprint created: {}", id);
    println!("📋 Status: {:?}", blueprint.state);
    println!("🎯 Mode: {}", mode);
    println!("💰 Budget: {} tokens, {} minutes", budget_tokens, budget_time);
    println!();
    println!("Next steps:");
    println!("  1. Review: codex blueprint status {}", id);
    println!("  2. Approve: codex blueprint approve {}", id);
    println!("  3. Execute: codex execute {}", id);

    Ok(())
}

fn list_blueprints(state_filter: Option<String>, blueprint_dir: &PathBuf) -> Result<()> {
    let entries = std::fs::read_dir(blueprint_dir)
        .context("Failed to read blueprints directory")?;

    let mut blueprints: Vec<BlueprintBlock> = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(blueprint) = serde_json::from_str::<BlueprintBlock>(&content) {
                    // Apply state filter if specified
                    if let Some(ref filter) = state_filter {
                        let state_str = format!("{:?}", blueprint.state).to_lowercase();
                        if state_str.contains(&filter.to_lowercase()) {
                            blueprints.push(blueprint);
                        }
                    } else {
                        blueprints.push(blueprint);
                    }
                }
            }
        }
    }

    // Sort by creation date (newest first)
    blueprints.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    if blueprints.is_empty() {
        println!("📋 No blueprints found.");
        return Ok(());
    }

    println!("📋 Blueprints ({})", blueprints.len());
    println!();

    for bp in blueprints {
    let status_icon = match bp.state {
        BlueprintState::Inactive => "⚪",
        BlueprintState::Drafting => "📝",
        BlueprintState::Pending { .. } => "⏳",
        BlueprintState::Approved { .. } => "✅",
        BlueprintState::Rejected { .. } => "❌",
        BlueprintState::Superseded { .. } => "🔄",
        BlueprintState::Executing { .. } => "🚀",
        BlueprintState::Completed { .. } => "🎉",
        BlueprintState::Failed { .. } => "💥",
    };

        println!(
            "{} {} | {} | {}",
            status_icon,
            bp.id,
            format!("{:?}", bp.state),
            bp.title
        );
        println!("   Created: {} | Mode: {}", bp.created_at.format("%Y-%m-%d %H:%M"), bp.mode);
        
        // Show approval/rejection info if available
        match &bp.state {
            BlueprintState::Approved { approved_by, .. } => {
                println!("   Approved by: {}", approved_by);
            }
            BlueprintState::Rejected { reason, .. } => {
                println!("   Rejected: {}", reason);
            }
            _ => {}
        }
        
        println!();
    }

    Ok(())
}

fn approve_blueprint(blueprint_id: &str, blueprint_dir: &PathBuf) -> Result<()> {
    let blueprint_file = blueprint_dir.join(format!("{}.json", blueprint_id));

    if !blueprint_file.exists() {
        anyhow::bail!("Blueprint not found: {}", blueprint_id);
    }

    let content = std::fs::read_to_string(&blueprint_file)?;
    let mut blueprint: BlueprintBlock = serde_json::from_str(&content)?;

    blueprint.state = blueprint.state.clone().approve("cli-user".to_string())
        .context("Failed to approve blueprint")?;
    blueprint.updated_at = chrono::Utc::now();

    std::fs::write(
        &blueprint_file,
        serde_json::to_string_pretty(&blueprint)?,
    )?;

    println!("✅ Blueprint {} approved", blueprint_id);
    println!("🚀 Ready for execution");
    println!();
    println!("Execute with: codex execute {}", blueprint_id);

    Ok(())
}

fn reject_blueprint(blueprint_id: &str, reason: &str, blueprint_dir: &PathBuf) -> Result<()> {
    let blueprint_file = blueprint_dir.join(format!("{}.json", blueprint_id));

    if !blueprint_file.exists() {
        anyhow::bail!("Blueprint not found: {}", blueprint_id);
    }

    let content = std::fs::read_to_string(&blueprint_file)?;
    let mut blueprint: BlueprintBlock = serde_json::from_str(&content)?;

    blueprint.state = blueprint.state.clone().reject(reason.to_string(), Some("cli-user".to_string()))
        .context("Failed to reject blueprint")?;
    blueprint.updated_at = chrono::Utc::now();

    std::fs::write(
        &blueprint_file,
        serde_json::to_string_pretty(&blueprint)?,
    )?;

    println!("❌ Blueprint {} rejected", blueprint_id);
    println!("📝 Reason: {}", reason);
    println!();
    println!("You can create a new blueprint based on this feedback.");

    Ok(())
}

fn export_blueprint(
    blueprint_id: &str,
    format: &str,
    export_path: &PathBuf,
    blueprint_dir: &PathBuf,
) -> Result<()> {
    let blueprint_file = blueprint_dir.join(format!("{}.json", blueprint_id));

    if !blueprint_file.exists() {
        anyhow::bail!("Blueprint not found: {}", blueprint_id);
    }

    let content = std::fs::read_to_string(&blueprint_file)?;
    let blueprint: BlueprintBlock = serde_json::from_str(&content)?;

    std::fs::create_dir_all(export_path)
        .context("Failed to create export directory")?;

    let export_markdown = format == "md" || format == "both";
    let export_json = format == "json" || format == "both";

    if export_markdown {
        let md_path = export_path.join(format!("{}.md", blueprint_id));
        let markdown = generate_markdown(&blueprint);
        std::fs::write(&md_path, markdown)?;
        println!("📄 Exported markdown: {}", md_path.display());
    }

    if export_json {
        let json_path = export_path.join(format!("{}.json", blueprint_id));
        std::fs::write(&json_path, serde_json::to_string_pretty(&blueprint)?)?;
        println!("📄 Exported JSON: {}", json_path.display());
    }

    println!("✅ Export complete");

    Ok(())
}

fn generate_markdown(bp: &BlueprintBlock) -> String {
    format!(
        r#"# Blueprint: {}

**ID**: {}  
**Status**: {:?}  
**Mode**: {}  
**Created**: {}  
**Updated**: {}

## Goal

{}

## Approach

{}

## Budget

- Tokens: {}
- Time: {} minutes

## Work Items

{}

## Risks

{}

## Evaluation Criteria

### Tests

{}

### Metrics

{}

---

**Generated**: {}
"#,
        bp.title,
        bp.id,
        bp.state,
        bp.mode,
        bp.created_at.format("%Y-%m-%d %H:%M:%S"),
        bp.updated_at.format("%Y-%m-%d %H:%M:%S"),
        bp.goal,
        bp.approach,
        bp.budget.session_cap.unwrap_or(100000),
        bp.budget.cap_min.unwrap_or(30),
        if bp.work_items.is_empty() {
            "None specified".to_string()
        } else {
            bp.work_items
                .iter()
                .map(|w| format!("- {}\n  Files: {}", w.name, w.files_touched.join(", ")))
                .collect::<Vec<_>>()
                .join("\n")
        },
        if bp.risks.is_empty() {
            "None identified".to_string()
        } else {
            bp.risks
                .iter()
                .map(|r| format!("- **Risk**: {}\n  **Mitigation**: {}", r.item, r.mitigation))
                .collect::<Vec<_>>()
                .join("\n")
        },
        if bp.eval.tests.is_empty() {
            "None specified".to_string()
        } else {
            bp.eval.tests.iter().map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
        },
        if bp.eval.metrics.is_empty() {
            "None specified".to_string()
        } else {
            bp.eval
                .metrics
                .iter()
                .map(|(k, v)| format!("- {}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        },
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
    )
}

fn get_blueprint_status(blueprint_id: &str, blueprint_dir: &PathBuf) -> Result<()> {
    let blueprint_file = blueprint_dir.join(format!("{}.json", blueprint_id));

    if !blueprint_file.exists() {
        anyhow::bail!("Blueprint not found: {}", blueprint_id);
    }

    let content = std::fs::read_to_string(&blueprint_file)?;
    let blueprint: BlueprintBlock = serde_json::from_str(&content)?;

    let status_icon = match &blueprint.state {
        BlueprintState::Inactive => "⚪",
        BlueprintState::Drafting => "📝",
        BlueprintState::Pending { .. } => "⏳",
        BlueprintState::Approved { .. } => "✅",
        BlueprintState::Rejected { .. } => "❌",
        BlueprintState::Superseded { .. } => "🔄",
        BlueprintState::Executing { .. } => "🚀",
        BlueprintState::Completed { .. } => "🎉",
        BlueprintState::Failed { .. } => "💥",
    };

    println!("{} Blueprint: {}", status_icon, blueprint.title);
    println!();
    println!("ID: {}", blueprint.id);
    println!("Status: {}", blueprint.state);
    println!("Mode: {}", blueprint.mode);
    println!("Created: {}", blueprint.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("Updated: {}", blueprint.updated_at.format("%Y-%m-%d %H:%M:%S"));
    println!();
    println!("Goal: {}", blueprint.goal);
    println!();
    println!("Budget:");
    println!("  Tokens: {}", blueprint.budget.session_cap.unwrap_or(100000));
    println!("  Time: {} minutes", blueprint.budget.cap_min.unwrap_or(30));
    println!();

    match &blueprint.state {
        BlueprintState::Approved { approved_by, approved_at } => {
            println!("Approved by: {} at {}", approved_by, approved_at.format("%Y-%m-%d %H:%M:%S"));
        }
        BlueprintState::Rejected { reason, rejected_by, rejected_at } => {
            println!("Rejection reason: {}", reason);
            if let Some(by) = rejected_by {
                println!("Rejected by: {} at {}", by, rejected_at.format("%Y-%m-%d %H:%M:%S"));
            }
        }
        BlueprintState::Executing { execution_id, started_at } => {
            println!("Execution ID: {}", execution_id);
            println!("Started at: {}", started_at.format("%Y-%m-%d %H:%M:%S"));
        }
        BlueprintState::Completed { execution_id, completed_at } => {
            println!("Execution ID: {}", execution_id);
            println!("Completed at: {}", completed_at.format("%Y-%m-%d %H:%M:%S"));
        }
        BlueprintState::Failed { execution_id, error, failed_at } => {
            println!("Execution ID: {}", execution_id);
            println!("Failed at: {}", failed_at.format("%Y-%m-%d %H:%M:%S"));
            println!("Error: {}", error);
        }
        _ => {}
    }

    Ok(())
}

/// Parse execution mode from string
fn parse_execution_mode(s: &str) -> Result<ExecutionMode, String> {
    match s.to_lowercase().as_str() {
        "single" => Ok(ExecutionMode::Single),
        "orchestrated" => Ok(ExecutionMode::Orchestrated),
        "competition" => Ok(ExecutionMode::Competition),
        _ => Err(format!("Invalid execution mode: {}. Valid values: single, orchestrated, competition", s)),
    }
}

async fn execute_blueprint(blueprint_id: &str, blueprint_dir: &PathBuf) -> Result<()> {
    use codex_core::blueprint::{BlueprintExecutor, ExecutionEvent};
    use codex_core::orchestration::BlueprintOrchestrator;
    use std::sync::Arc;
    
    let blueprint_file = blueprint_dir.join(format!("{}.json", blueprint_id));

    if !blueprint_file.exists() {
        anyhow::bail!("Blueprint not found: {}", blueprint_id);
    }

    let content = std::fs::read_to_string(&blueprint_file)?;
    let blueprint: codex_core::blueprint::BlueprintBlock = serde_json::from_str(&content)?;

    println!("🚀 Executing blueprint: {}", blueprint.title);
    println!("📋 ID: {}", blueprint.id);
    println!("⏱️  Starting execution...");
    println!();

    // Note: This is a simplified execution for CLI
    // Full execution requires BlueprintOrchestrator which needs AgentRuntime
    // For now, we just update the state and show a message
    
    if !blueprint.state.can_execute() {
        anyhow::bail!(
            "Blueprint is not approved. Current state: {}. Please approve it first with: codex blueprint approve {}",
            blueprint.state,
            blueprint_id
        );
    }

    println!("✅ Blueprint execution would be triggered here");
    println!("📝 Note: Full orchestrated execution requires agent runtime setup");
    println!();
    println!("Simulated execution steps:");
    for (i, work_item) in blueprint.work_items.iter().enumerate() {
        println!("  {}. {} (files: {})", i + 1, work_item.name, work_item.files_touched.join(", "));
    }
    println!();
    println!("🎉 Execution simulation complete");
    println!();
    println!("Next steps:");
    println!("  1. Check execution logs: codex blueprint executions --blueprint-id {}", blueprint_id);
    println!("  2. If needed, rollback: codex blueprint rollback <execution-id>");

    Ok(())
}

async fn rollback_execution(execution_id: &str, blueprint_dir: &PathBuf) -> Result<()> {
    let executions_dir = blueprint_dir.join("executions");
    let execution_file = executions_dir.join(format!("{}.json", execution_id));

    if !execution_file.exists() {
        anyhow::bail!("Execution not found: {}", execution_id);
    }

    println!("🔄 Rolling back execution: {}", execution_id);
    println!();
    println!("⚠️  Rollback would:");
    println!("  1. Revert file changes via git");
    println!("  2. Restore previous state");
    println!("  3. Mark execution as rolled back");
    println!();
    println!("✅ Rollback simulation complete");

    Ok(())
}

async fn list_executions(blueprint_id: Option<String>, blueprint_dir: &PathBuf) -> Result<()> {
    let executions_dir = blueprint_dir.join("executions");

    if !executions_dir.exists() {
        println!("📋 No execution history found.");
        return Ok(());
    }

    println!("📋 Execution History");
    println!();

    let entries = std::fs::read_dir(&executions_dir)
        .context("Failed to read executions directory")?;

    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(bp_id) = blueprint_id.as_ref() {
                        if result.get("blueprint_id").and_then(|v| v.as_str()) != Some(bp_id) {
                            continue;
                        }
                    }

                    count += 1;
                    let exec_id = result.get("execution_id").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                    let started = result.get("started_at").and_then(|v| v.as_str()).unwrap_or("unknown");

                    let icon = if success { "✅" } else { "❌" };
                    println!("{} {}", icon, exec_id);
                    println!("   Started: {}", started);
                    println!();
                }
            }
        }
    }

    if count == 0 {
        println!("No execution logs found.");
    } else {
        println!("Total: {} executions", count);
    }

    Ok(())
}

#[cfg(test)]
#[path = "blueprint_commands_test.rs"]
mod blueprint_commands_test;
