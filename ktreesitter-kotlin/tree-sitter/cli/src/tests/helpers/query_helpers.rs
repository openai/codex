use std::{cmp::Ordering, fmt::Write, ops::Range};

use rand::prelude::Rng;
use streaming_iterator::{IntoStreamingIterator, StreamingIterator};
use tree_sitter::{
    Language, Node, Parser, Point, Query, QueryCapture, QueryCursor, QueryMatch, Tree, TreeCursor,
};

#[derive(Debug)]
pub struct Pattern {
    kind: Option<&'static str>,
    named: bool,
    field: Option<&'static str>,
    capture: Option<String>,
    children: Vec<Pattern>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match<'a, 'tree> {
    pub captures: Vec<(&'a str, Node<'tree>)>,
    pub last_node: Option<Node<'tree>>,
}

const CAPTURE_NAMES: &[&str] = &[
    "one", "two", "three", "four", "five", "six", "seven", "eight",
];

impl Pattern {
    pub fn random_pattern_in_tree(tree: &Tree, rng: &mut impl Rng) -> (Self, Range<Point>) {
        let mut cursor = tree.walk();

        // Descend to the node at a random byte offset and depth.
        let mut max_depth = 0;
        let byte_offset = rng.gen_range(0..cursor.node().end_byte());
        while cursor.goto_first_child_for_byte(byte_offset).is_some() {
            max_depth += 1;
        }
        let depth = rng.gen_range(0..=max_depth);
        for _ in 0..depth {
            cursor.goto_parent();
        }

        // Build a pattern that matches that node.
        // Sometimes include subsequent siblings of the node.
        let pattern_start = cursor.node().start_position();
        let mut roots = vec![Self::random_pattern_for_node(&mut cursor, rng)];
        while roots.len() < 5 && cursor.goto_next_sibling() {
            if rng.gen_bool(0.2) {
                roots.push(Self::random_pattern_for_node(&mut cursor, rng));
            }
        }
        let pattern_end = cursor.node().end_position();

        let mut pattern = Self {
            kind: None,
            named: true,
            field: None,
            capture: None,
            children: roots,
        };

        if pattern.children.len() == 1 ||
        // In a parenthesized list of sibling patterns, the first
        // sibling can't be an anonymous `_` wildcard.
        (pattern.children[0].kind == Some("_") && !pattern.children[0].named)
        {
            pattern = pattern.children.pop().unwrap();
        }
        // In a parenthesized list of sibling patterns, the first
        // sibling can't have a field name.
        else {
            pattern.children[0].field = None;
        }

        (pattern, pattern_start..pattern_end)
    }

    fn random_pattern_for_node(cursor: &mut TreeCursor, rng: &mut impl Rng) -> Self {
        let node = cursor.node();

        // Sometimes specify the node's type, sometimes use a wildcard.
        let (kind, named) = if rng.gen_bool(0.9) {
            (Some(node.kind()), node.is_named())
        } else {
            (Some("_"), node.is_named() && rng.gen_bool(0.8))
        };

        // Sometimes specify the node's field.
        let field = if rng.gen_bool(0.75) {
            cursor.field_name()
        } else {
            None
        };

        // Sometimes capture the node.
        let capture = if rng.gen_bool(0.7) {
            Some(CAPTURE_NAMES[rng.gen_range(0..CAPTURE_NAMES.len())].to_string())
        } else {
            None
        };

        // Walk the children and include child patterns for some of them.
        let mut children = Vec::new();
        if named && cursor.goto_first_child() {
            let max_children = rng.gen_range(0..4);
            while cursor.goto_next_sibling() {
                if rng.gen_bool(0.6) {
                    let child_ast = Self::random_pattern_for_node(cursor, rng);
                    children.push(child_ast);
                    if children.len() >= max_children {
                        break;
                    }
                }
            }
            cursor.goto_parent();
        }

        Self {
            kind,
            named,
            field,
            capture,
            children,
        }
    }

    fn write_to_string(&self, string: &mut String, indent: usize) {
        if let Some(field) = self.field {
            write!(string, "{field}: ").unwrap();
        }

        if self.named {
            string.push('(');
            let mut has_contents = false;
            if let Some(kind) = &self.kind {
                write!(string, "{kind}").unwrap();
                has_contents = true;
            }
            for child in &self.children {
                let indent = indent + 2;
                if has_contents {
                    string.push('\n');
                    string.push_str(&" ".repeat(indent));
                }
                child.write_to_string(string, indent);
                has_contents = true;
            }
            string.push(')');
        } else if self.kind == Some("_") {
            string.push('_');
        } else {
            write!(string, "\"{}\"", self.kind.unwrap().replace('\"', "\\\"")).unwrap();
        }

        if let Some(capture) = &self.capture {
            write!(string, " @{capture}").unwrap();
        }
    }

    pub fn matches_in_tree<'tree>(&self, tree: &'tree Tree) -> Vec<Match<'_, 'tree>> {
        let mut matches = Vec::new();

        // Compute the matches naively: walk the tree and
        // retry the entire pattern for each node.
        let mut cursor = tree.walk();
        let mut ascending = false;
        loop {
            if ascending {
                if cursor.goto_next_sibling() {
                    ascending = false;
                } else if !cursor.goto_parent() {
                    break;
                }
            } else {
                let matches_here = self.match_node(&mut cursor);
                matches.extend_from_slice(&matches_here);
                if !cursor.goto_first_child() {
                    ascending = true;
                }
            }
        }

        matches.sort_unstable();
        matches.iter_mut().for_each(|m| m.last_node = None);
        matches.dedup();
        matches
    }

