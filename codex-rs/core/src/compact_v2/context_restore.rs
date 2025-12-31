//! Context restoration after compaction.
//!
//! Restores files, todos, and plan files after compaction to preserve context.

use std::path::PathBuf;

use codex_protocol::ConversationId;

use super::CompactConfig;
use super::token_counter::TokenCounter;
use crate::state::state_ext;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;

/// Get the .claude directory path using home directory for consistency.
fn get_claude_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
}

/// Restored context after compaction.
#[derive(Debug, Clone, Default)]
pub struct RestoredContext {
    /// Restored file attachments (recently read files)
    pub files: Vec<FileAttachment>,
    /// Restored todo list (if enabled)
    pub todos: Option<TodoAttachment>,
    /// Restored plan file (if in plan mode)
    pub plan: Option<PlanAttachment>,
}

/// A restored file attachment.
#[derive(Debug, Clone)]
pub struct FileAttachment {
    /// Absolute path to the file
    pub filename: String,
    /// File content (possibly truncated)
    pub content: String,
    /// Token count of the content
    pub token_count: i64,
}

/// A restored todo list attachment.
#[derive(Debug, Clone)]
pub struct TodoAttachment {
    /// JSON content of the todo list
    pub content: String,
}

/// A restored plan file attachment.
#[derive(Debug, Clone)]
pub struct PlanAttachment {
    /// Path to the plan file
    pub filename: String,
    /// Plan file content
    pub content: String,
}

/// Check if a file is agent-related and should be excluded from restoration.
///
/// Agent-related files are excluded because they are managed separately
/// and may contain stale or conflicting state.
pub fn is_agent_file(filename: &str, agent_id: &str) -> bool {
    filename.contains(".claude/plans/")
        || filename.contains(&format!(".claude/agents/{agent_id}"))
        || filename.ends_with(".agent.json")
}

/// Restore context after compaction.
///
/// Restores:
/// - Recently read files (from state_ext read file tracking)
/// - Todo list (from .claude/todos/{conversation_id}.json)
/// - Plan file (from .claude/plans/*-{conversation_id}.md)
pub fn restore_context(conversation_id: &str, config: &CompactConfig) -> RestoredContext {
    let mut result = RestoredContext::default();

    // Create TokenCounter from config for consistent token estimation
    let token_counter = TokenCounter::from(config);

    // Restore files (from state_ext read file tracking)
    if let Ok(conv_id) = ConversationId::from_string(conversation_id) {
        result.files = restore_file_reads(conv_id, conversation_id, config, &token_counter);
    }

    // Restore todos
    if config.restore_todos {
        result.todos = restore_todo_list(conversation_id);
    }

    // Restore plan
    if config.restore_plan {
        result.plan = restore_plan_file(conversation_id);
    }

    result
}

/// Restore recently read files for context restoration.
///
/// Applies the following constraints from config:
/// - `restore_max_files`: Maximum number of files to restore
/// - `restore_tokens_per_file`: Maximum tokens per file
/// - `restore_total_file_budget`: Total token budget for all files
///
/// Files are sorted by recency (most recent first) and agent files are excluded.
fn restore_file_reads(
    conversation_id: ConversationId,
    agent_id: &str,
    config: &CompactConfig,
    token_counter: &TokenCounter,
) -> Vec<FileAttachment> {
    // Get read files from state_ext (already sorted by timestamp descending)
    let read_files = state_ext::get_read_files(conversation_id);

    if read_files.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut total_tokens: i64 = 0;
    let max_files = config.restore_max_files as usize;
    let tokens_per_file = config.restore_tokens_per_file;
    let total_budget = config.restore_total_file_budget;

    for entry in read_files {
        // Check if we've reached max files
        if result.len() >= max_files {
            break;
        }

        // Skip agent files
        if is_agent_file(&entry.filename, agent_id) {
            continue;
        }

        // Calculate effective tokens for this file (capped at per-file limit)
        let file_tokens = entry.token_count.min(tokens_per_file);

        // Check if adding this file would exceed total budget
        if total_tokens + file_tokens > total_budget {
            // Try to fit partial content
            let remaining_budget = total_budget - total_tokens;
            if remaining_budget <= 0 {
                break;
            }
            // Use remaining budget for this file
            if let Some(attachment) =
                read_and_truncate_file(&entry.filename, remaining_budget, token_counter)
            {
                result.push(attachment);
            }
            break;
        }

        // Read and truncate file content
        if let Some(attachment) =
            read_and_truncate_file(&entry.filename, tokens_per_file, token_counter)
        {
            total_tokens += attachment.token_count;
            result.push(attachment);
        }
    }

    result
}

/// Read a file and truncate to fit within token budget.
fn read_and_truncate_file(
    filename: &str,
    max_tokens: i64,
    token_counter: &TokenCounter,
) -> Option<FileAttachment> {
    let content = std::fs::read_to_string(filename).ok()?;

    // Truncate if necessary
    let truncated = truncate_text(&content, TruncationPolicy::Tokens(max_tokens as usize));

    // Use provided TokenCounter for consistent token estimation
    let token_count = token_counter.approximate(&truncated);

    Some(FileAttachment {
        filename: filename.to_string(),
        content: truncated,
        token_count,
    })
}

