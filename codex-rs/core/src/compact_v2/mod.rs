//! Compact System V2 - Two-tier compaction with micro-compact and full compact.
//!
//! This module provides the new V2 compact implementation, designed to work
//! alongside the legacy `compact.rs`. The V2 system is toggled via `Feature::CompactV2`.
//!
//! ## Architecture
//!
//! - **Legacy** (`compact.rs`): Original compact implementation (default)
//! - **V2** (`compact_v2/`): New two-tier architecture with micro-compact → full compact
//!
//! ## Feature Flags
//!
//! - `CompactV2`: Enable new compact implementation
//! - `MicroCompact`: Enable micro-compact tier (requires CompactV2)

// V2 compact modules
mod config;
mod strategy;
mod threshold;
mod token_counter;

mod boundary;
mod message_filter;
mod summary;

mod micro_compact;

mod prompt;

mod context_restore;
mod dispatch;
mod full_compact;

// Re-export V2 types
pub use config::CompactConfig;
pub use strategy::CompactStrategy;
pub use threshold::ThresholdState;
pub use threshold::calculate_thresholds;
pub use threshold::get_auto_compact_threshold;
pub use token_counter::TokenCounter;

pub use boundary::CompactBoundary;
pub use boundary::CompactTrigger;
pub use message_filter::collect_user_message_texts;
pub use message_filter::filter_for_summarization;
pub use message_filter::is_summary_message_v2;
pub use message_filter::is_synthetic_error_message;
pub use message_filter::is_thinking_only_block;
pub use summary::SUMMARY_PREFIX_V2;
pub use summary::cleanup_summary_tags;
pub use summary::create_summary_message;
pub use summary::format_summary_content;
pub use summary::is_valid_summary;

pub use micro_compact::ELIGIBLE_TOOLS;
pub use micro_compact::MicroCompactConfig;
pub use micro_compact::MicroCompactResult;
pub use micro_compact::try_micro_compact;

pub use prompt::SUMMARIZATION_SYSTEM_PROMPT;
pub use prompt::generate_summarization_prompt;

pub use context_restore::FileAttachment;
pub use context_restore::PlanAttachment;
pub use context_restore::RestoredContext;
pub use context_restore::TodoAttachment;
pub use context_restore::format_restored_context;
pub use context_restore::is_agent_file;
pub use context_restore::restore_context;

pub(crate) use dispatch::auto_compact_dispatch;
pub(crate) use dispatch::manual_compact_dispatch;

use std::collections::HashMap;
use std::collections::HashSet;

/// Session-level compact state for idempotent operations.
///
/// Tracks which tool results have been compacted to prevent re-compression
/// and caches token counts for efficiency.
#[derive(Debug, Clone, Default)]
pub struct CompactState {
    /// Tool_use IDs that have already been compacted (prevents re-compression)
    /// Equivalent to Claude Code's DQ0 (ALREADY_COMPACTED) set
    pub compacted_tool_ids: HashSet<String>,

    /// Token count cache per tool_use_id (avoids recalculation)
    /// Equivalent to Claude Code's pI2 Map
    pub tool_token_cache: HashMap<String, i64>,
}

/// Read file state entry for context restoration.
#[derive(Debug, Clone)]
pub struct ReadFileEntry {
    /// File path
    pub filename: String,
    /// Unix timestamp of last read (for sorting by recency)
    pub timestamp: i64,
    /// Cached token count (approximate)
    pub token_count: i64,
}

/// Result of a compact operation.
#[derive(Debug, Clone)]
pub enum CompactResult {
    /// Compaction disabled or not triggered
    Skipped,
    /// Token count below threshold, no compaction needed
    NotNeeded,
    /// Micro-compact succeeded (fast, no API)
    MicroCompacted(MicroCompactResult),
    /// Full compact succeeded (LLM summarization)
    FullCompacted(CompactMetrics),
    /// Remote compact delegated to compact_remote.rs
    RemoteCompacted,
}

