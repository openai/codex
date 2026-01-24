//! LLM-driven merge command for SpawnTask.
//!
//! This module provides functionality to merge changes from completed
//! SpawnTask worktrees back into the main branch, with LLM assistance
//! for conflict resolution.
//!
//! This is a framework-level feature (like worktree) that works with
//! ALL spawn task types that use git worktrees.

use super::SpawnTaskMetadata;

/// Request to merge one or more spawn tasks.
#[derive(Debug, Clone)]
pub struct MergeRequest {
    /// Task IDs to merge.
    pub task_ids: Vec<String>,
    /// Optional user query for merge guidance.
    pub query: Option<String>,
}

/// Information about merge conflicts.
#[derive(Debug, Clone)]
pub struct ConflictInfo {
    /// Files with conflicts.
    pub conflicted_files: Vec<String>,
    /// Raw conflict markers.
    pub conflict_content: String,
}

/// Build merge prompt for main agent to execute.
///
/// This is a framework-level functionality that works with any SpawnTask
/// type that uses worktrees. The prompt instructs the main agent to perform
/// git merge operations and resolve any conflicts that arise.
pub fn build_merge_prompt(
    request: &MergeRequest,
    tasks_metadata: &[SpawnTaskMetadata],
    conflict_info: Option<&ConflictInfo>,
) -> String {
    let mut prompt = String::new();

    // Header
    prompt.push_str("## Merge Spawn Tasks\n\n");

    // Task summaries
    prompt.push_str("### Tasks to Merge\n\n");
    for task in tasks_metadata {
        prompt.push_str(&format!(
            "- **{}** ({}): {} iterations completed",
            task.task_id, task.task_type, task.iterations_completed
        ));
        if let Some(ref query) = task.user_query {
            prompt.push_str(&format!(" - \"{}\"", truncate_query(query, 50)));
        }
        prompt.push('\n');
        if let Some(ref branch) = task.branch_name {
            prompt.push_str(&format!("  - Branch: `{}`\n", branch));
        }
    }
    prompt.push('\n');

    // Merge instructions
    prompt.push_str("### Instructions\n\n");
    prompt.push_str("Please perform the following steps:\n\n");

    for task in tasks_metadata {
        if let Some(ref branch) = task.branch_name {
            prompt.push_str(&format!(
                "1. Merge branch `{}` into the current branch:\n",
                branch
            ));
            prompt.push_str(&format!("   ```bash\n   git merge {}\n   ```\n\n", branch));
        }
    }

    // Conflict resolution
    if let Some(conflict) = conflict_info {
        prompt.push_str("### Conflict Resolution\n\n");
        prompt.push_str("The following files have conflicts that need resolution:\n\n");
        for file in &conflict.conflicted_files {
            prompt.push_str(&format!("- `{}`\n", file));
        }
        prompt.push_str("\nPlease resolve these conflicts by:\n");
        prompt.push_str("1. Reading the conflicted files\n");
        prompt.push_str("2. Understanding both versions of the changes\n");
        prompt.push_str("3. Creating a merged version that incorporates the best of both\n");
        prompt.push_str("4. Removing conflict markers\n");
        prompt.push_str("5. Staging the resolved files with `git add`\n");
        prompt.push_str("6. Completing the merge with `git commit`\n\n");
    }

    // User guidance
    if let Some(ref query) = request.query {
        prompt.push_str("### User Guidance\n\n");
        prompt.push_str(query);
        prompt.push_str("\n\n");
    }

    // Final notes
    prompt.push_str("### Notes\n\n");
    prompt.push_str("- Review the changes carefully before committing\n");
    prompt.push_str("- Ensure all tests pass after the merge\n");
    prompt.push_str("- Commit the merge with a descriptive message\n");

    prompt
}

/// Truncate a query string for display.
fn truncate_query(query: &str, max_len: usize) -> String {
    if query.len() <= max_len {
        query.to_string()
    } else {
        format!("{}...", &query[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_driver::LoopCondition;
    use crate::spawn_task::SpawnTaskStatus;
    use crate::spawn_task::SpawnTaskType;
    use chrono::Utc;
    use std::path::PathBuf;

    fn create_test_metadata(task_id: &str) -> SpawnTaskMetadata {
        SpawnTaskMetadata {
            task_id: task_id.to_string(),
            task_type: SpawnTaskType::Agent,
            status: SpawnTaskStatus::Completed,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
            cwd: PathBuf::from("/test"),
            error_message: None,
            loop_condition: Some(LoopCondition::Iters { count: 5 }),
            user_query: Some("Implement feature X".to_string()),
            iterations_completed: 5,
            iterations_failed: 0,
            model_override: None,
            workflow_path: None,
            worktree_path: Some(PathBuf::from(
                "/home/user/.codex/spawn-tasks/worktrees/task1",
            )),
            branch_name: Some(task_id.to_string()),
            base_branch: Some("main".to_string()),
            log_file: None,
            execution_result: None,
        }
    }

    #[test]
    fn test_build_merge_prompt() {
        let request = MergeRequest {
            task_ids: vec!["task1".to_string()],
            query: Some("Review and merge the authentication changes".to_string()),
        };

        let metadata = vec![create_test_metadata("task1")];

        let prompt = build_merge_prompt(&request, &metadata, None);

        assert!(prompt.contains("Merge Spawn Tasks"));
        assert!(prompt.contains("task1"));
        assert!(prompt.contains("git merge"));
        assert!(prompt.contains("Review and merge the authentication"));
    }

    #[test]
    fn test_build_merge_prompt_with_conflicts() {
        let request = MergeRequest {
            task_ids: vec!["task1".to_string()],
            query: None,
        };

        let metadata = vec![create_test_metadata("task1")];

        let conflict = ConflictInfo {
            conflicted_files: vec!["src/auth.rs".to_string(), "src/lib.rs".to_string()],
            conflict_content: "<<<< HEAD\n...\n>>>>".to_string(),
        };

        let prompt = build_merge_prompt(&request, &metadata, Some(&conflict));

        assert!(prompt.contains("Conflict Resolution"));
        assert!(prompt.contains("src/auth.rs"));
        assert!(prompt.contains("src/lib.rs"));
    }

    #[test]
    fn test_truncate_query() {
        assert_eq!(truncate_query("short", 10), "short");
        assert_eq!(
            truncate_query("this is a very long query", 15),
            "this is a ve..."
        );
    }
}
