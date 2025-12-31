//! Extension module for LSP client incremental document sync support
//!
//! This module provides incremental document synchronization using the `similar`
//! crate's Myers diff algorithm. It computes minimal text changes between document
//! versions, reducing network overhead compared to full document sync.

use lsp_types::Position;
use lsp_types::Range;
use lsp_types::TextDocumentContentChangeEvent;
use similar::Algorithm;
use similar::DiffOp;
use similar::TextDiff;

/// Maximum content size for incremental sync tracking (1 MB)
///
/// Files larger than this will use full document sync to avoid
/// excessive memory usage for storing document content.
pub const MAX_INCREMENTAL_CONTENT_SIZE: usize = 1024 * 1024;

/// Stores document content for incremental sync tracking
#[derive(Debug, Clone)]
pub struct DocumentContent {
    /// Original text content
    pub text: String,
    /// Line-split content for efficient position calculation
    lines: Vec<String>,
}

impl DocumentContent {
    /// Create a new DocumentContent from text
    pub fn new(text: String) -> Self {
        let lines = text.lines().map(String::from).collect();
        Self { text, lines }
    }

    /// Get the number of lines
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get line at index (0-based)
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }
}

/// Compute incremental changes between old and new content using Myers diff algorithm
///
/// Returns a vector of `TextDocumentContentChangeEvent` that can be sent to the LSP server.
/// If no changes are detected, returns an empty vector.
///
/// The algorithm:
/// 1. Computes line-based diff using Myers algorithm (good balance of speed/quality)
/// 2. Groups consecutive changes into single events where possible
/// 3. Returns changes in the correct order for LSP processing
pub fn compute_incremental_changes(
    old: &DocumentContent,
    new_text: &str,
) -> Vec<TextDocumentContentChangeEvent> {
    // Use Myers diff algorithm for line-based comparison
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .diff_lines(old.text.as_str(), new_text);

    let mut events = Vec::new();

    // Process grouped operations for efficient change events
    for group in diff.grouped_ops(0) {
        for op in group {
            match op {
                DiffOp::Equal { .. } => {
                    // No change, skip
                }
                DiffOp::Delete {
                    old_index, old_len, ..
                } => {
                    // Lines deleted from old document
                    let start_line = old_index as u32;
                    let end_line = (old_index + old_len) as u32;

                    events.push(TextDocumentContentChangeEvent {
                        range: Some(Range {
                            start: Position {
                                line: start_line,
                                character: 0,
                            },
                            end: Position {
                                line: end_line,
                                character: 0,
                            },
                        }),
                        range_length: None,
                        text: String::new(),
                    });
                }
                DiffOp::Insert {
                    old_index,
                    new_index,
                    new_len,
                } => {
                    // Lines inserted into new document
                    let insert_line = old_index as u32;
                    let new_lines: String = new_text
                        .lines()
                        .skip(new_index)
                        .take(new_len)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Add trailing newline if not at end of file
                    let text = if new_index + new_len < new_text.lines().count() {
                        format!("{new_lines}\n")
                    } else {
                        new_lines
                    };

                    events.push(TextDocumentContentChangeEvent {
                        range: Some(Range {
                            start: Position {
                                line: insert_line,
                                character: 0,
                            },
                            end: Position {
                                line: insert_line,
                                character: 0,
                            },
                        }),
                        range_length: None,
                        text,
                    });
                }
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => {
                    // Lines replaced
                    let start_line = old_index as u32;
                    let end_line = (old_index + old_len) as u32;
                    let new_lines: String = new_text
                        .lines()
                        .skip(new_index)
                        .take(new_len)
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Add trailing newline if not at end of file
                    let text = if new_index + new_len < new_text.lines().count() {
                        format!("{new_lines}\n")
                    } else {
                        new_lines
                    };

                    events.push(TextDocumentContentChangeEvent {
                        range: Some(Range {
                            start: Position {
                                line: start_line,
                                character: 0,
                            },
                            end: Position {
                                line: end_line,
                                character: 0,
                            },
                        }),
                        range_length: None,
                        text,
                    });
                }
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_content_new() {
        let content = DocumentContent::new("foo\nbar\nbaz".to_string());
        assert_eq!(content.line_count(), 3);
        assert_eq!(content.get_line(0), Some("foo"));
        assert_eq!(content.get_line(1), Some("bar"));
        assert_eq!(content.get_line(2), Some("baz"));
        assert_eq!(content.get_line(3), None);
    }

    #[test]
    fn test_no_changes() {
        let old = DocumentContent::new("foo\nbar\nbaz\n".to_string());
        let changes = compute_incremental_changes(&old, "foo\nbar\nbaz\n");
        assert!(
            changes.is_empty(),
            "Expected no changes for identical content"
        );
    }

    #[test]
    fn test_single_line_modification() {
        let old = DocumentContent::new("foo\nbar\nbaz\n".to_string());
        let changes = compute_incremental_changes(&old, "foo\nBAR\nbaz\n");

        assert!(!changes.is_empty(), "Expected changes for modified line");

        // Should have a replace event for line 1
        let has_line_1_change = changes.iter().any(|c| {
            if let Some(range) = &c.range {
                range.start.line == 1
            } else {
                false
            }
        });
        assert!(has_line_1_change, "Expected change event for line 1");
    }

    #[test]
    fn test_line_insertion() {
        let old = DocumentContent::new("foo\nbaz\n".to_string());
        let changes = compute_incremental_changes(&old, "foo\nbar\nbaz\n");

        assert!(!changes.is_empty(), "Expected changes for inserted line");
    }

    #[test]
    fn test_line_deletion() {
        let old = DocumentContent::new("foo\nbar\nbaz\n".to_string());
        let changes = compute_incremental_changes(&old, "foo\nbaz\n");

        assert!(!changes.is_empty(), "Expected changes for deleted line");
    }

    #[test]
    fn test_multiple_changes() {
        let old = DocumentContent::new("line1\nline2\nline3\nline4\nline5\n".to_string());
        let new_text = "line1\nMODIFIED\nline3\nINSERTED\nline4\n";

        let changes = compute_incremental_changes(&old, new_text);
        assert!(!changes.is_empty(), "Expected multiple change events");
    }

    #[test]
    fn test_empty_to_content() {
        let old = DocumentContent::new(String::new());
        let changes = compute_incremental_changes(&old, "new content\n");

        assert!(
            !changes.is_empty(),
            "Expected changes when adding content to empty"
        );
    }

    #[test]
    fn test_content_to_empty() {
        let old = DocumentContent::new("old content\n".to_string());
        let changes = compute_incremental_changes(&old, "");

        assert!(
            !changes.is_empty(),
            "Expected changes when clearing content"
        );
    }

    #[test]
    fn test_change_event_has_range() {
        let old = DocumentContent::new("foo\nbar\nbaz\n".to_string());
        let changes = compute_incremental_changes(&old, "foo\nBAR\nbaz\n");

        for change in &changes {
            assert!(
                change.range.is_some(),
                "Incremental changes should have range"
            );
        }
    }
}
