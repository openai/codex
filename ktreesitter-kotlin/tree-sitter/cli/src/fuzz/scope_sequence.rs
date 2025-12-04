use tree_sitter::{Point, Range, Tree};

#[derive(Debug)]
pub struct ScopeSequence(Vec<ScopeStack>);

type ScopeStack = Vec<&'static str>;

impl ScopeSequence {
    #[must_use]
    pub fn new(tree: &Tree) -> Self {
        let mut result = Self(Vec::new());
        let mut scope_stack = Vec::new();

        let mut cursor = tree.walk();
        let mut visited_children = false;
        loop {
            let node = cursor.node();
            for _ in result.0.len()..node.start_byte() {
                result.0.push(scope_stack.clone());
            }
            if visited_children {
                for _ in result.0.len()..node.end_byte() {
                    result.0.push(scope_stack.clone());
                }
                scope_stack.pop();
                if cursor.goto_next_sibling() {
                    visited_children = false;
                } else if !cursor.goto_parent() {
                    break;
                }
            } else {
                scope_stack.push(cursor.node().kind());
                if !cursor.goto_first_child() {
                    visited_children = true;
                }
            }
        }

        result
    }

    pub fn check_changes(
        &self,
        other: &Self,
        text: &[u8],
        known_changed_ranges: &[Range],
    ) -> Result<(), String> {
        let mut position = Point { row: 0, column: 0 };
        for i in 0..(self.0.len().max(other.0.len())) {
            let stack = &self.0.get(i);
            let other_stack = &other.0.get(i);
            if *stack != *other_stack && ![b'\r', b'\n'].contains(&text[i]) {
                let containing_range = known_changed_ranges
                    .iter()
                    .find(|range| range.start_point <= position && position < range.end_point);
                if containing_range.is_none() {
                    let line = &text[(i - position.column)..]
                        .split(|c| *c == b'\n')
                        .next()
                        .unwrap();
                    return Err(format!(
                        concat!(
                            "Position: {}\n",
                            "Byte offset: {}\n",
                            "Line: {}\n",
                            "{}^\n",
                            "Old scopes: {:?}\n",
                            "New scopes: {:?}\n",
                            "Invalidated ranges: {:?}",
                        ),
                        position,
                        i,
                        String::from_utf8_lossy(line),
                        String::from(" ").repeat(position.column + "Line: ".len()),
                        stack,
                        other_stack,
                        known_changed_ranges,
                    ));
                }
            }

            if text[i] == b'\n' {
                position.row += 1;
                position.column = 0;
            } else {
                position.column += 1;
            }
        }
        Ok(())
    }
}
