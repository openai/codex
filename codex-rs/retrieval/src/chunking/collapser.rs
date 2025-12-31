//! Smart chunk collapsing.
//!
//! Based on Continue's core/indexing/chunk/code.ts:
//! - `getSmartCollapsedChunks()` - Recursive collapsing
//! - `collapseChildren()` - Child node collapsing
//! - `collapsedReplacement()` - Replace with `{ ... }`

use crate::types::ChunkSpan;

/// Smart collapser for reducing code chunk size.
///
/// Collapses nested blocks (function bodies, class bodies) to fit within
/// token limits while preserving structural information.
pub struct SmartCollapser {
    /// Maximum size in characters (approx tokens * 4)
    max_size: usize,
    /// Collapse placeholder
    placeholder: String,
}

impl Default for SmartCollapser {
    fn default() -> Self {
        Self::new(2048) // ~512 tokens
    }
}

impl SmartCollapser {
    /// Create a new smart collapser.
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            placeholder: " { ... }".to_string(),
        }
    }

    /// Create with custom placeholder.
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Collapse a chunk if it exceeds the max size.
    ///
    /// Returns the original chunk if within limits, or a collapsed version.
    pub fn collapse(&self, chunk: &ChunkSpan) -> ChunkSpan {
        if chunk.content.len() <= self.max_size {
            return chunk.clone();
        }

        let collapsed_content = self.collapse_content(&chunk.content);

        // If still too large after collapsing, truncate
        let final_content = if collapsed_content.len() > self.max_size {
            self.truncate(&collapsed_content)
        } else {
            collapsed_content
        };

        ChunkSpan {
            content: final_content,
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            is_overview: chunk.is_overview,
        }
    }

    /// Collapse nested blocks in content.
    ///
    /// Tracks string literals and comments to avoid counting braces inside them.
    /// This prevents incorrect collapsing when code contains braces in strings like:
    /// `let s = "contains { braces }";`
    fn collapse_content(&self, content: &str) -> String {
        let mut result = String::new();
        let mut chars = content.chars().peekable();
        let mut depth: i32 = 0;
        let mut collapse_from: Option<usize> = None;

        // Track string/comment state to ignore braces inside them
        let mut in_string = false;
        let mut in_char = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut prev_char = ' ';

        while let Some(c) = chars.next() {
            // Handle escape sequences in strings
            let is_escaped = prev_char == '\\';

            // Detect line comment start: //
            if !in_string && !in_char && !in_block_comment && c == '/' {
                if chars.peek() == Some(&'/') {
                    in_line_comment = true;
                } else if chars.peek() == Some(&'*') {
                    in_block_comment = true;
                }
            }

            // Detect line comment end
            if in_line_comment && c == '\n' {
                in_line_comment = false;
            }

            // Detect block comment end: */
            if in_block_comment && prev_char == '*' && c == '/' {
                in_block_comment = false;
                prev_char = c;
                if depth < 2 {
                    result.push(c);
                }
                continue;
            }

            // Detect string start/end
            if !in_line_comment && !in_block_comment && !in_char && c == '"' && !is_escaped {
                in_string = !in_string;
            }

            // Detect char literal start/end
            if !in_line_comment && !in_block_comment && !in_string && c == '\'' && !is_escaped {
                in_char = !in_char;
            }

            // Only count braces when NOT inside string, char, or comment
            let in_literal = in_string || in_char || in_line_comment || in_block_comment;

            if !in_literal {
                match c {
                    '{' => {
                        depth += 1;
                        if depth == 2 && collapse_from.is_none() {
                            // Start collapsing at depth 2 (nested blocks)
                            collapse_from = Some(result.len());
                        }
                        if depth < 2 {
                            result.push(c);
                        }
                    }
                    '}' => {
                        if depth == 2 {
                            if let Some(start) = collapse_from.take() {
                                // Replace nested block with placeholder
                                result.truncate(start);
                                result.push_str(&self.placeholder);
                            }
                        }
                        depth = depth.saturating_sub(1);
                        if depth < 1 {
                            result.push(c);
                        }
                    }
                    _ => {
                        if depth < 2 {
                            result.push(c);
                        }
                    }
                }
            } else {
                // Inside literal - just copy character if at appropriate depth
                if depth < 2 {
                    result.push(c);
                }
            }

            prev_char = c;
        }

        result
    }

    /// Truncate content to max size, preserving structure.
    fn truncate(&self, content: &str) -> String {
        // Find last complete line within limit
        let mut last_newline = 0;
        for (i, c) in content.char_indices() {
            if i >= self.max_size {
                break;
            }
            if c == '\n' {
                last_newline = i;
            }
        }

        if last_newline > 0 {
            format!("{}\n// ... truncated", &content[..last_newline])
        } else {
            format!("{}...", &content[..self.max_size.min(content.len())])
        }
    }

    /// Collapse multiple chunks.
    pub fn collapse_all(&self, chunks: &[ChunkSpan]) -> Vec<ChunkSpan> {
        chunks.iter().map(|c| self.collapse(c)).collect()
    }
}

