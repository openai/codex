//! Tree-sitter based shell parser with tokenizer fallback.

use std::path::Path;

use tree_sitter::Language;
use tree_sitter::Node;
use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;

use crate::tokenizer::Token;
use crate::tokenizer::TokenKind;
use crate::tokenizer::Tokenizer;

/// Get the Bash language for tree-sitter.
fn get_bash_language() -> Language {
    BASH.into()
}

/// Allowed node kinds for "safe" word-only commands (whitelist approach from bash.rs).
const ALLOWED_KINDS: &[&str] = &[
    "program",
    "list",
    "pipeline",
    "command",
    "command_name",
    "word",
    "string",
    "string_content",
    "raw_string",
    "number",
    "concatenation",
];

/// Safe punctuation/operator tokens.
const ALLOWED_PUNCT_TOKENS: &[&str] = &["&&", "||", ";", "|", "\"", "'"];

/// A parsed shell command.
#[derive(Debug)]
pub struct ParsedCommand {
    /// The original source string.
    source: String,
    /// Tree-sitter parse tree (if AST parsing succeeded).
    tree: Option<Tree>,
    /// Fallback tokens (used when AST parsing fails).
    tokens: Vec<Token>,
    /// Whether the command has syntax errors.
    has_errors: bool,
}

impl ParsedCommand {
    /// Returns the original source string.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns true if AST parsing succeeded.
    pub fn has_tree(&self) -> bool {
        self.tree.is_some()
    }

    /// Returns the tree-sitter tree if available.
    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    /// Returns the fallback tokens.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns true if the command has syntax errors.
    pub fn has_errors(&self) -> bool {
        self.has_errors
    }

    /// Extract plain commands from the parsed command.
    ///
    /// Returns `Some(commands)` if the command consists only of safe, word-only
    /// commands joined by safe operators (&&, ||, ;, |). Returns `None` if the
    /// command contains dangerous constructs (subshells, redirections, etc.).
    pub fn try_extract_safe_commands(&self) -> Option<Vec<Vec<String>>> {
        let tree = self.tree.as_ref()?;
        if tree.root_node().has_error() {
            return None;
        }
        try_parse_word_only_commands_sequence(tree, &self.source)
    }

    /// Extract all command words from the parsed command.
    ///
    /// Unlike `try_extract_safe_commands`, this extracts words from all commands
    /// regardless of safety, useful for security analysis.
    pub fn extract_commands(&self) -> Vec<Vec<String>> {
        if let Some(tree) = &self.tree {
            extract_all_commands(tree, &self.source)
        } else {
            // Fallback to token-based extraction
            extract_commands_from_tokens(&self.tokens)
        }
    }

    /// Check if the command is a simple "word only" command sequence.
    pub fn is_safe_command_sequence(&self) -> bool {
        self.try_extract_safe_commands().is_some()
    }
}

/// Shell command parser using tree-sitter with tokenizer fallback.
pub struct ShellParser {
    parser: Parser,
    tokenizer: Tokenizer,
}

impl Default for ShellParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellParser {
    /// Create a new shell parser.
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&get_bash_language())
            .expect("load bash grammar");
        Self {
            parser,
            tokenizer: Tokenizer::new(),
        }
    }

    /// Parse a shell command string.
    pub fn parse(&mut self, source: &str) -> ParsedCommand {
        let tree = self.parser.parse(source, None);
        let has_errors = tree.as_ref().is_some_and(|t| t.root_node().has_error());

        // Always tokenize for fallback and security analysis
        let tokens = self.tokenizer.tokenize(source).unwrap_or_default();

        ParsedCommand {
            source: source.to_string(),
            tree,
            tokens,
            has_errors,
        }
    }

    /// Parse a shell invocation from argv (e.g., ["bash", "-c", "command"]).
    ///
    /// Returns `Some(ParsedCommand)` if the argv represents a shell invocation
    /// with an embedded script, `None` otherwise.
    pub fn parse_shell_invocation(&mut self, argv: &[String]) -> Option<ParsedCommand> {
        let (_, script) = extract_shell_script(argv)?;
        Some(self.parse(script))
    }
}

/// Detect shell type from shell path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Sh,
    PowerShell,
    Cmd,
    Unknown,
}

/// Detect shell type from a path.
pub fn detect_shell_type(path: &Path) -> ShellType {
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .map(|s| s.to_ascii_lowercase());

    match name.as_deref() {
        Some("bash") => ShellType::Bash,
        Some("zsh") => ShellType::Zsh,
        Some("sh") => ShellType::Sh,
        Some("pwsh" | "powershell") => ShellType::PowerShell,
        Some("cmd") => ShellType::Cmd,
        _ => ShellType::Unknown,
    }
}

