//! Code chunking using text-splitter with token-aware splitting.
//!
//! Uses CodeSplitter (tree-sitter AST-aware) for supported languages,
//! MarkdownChunker for markdown files, with TextSplitter fallback for others.
//!
//! All splitting is token-aware using tiktoken (cl100k_base), ensuring chunks
//! respect both syntax boundaries AND token limits without post-processing.
//!
//! # Key Behaviors
//!
//! ## 1. Large Function Handling (AST-aware splitting)
//!
//! When a function exceeds `max_tokens`, CodeSplitter splits at AST boundaries:
//!
//! ```text
//! fn very_long_function(...) -> Result {  // 2000 tokens total
//!     let a = 1;
//!     let b = 2;
//!     if condition { ... }
//!     else { ... }
//! }
//!
//! // Splits into (example with 50 token limit):
//! Chunk 1: fn very_long_function(...) -> Result  // signature (complete)
//! Chunk 2: { let a = 1; let b = 2; }              // statements (complete)
//! Chunk 3: if condition { ... }                   // control block (complete)
//! Chunk 4: else { ... } }                         // control block (complete)
//! ```
//!
//! **Splitting hierarchy** (falls back to lower level if chunk still too large):
//! - File → Module/Class → Function → Statement → Line → Character
//!
//! **Result**: No content lost, each chunk is a semantically meaningful unit.
//!
//! ## 2. Overlap Disabled for Code
//!
//! Token-based overlap is **disabled** for code files because:
//!
//! 1. **Natural boundaries**: Functions/classes are self-contained semantic units
//! 2. **AST fragments**: Token overlap creates broken syntax (e.g., `}`, `else`)
//! 3. **Search quality**: Duplicate content distorts BM25/embedding scores
//!
//! ```text
//! // BAD: Token-based overlap creates fragments
//! Chunk 1: fn foo() { let a = 1; }
//! Chunk 2: "let a = 1; }" + fn bar() { ... }  // ❌ "}" is meaningless
//!
//! // GOOD: No overlap, clean AST boundaries
//! Chunk 1: fn foo() { let a = 1; }
//! Chunk 2: fn bar() { let b = 2; }            // ✅ complete functions
//! ```
//!
//! **Text files** (non-code) DO use overlap since prose benefits from context.
//!
//! # Reference
//!
//! Tabby's `tabby-index/src/code/intelligence.rs`

use crate::chunking::markdown::MarkdownChunker;
use crate::chunking::markdown::is_markdown_file;
use crate::error::Result;
use crate::types::ChunkSpan;
use once_cell::sync::Lazy;
use regex::Regex;
use text_splitter::ChunkConfig;
use text_splitter::CodeSplitter;
use text_splitter::TextSplitter;
use tiktoken_rs::CoreBPE;
use tiktoken_rs::cl100k_base;
use tree_sitter::Language;

/// Code chunking service with token-aware splitting.
///
/// Uses CodeSplitter (tree-sitter AST-aware) for supported languages,
/// MarkdownChunker for markdown files, with TextSplitter fallback for others.
///
/// All splitting is token-aware, ensuring chunks respect both syntax boundaries
/// AND embedding model token limits.
pub struct CodeChunkerService {
    max_tokens: usize,
    overlap_tokens: usize,
}

impl CodeChunkerService {
    /// Create a token-aware chunker.
    ///
    /// Uses tiktoken (cl100k_base) to count tokens during tree-sitter splitting.
    /// This ensures chunks respect both syntax boundaries AND token limits.
    ///
    /// # Arguments
    /// * `max_tokens` - Maximum tokens per chunk (industry recommendation: 256-512)
    /// * `overlap_tokens` - Overlap tokens between chunks (~10% of max_tokens)
    pub fn new(max_tokens: usize, overlap_tokens: usize) -> Self {
        Self {
            max_tokens,
            overlap_tokens,
        }
    }

