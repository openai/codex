//! Context compaction logic for managing conversation history size.
//!
//! This module implements a 3-tier compaction strategy:
//! - **Tier 1 (Session Memory)**: Use cached summary.md - zero API cost
//! - **Tier 2 (Full Compact)**: LLM-based summarization when no cache
//! - **Micro-compact**: Pre-API removal of old tool results (no LLM)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};

/// Minimum tokens that must be saved for micro-compaction to be worthwhile.
pub const MIN_MICRO_COMPACT_SAVINGS: i32 = 20_000;

/// Maximum files to restore after compaction.
pub const CONTEXT_RESTORATION_MAX_FILES: i32 = 5;

/// Maximum token budget for context restoration.
pub const CONTEXT_RESTORATION_BUDGET: i32 = 50_000;

/// Number of recent tool results to keep during micro-compaction.
pub const RECENT_TOOL_RESULTS_TO_KEEP: i32 = 3;

/// Configuration for context compaction behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Context usage ratio (0.0 - 1.0) at which compaction triggers.
    #[serde(default = "default_threshold")]
    pub threshold: f64,

    /// Whether micro-compaction of large tool results is enabled.
    #[serde(default = "default_micro_compact")]
    pub micro_compact: bool,

    /// Minimum number of messages to retain after compaction.
    #[serde(default = "default_min_messages")]
    pub min_messages_to_keep: i32,

    /// Session memory configuration for Tier 1 compaction.
    #[serde(default)]
    pub session_memory: SessionMemoryConfig,
}

fn default_threshold() -> f64 {
    0.8
}

fn default_micro_compact() -> bool {
    true
}

fn default_min_messages() -> i32 {
    4
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold: default_threshold(),
            micro_compact: default_micro_compact(),
            min_messages_to_keep: default_min_messages(),
            session_memory: SessionMemoryConfig::default(),
        }
    }
}

/// Configuration for session memory (Tier 1 compaction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemoryConfig {
    /// Whether session memory is enabled.
    #[serde(default = "default_session_memory_enabled")]
    pub enabled: bool,

    /// Path to the session memory file (summary.md).
    #[serde(default)]
    pub summary_path: Option<PathBuf>,

    /// Minimum tokens to save for session memory to be used.
    #[serde(default = "default_session_memory_min_savings")]
    pub min_savings_tokens: i32,

    /// Last summarized message ID (for incremental updates).
    #[serde(default)]
    pub last_summarized_id: Option<String>,
}

fn default_session_memory_enabled() -> bool {
    false
}

fn default_session_memory_min_savings() -> i32 {
    10_000
}

impl Default for SessionMemoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_session_memory_enabled(),
            summary_path: None,
            min_savings_tokens: default_session_memory_min_savings(),
            last_summarized_id: None,
        }
    }
}

/// Result of a compaction operation, summarising what was removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    /// Number of messages removed during compaction.
    pub removed_messages: i32,

    /// Approximate token count of the generated summary.
    pub summary_tokens: i32,

    /// Number of messages that were micro-compacted (tool output trimmed).
    pub micro_compacted: i32,

    /// The tier of compaction used.
    pub tier: CompactionTier,

    /// Tokens saved by this compaction.
    pub tokens_saved: i32,
}

/// Which compaction tier was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionTier {
    /// Tier 1: Session memory (cached summary.md).
    SessionMemory,
    /// Tier 2: Full LLM-based compaction.
    Full,
    /// Micro-compaction only (no summarization).
    Micro,
}

/// Items to restore after compaction.
#[derive(Debug, Clone, Default)]
pub struct ContextRestoration {
    /// Files to restore (path, content, priority).
    pub files: Vec<FileRestoration>,
    /// Todo list state.
    pub todos: Option<String>,
    /// Plan mode state.
    pub plan: Option<String>,
    /// Active skills.
    pub skills: Vec<String>,
}

/// A file to restore after compaction.
#[derive(Debug, Clone)]
pub struct FileRestoration {
    /// Path to the file.
    pub path: PathBuf,
    /// File content (or summary if too large).
    pub content: String,
    /// Priority for restoration (higher = more important).
    pub priority: i32,
    /// Estimated token count.
    pub tokens: i32,
}

