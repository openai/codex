//! Loop context for cross-iteration state passing.
//!
//! Stores iteration state for enhanced prompt injection.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

/// Loop execution context.
///
/// Stores cross-iteration state for building enhanced prompts and persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopContext {
    /// Base commit ID at task start.
    pub base_commit_id: String,

    /// Original user prompt.
    pub initial_prompt: String,

    /// Plan file content (if exists).
    pub plan_content: Option<String>,

    /// Completed iteration records.
    pub iterations: Vec<IterationRecord>,

    /// Total iterations (-1 for duration mode).
    pub total_iterations: i32,
}

/// Single iteration record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationRecord {
    /// Iteration number (0-indexed).
    pub iteration: i32,

    /// Commit ID (None if no changes).
    pub commit_id: Option<String>,

    /// Changed files list.
    pub changed_files: Vec<String>,

    /// LLM-generated summary.
    pub summary: String,

    /// Whether iteration succeeded.
    pub success: bool,

    /// Completion timestamp.
    pub timestamp: DateTime<Utc>,
}

impl LoopContext {
    /// Create new LoopContext.
    pub fn new(
        base_commit_id: String,
        initial_prompt: String,
        plan_content: Option<String>,
        total_iterations: i32,
    ) -> Self {
        Self {
            base_commit_id,
            initial_prompt,
            plan_content,
            iterations: Vec::new(),
            total_iterations,
        }
    }

    /// Add iteration record.
    pub fn add_iteration(&mut self, record: IterationRecord) {
        self.iterations.push(record);
    }

    /// Get current iteration (next to execute).
    pub fn current_iteration(&self) -> i32 {
        self.iterations.len() as i32
    }

    /// Get successful iteration count.
    pub fn successful_iterations(&self) -> i32 {
        self.iterations.iter().filter(|r| r.success).count() as i32
    }

    /// Get failed iteration count.
    pub fn failed_iterations(&self) -> i32 {
        self.iterations.iter().filter(|r| !r.success).count() as i32
    }
}

impl IterationRecord {
    /// Create new iteration record.
    pub fn new(
        iteration: i32,
        commit_id: Option<String>,
        changed_files: Vec<String>,
        summary: String,
        success: bool,
    ) -> Self {
        Self {
            iteration,
            commit_id,
            changed_files,
            summary,
            success,
            timestamp: Utc::now(),
        }
    }

    /// Format commit status for display.
    pub fn commit_status(&self) -> String {
        match &self.commit_id {
            Some(id) if id.len() >= 7 => format!("commit {}", &id[..7]),
            Some(id) => format!("commit {id}"),
            None => "no changes".to_string(),
        }
    }

    /// Format file list for display.
    pub fn files_display(&self) -> String {
        if self.changed_files.is_empty() {
            "none".to_string()
        } else if self.changed_files.len() <= 5 {
            self.changed_files.join(", ")
        } else {
            format!(
                "{}, ... ({} more)",
                self.changed_files[..5].join(", "),
                self.changed_files.len() - 5
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_loop_context_new() {
        let ctx = LoopContext::new(
            "abc123".to_string(),
            "Implement feature".to_string(),
            Some("## Plan\n1. Do X".to_string()),
            5,
        );

        assert_eq!(ctx.base_commit_id, "abc123");
        assert_eq!(ctx.initial_prompt, "Implement feature");
        assert!(ctx.plan_content.is_some());
        assert_eq!(ctx.total_iterations, 5);
        assert!(ctx.iterations.is_empty());
    }

    #[test]
    fn test_loop_context_add_iteration() {
        let mut ctx = LoopContext::new("abc".to_string(), "task".to_string(), None, 3);

        ctx.add_iteration(IterationRecord::new(
            0,
            Some("def456".to_string()),
            vec!["file1.rs".to_string()],
            "Did something".to_string(),
            true,
        ));

        assert_eq!(ctx.current_iteration(), 1);
        assert_eq!(ctx.successful_iterations(), 1);
        assert_eq!(ctx.failed_iterations(), 0);

        ctx.add_iteration(IterationRecord::new(
            1,
            None,
            vec![],
            "Failed".to_string(),
            false,
        ));

        assert_eq!(ctx.current_iteration(), 2);
        assert_eq!(ctx.successful_iterations(), 1);
        assert_eq!(ctx.failed_iterations(), 1);
    }

    #[test]
    fn test_iteration_record_commit_status() {
        let with_commit = IterationRecord::new(
            0,
            Some("abcdef1234567890".to_string()),
            vec![],
            "summary".to_string(),
            true,
        );
        assert_eq!(with_commit.commit_status(), "commit abcdef1");

        let no_commit = IterationRecord::new(0, None, vec![], "summary".to_string(), true);
        assert_eq!(no_commit.commit_status(), "no changes");

        let short_commit = IterationRecord::new(
            0,
            Some("abc".to_string()),
            vec![],
            "summary".to_string(),
            true,
        );
        assert_eq!(short_commit.commit_status(), "commit abc");
    }

    #[test]
    fn test_iteration_record_files_display() {
        let empty = IterationRecord::new(0, None, vec![], "s".to_string(), true);
        assert_eq!(empty.files_display(), "none");

        let few = IterationRecord::new(
            0,
            None,
            vec!["a.rs".to_string(), "b.rs".to_string()],
            "s".to_string(),
            true,
        );
        assert_eq!(few.files_display(), "a.rs, b.rs");

        let many = IterationRecord::new(
            0,
            None,
            vec![
                "1.rs".to_string(),
                "2.rs".to_string(),
                "3.rs".to_string(),
                "4.rs".to_string(),
                "5.rs".to_string(),
                "6.rs".to_string(),
                "7.rs".to_string(),
            ],
            "s".to_string(),
            true,
        );
        assert_eq!(
            many.files_display(),
            "1.rs, 2.rs, 3.rs, 4.rs, 5.rs, ... (2 more)"
        );
    }
}