    /// Chunk a source file.
    ///
    /// For markdown files, uses MarkdownChunker which respects header hierarchy.
    /// For supported languages (rust, go, python, java, typescript, javascript),
    /// uses CodeSplitter which is tree-sitter based and respects syntax boundaries.
    /// Falls back to TextSplitter for unsupported languages.
    ///
    /// Import blocks at the start of files are kept together as a single chunk
    /// to provide dependency context and enable queries like "what does this file import".
    pub fn chunk(&self, content: &str, language: &str) -> Result<Vec<ChunkSpan>> {
        // Load tokenizer (cl100k_base is OpenAI's tokenizer)
        let tokenizer = cl100k_base().expect("Failed to load cl100k_base tokenizer");

        // Create token-aware chunk config
        let chunk_config = ChunkConfig::new(self.max_tokens).with_sizer(tokenizer);

        // Markdown: use MarkdownChunker with token-based size estimation
        if is_markdown_file(language) {
            tracing::trace!(language = %language, "Using MarkdownChunker");
            // Convert tokens to chars estimate (avg 4 chars/token for code)
            let estimated_chars = self.max_tokens * 4;
            let md_chunker = MarkdownChunker::new(estimated_chars);
            return Ok(md_chunker.chunk(content));
        }

        // Detect and extract import block at the start of the file
        let (import_chunk, remaining_content, line_offset) =
            self.extract_import_block(content, language);

        // For supported languages, use token-aware CodeSplitter
        //
        // KEY BEHAVIOR #1: Large Function Splitting
        // When a function exceeds max_tokens, CodeSplitter splits at AST boundaries:
        //   - Hierarchy: File → Class → Function → Statement → Line → Char
        //   - Example: 2000-token function with 50-token limit becomes:
        //     Chunk 1: fn signature(...)       // complete signature
        //     Chunk 2: { stmt1; stmt2; }       // complete statements
        //     Chunk 3: if { ... }              // complete control block
        //   - No content lost, each chunk is semantically meaningful
        //
        // KEY BEHAVIOR #2: Overlap Disabled for Code
        // Token-based overlap is NOT applied to code because:
        //   1. Functions/classes are self-contained semantic units
        //   2. Token overlap creates AST fragments (e.g., `}`, `else`)
        //   3. Duplicate content distorts BM25/embedding search scores
        if let Some(ts_lang) = get_tree_sitter_language(language) {
            if let Ok(splitter) = CodeSplitter::new(ts_lang, chunk_config) {
                let raw_chunks: Vec<(usize, &str)> =
                    splitter.chunk_indices(&remaining_content).collect();
                tracing::trace!(
                    language = %language,
                    chunks = raw_chunks.len(),
                    import_chunk = import_chunk.is_some(),
                    max_tokens = self.max_tokens,
                    overlap = "disabled for code (AST boundaries sufficient)",
                    "CodeSplitter: AST-aware chunking"
                );

                let mut chunks: Vec<ChunkSpan> = raw_chunks
                    .into_iter()
                    .map(|(offset, chunk)| {
                        let mut span = Self::to_chunk_span(&remaining_content, offset, chunk);
                        // Adjust line numbers to account for extracted import block
                        span.start_line += line_offset;
                        span.end_line += line_offset;
                        span
                    })
                    .collect();

                // Prepend import chunk if present
                if let Some(import) = import_chunk {
                    chunks.insert(0, import);
                }

                // No overlap for code - AST boundaries provide natural semantic separation
                return Ok(chunks);
            }
        }

        // Fallback: TextSplitter with token-aware config
        // Overlap IS applied for text because prose benefits from context continuity
        tracing::trace!(
            language = %language,
            overlap_tokens = self.overlap_tokens,
            "Using TextSplitter fallback with overlap"
        );
        let tokenizer = cl100k_base().expect("tiktoken");
        let chunk_config = ChunkConfig::new(self.max_tokens).with_sizer(tokenizer.clone());
        let splitter = TextSplitter::new(chunk_config);
        let raw_chunks: Vec<(usize, &str)> = splitter.chunk_indices(&remaining_content).collect();

        let mut chunks: Vec<ChunkSpan> = raw_chunks
            .into_iter()
            .map(|(offset, chunk)| {
                let mut span = Self::to_chunk_span(&remaining_content, offset, chunk);
                // Adjust line numbers to account for extracted import block
                span.start_line += line_offset;
                span.end_line += line_offset;
                span
            })
            .collect();

        // Prepend import chunk if present
        if let Some(import) = import_chunk {
            chunks.insert(0, import);
        }

        // Apply overlap for text files
        if self.overlap_tokens > 0 && chunks.len() > 1 {
            Self::apply_overlap(&mut chunks, self.overlap_tokens, &tokenizer);
        }

        Ok(chunks)
    }

