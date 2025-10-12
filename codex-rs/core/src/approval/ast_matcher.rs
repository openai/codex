/*AST traversal and matching helpers.

This module provides functions to query and match patterns against an `AstNode` tree,
making the rule-matching logic more robust and less dependent on argument ordering.
*/

use super::ast::AstNode;

/// Recursively finds the first descendant node (including the starting node)
/// that has the specified kind.
///
/// # Arguments
/// * `node` - The `AstNode` to start the search from.
/// * `kind` - The `kind` of the node to find.
///
/// # Returns
/// * `Some(&AstNode)` if a matching node is found.
/// * `None` if no matching node is found.
pub fn find_descendant_by_kind<'a>(node: &'a AstNode, kind: &str) -> Option<&'a AstNode> {
    if node.kind == kind {
        return Some(node);
    }

    for child in &node.children {
        if let Some(found) = find_descendant_by_kind(child, kind) {
            return Some(found);
        }
    }

    None
}

/// Recursively collects all descendant nodes (including the starting node)
/// that have the specified kind.
///
/// # Arguments
/// * `node` - The `AstNode` to start the search from.
/// * `kind` - The `kind` of the nodes to collect.
///
/// # Returns
/// * A `Vec<&AstNode>` containing all matching nodes.
pub fn collect_descendants_by_kind<'a>(node: &'a AstNode, kind: &str) -> Vec<&'a AstNode> {
    let mut found_nodes = Vec::new();
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        if current.kind == kind {
            found_nodes.push(current);
        }
        stack.extend(current.children.iter());
    }

    found_nodes
}