/// Restore todo list from file.
fn restore_todo_list(conversation_id: &str) -> Option<TodoAttachment> {
    let claude_dir = get_claude_dir();
    let todos_dir = claude_dir.join("todos");

    // Try conversation-specific path first
    let todo_path = todos_dir.join(format!("{conversation_id}.json"));
    if let Ok(content) = std::fs::read_to_string(&todo_path) {
        return Some(TodoAttachment { content });
    }

    // Try default path
    let default_path = todos_dir.join("default.json");
    if let Ok(content) = std::fs::read_to_string(&default_path) {
        return Some(TodoAttachment { content });
    }

    None
}

/// Restore plan file from .claude/plans directory.
fn restore_plan_file(conversation_id: &str) -> Option<PlanAttachment> {
    // Use absolute path for .claude/plans directory
    let plans_dir = get_claude_dir().join("plans");
    if !plans_dir.exists() {
        return None;
    }

    // Try to find a plan file that matches the conversation ID pattern
    // Claude Code uses: {adjective}-{noun}-{animal}-{conversation_id}.md
    let suffix = format!("{conversation_id}.md");

    if let Ok(entries) = std::fs::read_dir(&plans_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            // Check if this file ends with the conversation ID
            if filename.ends_with(&suffix) {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    return Some(PlanAttachment {
                        filename: entry.path().to_string_lossy().to_string(),
                        content,
                    });
                }
            }
        }

        // Fall back to finding any .md file that was recently modified
        // This handles cases where the conversation ID isn't in the filename
        let mut entries: Vec<_> = std::fs::read_dir(&plans_dir)
            .ok()?
            .flatten()
            .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
            .collect();

        // Sort by modification time (most recent first)
        entries.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|m| m.modified()).ok();
            let b_time = b.metadata().and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        if let Some(entry) = entries.first() {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                return Some(PlanAttachment {
                    filename: entry.path().to_string_lossy().to_string(),
                    content,
                });
            }
        }
    }

    None
}

/// Format restored context as ResponseItem messages for inclusion in history.
///
/// Returns a vector of formatted content strings that can be included
/// in the compacted history.
pub fn format_restored_context(restored: &RestoredContext) -> Vec<String> {
    let mut parts = Vec::new();

    // Format files
    for file in &restored.files {
        parts.push(format!(
            "--- Restored file: {} ---\n{}",
            file.filename, file.content
        ));
    }

    // Format todos
    if let Some(todos) = &restored.todos {
        parts.push(format!("--- Restored todo list ---\n{}", todos.content));
    }

    // Format plan
    if let Some(plan) = &restored.plan {
        parts.push(format!(
            "--- Restored plan file: {} ---\n{}\n\n\
             If this plan is relevant to the current work and not already complete, \
             continue working on it.",
            plan.filename, plan.content
        ));
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_agent_file_detects_plans() {
        assert!(is_agent_file(".claude/plans/my-plan.md", "agent-123"));
        assert!(is_agent_file("/path/to/.claude/plans/test.md", "agent-123"));
    }

    #[test]
    fn is_agent_file_detects_agent_dirs() {
        assert!(is_agent_file(
            ".claude/agents/agent-123/state.json",
            "agent-123"
        ));
        assert!(!is_agent_file(
            ".claude/agents/other/state.json",
            "agent-123"
        ));
    }

    #[test]
    fn is_agent_file_detects_agent_json() {
        assert!(is_agent_file("some/path/file.agent.json", "any"));
        assert!(!is_agent_file("some/path/file.json", "any"));
    }

    #[test]
    fn is_agent_file_allows_regular_files() {
        assert!(!is_agent_file("src/main.rs", "agent-123"));
        assert!(!is_agent_file("README.md", "agent-123"));
        assert!(!is_agent_file(".claude/config.toml", "agent-123"));
    }

    #[test]
    fn restored_context_default() {
        let ctx = RestoredContext::default();
        assert!(ctx.files.is_empty());
        assert!(ctx.todos.is_none());
        assert!(ctx.plan.is_none());
    }

    #[test]
    fn format_restored_context_empty() {
        let ctx = RestoredContext::default();
        let formatted = format_restored_context(&ctx);
        assert!(formatted.is_empty());
    }

    #[test]
    fn format_restored_context_with_files() {
        let ctx = RestoredContext {
            files: vec![FileAttachment {
                filename: "test.rs".to_string(),
                content: "fn main() {}".to_string(),
                token_count: 10,
            }],
            todos: None,
            plan: None,
        };
        let formatted = format_restored_context(&ctx);
        assert_eq!(formatted.len(), 1);
        assert!(formatted[0].contains("test.rs"));
        assert!(formatted[0].contains("fn main()"));
    }

    #[test]
    fn format_restored_context_with_todos() {
        let ctx = RestoredContext {
            files: vec![],
            todos: Some(TodoAttachment {
                content: r#"[{"content": "Fix bug"}]"#.to_string(),
            }),
            plan: None,
        };
        let formatted = format_restored_context(&ctx);
        assert_eq!(formatted.len(), 1);
        assert!(formatted[0].contains("todo list"));
        assert!(formatted[0].contains("Fix bug"));
    }

    #[test]
    fn format_restored_context_with_plan() {
        let ctx = RestoredContext {
            files: vec![],
            todos: None,
            plan: Some(PlanAttachment {
                filename: "plan.md".to_string(),
                content: "# My Plan\n\n- Step 1".to_string(),
            }),
        };
        let formatted = format_restored_context(&ctx);
        assert_eq!(formatted.len(), 1);
        assert!(formatted[0].contains("plan file"));
        assert!(formatted[0].contains("Step 1"));
    }
}