    /// Extract import block from the start of content.
    ///
    /// Returns (import_chunk, remaining_content, line_offset).
    fn extract_import_block(
        &self,
        content: &str,
        language: &str,
    ) -> (Option<ChunkSpan>, String, i32) {
        // Detect import block
        let Some((end_line, import_content)) = detect_import_block(content, language) else {
            return (None, content.to_string(), 0);
        };

        // Create import chunk
        let import_chunk = ChunkSpan {
            content: import_content,
            start_line: 1,
            end_line,
            is_overview: false,
        };

        // Extract remaining content after imports
        let lines: Vec<&str> = content.lines().collect();
        let remaining_lines = if (end_line as usize) < lines.len() {
            lines[end_line as usize..].to_vec()
        } else {
            Vec::new()
        };

        // Skip empty lines at the start of remaining content
        let skip_empty = remaining_lines
            .iter()
            .take_while(|line| line.trim().is_empty())
            .count();

        let actual_remaining: Vec<&str> = remaining_lines.into_iter().skip(skip_empty).collect();
        let remaining_content = actual_remaining.join("\n");

        // Calculate line offset (import_end_line + skipped empty lines)
        let line_offset = end_line + skip_empty as i32;

        tracing::trace!(
            language = %language,
            import_end_line = end_line,
            line_offset = line_offset,
            "Extracted import block"
        );

        (Some(import_chunk), remaining_content, line_offset)
    }

    /// Apply overlap using token-based measurement.
    fn apply_overlap(chunks: &mut Vec<ChunkSpan>, overlap_tokens: usize, tokenizer: &CoreBPE) {
        for i in 1..chunks.len() {
            let prev_content = chunks[i - 1].content.clone();
            let prev_tokens = tokenizer.encode_with_special_tokens(&prev_content);

            if prev_tokens.len() > overlap_tokens {
                // Find the character position that corresponds to overlap_tokens from the end
                let overlap_start_token = prev_tokens.len() - overlap_tokens;
                let overlap_tokens_slice = &prev_tokens[overlap_start_token..];
                if let Ok(overlap_text) = tokenizer.decode(overlap_tokens_slice.to_vec()) {
                    // Find line boundary for clean overlap
                    let overlap = if let Some(pos) = overlap_text.find('\n') {
                        overlap_text[pos + 1..].to_string()
                    } else {
                        overlap_text
                    };
                    if !overlap.is_empty() {
                        chunks[i].content = format!("{}{}", overlap, chunks[i].content);
                    }
                }
            }
        }
    }

    fn to_chunk_span(full_content: &str, offset: usize, chunk: &str) -> ChunkSpan {
        let start_line = full_content[..offset].lines().count() as i32 + 1;
        let chunk_lines = chunk.lines().count() as i32;
        let end_line = start_line + chunk_lines.saturating_sub(1);

        ChunkSpan {
            content: chunk.to_string(),
            start_line,
            end_line: end_line.max(start_line),
            is_overview: false,
        }
    }
}

/// Get tree-sitter Language for supported languages.
///
/// Returns None for unsupported languages, triggering TextSplitter fallback.
/// Currently supports: rust, go, python, java, typescript, javascript
/// (matching tree-sitter-* dependencies).
fn get_tree_sitter_language(lang: &str) -> Option<Language> {
    match lang {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        // TypeScript/JavaScript support via tree-sitter-typescript
        // LANGUAGE_TYPESCRIPT: TS/JS without JSX syntax
        // LANGUAGE_TSX: TS/JS with JSX syntax support
        "typescript" | "javascript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        _ => None,
    }
}

/// Languages with CodeSplitter (tree-sitter AST) support.
pub const CODE_SPLITTER_LANGUAGES: &[&str] = &[
    "rust",
    "go",
    "python",
    "java",
    "typescript",
    "javascript",
    "tsx",
    "jsx",
];

/// Check if a language is supported by CodeSplitter.
pub fn is_code_splitter_supported(lang: &str) -> bool {
    get_tree_sitter_language(lang).is_some()
}

