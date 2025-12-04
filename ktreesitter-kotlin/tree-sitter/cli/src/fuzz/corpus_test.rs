use tree_sitter::{LogType, Node, Parser, Point, Range, Tree};

use super::{scope_sequence::ScopeSequence, LOG_ENABLED, LOG_GRAPH_ENABLED};
use crate::util;

pub fn check_consistent_sizes(tree: &Tree, input: &[u8]) {
    fn check(node: Node, line_offsets: &[usize]) {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let start_point = node.start_position();
        let end_point = node.end_position();

        assert!(start_byte <= end_byte);
        assert!(start_point <= end_point);
        assert_eq!(
            start_byte,
            line_offsets[start_point.row] + start_point.column
        );
        assert_eq!(end_byte, line_offsets[end_point.row] + end_point.column);

        let mut last_child_end_byte = start_byte;
        let mut last_child_end_point = start_point;
        let mut some_child_has_changes = false;
        let mut actual_named_child_count = 0;
        for i in 0..node.child_count() {
            let child = node.child(i).unwrap();
            assert!(child.start_byte() >= last_child_end_byte);
            assert!(child.start_position() >= last_child_end_point);
            check(child, line_offsets);
            if child.has_changes() {
                some_child_has_changes = true;
            }
            if child.is_named() {
                actual_named_child_count += 1;
            }
            last_child_end_byte = child.end_byte();
            last_child_end_point = child.end_position();
        }

        assert_eq!(actual_named_child_count, node.named_child_count());

        if node.child_count() > 0 {
            assert!(end_byte >= last_child_end_byte);
            assert!(end_point >= last_child_end_point);
        }

        if some_child_has_changes {
            assert!(node.has_changes());
        }
    }

    let mut line_offsets = vec![0];
    for (i, c) in input.iter().enumerate() {
        if *c == b'\n' {
            line_offsets.push(i + 1);
        }
    }

    check(tree.root_node(), &line_offsets);
}

pub fn check_changed_ranges(old_tree: &Tree, new_tree: &Tree, input: &[u8]) -> Result<(), String> {
    let changed_ranges = old_tree.changed_ranges(new_tree).collect::<Vec<_>>();
    let old_scope_sequence = ScopeSequence::new(old_tree);
    let new_scope_sequence = ScopeSequence::new(new_tree);

    let old_range = old_tree.root_node().range();
    let new_range = new_tree.root_node().range();

    let byte_range =
        old_range.start_byte.min(new_range.start_byte)..old_range.end_byte.max(new_range.end_byte);
    let point_range = old_range.start_point.min(new_range.start_point)
        ..old_range.end_point.max(new_range.end_point);

    for range in &changed_ranges {
        if range.end_byte > byte_range.end || range.end_point > point_range.end {
            return Err(format!(
                "changed range extends outside of the old and new trees {range:?}",
            ));
        }
    }

    old_scope_sequence.check_changes(&new_scope_sequence, input, &changed_ranges)
}

pub fn set_included_ranges(parser: &mut Parser, input: &[u8], delimiters: Option<(&str, &str)>) {
    if let Some((start, end)) = delimiters {
        let mut ranges = Vec::new();
        let mut ix = 0;
        while ix < input.len() {
            let Some(mut start_ix) = input[ix..]
                .windows(2)
                .position(|win| win == start.as_bytes())
            else {
                break;
            };
            start_ix += ix + start.len();
            let end_ix = input[start_ix..]
                .windows(2)
                .position(|win| win == end.as_bytes())
                .map_or(input.len(), |ix| start_ix + ix);
            ix = end_ix;
            ranges.push(Range {
                start_byte: start_ix,
                end_byte: end_ix,
                start_point: point_for_offset(input, start_ix),
                end_point: point_for_offset(input, end_ix),
            });
        }

        parser.set_included_ranges(&ranges).unwrap();
    } else {
        parser.set_included_ranges(&[]).unwrap();
    }
}

fn point_for_offset(text: &[u8], offset: usize) -> Point {
    let mut point = Point::default();
    for byte in &text[..offset] {
        if *byte == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    point
}

pub fn get_parser(session: &mut Option<util::LogSession>, log_filename: &str) -> Parser {
    let mut parser = Parser::new();

    if *LOG_ENABLED {
        parser.set_logger(Some(Box::new(|log_type, msg| {
            if log_type == LogType::Lex {
                eprintln!("  {msg}");
            } else {
                eprintln!("{msg}");
            }
        })));
    }
    if *LOG_GRAPH_ENABLED {
        *session = Some(util::log_graphs(&mut parser, log_filename, false).unwrap());
    }

    parser
}
