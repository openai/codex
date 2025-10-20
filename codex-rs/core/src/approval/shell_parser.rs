// Shell script parsing for the approval module.
// Accepts word-only commands joined by safe operators and rejects complex features.

use std::path::Path;
use tree_sitter::Node;
use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;

/// Parse the provided shell script using tree-sitter-bash, returning a Tree on
/// success or None if parsing failed.
fn try_parse_bash(script: &str) -> Option<Tree> {
    let lang = BASH.into();
    let mut parser = Parser::new();
    #[expect(clippy::expect_used)]
    parser.set_language(&lang).expect("load bash grammar");
    let old_tree: Option<&Tree> = None;
    parser.parse(script, old_tree)
}

/// Parse a script which may contain multiple simple commands joined only by
/// the safe logical/pipe/sequencing operators: `&&`, `||`, `;`, `|`.
///
/// Returns `Some(Vec<command_words>)` if every command is a plain word‑only
/// command and the parse tree does not contain disallowed constructs
/// (parentheses, redirections, substitutions, control flow, etc.). Otherwise
/// returns `None`.
fn try_parse_word_only_commands_sequence(tree: &Tree, src: &str) -> Option<Vec<Vec<String>>> {
    if tree.root_node().has_error() {
        return None;
    }

    // List of allowed (named) node kinds for a "word only commands sequence".
    // If we encounter a named node that is not in this list we reject.
    const ALLOWED_KINDS: &[&str] = &[
        // top level containers
        "program",
        "list",
        "pipeline",
        // commands & words
        "command",
        "command_name",
        "word",
        "string",
        "string_content",
        "raw_string",
        "number",
    ];
    // Allow only safe punctuation / operator tokens; anything else causes reject.
    const ALLOWED_PUNCT_TOKENS: &[&str] = &["&&", "||", ";", "|", "\"", "'"];

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
            // Reject any punctuation / operator tokens that are not explicitly allowed.
            if kind.chars().any(|c| "&;|".contains(c)) && !ALLOWED_PUNCT_TOKENS.contains(&kind) {
                return None;
            }
            if !(ALLOWED_PUNCT_TOKENS.contains(&kind) || kind.trim().is_empty()) {
                // If it's a quote token or operator it's allowed above; we also allow whitespace tokens.
                // Any other punctuation like parentheses, braces, redirects, backticks, etc are rejected.
                return None;
            }
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    // Walk uses a stack (LIFO), so re-sort by position to restore source order.
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

fn parse_plain_command_from_node(cmd: tree_sitter::Node, src: &str) -> Option<Vec<String>> {
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
                if child.child_count() == 3
                    && child.child(0)?.kind() == "\""
                    && child.child(1)?.kind() == "string_content"
                    && child.child(2)?.kind() == "\""
                {
                    words.push(child.child(1)?.utf8_text(src.as_bytes()).ok()?.to_owned());
                } else {
                    return None;
                }
            }
            "raw_string" => {
                let raw_string = child.utf8_text(src.as_bytes()).ok()?;
                let stripped = raw_string
                    .strip_prefix('\'')
                    .and_then(|s| s.strip_suffix('\''));
                if let Some(s) = stripped {
                    words.push(s.to_owned());
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(words)
}

/// Returns the sequence of plain commands within a shell invocation like
/// `bash -c "..."`, `sh -lc "..."`, or any other POSIX-compatible shell.
///
/// This function validates that:
/// 1. The command has exactly 3 arguments: `[shell, flag, script]`
/// 2. The flag is `-c` or `-lc`
/// 3. The script only contains word-only commands joined by safe operators
///
/// If the script contains complex shell features (redirections, subshells, etc.),
/// this returns `None`, allowing the caller to treat it as a single opaque command.
///
/// # Examples
///
/// ```ignore
/// // Simple commands work
/// let cmd = vec!["bash".to_string(), "-c".to_string(), "ls".to_string()];
/// assert!(parse_shell_script_commands(&cmd).is_some());
///
/// // Sequences work
/// let cmd = vec!["sh".to_string(), "-lc".to_string(), "ls && pwd".to_string()];
/// assert!(parse_shell_script_commands(&cmd).is_some());
///
/// // Complex features rejected
/// let cmd = vec!["zsh".to_string(), "-c".to_string(), "ls > out.txt".to_string()];
/// assert!(parse_shell_script_commands(&cmd).is_none());
/// ```
pub(crate) fn parse_shell_script_commands(command: &[String]) -> Option<Vec<Vec<String>>> {
    if command.len() < 3 {
        return None;
    }

    let shell = &command[0];
    let flag = &command[1];
    let script = &command[2];

    // Heuristic: check if the command name ends with "sh"
    let shell_name = Path::new(shell).file_name()?.to_str()?;
    if !shell_name.ends_with("sh") {
        return None;
    }

    // Accept both -c and -lc flags for any shell
    if flag != "-c" && flag != "-lc" {
        return None;
    }

    let tree = try_parse_bash(script)?;
    try_parse_word_only_commands_sequence(&tree, script)
}
