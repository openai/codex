//! Extension module for /spawn command handling in TUI.
//!
//! Re-exports command parsing from core and provides TUI-specific formatting.
//! Also provides SpawnAction enum to minimize inline dispatch code in chatwidget.rs.
//! Includes impl ChatWidget for handle_spawn_command to minimize upstream conflicts.

use crate::app_event::AppEvent;
use crate::chatwidget::ChatWidget;
use codex_core::spawn_task::SpawnCommand;
use codex_core::spawn_task::SpawnTaskMetadata;
use codex_core::spawn_task::SpawnTaskStatus;
use std::path::Path;

// Re-export from core for TUI usage
pub use codex_core::spawn_task::parse_spawn_command;

// ============================================================================
// SpawnAction - dispatch result to minimize inline code in chatwidget.rs
// ============================================================================

/// Result of spawn command dispatch for chatwidget.rs.
///
/// This enum allows the dispatch logic to be in the ext file while
/// chatwidget.rs only needs a simple match to execute the action.
#[derive(Debug)]
pub enum SpawnAction {
    /// Send an AppEvent to the event handler.
    SendEvent(AppEvent),
    /// Show help (calls handle_spawn_command).
    HandleHelp,
    /// Display error message.
    Error(String),
}

/// Dispatch a spawn command string to a SpawnAction.
///
/// Parses the command and returns the appropriate action for chatwidget.rs.
pub fn dispatch_spawn_command(text: &str) -> SpawnAction {
    match parse_spawn_command(text) {
        Ok(cmd) => match cmd {
            SpawnCommand::Start(args) => SpawnAction::SendEvent(AppEvent::StartSpawnTask { args }),
            SpawnCommand::Help => SpawnAction::HandleHelp,
            SpawnCommand::List => SpawnAction::SendEvent(AppEvent::SpawnListRequest),
            SpawnCommand::Status { task_id } => {
                SpawnAction::SendEvent(AppEvent::SpawnStatusRequest { task_id })
            }
            SpawnCommand::Kill { task_id } => {
                SpawnAction::SendEvent(AppEvent::SpawnKillRequest { task_id })
            }
            SpawnCommand::Drop { task_id } => {
                SpawnAction::SendEvent(AppEvent::SpawnDropRequest { task_id })
            }
            SpawnCommand::Merge { task_ids, prompt } => {
                SpawnAction::SendEvent(AppEvent::SpawnMergeRequest { task_ids, prompt })
            }
        },
        Err(e) => SpawnAction::Error(format!("Spawn error: {e}")),
    }
}

/// Format help message for /spawn command.
pub fn format_spawn_help() -> String {
    r#"Spawn Task Management

Commands:
  /spawn [options] --prompt <task>   Start a new spawn task
  /spawn --list                      List all spawn tasks
  /spawn --status <task-id>          Show task status
  /spawn --kill <task-id>            Stop a running task
  /spawn --drop <task-id>            Delete task metadata
  /spawn --merge <task-id>...        Merge task branches

Start Options:
  --name <id>           Task identifier (default: auto-generated)
  --model <provider_id> Provider ID (config key), or provider_id/model format
  --iter <n>            Run for n iterations
  --time <duration>     Run for duration (e.g., 1h, 30m)

Examples:
  /spawn --iter 5 --prompt implement user authentication
  /spawn --name auth-task --model deepseek --iter 3 --prompt add login
  /spawn --kill my-task
  /spawn --merge task-1 task-2 --prompt review and merge

Current Spawn Tasks:"#
        .to_string()
}

/// Format task list output.
pub fn format_task_list(tasks: &[SpawnTaskMetadata]) -> String {
    if tasks.is_empty() {
        return "  No spawn tasks found.".to_string();
    }

    let mut output = String::new();
    for task in tasks {
        let status_icon = match task.status {
            SpawnTaskStatus::Running => "▶",
            SpawnTaskStatus::Completed => "✓",
            SpawnTaskStatus::Failed => "✗",
            SpawnTaskStatus::Cancelled => "○",
        };

        output.push_str(&format!(
            "\n  {} {} [{}] - {} iterations",
            status_icon, task.task_id, task.status, task.iterations_completed
        ));

        if let Some(ref query) = task.user_query {
            let truncated = if query.len() > 40 {
                format!("{}...", &query[..37])
            } else {
                query.clone()
            };
            output.push_str(&format!(" - \"{}\"", truncated));
        }

        if let Some(ref branch) = task.branch_name {
            output.push_str(&format!("\n      Branch: {branch}"));
        }
    }

    output
}