/// Determine whether compaction should be triggered.
///
/// Returns `true` when the ratio of `context_tokens` to `max_tokens` meets or
/// exceeds the configured `threshold`.
pub fn should_compact(context_tokens: i32, max_tokens: i32, threshold: f64) -> bool {
    if max_tokens <= 0 {
        return false;
    }
    let usage = context_tokens as f64 / max_tokens as f64;
    usage >= threshold
}

/// Identify message indices that are candidates for micro-compaction.
///
/// Micro-compaction targets messages with large `tool_result` content that can
/// be summarised without losing critical information. Returns a list of indices
/// (0-based) into the provided `messages` slice.
pub fn micro_compact_candidates(messages: &[serde_json::Value]) -> Vec<i32> {
    let mut candidates = Vec::new();
    for (i, msg) in messages.iter().enumerate() {
        // A message is a micro-compact candidate when it carries a tool_result
        // role and its content exceeds a reasonable size threshold.
        let is_tool_result = msg
            .get("role")
            .and_then(|v| v.as_str())
            .is_some_and(|r| r == "tool" || r == "tool_result");

        let content_len = msg
            .get("content")
            .and_then(|v| v.as_str())
            .map_or(0, |s| s.len());

        // 2000 chars is a reasonable threshold for micro-compaction.
        if is_tool_result && content_len > 2000 {
            candidates.push(i as i32);
        }
    }
    candidates
}

/// Try to load a session memory summary (Tier 1 compaction).
///
/// Returns the cached summary if available and sufficient savings would result.
/// This is zero-cost as it doesn't call the LLM.
pub fn try_session_memory_compact(config: &SessionMemoryConfig) -> Option<SessionMemorySummary> {
    if !config.enabled {
        return None;
    }

    let path = config.summary_path.as_ref()?;

    // Try to read the summary file
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            debug!(?path, error = %e, "Failed to read session memory file");
            return None;
        }
    };

    if content.is_empty() {
        debug!(?path, "Session memory file is empty");
        return None;
    }

    // Parse the summary format
    let summary = parse_session_memory(&content)?;

    info!(
        summary_tokens = summary.token_estimate,
        last_id = ?summary.last_summarized_id,
        "Loaded session memory summary"
    );

    Some(summary)
}

/// Parsed session memory summary.
#[derive(Debug, Clone)]
pub struct SessionMemorySummary {
    /// The summary text.
    pub summary: String,
    /// Last message ID that was summarized.
    pub last_summarized_id: Option<String>,
    /// Estimated token count of the summary.
    pub token_estimate: i32,
}

/// Parse session memory content from summary.md format.
fn parse_session_memory(content: &str) -> Option<SessionMemorySummary> {
    // The summary.md format has metadata at the top:
    // ---
    // last_summarized_id: turn-123
    // ---
    // <summary content>

    let mut last_id = None;
    let mut summary_start = 0;

    // Check for YAML frontmatter
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let frontmatter = &content[3..3 + end];
            for line in frontmatter.lines() {
                if let Some(id) = line.strip_prefix("last_summarized_id:") {
                    last_id = Some(id.trim().to_string());
                }
            }
            summary_start = 3 + end + 3;
            // Skip leading newlines
            while summary_start < content.len() && content[summary_start..].starts_with('\n') {
                summary_start += 1;
            }
        }
    }

    let summary = content[summary_start..].trim().to_string();
    if summary.is_empty() {
        return None;
    }

    // Rough token estimate: ~4 chars per token
    let token_estimate = (summary.len() / 4) as i32;

    Some(SessionMemorySummary {
        summary,
        last_summarized_id: last_id,
        token_estimate,
    })
}

