// AST parsing functionality for the approval module.
// This module parses shell scripts using the tree-sitter bash grammar and returns a full,
// un-flattened Abstract Syntax Tree.

use serde::Serialize;
use tree_sitter::Node;
use tree_sitter::Tree;

use crate::bash;

/// A serializable, lifetime-free representation of a node in the Abstract Syntax Tree.
#[derive(Debug, Serialize)]
pub struct AstNode {
    pub kind: String,
    pub text: String,
    pub children: Vec<AstNode>,
}

/// Converts a `tree_sitter::Node` into our custom `AstNode`.
/// This is a recursive function that builds the tree.
fn convert_node(node: Node, source: &str) -> AstNode {
    let mut cursor = node.walk();
    let children: Vec<AstNode> = node
        .children(&mut cursor)
        .map(|child| convert_node(child, source))
        .collect();

    AstNode {
        kind: node.kind().to_string(),
        text: node.utf8_text(source.as_bytes()).unwrap_or("").to_string(),
        children,
    }
}

/// Parses a shell script and returns a full, un-flattened Abstract Syntax Tree.
///
/// This function reuses the shared `bash::try_parse_bash` helper to obtain a
/// `tree_sitter::Tree`, then converts it into our own serializable `AstNode` structure.
///
/// # Arguments
///
/// * `script` - A string slice that holds the script to parse.
///
/// # Returns
///
/// * `Some(AstNode)` - The root of the parsed AST if parsing is successful.
/// * `None` - If `tree-sitter` fails to parse the script.
pub fn build_ast(script: &str) -> Option<AstNode> {
    let tree: Tree = bash::try_parse_bash(script)?;
    let root_node = tree.root_node();
    Some(convert_node(root_node, script))
}

#[derive(Debug, Clone)]
pub struct SimpleAst {
    /// Basename of the tool (no path; e.g., "/usr/bin/grep" → "grep"; "sudo" stripped)
    pub tool: String,
    /// First non-flag token before `--`, if any (used by WithSubcommands rules)
    pub subcommand: Option<String>,
    /// Flags before `--`, exact tokens (used by WithoutForbiddenArgs rules)
    pub flags: Vec<String>,
    /// Tokens after `--` (operands/paths/etc.)
    pub operands: Vec<String>,
    /// Original argv of this simple command (after any sudo normalization)
    pub raw: Vec<String>,
}

/// Top-level AST: a command may expand into multiple simple commands (e.g., from `bash -lc`).
#[derive(Debug, Clone)]
pub enum CommandAst {
    Sequence(Vec<SimpleAst>),
    Unknown(Vec<String>), // fall back when parsing fails
}