    pub fn match_node<'tree>(&self, cursor: &mut TreeCursor<'tree>) -> Vec<Match<'_, 'tree>> {
        let node = cursor.node();

        // If a kind is specified, check that it matches the node.
        if let Some(kind) = self.kind {
            if kind == "_" {
                if self.named && !node.is_named() {
                    return Vec::new();
                }
            } else if kind != node.kind() || self.named != node.is_named() {
                return Vec::new();
            }
        }

        // If a field is specified, check that it matches the node.
        if let Some(field) = self.field {
            if cursor.field_name() != Some(field) {
                return Vec::new();
            }
        }

        // Create a match for the current node.
        let mat = Match {
            captures: self
                .capture
                .as_ref()
                .map_or_else(Vec::new, |name| vec![(name.as_str(), node)]),
            last_node: Some(node),
        };

        // If there are no child patterns to match, then return this single match.
        if self.children.is_empty() {
            return vec![mat];
        }

        // Find every matching combination of child patterns and child nodes.
        let mut finished_matches = Vec::<Match>::new();
        if cursor.goto_first_child() {
            let mut match_states = vec![(0, mat)];
            loop {
                let mut new_match_states = Vec::new();
                for (pattern_index, mat) in &match_states {
                    let child_pattern = &self.children[*pattern_index];
                    let child_matches = child_pattern.match_node(cursor);
                    for child_match in child_matches {
                        let mut combined_match = mat.clone();
                        combined_match.last_node = child_match.last_node;
                        combined_match
                            .captures
                            .extend_from_slice(&child_match.captures);
                        if pattern_index + 1 < self.children.len() {
                            new_match_states.push((*pattern_index + 1, combined_match));
                        } else {
                            let mut existing = false;
                            for existing_match in &mut finished_matches {
                                if existing_match.captures == combined_match.captures {
                                    if child_pattern.capture.is_some() {
                                        existing_match.last_node = combined_match.last_node;
                                    }
                                    existing = true;
                                }
                            }
                            if !existing {
                                finished_matches.push(combined_match);
                            }
                        }
                    }
                }
                match_states.extend_from_slice(&new_match_states);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
        finished_matches
    }
}

impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();
        self.write_to_string(&mut result, 0);
        write!(f, "{result}")
    }
}

impl PartialOrd for Match<'_, '_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Match<'_, '_> {
    // Tree-sitter returns matches in the order that they terminate
    // during a depth-first walk of the tree. If multiple matches
    // terminate on the same node, those matches are produced in the
    // order that their captures were discovered.
    fn cmp(&self, other: &Self) -> Ordering {
        if let Some((last_node_a, last_node_b)) = self.last_node.zip(other.last_node) {
            let cmp = compare_depth_first(last_node_a, last_node_b);
            if cmp.is_ne() {
                return cmp;
            }
        }

        for (a, b) in self.captures.iter().zip(other.captures.iter()) {
            let cmp = compare_depth_first(a.1, b.1);
            if !cmp.is_eq() {
                return cmp;
            }
        }

        self.captures.len().cmp(&other.captures.len())
    }
}

fn compare_depth_first(a: Node, b: Node) -> Ordering {
    let a = a.byte_range();
    let b = b.byte_range();
    a.start.cmp(&b.start).then_with(|| b.end.cmp(&a.end))
}

pub fn assert_query_matches(
    language: &Language,
    query: &Query,
    source: &str,
    expected: &[(usize, Vec<(&str, &str)>)],
) {
    let mut parser = Parser::new();
    parser.set_language(language).unwrap();
    let tree = parser.parse(source, None).unwrap();
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    pretty_assertions::assert_eq!(expected, collect_matches(matches, query, source));
    pretty_assertions::assert_eq!(false, cursor.did_exceed_match_limit());
}

pub fn collect_matches<'a>(
    mut matches: impl StreamingIterator<Item = QueryMatch<'a, 'a>>,
    query: &'a Query,
    source: &'a str,
) -> Vec<(usize, Vec<(&'a str, &'a str)>)> {
    let mut result = Vec::new();
    while let Some(m) = matches.next() {
        result.push((
            m.pattern_index,
            format_captures(m.captures.iter().into_streaming_iter_ref(), query, source),
        ));
    }
    result
}

pub fn collect_captures<'a>(
    captures: impl StreamingIterator<Item = (QueryMatch<'a, 'a>, usize)>,
    query: &'a Query,
    source: &'a str,
) -> Vec<(&'a str, &'a str)> {
    format_captures(captures.map(|(m, i)| m.captures[*i]), query, source)
}

fn format_captures<'a>(
    mut captures: impl StreamingIterator<Item = QueryCapture<'a>>,
    query: &'a Query,
    source: &'a str,
) -> Vec<(&'a str, &'a str)> {
    let mut result = Vec::new();
    while let Some(capture) = captures.next() {
        result.push((
            query.capture_names()[capture.index as usize],
            capture.node.utf8_text(source.as_bytes()).unwrap(),
        ));
    }
    result
}