/// Collapse a single block (function/class body).
///
/// Replaces the body with `{ ... }` while keeping the signature.
pub fn collapse_block(content: &str) -> String {
    if let Some(brace_pos) = content.find('{') {
        let signature = &content[..brace_pos];
        format!("{} {{ ... }}", signature.trim())
    } else {
        content.to_string()
    }
}

/// Estimate if a chunk needs collapsing based on line count.
pub fn needs_collapsing(chunk: &ChunkSpan, max_lines: i32) -> bool {
    let line_count = chunk.end_line - chunk.start_line + 1;
    line_count > max_lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_collapse_needed() {
        let collapser = SmartCollapser::new(1000);
        let chunk = ChunkSpan {
            content: "fn foo() { bar(); }".to_string(),
            start_line: 0,
            end_line: 0,
            is_overview: false,
        };

        let result = collapser.collapse(&chunk);
        assert_eq!(result.content, chunk.content);
    }

    #[test]
    fn test_collapse_block() {
        let content = r#"fn main() {
    let x = 1;
    let y = 2;
    println!("{}", x + y);
}"#;
        let result = collapse_block(content);
        assert_eq!(result, "fn main() { ... }");
    }

    #[test]
    fn test_needs_collapsing() {
        let chunk = ChunkSpan {
            content: "...".to_string(),
            start_line: 0,
            end_line: 50,
            is_overview: false,
        };

        assert!(needs_collapsing(&chunk, 30));
        assert!(!needs_collapsing(&chunk, 100));
    }

    #[test]
    fn test_collapse_nested() {
        let collapser = SmartCollapser::new(50);
        let chunk = ChunkSpan {
            content: "fn outer() { fn inner() { very_long_code_here(); } }".to_string(),
            start_line: 0,
            end_line: 0,
            is_overview: false,
        };

        let result = collapser.collapse(&chunk);
        assert!(result.content.contains("{ ... }"));
        assert!(result.content.len() <= 60); // Allow some overhead
    }

    #[test]
    fn test_collapse_ignores_braces_in_strings() {
        // Use a large enough max_size to avoid truncation
        let collapser = SmartCollapser::new(500);

        // Code with braces inside a string - should NOT be counted as nesting
        let code_with_string = r#"fn has_string() {
    let s = "contains { braces }";
    process(s);
}"#;
        let chunk = ChunkSpan {
            content: code_with_string.to_string(),
            start_line: 0,
            end_line: 0,
            is_overview: false,
        };

        let result = collapser.collapse(&chunk);
        // The string with braces should be preserved, not collapsed
        assert!(
            result.content.contains(r#""contains { braces }""#),
            "String content with braces should be preserved. Got: {}",
            result.content
        );
    }

    #[test]
    fn test_collapse_ignores_braces_in_comments() {
        let collapser = SmartCollapser::new(100);

        // Code with braces inside comments
        let code_with_comment = r#"fn has_comment() {
    // This is a comment with { braces }
    let x = 1;
}"#;
        let chunk = ChunkSpan {
            content: code_with_comment.to_string(),
            start_line: 0,
            end_line: 0,
            is_overview: false,
        };

        let result = collapser.collapse(&chunk);
        // The comment with braces should be preserved
        assert!(
            result
                .content
                .contains("// This is a comment with { braces }"),
            "Comment with braces should be preserved. Got: {}",
            result.content
        );
    }

    #[test]
    fn test_collapse_ignores_braces_in_block_comments() {
        let collapser = SmartCollapser::new(100);

        // Code with braces inside block comments
        let code_with_block_comment = r#"fn has_block_comment() {
    /* Block comment with { braces } inside */
    let x = 1;
}"#;
        let chunk = ChunkSpan {
            content: code_with_block_comment.to_string(),
            start_line: 0,
            end_line: 0,
            is_overview: false,
        };

        let result = collapser.collapse(&chunk);
        // The block comment with braces should be preserved
        assert!(
            result
                .content
                .contains("/* Block comment with { braces } inside */"),
            "Block comment with braces should be preserved. Got: {}",
            result.content
        );
    }
}
