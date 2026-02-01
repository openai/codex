//! Pipe segment extraction from shell commands.

use std::sync::LazyLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::Tree;

use crate::tokenizer::Span;
use crate::tokenizer::Token;
use crate::tokenizer::TokenKind;

/// A segment of a pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeSegment {
    /// Command words in this segment.
    pub command: Vec<String>,
    /// Span in the source string.
    pub span: Span,
    /// Whether this segment is part of a pipeline.
    pub is_piped: bool,
}

impl PipeSegment {
    /// Create a new pipe segment.
    pub fn new(command: Vec<String>, span: Span, is_piped: bool) -> Self {
        Self {
            command,
            span,
            is_piped,
        }
    }

    /// Returns the command name (first word).
    pub fn command_name(&self) -> Option<&str> {
        self.command.first().map(String::as_str)
    }

    /// Returns the command arguments (words after the first).
    pub fn arguments(&self) -> &[String] {
        if self.command.len() > 1 {
            &self.command[1..]
        } else {
            &[]
        }
    }
}

/// Tree-sitter query for extracting pipeline structure.
static PIPELINE_QUERY: LazyLock<Query> = LazyLock::new(|| {
    let language = tree_sitter_bash::LANGUAGE.into();
    Query::new(
        &language,
        r#"
        (pipeline) @pipeline
        (command) @command
        "#,
    )
    .expect("valid bash query")
});

/// Extract pipe segments from a tree-sitter tree.
pub fn extract_segments_from_tree(tree: &Tree, source: &str) -> Vec<PipeSegment> {
    let mut segments = Vec::new();
    let root = tree.root_node();
    let bytes = source.as_bytes();

    // Find all pipelines and their commands
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&PIPELINE_QUERY, root, bytes);

    // Track which commands are inside pipelines
    let mut pipeline_commands = std::collections::HashSet::new();

    // First pass: collect pipeline spans
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let name = PIPELINE_QUERY.capture_names()[capture.index as usize];
            if name == "pipeline" {
                // Mark all commands in this pipeline
                let pipeline_node = capture.node;
                let mut cursor = pipeline_node.walk();
                for child in pipeline_node.children(&mut cursor) {
                    if child.kind() == "command" {
                        pipeline_commands.insert(child.id());
                    }
                }
            }
        }
    }

    // Second pass: extract all commands with pipeline info
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&PIPELINE_QUERY, root, bytes);

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let name = PIPELINE_QUERY.capture_names()[capture.index as usize];
            if name == "command" {
                let node = capture.node;
                let is_piped = pipeline_commands.contains(&node.id());

                if let Some(command) = extract_command_words_from_node(node, source) {
                    segments.push(PipeSegment::new(
                        command,
                        Span::new(node.start_byte() as i32, node.end_byte() as i32),
                        is_piped,
                    ));
                }
            }
        }
    }

    // Sort by position and deduplicate
    segments.sort_by_key(|s| s.span.start);
    segments.dedup_by(|a, b| a.span == b.span);

    segments
}

/// Extract command words from a command node.
fn extract_command_words_from_node(node: tree_sitter::Node, source: &str) -> Option<Vec<String>> {
    if node.kind() != "command" {
        return None;
    }

    let mut words = Vec::new();
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "command_name" => {
                if let Some(name_child) = child.named_child(0) {
                    if let Ok(text) = name_child.utf8_text(source.as_bytes()) {
                        words.push(text.to_string());
                    }
                }
            }
            "word" | "number" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    words.push(text.to_string());
                }
            }
            "string" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    // Strip quotes
                    let stripped = text
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .unwrap_or(text);
                    words.push(stripped.to_string());
                }
            }
            "raw_string" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    // Strip quotes
                    let stripped = text
                        .strip_prefix('\'')
                        .and_then(|s| s.strip_suffix('\''))
                        .unwrap_or(text);
                    words.push(stripped.to_string());
                }
            }
            "concatenation" => {
                let mut concat = String::new();
                let mut concat_cursor = child.walk();
                for part in child.named_children(&mut concat_cursor) {
                    if let Ok(text) = part.utf8_text(source.as_bytes()) {
                        let stripped = match part.kind() {
                            "string" => text
                                .strip_prefix('"')
                                .and_then(|s| s.strip_suffix('"'))
                                .unwrap_or(text),
                            "raw_string" => text
                                .strip_prefix('\'')
                                .and_then(|s| s.strip_suffix('\''))
                                .unwrap_or(text),
                            _ => text,
                        };
                        concat.push_str(stripped);
                    }
                }
                if !concat.is_empty() {
                    words.push(concat);
                }
            }
            // Include expansions for visibility
            "simple_expansion" | "expansion" | "command_substitution" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    words.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    if words.is_empty() { None } else { Some(words) }
}