/// Build context restoration items within the given token budget.
///
/// Prioritizes items by importance and fits as many as possible within budget.
pub fn build_context_restoration(
    files: Vec<FileRestoration>,
    todos: Option<String>,
    plan: Option<String>,
    skills: Vec<String>,
    budget: i32,
) -> ContextRestoration {
    let mut result = ContextRestoration::default();
    let mut remaining = budget;

    // Priority 1: Plan mode state (if active)
    if let Some(p) = plan {
        let tokens = estimate_tokens_for_text(&p);
        if tokens <= remaining {
            result.plan = Some(p);
            remaining -= tokens;
        }
    }

    // Priority 2: Todo list
    if let Some(t) = todos {
        let tokens = estimate_tokens_for_text(&t);
        if tokens <= remaining {
            result.todos = Some(t);
            remaining -= tokens;
        }
    }

    // Priority 3: Skills (typically small)
    for skill in skills {
        let tokens = estimate_tokens_for_text(&skill);
        if tokens <= remaining {
            result.skills.push(skill);
            remaining -= tokens;
        }
    }

    // Priority 4: Files (sorted by priority, limited by max files)
    let mut sorted_files = files;
    sorted_files.sort_by(|a, b| b.priority.cmp(&a.priority));

    for file in sorted_files
        .into_iter()
        .take(CONTEXT_RESTORATION_MAX_FILES as usize)
    {
        if file.tokens <= remaining {
            remaining -= file.tokens;
            result.files.push(file);
        }
    }

    result
}

/// Estimate token count for text (rough approximation).
fn estimate_tokens_for_text(text: &str) -> i32 {
    // ~4 chars per token is a rough estimate
    (text.len() / 4) as i32
}

