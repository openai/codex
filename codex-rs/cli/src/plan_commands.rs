//! Plan Mode CLI commands
//!
//! Provides command-line interface for creating, managing, and executing plan.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use codex_core::plan::{ExecutionMode, PlanBlock, PlanState};
use std::path::PathBuf;

/// Plan Mode commands
#[derive(Debug, Parser)]
pub struct PlanCli {
    #[clap(subcommand)]
    pub command: PlanCommand,
}

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// Toggle plan mode on/off
    Toggle {
        /// Enable or disable plan mode
        #[clap(value_parser = parse_bool_flag)]
        enabled: bool,
    },

    /// Create a new Plan
    Create {
        /// Plan title or goal
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

    /// List all Plans
    List {
        /// Filter by state (drafting, pending, approved, rejected)
        #[clap(long)]
        state: Option<String>,
    },

    /// Approve a Plan
    Approve {
        /// Plan ID
        Plan_id: String,
    },

    /// Reject a Plan
    Reject {
        /// Plan ID
        Plan_id: String,

        /// Rejection reason
        #[clap(long)]
        reason: String,
    },

    /// Export a Plan
    Export {
        /// Plan ID
        Plan_id: String,

        /// Export format (md, json, both)
        #[clap(long, default_value = "both")]
        format: String,

        /// Export path
        #[clap(long, default_value = "docs/Plans")]
        path: PathBuf,
    },

    /// Get Plan status
    Status {
        /// Plan ID
        Plan_id: String,
    },

    /// Execute an approved Plan
    Execute {
        /// Plan ID
        Plan_id: String,
    },

    /// Rollback a Plan execution
    Rollback {
        /// Execution ID
        execution_id: String,
    },

    /// List execution logs
    Executions {
        /// Filter by Plan ID
        #[clap(long)]
        Plan_id: Option<String>,
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

/// Run Plan CLI command
pub async fn run_Plan_command(cli: PlanCli) -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let Plan_dir = home_dir.join(".codex").join("Plans");

    std::fs::create_dir_all(&Plan_dir).context("Failed to create Plans directory")?;

    match cli.command {
        PlanCommand::Toggle { enabled } => {
            toggle_Plan_mode(enabled, &Plan_dir)?;
        }
        PlanCommand::Create {
            title,
            mode,
            budget_tokens,
            budget_time,
        } => {
            create_Plan(title, mode, budget_tokens, budget_time, &Plan_dir)?;
        }
        PlanCommand::List { state } => {
            list_Plans(state, &Plan_dir)?;
        }
        PlanCommand::Approve { Plan_id } => {
            approve_Plan(&Plan_id, &Plan_dir)?;
        }
        PlanCommand::Reject { Plan_id, reason } => {
            reject_Plan(&Plan_id, &reason, &Plan_dir)?;
        }
        PlanCommand::Export {
            Plan_id,
            format,
            path,
        } => {
            export_Plan(&Plan_id, &format, &path, &Plan_dir)?;
        }
        PlanCommand::Status { Plan_id } => {
            get_Plan_status(&Plan_id, &Plan_dir)?;
        }
        PlanCommand::Execute { Plan_id } => {
            execute_Plan(&Plan_id, &Plan_dir).await?;
        }
        PlanCommand::Rollback { execution_id } => {
            rollback_execution(&execution_id, &Plan_dir).await?;
        }
        PlanCommand::Executions { Plan_id } => {
            list_executions(Plan_id, &Plan_dir).await?;
        }
    }

    Ok(())
}

fn toggle_Plan_mode(enabled: bool, Plan_dir: &PathBuf) -> Result<()> {
    let state_file = Plan_dir.join("mode_state.json");

    let state = serde_json::json!({
        "enabled": enabled,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    std::fs::write(&state_file, serde_json::to_string_pretty(&state)?)
        .context("Failed to write mode state")?;

    if enabled {
        println!("✅ plan mode: ON");
        println!("📋 All operations are now read-only until Plans are approved.");
    } else {
        println!("✅ plan mode: OFF");
        println!("🚀 Normal operation resumed.");
    }

    Ok(())
}

fn create_Plan(
    title: String,
    mode: ExecutionMode,
    budget_tokens: u64,
    budget_time: u64,
    Plan_dir: &PathBuf,
) -> Result<()> {
    let now = chrono::Utc::now();
    let id = format!(
        "bp-{}-{}",
        now.format("%Y%m%d-%H%M%S"),
        slug::slugify(&title)
    );

    let Plan = PlanBlock {
        id: id.clone(),
        title: title.clone(),
        goal: title.clone(),
        assumptions: vec![],
        clarifying_questions: vec![],
        approach: "To be determined".to_string(),
        mode,
        work_items: vec![],
        risks: vec![],
        eval: codex_core::plan::EvalCriteria::default(),
        budget: codex_core::plan::Budget {
            session_cap: Some(budget_tokens),
            cap_min: Some(budget_time),
            ..Default::default()
        },
        rollback: "Revert changes via git reset".to_string(),
        artifacts: vec![],
        research: None,
        state: PlanState::Drafting,
        need_approval: true,
        created_at: now,
        updated_at: now,
        created_by: Some("cli-user".to_string()),
    };

    let Plan_file = Plan_dir.join(format!("{}.json", id));
    std::fs::write(&Plan_file, serde_json::to_string_pretty(&Plan)?)
        .context("Failed to write Plan")?;

    println!("✅ Plan created: {}", id);
    println!("📋 Status: {:?}", Plan.state);
    println!("🎯 Mode: {}", mode);
    println!(
        "💰 Budget: {} tokens, {} minutes",
        budget_tokens, budget_time
    );
    println!();
    println!("Next steps:");
    println!("  1. Review: codex Plan status {}", id);
    println!("  2. Approve: codex Plan approve {}", id);
    println!("  3. Execute: codex execute {}", id);

    Ok(())
}

fn list_Plans(state_filter: Option<String>, Plan_dir: &PathBuf) -> Result<()> {
    let entries = std::fs::read_dir(Plan_dir).context("Failed to read Plans directory")?;

    let mut plan: Vec<PlanBlock> = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(Plan) = serde_json::from_str::<PlanBlock>(&content) {
                    // Apply state filter if specified
                    if let Some(ref filter) = state_filter {
                        let state_str = format!("{:?}", Plan.state).to_lowercase();
                        if state_str.contains(&filter.to_lowercase()) {
                            plan.push(Plan);
                        }
                    } else {
                        plan.push(Plan);
                    }
                }
            }
        }
    }

    // Sort by creation date (newest first)
    plan.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    if plan.is_empty() {
        println!("📋 No Plans found.");
        return Ok(());
    }

    println!("📋 Plans ({})", plan.len());
    println!();

    for bp in plan {
        let status_icon = match bp.state {
            PlanState::Inactive => "⚪",
            PlanState::Drafting => "📝",
            PlanState::Pending { .. } => "⏳",
            PlanState::Approved { .. } => "✅",
            PlanState::Rejected { .. } => "❌",
            PlanState::Superseded { .. } => "🔄",
            PlanState::Executing { .. } => "🚀",
            PlanState::Completed { .. } => "🎉",
            PlanState::Failed { .. } => "💥",
        };

        println!(
            "{} {} | {} | {}",
            status_icon,
            bp.id,
            format!("{:?}", bp.state),
            bp.title
        );
        println!(
            "   Created: {} | Mode: {}",
            bp.created_at.format("%Y-%m-%d %H:%M"),
            bp.mode
        );

        // Show approval/rejection info if available
        match &bp.state {
            PlanState::Approved { approved_by, .. } => {
                println!("   Approved by: {}", approved_by);
            }
            PlanState::Rejected { reason, .. } => {
                println!("   Rejected: {}", reason);
            }
            _ => {}
        }

        println!();
    }

    Ok(())
}

