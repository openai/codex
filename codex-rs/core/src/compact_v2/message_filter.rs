//! Message filtering for compaction.
//!
//! Provides predicates and filters for selecting which messages
//! should be included in LLM summarization.

use super::boundary::CompactBoundary;
use super::summary::SUMMARY_PREFIX_V2;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

/// Check if message is a previous summary (from compact).
///
/// V2 version with updated prefix detection.
pub fn is_summary_message_v2(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { content, role, .. } if role == "user" => content.iter().any(|c| {
            if let ContentItem::InputText { text } = c {
                text.starts_with(SUMMARY_PREFIX_V2)
            } else {
                false
            }
        }),
        _ => false,
    }
}

/// Check if assistant message contains only thinking blocks (no output).
///
/// Matches Claude Code's NQ0 / isThinkingOnlyBlock function.
pub fn is_thinking_only_block(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Reasoning { content, .. } => {
            // Reasoning items are thinking blocks
            content.is_none() || content.as_ref().map(|c| c.is_empty()).unwrap_or(true)
        }
        ResponseItem::Message { role, id, .. } if role == "assistant" => {
            // Only filter if ID explicitly indicates thinking/reasoning
            // Don't filter empty assistant messages - they may be legitimate (e.g., acknowledgements)
            if let Some(id) = id {
                return id.contains("thinking") || id.contains("reasoning");
            }
            false
        }
        _ => false,
    }
}

/// Check if message is a synthetic error placeholder.
///
/// Matches Claude Code's wb3 / isSyntheticErrorMessage function.
pub fn is_synthetic_error_message(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { id, role, .. } if role == "assistant" => {
            if let Some(id) = id {
                id.contains("synthetic") || id.contains("<synthetic>")
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Check if message is a progress/status message that should be filtered.
pub fn is_progress_message(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { id, content, .. } => {
            // Check ID for progress markers
            if let Some(id) = id {
                if id.contains("progress") || id.contains("status") {
                    return true;
                }
            }
            // Check content for progress patterns
            content.iter().any(|c| {
                if let ContentItem::InputText { text } = c {
                    text.starts_with("[Progress:") || text.starts_with("[Status:")
                } else {
                    false
                }
            })
        }
        _ => false,
    }
}

/// Check if this is a system message (not a boundary).
pub fn is_system_message(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, .. } if role == "system" => {
            !CompactBoundary::is_boundary_marker(item)
        }
        _ => false,
    }
}

/// Check if message is a system reminder.
///
/// Used to filter system reminders during compaction.
pub fn is_system_reminder_message(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { content, role, .. } if role == "user" => content.iter().any(|c| {
            if let ContentItem::InputText { text } = c {
                text.starts_with("<system-reminder>")
                    || text.starts_with("<system-notification>")
                    || text.starts_with("<session-memory>")
                    || text.starts_with("<new-diagnostics>")
            } else {
                false
            }
        }),
        _ => false,
    }
}

/// Merge consecutive user messages into single messages.
///
/// Matches Claude Code's message merging in WZ() function.
fn merge_consecutive_user_messages(items: Vec<ResponseItem>) -> Vec<ResponseItem> {
    let mut result: Vec<ResponseItem> = Vec::new();

    for item in items {
        match (&item, result.last_mut()) {
            // Merge consecutive user messages
            (
                ResponseItem::Message {
                    role: r1,
                    content: c1,
                    ..
                },
                Some(ResponseItem::Message {
                    role: r2,
                    content: c2,
                    ..
                }),
            ) if r1 == "user" && r2 == "user" => {
                c2.extend(c1.clone());
            }
            // Otherwise, add as new item
            _ => {
                result.push(item);
            }
        }
    }

    result
}