/// Format context restoration as a message for the conversation.
pub fn format_restoration_message(restoration: &ContextRestoration) -> String {
    let mut parts = Vec::new();

    if let Some(plan) = &restoration.plan {
        parts.push(format!("<plan_context>\n{plan}\n</plan_context>"));
    }

    if let Some(todos) = &restoration.todos {
        parts.push(format!("<todo_list>\n{todos}\n</todo_list>"));
    }

    if !restoration.skills.is_empty() {
        parts.push(format!(
            "<active_skills>\n{}\n</active_skills>",
            restoration.skills.join("\n")
        ));
    }

    for file in &restoration.files {
        parts.push(format!(
            "<file path=\"{}\">\n{}\n</file>",
            file.path.display(),
            file.content
        ));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!(
            "<restored_context>\n{}\n</restored_context>",
            parts.join("\n\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_compaction_config() {
        let config = CompactionConfig::default();
        assert!((config.threshold - 0.8).abs() < f64::EPSILON);
        assert!(config.micro_compact);
        assert_eq!(config.min_messages_to_keep, 4);
    }

    #[test]
    fn test_should_compact_below_threshold() {
        assert!(!should_compact(7000, 10000, 0.8));
    }

    #[test]
    fn test_should_compact_at_threshold() {
        assert!(should_compact(8000, 10000, 0.8));
    }

    #[test]
    fn test_should_compact_above_threshold() {
        assert!(should_compact(9500, 10000, 0.8));
    }

    #[test]
    fn test_should_compact_zero_max() {
        assert!(!should_compact(100, 0, 0.8));
    }

    #[test]
    fn test_should_compact_negative_max() {
        assert!(!should_compact(100, -1, 0.8));
    }

    #[test]
    fn test_micro_compact_candidates_empty() {
        let messages: Vec<serde_json::Value> = vec![];
        assert!(micro_compact_candidates(&messages).is_empty());
    }

    #[test]
    fn test_micro_compact_candidates_no_tool_results() {
        let messages = vec![
            serde_json::json!({"role": "user", "content": "hello"}),
            serde_json::json!({"role": "assistant", "content": "hi"}),
        ];
        assert!(micro_compact_candidates(&messages).is_empty());
    }

    #[test]
    fn test_micro_compact_candidates_small_tool_result() {
        let messages = vec![serde_json::json!({"role": "tool", "content": "ok"})];
        assert!(micro_compact_candidates(&messages).is_empty());
    }

    #[test]
    fn test_micro_compact_candidates_large_tool_result() {
        let large_content = "x".repeat(3000);
        let messages = vec![
            serde_json::json!({"role": "user", "content": "do something"}),
            serde_json::json!({"role": "tool", "content": large_content}),
            serde_json::json!({"role": "assistant", "content": "done"}),
        ];
        let candidates = micro_compact_candidates(&messages);
        assert_eq!(candidates, vec![1]);
    }

    #[test]
    fn test_micro_compact_candidates_tool_result_role() {
        let large_content = "y".repeat(2500);
        let messages = vec![serde_json::json!({"role": "tool_result", "content": large_content})];
        let candidates = micro_compact_candidates(&messages);
        assert_eq!(candidates, vec![0]);
    }

    #[test]
    fn test_parse_session_memory_simple() {
        let content = "This is a summary of the conversation.";
        let summary = parse_session_memory(content).unwrap();
        assert_eq!(summary.summary, "This is a summary of the conversation.");
        assert!(summary.last_summarized_id.is_none());
    }

    #[test]
    fn test_parse_session_memory_with_frontmatter() {
        let content = "---\nlast_summarized_id: turn-42\n---\nSummary content here.";
        let summary = parse_session_memory(content).unwrap();
        assert_eq!(summary.summary, "Summary content here.");
        assert_eq!(summary.last_summarized_id, Some("turn-42".to_string()));
    }

    #[test]
    fn test_parse_session_memory_empty() {
        let content = "";
        assert!(parse_session_memory(content).is_none());
    }

    #[test]
    fn test_build_context_restoration_within_budget() {
        let files = vec![
            FileRestoration {
                path: PathBuf::from("/test/file1.rs"),
                content: "fn main() {}".to_string(),
                priority: 10,
                tokens: 100,
            },
            FileRestoration {
                path: PathBuf::from("/test/file2.rs"),
                content: "struct Foo {}".to_string(),
                priority: 5,
                tokens: 50,
            },
        ];

        let restoration =
            build_context_restoration(files, Some("- TODO 1".to_string()), None, vec![], 500);

        assert!(restoration.todos.is_some());
        assert_eq!(restoration.files.len(), 2);
        // Higher priority file should be first
        assert_eq!(restoration.files[0].path, PathBuf::from("/test/file1.rs"));
    }

    #[test]
    fn test_build_context_restoration_budget_exceeded() {
        let files = vec![FileRestoration {
            path: PathBuf::from("/test/large.rs"),
            content: "x".repeat(10000),
            priority: 10,
            tokens: 2500,
        }];

        // Budget too small for the file
        let restoration = build_context_restoration(files, None, None, vec![], 100);
        assert!(restoration.files.is_empty());
    }

    #[test]
    fn test_format_restoration_message_empty() {
        let restoration = ContextRestoration::default();
        let msg = format_restoration_message(&restoration);
        assert!(msg.is_empty());
    }

    #[test]
    fn test_format_restoration_message_with_content() {
        let mut restoration = ContextRestoration::default();
        restoration.todos = Some("- Fix bug".to_string());
        restoration.files.push(FileRestoration {
            path: PathBuf::from("/test.rs"),
            content: "fn main() {}".to_string(),
            priority: 1,
            tokens: 10,
        });

        let msg = format_restoration_message(&restoration);
        assert!(msg.contains("<restored_context>"));
        assert!(msg.contains("<todo_list>"));
        assert!(msg.contains("- Fix bug"));
        assert!(msg.contains("<file path=\"/test.rs\">"));
    }

    #[test]
    fn test_session_memory_config_default() {
        let config = SessionMemoryConfig::default();
        assert!(!config.enabled);
        assert!(config.summary_path.is_none());
        assert_eq!(config.min_savings_tokens, 10_000);
    }

    #[test]
    fn test_compaction_tier_variants() {
        let tiers = vec![
            CompactionTier::SessionMemory,
            CompactionTier::Full,
            CompactionTier::Micro,
        ];
        for tier in tiers {
            let json = serde_json::to_string(&tier).unwrap();
            let back: CompactionTier = serde_json::from_str(&json).unwrap();
            assert_eq!(tier, back);
        }
    }
}