fn approve_Plan(Plan_id: &str, Plan_dir: &PathBuf) -> Result<()> {
    let Plan_file = Plan_dir.join(format!("{}.json", Plan_id));

    if !Plan_file.exists() {
        anyhow::bail!("Plan not found: {}", Plan_id);
    }

    let content = std::fs::read_to_string(&Plan_file)?;
    let mut Plan: PlanBlock = serde_json::from_str(&content)?;

    Plan.state = Plan
        .state
        .clone()
        .approve("cli-user".to_string())
        .context("Failed to approve Plan")?;
    Plan.updated_at = chrono::Utc::now();

    std::fs::write(&Plan_file, serde_json::to_string_pretty(&Plan)?)?;

    println!("✅ Plan {} approved", Plan_id);
    println!("🚀 Ready for execution");
    println!();
    println!("Execute with: codex execute {}", Plan_id);

    Ok(())
}

fn reject_Plan(Plan_id: &str, reason: &str, Plan_dir: &PathBuf) -> Result<()> {
    let Plan_file = Plan_dir.join(format!("{}.json", Plan_id));

    if !Plan_file.exists() {
        anyhow::bail!("Plan not found: {}", Plan_id);
    }

    let content = std::fs::read_to_string(&Plan_file)?;
    let mut Plan: PlanBlock = serde_json::from_str(&content)?;

    Plan.state = Plan
        .state
        .clone()
        .reject(reason.to_string(), Some("cli-user".to_string()))
        .context("Failed to reject Plan")?;
    Plan.updated_at = chrono::Utc::now();

    std::fs::write(&Plan_file, serde_json::to_string_pretty(&Plan)?)?;

    println!("❌ Plan {} rejected", Plan_id);
    println!("📝 Reason: {}", reason);
    println!();
    println!("You can create a new Plan based on this feedback.");

    Ok(())
}