/// Metrics from a full compact operation.
#[derive(Debug, Clone, Default)]
pub struct CompactMetrics {
    /// Token count before compaction
    pub pre_compact_tokens: i64,
    /// Token count after compaction
    pub post_compact_tokens: i64,
    /// Tokens used for summarization input
    pub compaction_input_tokens: i64,
    /// Tokens generated in summary
    pub compaction_output_tokens: i64,
    /// Strategy used for compaction
    pub strategy_used: CompactStrategy,
    /// Number of files restored
    pub files_restored: i32,
    /// Duration in milliseconds
    pub duration_ms: i64,
}

/// Try to run auto-compact using the V2 system.
///
/// Returns `true` if CompactV2 is enabled and handled the auto-compact request.
/// Returns `false` if CompactV2 is not enabled (caller should use legacy compact).
///
/// This function encapsulates the Feature::CompactV2 check to minimize changes
/// in codex.rs during upstream syncs.
pub(crate) async fn try_auto_compact(
    sess: std::sync::Arc<crate::codex::Session>,
    turn_context: std::sync::Arc<crate::codex::TurnContext>,
) -> bool {
    if !sess.enabled(crate::features::Feature::CompactV2) {
        return false;
    }
    auto_compact_dispatch(sess, turn_context).await;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn compact_state_defaults_to_empty() {
        let state = CompactState::default();
        assert!(state.compacted_tool_ids.is_empty());
        assert!(state.tool_token_cache.is_empty());
    }

    #[test]
    fn compact_result_variants() {
        let skipped = CompactResult::Skipped;
        assert!(matches!(skipped, CompactResult::Skipped));

        let not_needed = CompactResult::NotNeeded;
        assert!(matches!(not_needed, CompactResult::NotNeeded));

        let remote = CompactResult::RemoteCompacted;
        assert!(matches!(remote, CompactResult::RemoteCompacted));
    }

    // --- Integration Tests ---

    #[test]
    fn config_validation_rejects_invalid_pct_override() {
        let mut config = CompactConfig::default();
        config.auto_compact_pct_override = Some(150); // > 100 is invalid
        assert!(config.validate().is_err());

        config.auto_compact_pct_override = Some(-10); // < 0 is invalid
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_accepts_valid_config() {
        let config = CompactConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_validation_rejects_zero_bytes_per_token() {
        let mut config = CompactConfig::default();
        config.approx_bytes_per_token = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn micro_compact_config_derives_from_compact_config() {
        let compact_config = CompactConfig {
            micro_compact_min_tokens_to_save: 15000,
            micro_compact_keep_last_n_tools: 5,
            tokens_per_image: 3000,
            ..Default::default()
        };
        let micro_config = MicroCompactConfig::from(&compact_config);
        assert_eq!(micro_config.min_tokens_to_save, 15000);
        assert_eq!(micro_config.keep_last_n_tools, 5);
        assert_eq!(micro_config.tokens_per_image, 3000);
    }

    #[test]
    fn threshold_calculation_with_pct_override() {
        let mut config = CompactConfig::default();
        config.auto_compact_pct_override = Some(80);
        // 80% of 200k = 160k threshold
        let state = calculate_thresholds(165_000, 200_000, &config);
        assert!(state.is_above_auto_compact);
    }

    #[test]
    fn threshold_not_reached_below_limit() {
        let config = CompactConfig::default();
        let state = calculate_thresholds(50_000, 200_000, &config);
        assert!(!state.is_above_auto_compact);
        assert!(!state.is_above_warning);
    }

    #[test]
    fn threshold_triggers_auto_compact() {
        let config = CompactConfig::default();
        // Default threshold is context_limit - free_space_buffer = 200k - 13k = 187k
        let state = calculate_thresholds(190_000, 200_000, &config);
        assert!(state.is_above_auto_compact);
    }

    #[test]
    fn message_filter_full_flow() {
        let items = vec![
            // Regular user message - should be kept
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Hello".to_string(),
                }],
            },
            // Regular assistant message - should be kept
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "Hi there".to_string(),
                }],
            },
            // Ghost snapshot - should be filtered
            ResponseItem::GhostSnapshot {
                ghost_commit: codex_git::GhostCommit::new("abc".to_string(), None, vec![], vec![]),
            },
            // Previous summary - should be filtered
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{}\nPrevious summary content", SUMMARY_PREFIX_V2),
                }],
            },
            // Another user message - should be kept
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Follow-up question".to_string(),
                }],
            },
        ];

        let filtered = filter_for_summarization(&items);
        // Should have: user "Hello" + assistant "Hi there" (merged) + user "Follow-up"
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn summary_message_creation_includes_prefix() {
        let summary = "User discussed Rust ownership. Key topics covered.";
        let msg = create_summary_message(summary, false);

        if let ResponseItem::Message { id, role, content } = msg {
            assert_eq!(id, Some("compact_summary".to_string()));
            assert_eq!(role, "user");
            assert_eq!(content.len(), 1);
            if let ContentItem::InputText { text } = &content[0] {
                assert!(text.starts_with(SUMMARY_PREFIX_V2));
                assert!(text.contains("The conversation is summarized below:"));
                assert!(text.contains(summary));
            } else {
                panic!("Expected InputText");
            }
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn summary_message_with_continue_instruction() {
        let summary = "Working on feature X";
        let msg = create_summary_message(summary, true);

        if let ResponseItem::Message { content, .. } = msg {
            if let ContentItem::InputText { text } = &content[0] {
                assert!(text.contains("Continue with the last task"));
                assert!(text.contains("without asking the user any further questions"));
            } else {
                panic!("Expected InputText");
            }
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn boundary_marker_creation_and_detection() {
        let boundary = CompactBoundary::create(CompactTrigger::Auto, 150_000);

        // Verify it's detected as a boundary
        assert!(CompactBoundary::is_boundary_marker(&boundary));

        // Verify regular messages are not boundaries
        let regular = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Regular message".to_string(),
            }],
        };
        assert!(!CompactBoundary::is_boundary_marker(&regular));
    }

    #[test]
    fn context_restoration_formats_files() {
        let context = RestoredContext {
            files: vec![FileAttachment {
                filename: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
                token_count: 10,
            }],
            todos: None,
            plan: None,
        };

        let formatted = format_restored_context(&context);
        assert_eq!(formatted.len(), 1);
        assert!(formatted[0].contains("Restored file: src/main.rs"));
        assert!(formatted[0].contains("fn main() {}"));
    }

    #[test]
    fn context_restoration_formats_todos() {
        let context = RestoredContext {
            files: vec![],
            todos: Some(TodoAttachment {
                content: r#"[{"content": "Fix bug", "status": "pending"}]"#.to_string(),
            }),
            plan: None,
        };

        let formatted = format_restored_context(&context);
        assert_eq!(formatted.len(), 1);
        assert!(formatted[0].contains("Restored todo list"));
        assert!(formatted[0].contains("Fix bug"));
    }

    #[test]
    fn context_restoration_formats_plan() {
        let context = RestoredContext {
            files: vec![],
            todos: None,
            plan: Some(PlanAttachment {
                filename: "plan.md".to_string(),
                content: "# Implementation Plan\n\n- Step 1\n- Step 2".to_string(),
            }),
        };

        let formatted = format_restored_context(&context);
        assert_eq!(formatted.len(), 1);
        assert!(formatted[0].contains("Restored plan file: plan.md"));
        assert!(formatted[0].contains("Step 1"));
    }

    #[test]
    fn agent_file_detection() {
        // Plan files should be detected
        assert!(is_agent_file(".claude/plans/my-plan.md", "agent-123"));
        assert!(is_agent_file(
            "/home/user/.claude/plans/test.md",
            "agent-123"
        ));

        // Agent-specific files should be detected
        assert!(is_agent_file(
            ".claude/agents/agent-123/state.json",
            "agent-123"
        ));

        // Other agent's files should NOT be detected
        assert!(!is_agent_file(
            ".claude/agents/other-agent/state.json",
            "agent-123"
        ));

        // Regular files should NOT be detected
        assert!(!is_agent_file("src/main.rs", "agent-123"));
        assert!(!is_agent_file("README.md", "agent-123"));
    }

    #[test]
    fn token_counter_with_safety_margin() {
        let counter = TokenCounter::default();
        let text = "Hello world! This is a test."; // ~7 words
        let count = counter.approximate(text);

        // Should apply safety margin (1.33x default)
        // Rough estimate: ~28 chars / 4 bytes per token = ~7 tokens
        // With 1.33x margin: ~9 tokens
        assert!(count > 0);
        assert!(count < 100); // Sanity check
    }

    #[test]
    fn compact_metrics_default_values() {
        let metrics = CompactMetrics::default();
        assert_eq!(metrics.pre_compact_tokens, 0);
        assert_eq!(metrics.post_compact_tokens, 0);
        assert_eq!(metrics.compaction_input_tokens, 0);
        assert_eq!(metrics.compaction_output_tokens, 0);
        assert_eq!(metrics.files_restored, 0);
        assert_eq!(metrics.duration_ms, 0);
    }

    #[test]
    fn compact_state_tracks_compacted_tools() {
        let mut state = CompactState::default();

        // Add some compacted tool IDs
        state.compacted_tool_ids.insert("tool-1".to_string());
        state.compacted_tool_ids.insert("tool-2".to_string());

        assert!(state.compacted_tool_ids.contains("tool-1"));
        assert!(state.compacted_tool_ids.contains("tool-2"));
        assert!(!state.compacted_tool_ids.contains("tool-3"));
    }

    #[test]
    fn compact_state_caches_token_counts() {
        let mut state = CompactState::default();

        state.tool_token_cache.insert("tool-1".to_string(), 1500);
        state.tool_token_cache.insert("tool-2".to_string(), 2500);

        assert_eq!(state.tool_token_cache.get("tool-1"), Some(&1500));
        assert_eq!(state.tool_token_cache.get("tool-2"), Some(&2500));
        assert_eq!(state.tool_token_cache.get("tool-3"), None);
    }

    #[test]
    fn read_file_entry_structure() {
        let entry = ReadFileEntry {
            filename: "/path/to/file.rs".to_string(),
            timestamp: 1700000000,
            token_count: 500,
        };

        assert_eq!(entry.filename, "/path/to/file.rs");
        assert_eq!(entry.timestamp, 1700000000);
        assert_eq!(entry.token_count, 500);
    }

    #[test]
    fn summarization_prompt_generation() {
        let prompt = generate_summarization_prompt(None);

        // Should contain key instructions
        assert!(prompt.contains("summarize") || prompt.contains("summary"));
    }

    #[test]
    fn cleanup_summary_tags_transforms_analysis() {
        let raw = "<analysis>This is analysis content.</analysis>";
        let cleaned = cleanup_summary_tags(raw);
        assert!(cleaned.contains("Analysis:"));
        assert!(cleaned.contains("This is analysis content."));
        assert!(!cleaned.contains("<analysis>"));
    }

    #[test]
    fn cleanup_summary_tags_transforms_summary() {
        let raw = "<summary>This is summary content.</summary>";
        let cleaned = cleanup_summary_tags(raw);
        assert!(cleaned.contains("Summary:"));
        assert!(cleaned.contains("This is summary content."));
        assert!(!cleaned.contains("<summary>"));
    }

    #[test]
    fn is_valid_summary_rejects_short_content() {
        assert!(!is_valid_summary("Too short"));
        assert!(!is_valid_summary(
            "This is still too short for a meaningful summary."
        ));
    }

    #[test]
    fn is_valid_summary_rejects_errors() {
        assert!(!is_valid_summary("API_ERROR: rate limited"));
        assert!(!is_valid_summary("Error: something went wrong"));
    }

    #[test]
    fn is_valid_summary_accepts_long_content() {
        let valid = "This is a comprehensive summary of the conversation that includes enough context to be meaningful. The user was working on implementing a new feature and we discussed various aspects of the implementation. The conversation covered error handling, testing strategies, and best practices for Rust development.";
        assert!(is_valid_summary(valid));
    }
}
