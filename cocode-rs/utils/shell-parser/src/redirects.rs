//! Redirection parsing from shell commands.

use std::sync::LazyLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::Tree;

use crate::tokenizer::Span;
use crate::tokenizer::Token;
use crate::tokenizer::TokenKind;

/// Types of shell redirections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectKind {
    /// Output redirection (>).
    Output,
    /// Output append (>>).
    Append,
    /// Input redirection (<).
    Input,
    /// Here document (<<).
    HereDoc,
    /// Here string (<<<).
    HereString,
    /// File descriptor duplication (>&, <&).
    Duplicate,
    /// Clobber (>|).
    Clobber,
    /// Read-write (<>).
    ReadWrite,
    /// Unknown redirection type.
    Unknown,
}

impl RedirectKind {
    /// Returns true if this is an output-type redirection.
    pub fn is_output(&self) -> bool {
        matches!(
            self,
            RedirectKind::Output
                | RedirectKind::Append
                | RedirectKind::Clobber
                | RedirectKind::Duplicate
        )
    }

    /// Returns true if this is an input-type redirection.
    pub fn is_input(&self) -> bool {
        matches!(
            self,
            RedirectKind::Input
                | RedirectKind::HereDoc
                | RedirectKind::HereString
                | RedirectKind::ReadWrite
        )
    }
}

/// A shell redirection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    /// The kind of redirection.
    pub kind: RedirectKind,
    /// The target (file path, file descriptor, or heredoc delimiter).
    pub target: String,
    /// The file descriptor being redirected (if specified).
    pub fd: Option<i32>,
    /// Span in the source string.
    pub span: Span,
    /// Whether this redirect is at the top level (not inside a subshell).
    pub is_top_level: bool,
}

impl Redirect {
    /// Create a new redirect.
    pub fn new(
        kind: RedirectKind,
        target: String,
        fd: Option<i32>,
        span: Span,
        is_top_level: bool,
    ) -> Self {
        Self {
            kind,
            target,
            fd,
            span,
            is_top_level,
        }
    }

    /// Returns true if this redirect writes to a file.
    pub fn writes_to_file(&self) -> bool {
        self.kind.is_output() && !self.target.starts_with('&')
    }

    /// Returns true if this redirect reads from a file.
    pub fn reads_from_file(&self) -> bool {
        self.kind.is_input()
            && !matches!(self.kind, RedirectKind::HereDoc | RedirectKind::HereString)
    }
}

/// Tree-sitter query for extracting redirections.
static REDIRECT_QUERY: LazyLock<Query> = LazyLock::new(|| {
    let language = tree_sitter_bash::LANGUAGE.into();
    Query::new(
        &language,
        r#"
        (file_redirect) @redirect
        (heredoc_redirect) @heredoc
        (herestring_redirect) @herestring
        "#,
    )
    .expect("valid bash query")
});

/// Extract redirections from a tree-sitter tree.
pub fn extract_redirects_from_tree(tree: &Tree, source: &str) -> Vec<Redirect> {
    let mut redirects = Vec::new();
    let root = tree.root_node();
    let bytes = source.as_bytes();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&REDIRECT_QUERY, root, bytes);

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let name = REDIRECT_QUERY.capture_names()[capture.index as usize];
            let node = capture.node;

            // Determine if this is at the top level
            let is_top_level = !is_inside_subshell(node);

            match name {
                "redirect" => {
                    if let Some(redirect) = parse_file_redirect(node, source, is_top_level) {
                        redirects.push(redirect);
                    }
                }
                "heredoc" => {
                    if let Some(redirect) = parse_heredoc_redirect(node, source, is_top_level) {
                        redirects.push(redirect);
                    }
                }
                "herestring" => {
                    if let Some(redirect) = parse_herestring_redirect(node, source, is_top_level) {
                        redirects.push(redirect);
                    }
                }
                _ => {}
            }
        }
    }

    // Sort by position
    redirects.sort_by_key(|r| r.span.start);

    redirects
}

/// Check if a node is inside a subshell.
fn is_inside_subshell(node: tree_sitter::Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), "subshell" | "command_substitution") {
            return true;
        }
        current = parent.parent();
    }
    false
}

/// Parse a file_redirect node.
fn parse_file_redirect(
    node: tree_sitter::Node,
    source: &str,
    is_top_level: bool,
) -> Option<Redirect> {
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let span = Span::new(node.start_byte() as i32, node.end_byte() as i32);

    // Parse the redirect operator and target
    let (kind, fd, target) = parse_redirect_text(text)?;

    Some(Redirect::new(kind, target, fd, span, is_top_level))
}

/// Parse redirect operator text into components.
fn parse_redirect_text(text: &str) -> Option<(RedirectKind, Option<i32>, String)> {
    let text = text.trim();

    // Check for file descriptor prefix
    let (fd, rest) = if text.chars().next()?.is_ascii_digit() {
        let fd_end = text.find(|c: char| !c.is_ascii_digit())?;
        let fd: i32 = text[..fd_end].parse().ok()?;
        (Some(fd), &text[fd_end..])
    } else {
        (None, text)
    };

    // Determine redirect kind
    let (kind, target_start) = if rest.starts_with(">>") {
        (RedirectKind::Append, 2)
    } else if rest.starts_with(">&") || rest.starts_with("<&") {
        (RedirectKind::Duplicate, 2)
    } else if rest.starts_with(">|") {
        (RedirectKind::Clobber, 2)
    } else if rest.starts_with("<>") {
        (RedirectKind::ReadWrite, 2)
    } else if rest.starts_with('>') {
        (RedirectKind::Output, 1)
    } else if rest.starts_with('<') {
        (RedirectKind::Input, 1)
    } else {
        return None;
    };

    let target = rest[target_start..].trim().to_string();

    Some((kind, fd, target))
}