/// Precompiled regex patterns for import detection.
/// Using `once_cell::sync::Lazy` to avoid recompiling on every call.

/// Rust: use, mod, extern crate, #![...], #[...]
static RUST_IMPORT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(use\s|mod\s|pub\s+use\s|pub\s+mod\s|extern\s+crate\s|#\[|#!\[)")
        .expect("invalid rust import regex")
});

/// Python: import, from ... import
static PYTHON_IMPORT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(import\s|from\s+\S+\s+import\s)").expect("invalid python import regex")
});

/// JS/TS: import, export, require (all forms), "use strict", "use client"
static JS_IMPORT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"^\s*(import\s|export\s|(const|let|var)\s+[\w\{\s,\}]+\s*=\s*require\(|['"]use\s)"#,
    )
    .expect("invalid js import regex")
});

/// Go/Java: package, import
static GO_JAVA_IMPORT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(package\s|import\s)").expect("invalid go/java import regex"));

/// Detect the import block at the start of a file.
///
/// Returns `Some((end_line, import_content))` if an import block is found,
/// where `end_line` is the 1-indexed line number where imports end.
///
/// Import blocks are kept together as a single chunk to:
/// - Provide context about dependencies when searching
/// - Avoid fragmenting import statements across chunks
/// - Enable queries like "what does this file import"
pub fn detect_import_block(content: &str, language: &str) -> Option<(i32, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // Language-specific import patterns (precompiled, see static definitions above)
    let pattern: &Regex = match language {
        "rust" => &RUST_IMPORT_REGEX,
        "python" => &PYTHON_IMPORT_REGEX,
        "typescript" | "javascript" | "tsx" | "jsx" => &JS_IMPORT_REGEX,
        "go" | "java" => &GO_JAVA_IMPORT_REGEX,
        _ => return None,
    };

    // Find the end of the import block
    let mut end_line = 0;
    let mut in_multiline_import = false;
    let mut brace_depth = 0; // Track brace depth for JS/TS multi-line imports

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments at the start
        if trimmed.is_empty() || is_comment_line(trimmed, language) {
            // Only count if we've already started finding imports or in multi-line import
            if end_line > 0 || in_multiline_import {
                end_line = i as i32 + 1;
            }
            continue;
        }

        // Handle Go's multi-line import block: import ( ... )
        if language == "go" {
            if trimmed.starts_with("import (") || trimmed == "import(" {
                in_multiline_import = true;
                end_line = i as i32 + 1;
                continue;
            }
            if in_multiline_import {
                end_line = i as i32 + 1;
                if trimmed.starts_with(')') {
                    in_multiline_import = false;
                }
                continue;
            }
        }

        // Handle JS/TS multi-line imports: import { ... } from '...';
        if matches!(language, "typescript" | "javascript" | "tsx" | "jsx") {
            if in_multiline_import {
                end_line = i as i32 + 1;
                // Count braces to track when import ends
                for c in line.chars() {
                    match c {
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
                // Import ends when we see 'from' or ';' and braces are balanced
                if brace_depth <= 0
                    && (trimmed.contains("from ")
                        || trimmed.contains("from'")
                        || trimmed.ends_with(';'))
                {
                    in_multiline_import = false;
                    brace_depth = 0;
                }
                continue;
            }

            // Detect start of multi-line import
            if pattern.is_match(line) {
                end_line = i as i32 + 1;

                // Check if this import opens a brace that's not closed on same line
                let open_braces = line.matches('{').count();
                let close_braces = line.matches('}').count();
                if open_braces > close_braces {
                    in_multiline_import = true;
                    brace_depth = (open_braces - close_braces) as i32;
                }
                continue;
            }
        }

        // Check if line matches import pattern
        if pattern.is_match(line) {
            end_line = i as i32 + 1;
        } else if end_line > 0 && !in_multiline_import {
            // First non-import line after imports - stop here
            break;
        } else {
            // No imports found yet and this isn't an import line
            // Could be a shebang, pragma, etc. - keep looking for a few lines
            if i > 5 {
                break;
            }
        }
    }

    if end_line == 0 {
        return None;
    }

    // Extract the import block content
    let import_content: String = lines[..end_line as usize].join("\n");

    Some((end_line, import_content))
}

/// Check if a line is a comment.
fn is_comment_line(line: &str, language: &str) -> bool {
    match language {
        "rust" | "go" | "java" | "typescript" | "javascript" | "tsx" | "jsx" => {
            line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
        }
        "python" => line.starts_with('#'),
        _ => false,
    }
}

/// Get formatted string of supported languages for logging.
pub fn supported_languages_info() -> String {
    format!(
        "CodeSplitter (AST-aware): {} | Others: TextSplitter fallback",
        CODE_SPLITTER_LANGUAGES.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_code() {
        let code = r#"
fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let chunker = CodeChunkerService::new(512, 50);
        let chunks = chunker.chunk(code, "rust").expect("chunking failed");

        assert!(!chunks.is_empty());
        // Chunks should cover the entire content
        let total_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert_eq!(total_content.trim(), code.trim());
    }

    #[test]
    fn test_line_numbers() {
        let code = "line1\nline2\nline3\nline4\nline5";
        let chunker = CodeChunkerService::new(1000, 0);
        let chunks = chunker.chunk(code, "text").expect("chunking failed");

        assert_eq!(chunks.len(), 1);
        // Line numbers are 1-indexed
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 5);
    }

    #[test]
    fn test_multiple_chunks() {
        // Use small token limit to force multiple chunks
        let code = "a".repeat(100) + "\n" + &"b".repeat(100);
        let chunker = CodeChunkerService::new(20, 0);
        let chunks = chunker.chunk(&code, "text").expect("chunking failed");

        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_code_splitter_supported_languages() {
        // Supported languages
        assert!(is_code_splitter_supported("rust"));
        assert!(is_code_splitter_supported("go"));
        assert!(is_code_splitter_supported("python"));
        assert!(is_code_splitter_supported("java"));
        assert!(is_code_splitter_supported("typescript"));
        assert!(is_code_splitter_supported("javascript"));
        assert!(is_code_splitter_supported("tsx"));
        assert!(is_code_splitter_supported("jsx"));
        // Unsupported languages
        assert!(!is_code_splitter_supported("markdown"));
        assert!(!is_code_splitter_supported("unknown"));
        assert!(!is_code_splitter_supported("c"));
        assert!(!is_code_splitter_supported("cpp"));
    }

    #[test]
    fn test_code_splitter_typescript() {
        let code = r#"interface User {
    id: number;
    name: string;
}

function greet(user: User): string {
    return `Hello, ${user.name}!`;
}

class UserService {
    private users: User[] = [];

    addUser(user: User): void {
        this.users.push(user);
    }

    getUser(id: number): User | undefined {
        return this.users.find(u => u.id === id);
    }
}
"#;
        let chunker = CodeChunkerService::new(100, 0);
        let chunks = chunker.chunk(code, "typescript").expect("chunking failed");

        assert!(!chunks.is_empty());
        let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(total.contains("interface User"));
        assert!(total.contains("function greet"));
        assert!(total.contains("class UserService"));
    }

    #[test]
    fn test_code_splitter_javascript() {
        let code = r#"const express = require('express');
const app = express();

function handleRequest(req, res) {
    res.json({ message: 'Hello, World!' });
}

app.get('/hello', handleRequest);

class Router {
    constructor() {
        this.routes = [];
    }

    add(path, handler) {
        this.routes.push({ path, handler });
    }
}
"#;
        let chunker = CodeChunkerService::new(100, 0);
        let chunks = chunker.chunk(code, "javascript").expect("chunking failed");

        assert!(!chunks.is_empty());
        let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(total.contains("const express"));
        assert!(total.contains("function handleRequest"));
        assert!(total.contains("class Router"));
    }

    #[test]
    fn test_code_splitter_rust() {
        let code = r#"fn hello() {
    println!("Hello");
}

fn world() {
    println!("World");
}

fn long_function() {
    let x = 1;
    let y = 2;
    let z = 3;
    println!("{} {} {}", x, y, z);
}
"#;
        let chunker = CodeChunkerService::new(100, 0);
        let chunks = chunker.chunk(code, "rust").expect("chunking failed");

        assert!(!chunks.is_empty());
        let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(total.contains("fn hello()"));
        assert!(total.contains("fn world()"));
        assert!(total.contains("fn long_function()"));
    }

    #[test]
    fn test_code_splitter_python() {
        let code = r#"def hello():
    print("Hello")

def world():
    print("World")

class Greeter:
    def greet(self, name):
        return f"Hello, {name}"
"#;
        let chunker = CodeChunkerService::new(100, 0);
        let chunks = chunker.chunk(code, "python").expect("chunking failed");

        assert!(!chunks.is_empty());
        let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(total.contains("def hello()"));
        assert!(total.contains("def world()"));
        assert!(total.contains("class Greeter"));
    }

    #[test]
    fn test_code_splitter_go() {
        let code = r#"package main

func hello() {
    fmt.Println("Hello")
}

func world() {
    fmt.Println("World")
}
"#;
        let chunker = CodeChunkerService::new(100, 0);
        let chunks = chunker.chunk(code, "go").expect("chunking failed");

        assert!(!chunks.is_empty());
        let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(total.contains("func hello()"));
        assert!(total.contains("func world()"));
    }

    #[test]
    fn test_text_splitter_fallback() {
        let code = "const x = 1;\nconst y = 2;\nconst z = 3;";
        let chunker = CodeChunkerService::new(1000, 0);
        let chunks = chunker.chunk(code, "javascript").expect("chunking failed");

        assert!(!chunks.is_empty());
        let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert_eq!(total.trim(), code.trim());
    }

    #[test]
    fn test_chunk_with_overlap() {
        let lines: Vec<String> = (1..=20).map(|i| format!("line{i}")).collect();
        let code = lines.join("\n");

        // Without overlap
        let chunker_no_overlap = CodeChunkerService::new(30, 0);
        let chunks_no_overlap = chunker_no_overlap
            .chunk(&code, "text")
            .expect("chunking failed");

        // With overlap
        let chunker_with_overlap = CodeChunkerService::new(30, 5);
        let chunks_with_overlap = chunker_with_overlap
            .chunk(&code, "text")
            .expect("chunking failed");

        assert!(chunks_no_overlap.len() > 1);
        assert!(chunks_with_overlap.len() > 1);

        // With overlap, subsequent chunks should have extra content
        if chunks_with_overlap.len() >= 2 {
            assert!(
                chunks_with_overlap[1].content.len() >= chunks_no_overlap[1].content.len(),
                "Overlapped chunk should be at least as long as non-overlapped"
            );
        }
    }

    #[test]
    fn test_single_chunk_no_overlap_effect() {
        let code = "short content";
        let chunker = CodeChunkerService::new(1000, 50);
        let chunks = chunker.chunk(code, "text").expect("chunking failed");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content.trim(), code.trim());
    }

    #[test]
    fn test_code_overlap_disabled() {
        // Overlap is DISABLED for code because token-based overlap creates AST fragments.
        // This test verifies that overlap_tokens parameter has no effect for code.
        let code = r#"fn first_function() {
    let a = 1;
    let b = 2;
}