/// Filter messages for LLM summarization.
///
/// Matches Claude Code's WZ / filterAndNormalizeMessages function.
///
/// Filters out:
/// - Progress messages
/// - System messages (except boundaries)
/// - Synthetic error messages
/// - Thinking-only blocks
/// - Previous summary messages
///
/// Then merges consecutive user messages.
pub fn filter_for_summarization(items: &[ResponseItem]) -> Vec<ResponseItem> {
    // Step 1: Filter out unwanted message types
    let filtered: Vec<ResponseItem> = items
        .iter()
        .filter(|item| {
            // Exclude progress messages
            if is_progress_message(item) {
                return false;
            }
            // Exclude system messages (except boundaries)
            if is_system_message(item) {
                return false;
            }
            // Exclude synthetic errors
            if is_synthetic_error_message(item) {
                return false;
            }
            // Exclude thinking-only blocks
            if is_thinking_only_block(item) {
                return false;
            }
            // Exclude previous summaries
            if is_summary_message_v2(item) {
                return false;
            }
            // Exclude system reminders
            if is_system_reminder_message(item) {
                return false;
            }
            // Exclude ghost snapshots (internal use)
            if matches!(item, ResponseItem::GhostSnapshot { .. }) {
                return false;
            }
            // Exclude compaction items (encrypted content)
            if matches!(item, ResponseItem::Compaction { .. }) {
                return false;
            }
            // Exclude other unknown items
            if matches!(item, ResponseItem::Other) {
                return false;
            }
            true
        })
        .cloned()
        .collect();

    // Step 2: Merge consecutive user messages
    merge_consecutive_user_messages(filtered)
}

/// Collect user message texts from history.
///
/// Filters out summary messages and system prefixes.
/// Used for preserving user context in compacted history.
pub fn collect_user_message_texts(items: &[ResponseItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                let text = content
                    .iter()
                    .filter_map(|c| {
                        if let ContentItem::InputText { text } = c {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                // Filter out summary messages
                if text.starts_with(SUMMARY_PREFIX_V2) {
                    return None;
                }
                // Filter out system prefixes (AGENTS.md, environment context)
                if text.starts_with("# AGENTS.md")
                    || text.starts_with("<ENVIRONMENT_CONTEXT>")
                    || text.starts_with("<INSTRUCTIONS>")
                {
                    return None;
                }

                if text.is_empty() { None } else { Some(text) }
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn user_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
        }
    }

    fn assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
    }

    #[test]
    fn is_summary_message_detects_v2_prefix() {
        let summary = user_msg(&format!(
            "{}\nThe conversation is summarized below:\n...",
            SUMMARY_PREFIX_V2
        ));
        assert!(is_summary_message_v2(&summary));

        let regular = user_msg("Hello, how are you?");
        assert!(!is_summary_message_v2(&regular));
    }

    #[test]
    fn is_thinking_only_block_detects_reasoning() {
        let reasoning = ResponseItem::Reasoning {
            id: "r1".to_string(),
            summary: vec![],
            content: None,
            encrypted_content: None,
        };
        assert!(is_thinking_only_block(&reasoning));
    }

    #[test]
    fn is_synthetic_error_detects_synthetic_id() {
        let synthetic = ResponseItem::Message {
            id: Some("<synthetic>_error".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Error".to_string(),
            }],
        };
        assert!(is_synthetic_error_message(&synthetic));

        let normal = assistant_msg("Hello");
        assert!(!is_synthetic_error_message(&normal));
    }

    #[test]
    fn filter_for_summarization_excludes_unwanted() {
        let items = vec![
            user_msg("Hello"),
            ResponseItem::GhostSnapshot {
                ghost_commit: codex_git::GhostCommit::new("abc".to_string(), None, vec![], vec![]),
            },
            assistant_msg("Hi there"),
            ResponseItem::Other,
        ];

        let filtered = filter_for_summarization(&items);
        assert_eq!(filtered.len(), 2); // Only user + assistant messages
    }

    #[test]
    fn merge_consecutive_user_messages() {
        let items = vec![
            user_msg("First"),
            user_msg("Second"),
            assistant_msg("Response"),
            user_msg("Third"),
        ];

        let filtered = filter_for_summarization(&items);
        assert_eq!(filtered.len(), 3); // Merged first two + assistant + third
    }

    #[test]
    fn collect_user_message_texts_filters_system() {
        let items = vec![
            user_msg("# AGENTS.md instructions"),
            user_msg("<ENVIRONMENT_CONTEXT>cwd=/tmp</ENVIRONMENT_CONTEXT>"),
            user_msg("Real user question"),
        ];

        let texts = collect_user_message_texts(&items);
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "Real user question");
    }

    #[test]
    fn collect_user_message_texts_filters_summaries() {
        let items = vec![
            user_msg(&format!("{}\nSome summary...", SUMMARY_PREFIX_V2)),
            user_msg("Real message"),
        ];

        let texts = collect_user_message_texts(&items);
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "Real message");
    }
}