fn export_Plan(
    Plan_id: &str,
    format: &str,
    export_path: &PathBuf,
    Plan_dir: &PathBuf,
) -> Result<()> {
    let Plan_file = Plan_dir.join(format!("{}.json", Plan_id));

    if !Plan_file.exists() {
        anyhow::bail!("Plan not found: {}", Plan_id);
    }

    let content = std::fs::read_to_string(&Plan_file)?;
    let Plan: PlanBlock = serde_json::from_str(&content)?;

    std::fs::create_dir_all(export_path).context("Failed to create export directory")?;

    let export_markdown = format == "md" || format == "both";
    let export_json = format == "json" || format == "both";

    if export_markdown {
        let md_path = export_path.join(format!("{}.md", Plan_id));
        let markdown = generate_markdown(&Plan);
        std::fs::write(&md_path, markdown)?;
        println!("📄 Exported markdown: {}", md_path.display());
    }

    if export_json {
        let json_path = export_path.join(format!("{}.json", Plan_id));
        std::fs::write(&json_path, serde_json::to_string_pretty(&Plan)?)?;
        println!("📄 Exported JSON: {}", json_path.display());
    }

    println!("✅ Export complete");

    Ok(())
}

fn generate_markdown(bp: &PlanBlock) -> String {
    format!(
        r#"# Plan: {}

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
            bp.eval
                .tests
                .iter()
                .map(|t| format!("- {}", t))
                .collect::<Vec<_>>()
                .join("\n")
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

fn get_Plan_status(Plan_id: &str, Plan_dir: &PathBuf) -> Result<()> {
    let Plan_file = Plan_dir.join(format!("{}.json", Plan_id));

    if !Plan_file.exists() {
        anyhow::bail!("Plan not found: {}", Plan_id);
    }

    let content = std::fs::read_to_string(&Plan_file)?;
    let Plan: PlanBlock = serde_json::from_str(&content)?;

    let status_icon = match &Plan.state {
        PlanState::Inactive => "⚪",
        PlanState::Drafting => "📝",
        PlanState::Pending { .. } => "⏳",
        PlanState::Approved { .. } => "✅",
        PlanState::Rejected { .. } => "❌",
        PlanState::Superseded { .. } => "🔄",
        PlanState::Executing { .. } => "🚀",
        PlanState::Completed { .. } => "🎉",
        PlanState::Failed { .. } => "💥",
    };

    println!("{} Plan: {}", status_icon, Plan.title);
    println!();
    println!("ID: {}", Plan.id);
    println!("Status: {}", Plan.state);
    println!("Mode: {}", Plan.mode);
    println!("Created: {}", Plan.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("Updated: {}", Plan.updated_at.format("%Y-%m-%d %H:%M:%S"));
    println!();
    println!("Goal: {}", Plan.goal);
    println!();
    println!("Budget:");
    println!("  Tokens: {}", Plan.budget.session_cap.unwrap_or(100000));
    println!("  Time: {} minutes", Plan.budget.cap_min.unwrap_or(30));
    println!();

    match &Plan.state {
        PlanState::Approved {
            approved_by,
            approved_at,
        } => {
            println!(
                "Approved by: {} at {}",
                approved_by,
                approved_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
        PlanState::Rejected {
            reason,
            rejected_by,
            rejected_at,
        } => {
            println!("Rejection reason: {}", reason);
            if let Some(by) = rejected_by {
                println!(
                    "Rejected by: {} at {}",
                    by,
                    rejected_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
        PlanState::Executing {
            execution_id,
            started_at,
        } => {
            println!("Execution ID: {}", execution_id);
            println!("Started at: {}", started_at.format("%Y-%m-%d %H:%M:%S"));
        }
        PlanState::Completed {
            execution_id,
            completed_at,
        } => {
            println!("Execution ID: {}", execution_id);
            println!("Completed at: {}", completed_at.format("%Y-%m-%d %H:%M:%S"));
        }
        PlanState::Failed {
            execution_id,
            error,
            failed_at,
        } => {
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
        _ => Err(format!(
            "Invalid execution mode: {}. Valid values: single, orchestrated, competition",
            s
        )),
    }
}

async fn execute_Plan(Plan_id: &str, Plan_dir: &PathBuf) -> Result<()> {
    use codex_core::plan::{ExecutionEvent, PlanExecutor};
    use codex_core::orchestration::PlanOrchestrator;
    use std::sync::Arc;

    let Plan_file = Plan_dir.join(format!("{}.json", Plan_id));

    if !Plan_file.exists() {
        anyhow::bail!("Plan not found: {}", Plan_id);
    }

    let content = std::fs::read_to_string(&Plan_file)?;
    let Plan: codex_core::plan::PlanBlock = serde_json::from_str(&content)?;

    println!("🚀 Executing Plan: {}", Plan.title);
    println!("📋 ID: {}", Plan.id);
    println!("⏱️  Starting execution...");
    println!();

    // Note: This is a simplified execution for CLI
    // Full execution requires PlanOrchestrator which needs AgentRuntime
    // For now, we just update the state and show a message

    if !Plan.state.can_execute() {
        anyhow::bail!(
            "Plan is not approved. Current state: {}. Please approve it first with: codex Plan approve {}",
            Plan.state,
            Plan_id
        );
    }

    println!("✅ Plan execution would be triggered here");
    println!("📝 Note: Full orchestrated execution requires agent runtime setup");
    println!();
    println!("Simulated execution steps:");
    for (i, work_item) in Plan.work_items.iter().enumerate() {
        println!(
            "  {}. {} (files: {})",
            i + 1,
            work_item.name,
            work_item.files_touched.join(", ")
        );
    }
    println!();
    println!("🎉 Execution simulation complete");
    println!();
    println!("Next steps:");
    println!(
        "  1. Check execution logs: codex Plan executions --Plan-id {}",
        Plan_id
    );
    println!("  2. If needed, rollback: codex Plan rollback <execution-id>");

    Ok(())
}

async fn rollback_execution(execution_id: &str, Plan_dir: &PathBuf) -> Result<()> {
    let executions_dir = Plan_dir.join("executions");
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

async fn list_executions(Plan_id: Option<String>, Plan_dir: &PathBuf) -> Result<()> {
    let executions_dir = Plan_dir.join("executions");

    if !executions_dir.exists() {
        println!("📋 No execution history found.");
        return Ok(());
    }

    println!("📋 Execution History");
    println!();

    let entries =
        std::fs::read_dir(&executions_dir).context("Failed to read executions directory")?;

    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(bp_id) = Plan_id.as_ref() {
                        if result.get("Plan_id").and_then(|v| v.as_str()) != Some(bp_id) {
                            continue;
                        }
                    }

                    count += 1;
                    let exec_id = result
                        .get("execution_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let success = result
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let started = result
                        .get("started_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

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
#[path = "Plan_commands_test.rs"]
mod Plan_commands_test;