/// Parse a heredoc_redirect node.
fn parse_heredoc_redirect(
    node: tree_sitter::Node,
    source: &str,
    is_top_level: bool,
) -> Option<Redirect> {
    let span = Span::new(node.start_byte() as i32, node.end_byte() as i32);

    // Find the heredoc_start child for the delimiter
    let mut cursor = node.walk();
    let mut delimiter = String::new();

    for child in node.children(&mut cursor) {
        if child.kind() == "heredoc_start" {
            delimiter = child.utf8_text(source.as_bytes()).ok()?.to_string();
            // Strip quotes from delimiter
            if delimiter.starts_with('\'') && delimiter.ends_with('\'') {
                delimiter = delimiter[1..delimiter.len() - 1].to_string();
            } else if delimiter.starts_with('"') && delimiter.ends_with('"') {
                delimiter = delimiter[1..delimiter.len() - 1].to_string();
            }
            break;
        }
    }

    Some(Redirect::new(
        RedirectKind::HereDoc,
        delimiter,
        None,
        span,
        is_top_level,
    ))
}

/// Parse a herestring_redirect node.
fn parse_herestring_redirect(
    node: tree_sitter::Node,
    source: &str,
    is_top_level: bool,
) -> Option<Redirect> {
    let span = Span::new(node.start_byte() as i32, node.end_byte() as i32);
    let text = node.utf8_text(source.as_bytes()).ok()?;

    // Extract the string after <<<
    let target = text.trim_start_matches('<').trim().to_string();

    Some(Redirect::new(
        RedirectKind::HereString,
        target,
        None,
        span,
        is_top_level,
    ))
}

/// Extract redirections from tokens (fallback).
pub fn extract_redirects_from_tokens(tokens: &[Token]) -> Vec<Redirect> {
    let mut redirects = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];
        if token.kind == TokenKind::Redirect {
            let (kind, fd) = classify_redirect_token(&token.text);

            // Look for target in next non-whitespace token
            let target = tokens[i + 1..]
                .iter()
                .find(|t| !matches!(t.kind, TokenKind::Whitespace))
                .map(|t| {
                    if matches!(t.kind, TokenKind::SingleQuoted | TokenKind::DoubleQuoted) {
                        t.unquoted_content().to_string()
                    } else {
                        t.text.clone()
                    }
                })
                .unwrap_or_default();

            redirects.push(Redirect::new(
                kind, target, fd, token.span,
                true, // Can't reliably detect subshells from tokens
            ));
        }
        i += 1;
    }

    redirects
}

/// Classify a redirect token.
fn classify_redirect_token(text: &str) -> (RedirectKind, Option<i32>) {
    let text = text.trim();

    // Check for file descriptor prefix
    let (fd, rest) = if text.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        if let Some(fd_end) = text.find(|c: char| !c.is_ascii_digit()) {
            let fd: i32 = text[..fd_end].parse().unwrap_or(0);
            (Some(fd), &text[fd_end..])
        } else {
            (None, text)
        }
    } else {
        (None, text)
    };

    let kind = match rest {
        ">>" | "&>>" => RedirectKind::Append,
        ">&" | "<&" => RedirectKind::Duplicate,
        ">|" => RedirectKind::Clobber,
        "<>" => RedirectKind::ReadWrite,
        ">" | "&>" => RedirectKind::Output,
        "<" => RedirectKind::Input,
        "<<<" => RedirectKind::HereString,
        "<<" | "<<-" => RedirectKind::HereDoc,
        _ => RedirectKind::Unknown,
    };

    (kind, fd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ShellParser;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_output_redirect() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("echo hi > output.txt");
        let redirects = extract_redirects_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].kind, RedirectKind::Output);
        assert_eq!(redirects[0].target, "output.txt");
        assert!(redirects[0].is_top_level);
    }

    #[test]
    fn test_append_redirect() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("echo hi >> output.txt");
        let redirects = extract_redirects_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].kind, RedirectKind::Append);
    }

    #[test]
    fn test_input_redirect() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("cat < input.txt");
        let redirects = extract_redirects_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].kind, RedirectKind::Input);
        assert_eq!(redirects[0].target, "input.txt");
    }

    #[test]
    fn test_fd_redirect() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("command 2>&1");
        let redirects = extract_redirects_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].kind, RedirectKind::Duplicate);
        assert_eq!(redirects[0].fd, Some(2));
    }

    #[test]
    fn test_multiple_redirects() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("command < input.txt > output.txt 2>&1");
        let redirects = extract_redirects_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(redirects.len(), 3);
    }

    #[test]
    fn test_heredoc() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("cat <<'EOF'\nhello\nEOF");
        let redirects = extract_redirects_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].kind, RedirectKind::HereDoc);
        assert_eq!(redirects[0].target, "EOF");
    }

    #[test]
    fn test_redirect_in_subshell() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("(echo hi > output.txt)");
        let redirects = extract_redirects_from_tree(cmd.tree().unwrap(), cmd.source());
        assert_eq!(redirects.len(), 1);
        assert!(!redirects[0].is_top_level);
    }

    #[test]
    fn test_writes_to_file() {
        let redirect = Redirect::new(
            RedirectKind::Output,
            "file.txt".to_string(),
            None,
            Span::new(0, 10),
            true,
        );
        assert!(redirect.writes_to_file());

        let redirect2 = Redirect::new(
            RedirectKind::Duplicate,
            "&1".to_string(),
            Some(2),
            Span::new(0, 10),
            true,
        );
        assert!(!redirect2.writes_to_file());
    }
}
