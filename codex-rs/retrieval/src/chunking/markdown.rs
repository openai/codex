//! Markdown-aware chunking.
//!
//! Recursively splits markdown by header levels (h1 → h6),
//! preserving parent headers in child chunks for context.
//!
//! Reference: Continue's `core/indexing/chunk/markdown.ts`

use crate::types::ChunkSpan;

/// Markdown chunker that respects header structure.
pub struct MarkdownChunker {
    max_chunk_size: usize,
}

impl MarkdownChunker {
    /// Create a new markdown chunker.
    pub fn new(max_chunk_size: usize) -> Self {
        Self { max_chunk_size }
    }

    /// Chunk markdown content by header hierarchy.
    ///
    /// Returns chunks where each chunk includes its parent headers for context.
    pub fn chunk(&self, content: &str) -> Vec<ChunkSpan> {
        self.chunk_recursive(content, 1, 0)
    }

    fn chunk_recursive(&self, content: &str, h_level: usize, base_line: i32) -> Vec<ChunkSpan> {
        // If content is small enough, return as single chunk
        if content.len() <= self.max_chunk_size {
            let lines = content.lines().count() as i32;
            return vec![ChunkSpan {
                content: content.to_string(),
                start_line: base_line + 1,
                end_line: base_line + lines.max(1),
                is_overview: false,
            }];
        }

        // At h5+, fall back to simple line-based chunking
        if h_level > 4 {
            return self.chunk_by_lines(content, base_line);
        }

        // Split by current header level
        let header_prefix = "#".repeat(h_level + 1) + " ";
        let sections = self.split_by_header(content, &header_prefix);

        let mut chunks = Vec::new();

        for section in sections {
            // Recursively chunk each section
            let sub_chunks = self.chunk_recursive(
                &section.content,
                h_level + 1,
                base_line + section.start_line,
            );

            for mut chunk in sub_chunks {
                // Prepend section header to chunk if present
                if let Some(ref header) = section.header {
                    chunk.content = format!("{}\n{}", header, chunk.content);
                }
                chunks.push(chunk);
            }
        }

        // If no sections were created, chunk by lines
        if chunks.is_empty() {
            return self.chunk_by_lines(content, base_line);
        }

        chunks
    }

    fn split_by_header(&self, content: &str, header_prefix: &str) -> Vec<Section> {
        let lines: Vec<&str> = content.lines().collect();
        let mut sections = Vec::new();
        let mut current_lines = Vec::new();
        let mut current_header: Option<String> = None;
        let mut current_start = 0;

        for (i, line) in lines.iter().enumerate() {
            if line.starts_with(header_prefix) {
                // Save previous section
                if !current_lines.is_empty() || current_header.is_some() {
                    sections.push(Section {
                        header: current_header.take(),
                        content: current_lines.join("\n"),
                        start_line: current_start as i32,
                    });
                    current_lines.clear();
                }
                current_header = Some(line.to_string());
                current_start = i;
            } else {
                current_lines.push(*line);
            }
        }

        // Don't forget the last section
        if !current_lines.is_empty() || current_header.is_some() {
            sections.push(Section {
                header: current_header,
                content: current_lines.join("\n"),
                start_line: current_start as i32,
            });
        }

        sections
    }

    fn chunk_by_lines(&self, content: &str, base_line: i32) -> Vec<ChunkSpan> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut chunk_start = 0;

        for (i, line) in lines.iter().enumerate() {
            let line_with_newline = if current_chunk.is_empty() {
                line.to_string()
            } else {
                format!("\n{}", line)
            };

            if current_chunk.len() + line_with_newline.len() > self.max_chunk_size
                && !current_chunk.is_empty()
            {
                // Save current chunk
                let line_count = current_chunk.lines().count() as i32;
                chunks.push(ChunkSpan {
                    content: current_chunk,
                    start_line: base_line + chunk_start as i32 + 1,
                    end_line: base_line + chunk_start as i32 + line_count,
                    is_overview: false,
                });
                current_chunk = line.to_string();
                chunk_start = i;
            } else {
                current_chunk.push_str(&line_with_newline);
            }
        }

        // Don't forget the last chunk
        if !current_chunk.is_empty() {
            let line_count = current_chunk.lines().count() as i32;
            chunks.push(ChunkSpan {
                content: current_chunk,
                start_line: base_line + chunk_start as i32 + 1,
                end_line: base_line + chunk_start as i32 + line_count.max(1),
                is_overview: false,
            });
        }

        chunks
    }
}

struct Section {
    header: Option<String>,
    content: String,
    start_line: i32,
}

/// Clean a markdown header to create a URL fragment.
///
/// - Removes special characters except alphanumeric, hyphen, space, underscore
/// - Converts to lowercase
/// - Replaces spaces with hyphens
pub fn clean_fragment(header: &str) -> String {
    let header = header.trim_start_matches('#').trim();

    // Remove link syntax if present
    let header = if let Some(idx) = header.find("](") {
        &header[..idx]
    } else {
        header
    };

    // Keep only alphanumeric, hyphen, space, underscore
    let cleaned: String = header
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ' || *c == '_')
        .collect();

    // Lowercase and replace spaces with hyphens
    cleaned.to_lowercase().replace(' ', "-")
}

/// Check if a file extension indicates markdown content.
pub fn is_markdown_file(extension: &str) -> bool {
    matches!(extension.to_lowercase().as_str(), "md" | "markdown" | "mdx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_content_single_chunk() {
        let content = "# Title\n\nSmall content.";
        let chunker = MarkdownChunker::new(1000);
        let chunks = chunker.chunk(content);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, content);
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn test_split_by_h2_headers() {
        let content = r#"# Main Title

Intro text.

## Section 1

Content of section 1.

## Section 2

Content of section 2.
"#;
        let chunker = MarkdownChunker::new(50);
        let chunks = chunker.chunk(content);

        // Should create multiple chunks based on h2 headers
        assert!(chunks.len() >= 2);

        // Each section chunk should include its header
        let combined: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(combined.contains("## Section 1"));
        assert!(combined.contains("## Section 2"));
    }

    #[test]
    fn test_nested_headers() {
        let content = r#"# Top

## Sub1

### SubSub1

Content here.

## Sub2

More content."#;
        let chunker = MarkdownChunker::new(30);
        let chunks = chunker.chunk(content);

        // Should have chunks with nested headers preserved
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_clean_fragment() {
        assert_eq!(clean_fragment("# Hello World"), "hello-world");
        assert_eq!(clean_fragment("## API Reference (v2)"), "api-reference-v2");
        assert_eq!(
            clean_fragment("### Link [Example](http://example.com)"),
            "link-example"
        );
        assert_eq!(
            clean_fragment("Special $chars% here!"),
            "special-chars-here"
        );
    }

    #[test]
    fn test_is_markdown_file() {
        assert!(is_markdown_file("md"));
        assert!(is_markdown_file("MD"));
        assert!(is_markdown_file("markdown"));
        assert!(is_markdown_file("mdx"));
        assert!(!is_markdown_file("txt"));
        assert!(!is_markdown_file("rs"));
    }

    #[test]
    fn test_line_numbers() {
        let content = "# Title\n\nLine 3\nLine 4\nLine 5";
        let chunker = MarkdownChunker::new(1000);
        let chunks = chunker.chunk(content);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 5);
    }

    #[test]
    fn test_fallback_to_line_chunking() {
        // Content with no headers should still be chunked
        let content = "Line 1\n".repeat(100);
        let chunker = MarkdownChunker::new(50);
        let chunks = chunker.chunk(&content);

        assert!(chunks.len() > 1);
    }
}