/// Extract pipe segments from tokens (fallback).
pub fn extract_segments_from_tokens(tokens: &[Token]) -> Vec<PipeSegment> {
    let mut segments = Vec::new();
    let mut current_cmd = Vec::new();
    let mut cmd_start: Option<i32> = None;
    let mut in_pipeline = false;

    for token in tokens {
        match token.kind {
            TokenKind::Word | TokenKind::SingleQuoted | TokenKind::DoubleQuoted => {
                if cmd_start.is_none() {
                    cmd_start = Some(token.span.start);
                }
                current_cmd.push(token.unquoted_content().to_string());
            }
            TokenKind::Operator if token.text == "|" => {
                if !current_cmd.is_empty() {
                    let span = Span::new(cmd_start.unwrap_or(token.span.start), token.span.start);
                    segments.push(PipeSegment::new(
                        std::mem::take(&mut current_cmd),
                        span,
                        true,
                    ));
                    cmd_start = None;
                }
                in_pipeline = true;
            }
            TokenKind::Operator => {
                // Other operators (&&, ||, ;) end the current segment
                if !current_cmd.is_empty() {
                    let span = Span::new(cmd_start.unwrap_or(token.span.start), token.span.start);
                    segments.push(PipeSegment::new(
                        std::mem::take(&mut current_cmd),
                        span,
                        in_pipeline,
                    ));
                    cmd_start = None;
                }
                in_pipeline = false;
            }
            TokenKind::Whitespace | TokenKind::Comment => {}
            _ => {
                // Include other tokens (variables, substitutions) in command
                if cmd_start.is_none() {
                    cmd_start = Some(token.span.start);
                }
                current_cmd.push(token.text.clone());
            }
        }
    }

    // Handle final segment
    if !current_cmd.is_empty() {
        let span = if let (Some(start), Some(last)) = (cmd_start, tokens.last()) {
            Span::new(start, last.span.end)
        } else {
            Span::new(0, 0)
        };
        segments.push(PipeSegment::new(current_cmd, span, in_pipeline));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ShellParser;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_single_command() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("ls -la");
        let segments = extract_segments_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].command, vec!["ls", "-la"]);
        assert!(!segments[0].is_piped);
    }

    #[test]
    fn test_pipeline() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("cat file | grep pattern | wc -l");
        let segments = extract_segments_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].command, vec!["cat", "file"]);
        assert!(segments[0].is_piped);
        assert_eq!(segments[1].command, vec!["grep", "pattern"]);
        assert!(segments[1].is_piped);
        assert_eq!(segments[2].command, vec!["wc", "-l"]);
        assert!(segments[2].is_piped);
    }

    #[test]
    fn test_and_chain() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("ls && pwd && echo done");
        let segments = extract_segments_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(segments.len(), 3);
        assert!(!segments[0].is_piped);
        assert!(!segments[1].is_piped);
        assert!(!segments[2].is_piped);
    }

    #[test]
    fn test_mixed_pipeline_and_chain() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("cat file | grep pattern && echo done");
        let segments = extract_segments_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(segments.len(), 3);
        // First two are piped together
        assert!(segments[0].is_piped);
        assert!(segments[1].is_piped);
        // Last one is not piped
        assert!(!segments[2].is_piped);
    }

    #[test]
    fn test_token_fallback() {
        use crate::tokenizer::Tokenizer;
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("cat file | grep pattern").unwrap();
        let segments = extract_segments_from_tokens(&tokens);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].command, vec!["cat", "file"]);
        assert_eq!(segments[1].command, vec!["grep", "pattern"]);
    }
}