fn second_function() {
    let c = 3;
    let d = 4;
}

fn third_function() {
    let e = 5;
}"#;

        // Without overlap
        let chunker_no_overlap = CodeChunkerService::new(40, 0);
        let chunks_no = chunker_no_overlap.chunk(code, "rust").unwrap();

        // With overlap parameter (10 tokens) - should have NO effect for code
        let chunker_with_overlap = CodeChunkerService::new(40, 10);
        let chunks_with = chunker_with_overlap.chunk(code, "rust").unwrap();

        // For code, both should produce identical chunks (overlap disabled)
        assert_eq!(
            chunks_no.len(),
            chunks_with.len(),
            "Code chunking should ignore overlap parameter"
        );

        for (no, with) in chunks_no.iter().zip(chunks_with.iter()) {
            assert_eq!(
                no.content, with.content,
                "Code chunks should be identical regardless of overlap setting"
            );
        }

        // Verify chunks don't start with AST fragments
        for (i, chunk) in chunks_no.iter().enumerate() {
            let trimmed = chunk.content.trim();
            // Chunks shouldn't start with closing braces or partial expressions
            let bad_starts = ["}", ")", "]", ",", "else", "&&", "||"];
            for bad in &bad_starts {
                assert!(
                    !trimmed.starts_with(bad),
                    "Chunk {} should not start with '{}': {}",
                    i + 1,
                    bad,
                    &trimmed[..trimmed.len().min(30)]
                );
            }
        }
    }

    #[test]
    fn test_text_overlap_works() {
        // Overlap SHOULD work for plain text (non-code)
        let text = "Line one.\nLine two.\nLine three.\nLine four.\nLine five.\nLine six.";

        // Small token limit to force multiple chunks
        let chunker_no_overlap = CodeChunkerService::new(10, 0);
        let chunks_no = chunker_no_overlap.chunk(text, "text").unwrap();

        let chunker_with_overlap = CodeChunkerService::new(10, 3);
        let chunks_with = chunker_with_overlap.chunk(text, "text").unwrap();

        // With overlap, chunks should be different (overlap applied)
        if chunks_no.len() > 1 && chunks_with.len() > 1 {
            // Second chunk with overlap should contain content from end of first chunk
            let second_no = &chunks_no[1].content;
            let second_with = &chunks_with[1].content;

            // The overlapped chunk should be longer or start with content from previous
            assert!(
                second_with.len() >= second_no.len() || chunks_with.len() != chunks_no.len(),
                "Text overlap should produce different chunks"
            );
        }
    }

    #[test]
    fn test_oversized_function_handling() {
        // Test what happens when a single function exceeds max_tokens
        // This is a ~150 token function, we'll use 30 token limit
        let code = r#"fn very_long_function(a: i32, b: i32, c: String) -> Result<String, Error> {
    let result_one = process_first_step(a, b);
    let result_two = process_second_step(result_one, &c);
    let result_three = process_third_step(result_two);
    if result_three.is_ok() {
        println!("Success: {:?}", result_three);
        Ok(result_three.unwrap())
    } else {
        Err(Error::new("Failed"))
    }
}"#;
        let chunker = CodeChunkerService::new(30, 0);
        let chunks = chunker.chunk(code, "rust").expect("chunking failed");

        // Key assertions:
        // 1. Content is NOT lost
        let combined: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            combined.contains("very_long_function"),
            "Function name should be present"
        );
        assert!(
            combined.contains("result_one"),
            "Variable should be present"
        );
        assert!(
            combined.contains("result_three"),
            "Last variable should be present"
        );

        // 2. Multiple chunks are produced (function was split)
        assert!(
            chunks.len() > 1,
            "Long function should be split into multiple chunks"
        );
    }

    #[test]
    fn test_token_mode_respects_syntax() {
        // This test verifies that token mode produces valid chunks that don't break
        // in the middle of statements. A chunk may contain:
        // - A complete function
        // - Multiple complete functions
        // - The closing brace of one function + another complete function
        // What we want to AVOID is a chunk ending mid-statement, e.g.:
        // "fn foo() {\n    let x = 1;" without the closing brace
        let code = r#"fn process_data(input: &str) -> Result<String, Error> {
    let mut result = String::new();
    for line in input.lines() {
        if line.starts_with("//") {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    Ok(result)
}

fn another_function() {
    println!("test");
}
"#;
        // Use small token limit to force chunking
        let chunker = CodeChunkerService::new(50, 0);
        let chunks = chunker.chunk(code, "rust").expect("chunking failed");

        // We should have at least one chunk
        assert!(!chunks.is_empty(), "Should produce at least one chunk");

        // Verify all code is covered (no content lost)
        let combined: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            combined.contains("fn process_data"),
            "Should contain first function"
        );
        assert!(
            combined.contains("fn another_function"),
            "Should contain second function"
        );

        // Verify each chunk that contains a function body has balanced braces
        // (meaning we didn't split mid-function)
        for chunk in &chunks {
            let open_braces = chunk.content.matches('{').count();
            let close_braces = chunk.content.matches('}').count();
            // Allow for partial functions at boundaries, but the imbalance shouldn't be extreme
            let imbalance = (open_braces as i32 - close_braces as i32).abs();
            assert!(
                imbalance <= 2,
                "Chunk has excessive brace imbalance ({}): {}",
                imbalance,
                &chunk.content[..chunk.content.len().min(100)]
            );
        }
    }

    #[test]
    fn test_detect_import_block_rust() {
        let code = r#"use std::io;
use std::path::Path;
use crate::error::Result;

fn main() {
    println!("Hello");
}
"#;
        let result = detect_import_block(code, "rust");
        assert!(result.is_some());
        let (end_line, content) = result.unwrap();
        // Import block includes trailing empty line (line 4)
        assert_eq!(end_line, 4);
        assert!(content.contains("use std::io"));
        assert!(content.contains("use crate::error::Result"));
        assert!(!content.contains("fn main"));
    }

    #[test]
    fn test_detect_import_block_python() {
        let code = r#"import os
import sys
from typing import List, Optional
from pathlib import Path

def main():
    print("Hello")
"#;
        let result = detect_import_block(code, "python");
        assert!(result.is_some());
        let (end_line, content) = result.unwrap();
        // Import block includes trailing empty line (line 5)
        assert_eq!(end_line, 5);
        assert!(content.contains("import os"));
        assert!(content.contains("from pathlib import Path"));
        assert!(!content.contains("def main"));
    }

    #[test]
    fn test_detect_import_block_typescript() {
        let code = r#"import React from 'react';
import { useState, useEffect } from 'react';
import type { User } from './types';

export function App() {
    return <div>Hello</div>;
}
"#;
        let result = detect_import_block(code, "typescript");
        assert!(result.is_some());
        let (end_line, content) = result.unwrap();
        assert!(end_line >= 3);
        assert!(content.contains("import React"));
        assert!(content.contains("import type { User }"));
    }

    #[test]
    fn test_detect_import_block_typescript_multiline() {
        let code = r#"import {
    useState,
    useEffect,
    useCallback,
    useMemo
} from 'react';
import { Button } from './components';