/// Format detailed task status output.
pub fn format_task_status(task: &SpawnTaskMetadata) -> String {
    let status_icon = match task.status {
        SpawnTaskStatus::Running => "▶",
        SpawnTaskStatus::Completed => "✓",
        SpawnTaskStatus::Failed => "✗",
        SpawnTaskStatus::Cancelled => "○",
    };

    let mut output = format!(
        "Task: {} {}\nStatus: {}\nType: {}\nIterations: {} completed, {} failed",
        status_icon,
        task.task_id,
        task.status,
        task.task_type,
        task.iterations_completed,
        task.iterations_failed
    );

    if let Some(ref query) = task.user_query {
        output.push_str(&format!("\nQuery: {query}"));
    }

    if let Some(ref model) = task.model_override {
        output.push_str(&format!("\nModel: {model}"));
    }

    if let Some(ref branch) = task.branch_name {
        output.push_str(&format!("\nBranch: {branch}"));
    }

    if let Some(ref base) = task.base_branch {
        output.push_str(&format!("\nBase: {base}"));
    }

    if let Some(ref worktree) = task.worktree_path {
        output.push_str(&format!("\nWorktree: {}", worktree.display()));
    }

    if let Some(ref error) = task.error_message {
        output.push_str(&format!("\nError: {error}"));
    }

    output.push_str(&format!(
        "\nCreated: {}",
        task.created_at.format("%Y-%m-%d %H:%M:%S")
    ));

    if let Some(ref completed) = task.completed_at {
        output.push_str(&format!(
            "\nCompleted: {}",
            completed.format("%Y-%m-%d %H:%M:%S")
        ));
    }

    output
}

/// List all task metadata from the spawn tasks directory (synchronous).
///
/// This is a sync version for TUI use since the TUI event handlers are synchronous.
pub fn list_task_metadata_sync(codex_home: &Path) -> Result<Vec<SpawnTaskMetadata>, String> {
    let dir = codex_home.join("spawn-tasks");

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(&dir).map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut result: Vec<SpawnTaskMetadata> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(metadata) = serde_json::from_str::<SpawnTaskMetadata>(&content) {
                    result.push(metadata);
                }
            }
        }
    }

    // Sort by creation time, newest first
    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(result)
}

// =============================================================================
// impl ChatWidget - Spawn command handler moved from chatwidget.rs
// =============================================================================

impl ChatWidget {
    /// Try to dispatch /spawn command. Returns true if handled.
    /// Moved from chatwidget.rs to minimize upstream merge conflicts.
    pub(crate) fn try_dispatch_spawn_command(&mut self, text: &str) -> bool {
        if text.trim() == "/spawn" || text.trim().starts_with("/spawn ") {
            match dispatch_spawn_command(text) {
                SpawnAction::SendEvent(ev) => self.app_event_tx().send(ev),
                SpawnAction::HandleHelp => self.handle_spawn_command(),
                SpawnAction::Error(msg) => self.add_info_message(msg, None),
            }
            self.request_redraw();
            return true;
        }
        false
    }

    /// Handle /spawn command - show help and current task list.
    /// Moved from chatwidget.rs to minimize upstream merge conflicts.
    pub(crate) fn handle_spawn_command(&mut self) {
        // Show help and current task list
        let mut output = format_spawn_help();
        output.push_str("\n\n");

        match list_task_metadata_sync(self.codex_home()) {
            Ok(tasks) => {
                output.push_str(&format_task_list(&tasks));
            }
            Err(e) => {
                output.push_str(&format!("Error listing tasks: {e}"));
            }
        }

        self.add_info_message(output, None);
        self.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use codex_core::loop_driver::LoopCondition;
    use codex_core::spawn_task::SpawnTaskType;
    use std::path::PathBuf;

    fn create_test_metadata(task_id: &str) -> SpawnTaskMetadata {
        SpawnTaskMetadata {
            task_id: task_id.to_string(),
            task_type: SpawnTaskType::Agent,
            status: SpawnTaskStatus::Running,
            created_at: Utc::now(),
            completed_at: None,
            cwd: PathBuf::from("/test"),
            error_message: None,
            loop_condition: Some(LoopCondition::Iters { count: 5 }),
            user_query: Some("Implement feature X".to_string()),
            iterations_completed: 2,
            iterations_failed: 0,
            model_override: None,
            workflow_path: None,
            worktree_path: None,
            branch_name: Some("spawn-task1".to_string()),
            base_branch: Some("main".to_string()),
            log_file: None,
            execution_result: None,
        }
    }

    #[test]
    fn format_empty_list() {
        let output = format_task_list(&[]);
        assert!(output.contains("No spawn tasks found"));
    }

    #[test]
    fn format_task_list_with_tasks() {
        let tasks = vec![create_test_metadata("task-1")];
        let output = format_task_list(&tasks);

        assert!(output.contains("task-1"));
        assert!(output.contains("▶")); // Running icon
        assert!(output.contains("2 iterations"));
        assert!(output.contains("Implement feature X"));
        assert!(output.contains("spawn-task1"));
    }
}