/// Extract shell script from argv.
///
/// Returns `Some((shell_type, script))` if argv is a shell invocation.
pub fn extract_shell_script(argv: &[String]) -> Option<(ShellType, &str)> {
    match argv {
        [shell, flag, script] => {
            let shell_type = detect_shell_type(Path::new(shell));
            let valid = match shell_type {
                ShellType::Bash | ShellType::Zsh | ShellType::Sh => {
                    matches!(flag.as_str(), "-c" | "-lc")
                }
                ShellType::PowerShell => flag.eq_ignore_ascii_case("-command"),
                ShellType::Cmd => flag.eq_ignore_ascii_case("/c"),
                ShellType::Unknown => false,
            };
            if valid {
                Some((shell_type, script.as_str()))
            } else {
                None
            }
        }
        [shell, skip_flag, flag, script] => {
            let shell_type = detect_shell_type(Path::new(shell));
            // Handle PowerShell -NoProfile flag
            if shell_type == ShellType::PowerShell
                && skip_flag.eq_ignore_ascii_case("-noprofile")
                && flag.eq_ignore_ascii_case("-command")
            {
                Some((shell_type, script.as_str()))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse word-only commands sequence (from bash.rs).
fn try_parse_word_only_commands_sequence(tree: &Tree, src: &str) -> Option<Vec<Vec<String>>> {
    if tree.root_node().has_error() {
        return None;
    }

    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut command_nodes = Vec::new();

    while let Some(node) = stack.pop() {
        let kind = node.kind();
        if node.is_named() {
            if !ALLOWED_KINDS.contains(&kind) {
                return None;
            }
            if kind == "command" {
                command_nodes.push(node);
            }
        } else {
            // Reject any punctuation/operator tokens not explicitly allowed
            if kind.chars().any(|c| "&;|".contains(c)) && !ALLOWED_PUNCT_TOKENS.contains(&kind) {
                return None;
            }
            if !(ALLOWED_PUNCT_TOKENS.contains(&kind) || kind.trim().is_empty()) {
                return None;
            }
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    // Sort by position to restore source order
    command_nodes.sort_by_key(Node::start_byte);

    let mut commands = Vec::new();
    for node in command_nodes {
        if let Some(words) = parse_plain_command_from_node(node, src) {
            commands.push(words);
        } else {
            return None;
        }
    }
    Some(commands)
}

/// Parse a plain command node into words.
fn parse_plain_command_from_node(cmd: Node, src: &str) -> Option<Vec<String>> {
    if cmd.kind() != "command" {
        return None;
    }
    let mut words = Vec::new();
    let mut cursor = cmd.walk();

    for child in cmd.named_children(&mut cursor) {
        match child.kind() {
            "command_name" => {
                let word_node = child.named_child(0)?;
                if word_node.kind() != "word" {
                    return None;
                }
                words.push(word_node.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "word" | "number" => {
                words.push(child.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "string" => {
                let parsed = parse_double_quoted_string(child, src)?;
                words.push(parsed);
            }
            "raw_string" => {
                let parsed = parse_raw_string(child, src)?;
                words.push(parsed);
            }
            "concatenation" => {
                let mut concatenated = String::new();
                let mut concat_cursor = child.walk();
                for part in child.named_children(&mut concat_cursor) {
                    match part.kind() {
                        "word" | "number" => {
                            concatenated.push_str(part.utf8_text(src.as_bytes()).ok()?);
                        }
                        "string" => {
                            let parsed = parse_double_quoted_string(part, src)?;
                            concatenated.push_str(&parsed);
                        }
                        "raw_string" => {
                            let parsed = parse_raw_string(part, src)?;
                            concatenated.push_str(&parsed);
                        }
                        _ => return None,
                    }
                }
                if concatenated.is_empty() {
                    return None;
                }
                words.push(concatenated);
            }
            _ => return None,
        }
    }
    Some(words)
}

/// Parse double-quoted string content.
fn parse_double_quoted_string(node: Node, src: &str) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }

    let mut cursor = node.walk();
    for part in node.named_children(&mut cursor) {
        if part.kind() != "string_content" {
            return None;
        }
    }
    let raw = node.utf8_text(src.as_bytes()).ok()?;
    let stripped = raw
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))?;
    Some(stripped.to_string())
}

/// Parse raw (single-quoted) string content.
fn parse_raw_string(node: Node, src: &str) -> Option<String> {
    if node.kind() != "raw_string" {
        return None;
    }

    let raw_string = node.utf8_text(src.as_bytes()).ok()?;
    let stripped = raw_string
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''));
    stripped.map(str::to_owned)
}

/// Extract all commands from a tree (not just safe ones).
fn extract_all_commands(tree: &Tree, src: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if node.kind() == "command" {
            if let Some(words) = extract_command_words(node, src) {
                commands.push(words);
            }
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    // Sort by position
    commands.reverse();
    commands
}

/// Extract words from a command node (relaxed version).
fn extract_command_words(cmd: Node, src: &str) -> Option<Vec<String>> {
    if cmd.kind() != "command" {
        return None;
    }
    let mut words = Vec::new();
    let mut cursor = cmd.walk();

    for child in cmd.named_children(&mut cursor) {
        if let Some(text) = extract_word_text(child, src) {
            words.push(text);
        }
    }

    if words.is_empty() { None } else { Some(words) }
}

/// Extract text from a word-like node.
fn extract_word_text(node: Node, src: &str) -> Option<String> {
    match node.kind() {
        "command_name" => {
            let child = node.named_child(0)?;
            extract_word_text(child, src)
        }
        "word" | "number" => node.utf8_text(src.as_bytes()).ok().map(String::from),
        "string" => {
            let raw = node.utf8_text(src.as_bytes()).ok()?;
            raw.strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .map(String::from)
        }
        "raw_string" => {
            let raw = node.utf8_text(src.as_bytes()).ok()?;
            raw.strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .map(String::from)
        }
        "concatenation" => {
            let mut result = String::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(text) = extract_word_text(child, src) {
                    result.push_str(&text);
                }
            }
            if result.is_empty() {
                None
            } else {
                Some(result)
            }
        }
        _ => None,
    }
}

/// Extract commands from tokens (fallback).
fn extract_commands_from_tokens(tokens: &[Token]) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut current_cmd = Vec::new();

    for token in tokens {
        match token.kind {
            TokenKind::Word | TokenKind::SingleQuoted | TokenKind::DoubleQuoted => {
                current_cmd.push(token.unquoted_content().to_string());
            }
            TokenKind::Operator => {
                if !current_cmd.is_empty() {
                    commands.push(std::mem::take(&mut current_cmd));
                }
            }
            TokenKind::Whitespace | TokenKind::Comment => {}
            _ => {
                // Include variable expansions, substitutions as-is for analysis
                current_cmd.push(token.text.clone());
            }
        }
    }

    if !current_cmd.is_empty() {
        commands.push(current_cmd);
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_simple_command() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("ls -la");
        assert!(cmd.has_tree());
        assert!(!cmd.has_errors());
    }

    #[test]
    fn test_extract_safe_commands() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("ls -la && pwd");
        let commands = cmd.try_extract_safe_commands().unwrap();
        assert_eq!(commands, vec![vec!["ls", "-la"], vec!["pwd"]]);
    }

    #[test]
    fn test_extract_piped_commands() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("cat file | grep pattern | wc -l");
        let commands = cmd.try_extract_safe_commands().unwrap();
        assert_eq!(
            commands,
            vec![
                vec!["cat", "file"],
                vec!["grep", "pattern"],
                vec!["wc", "-l"]
            ]
        );
    }

    #[test]
    fn test_reject_redirections() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("echo hi > output.txt");
        assert!(cmd.try_extract_safe_commands().is_none());
    }

    #[test]
    fn test_reject_subshells() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("(ls && pwd)");
        assert!(cmd.try_extract_safe_commands().is_none());
    }

    #[test]
    fn test_reject_command_substitution() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("echo $(pwd)");
        assert!(cmd.try_extract_safe_commands().is_none());
    }

    #[test]
    fn test_reject_variable_expansion() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("echo $HOME");
        assert!(cmd.try_extract_safe_commands().is_none());
    }

    #[test]
    fn test_extract_commands_unsafe() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("echo $HOME && ls");
        // extract_commands works even for unsafe commands
        let commands = cmd.extract_commands();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_parse_shell_invocation() {
        let mut parser = ShellParser::new();
        let argv = vec!["bash".to_string(), "-c".to_string(), "ls -la".to_string()];
        let cmd = parser.parse_shell_invocation(&argv).unwrap();
        let commands = cmd.try_extract_safe_commands().unwrap();
        assert_eq!(commands, vec![vec!["ls", "-la"]]);
    }

    #[test]
    fn test_detect_shell_type() {
        assert_eq!(detect_shell_type(Path::new("/bin/bash")), ShellType::Bash);
        assert_eq!(detect_shell_type(Path::new("/usr/bin/zsh")), ShellType::Zsh);
        assert_eq!(detect_shell_type(Path::new("sh")), ShellType::Sh);
        assert_eq!(
            detect_shell_type(Path::new("powershell.exe")),
            ShellType::PowerShell
        );
        assert_eq!(detect_shell_type(Path::new("cmd.exe")), ShellType::Cmd);
    }

    #[test]
    fn test_quoted_strings() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("echo 'hello world' \"foo bar\"");
        let commands = cmd.try_extract_safe_commands().unwrap();
        assert_eq!(commands, vec![vec!["echo", "hello world", "foo bar"]]);
    }

    #[test]
    fn test_concatenated_args() {
        let mut parser = ShellParser::new();
        let cmd = parser.parse("rg -g\"*.py\" pattern");
        let commands = cmd.try_extract_safe_commands().unwrap();
        assert_eq!(commands, vec![vec!["rg", "-g*.py", "pattern"]]);
    }
}