function App() {
    return <div>Hello</div>;
}
"#;
        let result = detect_import_block(code, "typescript");
        assert!(result.is_some());
        let (end_line, content) = result.unwrap();
        // Should include both multi-line import and single-line import (line 8 with trailing empty)
        assert!(
            end_line >= 7,
            "end_line should be at least 7, got {end_line}"
        );
        assert!(content.contains("useState"));
        assert!(content.contains("useMemo"));
        assert!(content.contains("} from 'react'"));
        assert!(content.contains("Button"));
        assert!(!content.contains("function App"));
    }

    #[test]
    fn test_detect_import_block_javascript_multiline() {
        let code = r#"const {
    readFile,
    writeFile
} = require('fs');
const path = require('path');

function main() {
    console.log('Hello');
}
"#;
        let result = detect_import_block(code, "javascript");
        assert!(result.is_some());
        let (end_line, content) = result.unwrap();
        assert!(
            end_line >= 5,
            "end_line should be at least 5, got {end_line}"
        );
        assert!(content.contains("readFile"));
        assert!(content.contains("writeFile"));
        assert!(content.contains("path"));
        assert!(!content.contains("function main"));
    }

    #[test]
    fn test_detect_import_block_javascript_require_variants() {
        // Test all require() variants: const, let, var, destructured
        let code = r#"const fs = require('fs');
let path = require('path');
var os = require('os');
const { join, resolve } = require('path');

function main() {}
"#;
        let result = detect_import_block(code, "javascript");
        assert!(result.is_some());
        let (end_line, content) = result.unwrap();
        assert!(
            end_line >= 4,
            "end_line should be at least 4, got {end_line}"
        );
        assert!(content.contains("const fs"));
        assert!(content.contains("let path"));
        assert!(content.contains("var os"));
        assert!(content.contains("join, resolve"));
        assert!(!content.contains("function main"));
    }

    #[test]
    fn test_detect_import_block_go_multiline() {
        let code = r#"package main

import (
    "fmt"
    "os"
    "path/filepath"
)

func main() {
    fmt.Println("Hello")
}
"#;
        let result = detect_import_block(code, "go");
        assert!(result.is_some());
        let (end_line, content) = result.unwrap();
        // Import block includes trailing empty line (line 8)
        assert_eq!(end_line, 8);
        assert!(content.contains("package main"));
        assert!(content.contains("\"fmt\""));
        assert!(content.contains("\"path/filepath\""));
        assert!(!content.contains("func main"));
    }

    #[test]
    fn test_detect_import_block_no_imports() {
        let code = r#"fn main() {
    println!("Hello");
}
"#;
        let result = detect_import_block(code, "rust");
        assert!(result.is_none());
    }
}
