//! Compact boundary markers.
//!
//! Boundary markers track compaction points in conversation history,
//! enabling multi-round compression with clear history tracking.

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use serde::Deserialize;
use serde::Serialize;

/// Trigger type for compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactTrigger {
    /// Automatic compaction triggered by threshold
    Auto,
    /// Manual compaction triggered by user
    Manual,
}

impl std::fmt::Display for CompactTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompactTrigger::Auto => write!(f, "auto"),
            CompactTrigger::Manual => write!(f, "manual"),
        }
    }
}

/// Metadata attached to a compact boundary marker.
#[allow(dead_code)] // Metadata structure for compaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactMetadata {
    /// How the compaction was triggered
    pub trigger: String,
    /// Token count before compaction
    pub pre_tokens: i64,
}

/// Compact boundary marker for tracking compaction points.
///
/// Used to identify where in the conversation history compaction occurred,
/// enabling tweakcc summarization of only the new messages since the
/// last compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundary {
    /// How the compaction was triggered
    pub trigger: CompactTrigger,
    /// Token count before compaction
    pub pre_compact_tokens: i64,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Unique identifier
    pub uuid: String,
}

impl CompactBoundary {
    /// Create a new boundary marker.
    pub fn new(trigger: CompactTrigger, pre_tokens: i64) -> Self {
        Self {
            trigger,
            pre_compact_tokens: pre_tokens,
            timestamp: chrono::Utc::now().to_rfc3339(),
            uuid: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create a boundary marker as a ResponseItem message.
    ///
    /// Matches Claude Code's S91 function.
    ///
    /// The boundary is stored as a user message with a special marker text
    /// that can be detected during filtering.
    pub fn create(trigger: CompactTrigger, pre_tokens: i64) -> ResponseItem {
        let boundary = Self::new(trigger, pre_tokens);
        let marker_text = format!(
            "[Conversation compacted at {} ({}), pre-compact tokens: {}]",
            boundary.timestamp, boundary.trigger, boundary.pre_compact_tokens
        );

        ResponseItem::Message {
            id: Some(format!("compact_boundary_{}", boundary.uuid)),
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: marker_text }],
        }
    }

    /// Check if an item is a boundary marker.
    pub fn is_boundary_marker(item: &ResponseItem) -> bool {
        match item {
            ResponseItem::Message { id, content, .. } => {
                // Check by ID prefix
                if let Some(id) = id {
                    if id.starts_with("compact_boundary_") {
                        return true;
                    }
                }
                // Check by content marker
                content.iter().any(|c| {
                    if let ContentItem::InputText { text } = c {
                        text.starts_with("[Conversation compacted at ")
                    } else {
                        false
                    }
                })
            }
            _ => false,
        }
    }

    /// Find index of last boundary marker (inclusive for extraction).
    pub fn find_last_boundary_index(items: &[ResponseItem]) -> Option<usize> {
        items.iter().rposition(Self::is_boundary_marker)
    }

    /// Extract messages from last boundary onward (for re-summarization).
    ///
    /// First compaction returns all messages (no boundary found).
    pub fn extract_messages_after_boundary(items: &[ResponseItem]) -> Vec<ResponseItem> {
        match Self::find_last_boundary_index(items) {
            Some(idx) => items[idx..].to_vec(), // Include boundary
            None => items.to_vec(),             // No boundary = first compact
        }
    }

    /// Count the number of boundary markers in history.
    pub fn count_boundaries(items: &[ResponseItem]) -> i32 {
        items
            .iter()
            .filter(|item| Self::is_boundary_marker(item))
            .count() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn boundary_creation() {
        let boundary = CompactBoundary::new(CompactTrigger::Auto, 150_000);
        assert_eq!(boundary.trigger, CompactTrigger::Auto);
        assert_eq!(boundary.pre_compact_tokens, 150_000);
        assert!(!boundary.timestamp.is_empty());
        assert!(!boundary.uuid.is_empty());
    }

    #[test]
    fn boundary_as_response_item() {
        let item = CompactBoundary::create(CompactTrigger::Manual, 100_000);

        match &item {
            ResponseItem::Message { id, content, role } => {
                assert_eq!(role, "user");
                assert!(
                    id.as_ref()
                        .map(|i| i.starts_with("compact_boundary_"))
                        .unwrap_or(false)
                );
                assert_eq!(content.len(), 1);
                if let ContentItem::InputText { text } = &content[0] {
                    assert!(text.contains("[Conversation compacted at "));
                    assert!(text.contains("manual"));
                    assert!(text.contains("100000"));
                } else {
                    panic!("expected InputText");
                }
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn is_boundary_marker_by_id() {
        let item = ResponseItem::Message {
            id: Some("compact_boundary_abc123".to_string()),
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "any text".to_string(),
            }],
        };
        assert!(CompactBoundary::is_boundary_marker(&item));
    }

    #[test]
    fn is_boundary_marker_by_content() {
        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "[Conversation compacted at 2024-01-01T00:00:00Z (auto), pre-compact tokens: 100000]".to_string(),
            }],
        };
        assert!(CompactBoundary::is_boundary_marker(&item));
    }

    #[test]
    fn is_not_boundary_marker() {
        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Hello, how are you?".to_string(),
            }],
        };
        assert!(!CompactBoundary::is_boundary_marker(&item));
    }

    #[test]
    fn find_last_boundary() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "first".to_string(),
                }],
            },
            CompactBoundary::create(CompactTrigger::Auto, 50_000),
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "second".to_string(),
                }],
            },
            CompactBoundary::create(CompactTrigger::Auto, 100_000),
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "third".to_string(),
                }],
            },
        ];

        let idx = CompactBoundary::find_last_boundary_index(&items);
        assert_eq!(idx, Some(3));
    }

    #[test]
    fn extract_messages_after_boundary() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "old message".to_string(),
                }],
            },
            CompactBoundary::create(CompactTrigger::Auto, 100_000),
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "new message".to_string(),
                }],
            },
        ];

        let after = CompactBoundary::extract_messages_after_boundary(&items);
        assert_eq!(after.len(), 2); // boundary + new message
    }

    #[test]
    fn extract_all_when_no_boundary() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "first".to_string(),
                }],
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "second".to_string(),
                }],
            },
        ];

        let after = CompactBoundary::extract_messages_after_boundary(&items);
        assert_eq!(after.len(), 2); // All messages
    }

    #[test]
    fn count_boundaries() {
        let items = vec![
            CompactBoundary::create(CompactTrigger::Auto, 50_000),
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "message".to_string(),
                }],
            },
            CompactBoundary::create(CompactTrigger::Manual, 100_000),
        ];

        assert_eq!(CompactBoundary::count_boundaries(&items), 2);
    }

    #[test]
    fn trigger_display() {
        assert_eq!(CompactTrigger::Auto.to_string(), "auto");
        assert_eq!(CompactTrigger::Manual.to_string(), "manual");
    }
}
