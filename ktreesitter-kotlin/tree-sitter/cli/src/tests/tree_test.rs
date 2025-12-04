use std::str;

use tree_sitter::{InputEdit, Parser, Point, Range, Tree};

use super::helpers::fixtures::get_language;
use crate::{
    fuzz::edits::Edit,
    parse::perform_edit,
    tests::{helpers::fixtures::get_test_fixture_language, invert_edit},
};

#[test]
fn test_tree_edit() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("  abc  !==  def", None).unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        "(program (expression_statement (binary_expression left: (identifier) right: (identifier))))"
    );

    // edit entirely within the tree's padding:
    // resize the padding of the tree and its leftmost descendants.
    {
        let mut tree = tree.clone();
        tree.edit(&InputEdit {
            start_byte: 1,
            old_end_byte: 1,
            new_end_byte: 2,
            start_position: Point::new(0, 1),
            old_end_position: Point::new(0, 1),
            new_end_position: Point::new(0, 2),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();

        assert!(expr.has_changes());
        assert_eq!(expr.start_byte(), 3);
        assert_eq!(expr.end_byte(), 16);
        assert!(child1.has_changes());
        assert_eq!(child1.start_byte(), 3);
        assert_eq!(child1.end_byte(), 6);
        assert!(!child2.has_changes());
        assert_eq!(child2.start_byte(), 8);
        assert_eq!(child2.end_byte(), 11);
    }

    // edit starting in the tree's padding but extending into its content:
    // shrink the content to compensate for the expanded padding.
    {
        let mut tree = tree.clone();
        tree.edit(&InputEdit {
            start_byte: 1,
            old_end_byte: 4,
            new_end_byte: 5,
            start_position: Point::new(0, 1),
            old_end_position: Point::new(0, 5),
            new_end_position: Point::new(0, 5),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();

        assert!(expr.has_changes());
        assert_eq!(expr.start_byte(), 5);
        assert_eq!(expr.end_byte(), 16);
        assert!(child1.has_changes());
        assert_eq!(child1.start_byte(), 5);
        assert_eq!(child1.end_byte(), 6);
        assert!(!child2.has_changes());
        assert_eq!(child2.start_byte(), 8);
        assert_eq!(child2.end_byte(), 11);
    }

    // insertion at the edge of a tree's padding:
    // expand the tree's padding.
    {
        let mut tree = tree.clone();
        tree.edit(&InputEdit {
            start_byte: 2,
            old_end_byte: 2,
            new_end_byte: 4,
            start_position: Point::new(0, 2),
            old_end_position: Point::new(0, 2),
            new_end_position: Point::new(0, 4),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();

        assert!(expr.has_changes());
        assert_eq!(expr.byte_range(), 4..17);
        assert!(child1.has_changes());
        assert_eq!(child1.byte_range(), 4..7);
        assert!(!child2.has_changes());
        assert_eq!(child2.byte_range(), 9..12);
    }

    // replacement starting at the edge of the tree's padding:
    // resize the content and not the padding.
    {
        let mut tree = tree.clone();
        tree.edit(&InputEdit {
            start_byte: 2,
            old_end_byte: 2,
            new_end_byte: 4,
            start_position: Point::new(0, 2),
            old_end_position: Point::new(0, 2),
            new_end_position: Point::new(0, 4),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();

        assert!(expr.has_changes());
        assert_eq!(expr.byte_range(), 4..17);
        assert!(child1.has_changes());
        assert_eq!(child1.byte_range(), 4..7);
        assert!(!child2.has_changes());
        assert_eq!(child2.byte_range(), 9..12);
    }

    // deletion that spans more than one child node:
    // shrink subsequent child nodes.
    {
        let mut tree = tree.clone();
        tree.edit(&InputEdit {
            start_byte: 1,
            old_end_byte: 11,
            new_end_byte: 4,
            start_position: Point::new(0, 1),
            old_end_position: Point::new(0, 11),
            new_end_position: Point::new(0, 4),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();
        let child3 = expr.child(2).unwrap();

        assert!(expr.has_changes());
        assert_eq!(expr.byte_range(), 4..8);
        assert!(child1.has_changes());
        assert_eq!(child1.byte_range(), 4..4);
        assert!(child2.has_changes());
        assert_eq!(child2.byte_range(), 4..4);
        assert!(child3.has_changes());
        assert_eq!(child3.byte_range(), 5..8);
    }

    // insertion at the end of the tree:
    // extend the tree's content.
    {
        let mut tree = tree.clone();
        tree.edit(&InputEdit {
            start_byte: 15,
            old_end_byte: 15,
            new_end_byte: 16,
            start_position: Point::new(0, 15),
            old_end_position: Point::new(0, 15),
            new_end_position: Point::new(0, 16),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();
        let child3 = expr.child(2).unwrap();

        assert!(expr.has_changes());
        assert_eq!(expr.byte_range(), 2..16);
        assert!(!child1.has_changes());
        assert_eq!(child1.byte_range(), 2..5);
        assert!(!child2.has_changes());
        assert_eq!(child2.byte_range(), 7..10);
        assert!(child3.has_changes());
        assert_eq!(child3.byte_range(), 12..16);
    }

    // replacement that starts within a token and extends beyond the end of the tree:
    // resize the token and empty out any subsequent child nodes.
    {
        let mut tree = tree.clone();
        tree.edit(&InputEdit {
            start_byte: 3,
            old_end_byte: 90,
            new_end_byte: 4,
            start_position: Point::new(0, 3),
            old_end_position: Point::new(0, 90),
            new_end_position: Point::new(0, 4),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();
        let child3 = expr.child(2).unwrap();
        assert_eq!(expr.byte_range(), 2..4);
        assert!(expr.has_changes());
        assert_eq!(child1.byte_range(), 2..4);
        assert!(child1.has_changes());
        assert_eq!(child2.byte_range(), 4..4);
        assert!(child2.has_changes());
        assert_eq!(child3.byte_range(), 4..4);
        assert!(child3.has_changes());
    }

    // replacement that starts in whitespace and extends beyond the end of the tree:
    // shift the token's start position and empty out its content.
    {
        let mut tree = tree;
        tree.edit(&InputEdit {
            start_byte: 6,
            old_end_byte: 90,
            new_end_byte: 8,
            start_position: Point::new(0, 6),
            old_end_position: Point::new(0, 90),
            new_end_position: Point::new(0, 8),
        });

        let expr = tree.root_node().child(0).unwrap().child(0).unwrap();
        let child1 = expr.child(0).unwrap();
        let child2 = expr.child(1).unwrap();
        let child3 = expr.child(2).unwrap();
        assert_eq!(expr.byte_range(), 2..8);
        assert!(expr.has_changes());
        assert_eq!(child1.byte_range(), 2..5);
        assert!(!child1.has_changes());
        assert_eq!(child2.byte_range(), 8..8);
        assert!(child2.has_changes());
        assert_eq!(child3.byte_range(), 8..8);
        assert!(child3.has_changes());
    }
}

#[test]
fn test_tree_edit_with_included_ranges() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("html")).unwrap();

    let source = "<div><% if a %><span>a</span><% else %><span>b</span><% end %></div>";

    let ranges = [0..5, 15..29, 39..53, 62..68];

    parser
        .set_included_ranges(
            &ranges
                .iter()
                .map(|range| Range {
                    start_byte: range.start,
                    end_byte: range.end,
                    start_point: Point::new(0, range.start),
                    end_point: Point::new(0, range.end),
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();

    let mut tree = parser.parse(source, None).unwrap();

    tree.edit(&InputEdit {
        start_byte: 29,
        old_end_byte: 53,
        new_end_byte: 29,
        start_position: Point::new(0, 29),
        old_end_position: Point::new(0, 53),
        new_end_position: Point::new(0, 29),
    });

    assert_eq!(
        tree.included_ranges(),
        &[
            Range {
                start_byte: 0,
                end_byte: 5,
                start_point: Point::new(0, 0),
                end_point: Point::new(0, 5),
            },
            Range {
                start_byte: 15,
                end_byte: 29,
                start_point: Point::new(0, 15),
                end_point: Point::new(0, 29),
            },
            Range {
                start_byte: 29,
                end_byte: 29,
                start_point: Point::new(0, 29),
                end_point: Point::new(0, 29),
            },
            Range {
                start_byte: 38,
                end_byte: 44,
                start_point: Point::new(0, 38),
                end_point: Point::new(0, 44),
            }
        ]
    );
}

#[test]
fn test_tree_cursor() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let tree = parser
        .parse(
            "
                struct Stuff {
                    a: A,
                    b: Option<B>,
                }
            ",
            None,
        )
        .unwrap();

    let mut cursor = tree.walk();
    assert_eq!(cursor.node().kind(), "source_file");

    assert!(cursor.goto_first_child());
    assert_eq!(cursor.node().kind(), "struct_item");

    assert!(cursor.goto_first_child());
    assert_eq!(cursor.node().kind(), "struct");
    assert!(!cursor.node().is_named());

    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind(), "type_identifier");
    assert!(cursor.node().is_named());

    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind(), "field_declaration_list");
    assert!(cursor.node().is_named());

    assert!(cursor.goto_last_child());
    assert_eq!(cursor.node().kind(), "}");
    assert!(!cursor.node().is_named());
    assert_eq!(cursor.node().start_position(), Point { row: 4, column: 16 });

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), ",");
    assert!(!cursor.node().is_named());
    assert_eq!(cursor.node().start_position(), Point { row: 3, column: 32 });

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), "field_declaration");
    assert!(cursor.node().is_named());
    assert_eq!(cursor.node().start_position(), Point { row: 3, column: 20 });

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), ",");
    assert!(!cursor.node().is_named());
    assert_eq!(cursor.node().start_position(), Point { row: 2, column: 24 });

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), "field_declaration");
    assert!(cursor.node().is_named());
    assert_eq!(cursor.node().start_position(), Point { row: 2, column: 20 });

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), "{");
    assert!(!cursor.node().is_named());
    assert_eq!(cursor.node().start_position(), Point { row: 1, column: 29 });

    let mut copy = tree.walk();
    copy.reset_to(&cursor);

    assert_eq!(copy.node().kind(), "{");
    assert!(!copy.node().is_named());

    assert!(copy.goto_parent());
    assert_eq!(copy.node().kind(), "field_declaration_list");
    assert!(copy.node().is_named());

    assert!(copy.goto_parent());
    assert_eq!(copy.node().kind(), "struct_item");
}

#[test]
fn test_tree_cursor_previous_sibling_with_aliases() {
    let mut parser = Parser::new();
    parser
        .set_language(&get_test_fixture_language("aliases_in_root"))
        .unwrap();

    let text = "# comment\n# \nfoo foo";
    let tree = parser.parse(text, None).unwrap();
    let mut cursor = tree.walk();
    assert_eq!(cursor.node().kind(), "document");

    cursor.goto_first_child();
    assert_eq!(cursor.node().kind(), "comment");

    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind(), "comment");

    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind(), "bar");

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), "comment");

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), "comment");

    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind(), "comment");

    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind(), "bar");
}

#[test]
fn test_tree_cursor_previous_sibling() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let text = "
    // Hi there
    // This is fun!
    // Another one!
";
    let tree = parser.parse(text, None).unwrap();

    let mut cursor = tree.walk();
    assert_eq!(cursor.node().kind(), "source_file");

    assert!(cursor.goto_last_child());
    assert_eq!(cursor.node().kind(), "line_comment");
    assert_eq!(
        cursor.node().utf8_text(text.as_bytes()).unwrap(),
        "// Another one!"
    );

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), "line_comment");
    assert_eq!(
        cursor.node().utf8_text(text.as_bytes()).unwrap(),
        "// This is fun!"
    );

    assert!(cursor.goto_previous_sibling());
    assert_eq!(cursor.node().kind(), "line_comment");
    assert_eq!(
        cursor.node().utf8_text(text.as_bytes()).unwrap(),
        "// Hi there"
    );

    assert!(!cursor.goto_previous_sibling());
}

#[test]
fn test_tree_cursor_fields() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();

    let tree = parser
        .parse("function /*1*/ bar /*2*/ () {}", None)
        .unwrap();

    let mut cursor = tree.walk();
    assert_eq!(cursor.node().kind(), "program");

    cursor.goto_first_child();
    assert_eq!(cursor.node().kind(), "function_declaration");
    assert_eq!(cursor.field_name(), None);

    cursor.goto_first_child();
    assert_eq!(cursor.node().kind(), "function");
    assert_eq!(cursor.field_name(), None);

    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "comment");
    assert_eq!(cursor.field_name(), None);

    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "identifier");
    assert_eq!(cursor.field_name(), Some("name"));

    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "comment");
    assert_eq!(cursor.field_name(), None);

    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "formal_parameters");
    assert_eq!(cursor.field_name(), Some("parameters"));
}

#[test]
fn test_tree_cursor_child_for_point() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let source = &"
    [
        one,
        {
            two: tree
        },
        four, five, six
    ];"[1..];
    let tree = parser.parse(source, None).unwrap();

    let mut c = tree.walk();
    assert_eq!(c.node().kind(), "program");

    assert_eq!(c.goto_first_child_for_point(Point::new(7, 0)), None);
    assert_eq!(c.goto_first_child_for_point(Point::new(6, 7)), None);
    assert_eq!(c.node().kind(), "program");

    // descend to expression statement
    assert_eq!(c.goto_first_child_for_point(Point::new(6, 5)), Some(0));
    assert_eq!(c.node().kind(), "expression_statement");

    // step into ';' and back up
    assert_eq!(c.goto_first_child_for_point(Point::new(7, 0)), None);
    assert_eq!(c.goto_first_child_for_point(Point::new(6, 6)), None);
    assert_eq!(c.goto_first_child_for_point(Point::new(6, 5)), Some(1));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        (";", Point::new(6, 5))
    );
    assert!(c.goto_parent());

    // descend into array
    assert_eq!(c.goto_first_child_for_point(Point::new(6, 4)), Some(0));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("array", Point::new(0, 4))
    );

    // step into '[' and back up
    assert_eq!(c.goto_first_child_for_point(Point::new(0, 4)), Some(0));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("[", Point::new(0, 4))
    );
    assert!(c.goto_parent());

    // step into identifier 'one' and back up
    assert_eq!(c.goto_first_child_for_point(Point::new(1, 0)), Some(1));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("identifier", Point::new(1, 8))
    );
    assert!(c.goto_parent());
    assert_eq!(c.goto_first_child_for_point(Point::new(1, 10)), Some(1));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("identifier", Point::new(1, 8))
    );
    assert!(c.goto_parent());

    // step into first ',' and back up
    assert_eq!(c.goto_first_child_for_point(Point::new(1, 11)), Some(2));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        (",", Point::new(1, 11))
    );
    assert!(c.goto_parent());

    // step into identifier 'four' and back up
    assert_eq!(c.goto_first_child_for_point(Point::new(5, 0)), Some(5));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("identifier", Point::new(5, 8))
    );
    assert!(c.goto_parent());
    assert_eq!(c.goto_first_child_for_point(Point::new(5, 0)), Some(5));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("identifier", Point::new(5, 8))
    );
    assert!(c.goto_parent());

    // step into ']' and back up
    assert_eq!(c.goto_first_child_for_point(Point::new(6, 0)), Some(10));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("]", Point::new(6, 4))
    );
    assert!(c.goto_parent());
    assert_eq!(c.goto_first_child_for_point(Point::new(6, 0)), Some(10));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("]", Point::new(6, 4))
    );
    assert!(c.goto_parent());

    // descend into object
    assert_eq!(c.goto_first_child_for_point(Point::new(2, 0)), Some(3));
    assert_eq!(
        (c.node().kind(), c.node().start_position()),
        ("object", Point::new(2, 8))
    );
}

#[test]
fn test_tree_node_equality() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();
    let tree = parser.parse("struct A {}", None).unwrap();
    let node1 = tree.root_node();
    let node2 = tree.root_node();
    assert_eq!(node1, node2);
    assert_eq!(node1.child(0).unwrap(), node2.child(0).unwrap());
    assert_ne!(node1.child(0).unwrap(), node2);
}

#[test]
fn test_get_changed_ranges() {
    let source_code = b"{a: null};\n".to_vec();

    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse(&source_code, None).unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        "(program (expression_statement (object (pair key: (property_identifier) value: (null)))))"
    );

    // Updating one token
    {
        let mut tree = tree.clone();
        let mut source_code = source_code.clone();

        // Replace `null` with `nothing` - that token has changed syntax
        let edit = Edit {
            position: index_of(&source_code, "ull"),
            deleted_length: 3,
            inserted_text: b"othing".to_vec(),
        };
        let inverse_edit = invert_edit(&source_code, &edit);
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &edit);
        assert_eq!(ranges, vec![range_of(&source_code, "nothing")]);

        // Replace `nothing` with `null` - that token has changed syntax
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &inverse_edit);
        assert_eq!(ranges, vec![range_of(&source_code, "null")]);
    }

    // Changing only leading whitespace
    {
        let mut tree = tree.clone();
        let mut source_code = source_code.clone();

        // Insert leading newline - no changed ranges
        let edit = Edit {
            position: 0,
            deleted_length: 0,
            inserted_text: b"\n".to_vec(),
        };
        let inverse_edit = invert_edit(&source_code, &edit);
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &edit);
        assert_eq!(ranges, vec![]);

        // Remove leading newline - no changed ranges
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &inverse_edit);
        assert_eq!(ranges, vec![]);
    }

    // Inserting elements
    {
        let mut tree = tree.clone();
        let mut source_code = source_code.clone();

        // Insert a key-value pair before the `}` - those tokens are changed
        let edit1 = Edit {
            position: index_of(&source_code, "}"),
            deleted_length: 0,
            inserted_text: b", b: false".to_vec(),
        };
        let inverse_edit1 = invert_edit(&source_code, &edit1);
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &edit1);
        assert_eq!(ranges, vec![range_of(&source_code, ", b: false")]);

        let edit2 = Edit {
            position: index_of(&source_code, ", b"),
            deleted_length: 0,
            inserted_text: b", c: 1".to_vec(),
        };
        let inverse_edit2 = invert_edit(&source_code, &edit2);
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &edit2);
        assert_eq!(ranges, vec![range_of(&source_code, ", c: 1")]);

        // Remove the middle pair
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &inverse_edit2);
        assert_eq!(ranges, vec![]);

        // Remove the second pair
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &inverse_edit1);
        assert_eq!(ranges, vec![]);
    }

    // Wrapping elements in larger expressions
    {
        let mut tree = tree;
        let mut source_code = source_code.clone();

        // Replace `null` with the binary expression `b === null`
        let edit1 = Edit {
            position: index_of(&source_code, "null"),
            deleted_length: 0,
            inserted_text: b"b === ".to_vec(),
        };
        let inverse_edit1 = invert_edit(&source_code, &edit1);
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &edit1);
        assert_eq!(ranges, vec![range_of(&source_code, "b === null")]);

        // Undo
        let ranges = get_changed_ranges(&mut parser, &mut tree, &mut source_code, &inverse_edit1);
        assert_eq!(ranges, vec![range_of(&source_code, "null")]);
    }
}

#[test]
fn test_consistency_with_mid_codepoint_edit() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("php/php")).unwrap();
    let mut source_code =
        b"\n<?php\n\n<<<'\xE5\xAD\x97\xE6\xBC\xA2'\n  T\n\xE5\xAD\x97\xE6\xBC\xA2;".to_vec();
    let mut tree = parser.parse(&source_code, None).unwrap();

    let edit = Edit {
        position: 17,
        deleted_length: 0,
        inserted_text: vec![46],
    };
    perform_edit(&mut tree, &mut source_code, &edit).unwrap();
    let mut tree2 = parser.parse(&source_code, Some(&tree)).unwrap();

    let inverted = invert_edit(&source_code, &edit);
    perform_edit(&mut tree2, &mut source_code, &inverted).unwrap();
    let tree3 = parser.parse(&source_code, Some(&tree2)).unwrap();

    assert_eq!(tree3.root_node().to_sexp(), tree.root_node().to_sexp());
}

#[test]
fn test_tree_cursor_on_aliased_root_with_extra_child() {
    let source = r"
fn main() {
    C/* hi */::<D>::E;
}
";

    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let tree = parser.parse(source, None).unwrap();

    let function = tree.root_node().child(0).unwrap();
    let block = function.child(3).unwrap();
    let expression_statement = block.child(1).unwrap();
    let scoped_identifier = expression_statement.child(0).unwrap();
    let generic_type = scoped_identifier.child(0).unwrap();
    assert_eq!(generic_type.kind(), "generic_type");

    let mut cursor = generic_type.walk();
    assert!(cursor.goto_first_child());
    assert_eq!(cursor.node().kind(), "type_identifier");
    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind(), "block_comment");
}

fn index_of(text: &[u8], substring: &str) -> usize {
    str::from_utf8(text).unwrap().find(substring).unwrap()
}

fn range_of(text: &[u8], substring: &str) -> Range {
    let start_byte = index_of(text, substring);
    let end_byte = start_byte + substring.len();
    Range {
        start_byte,
        end_byte,
        start_point: Point::new(0, start_byte),
        end_point: Point::new(0, end_byte),
    }
}

fn get_changed_ranges(
    parser: &mut Parser,
    tree: &mut Tree,
    source_code: &mut Vec<u8>,
    edit: &Edit,
) -> Vec<Range> {
    perform_edit(tree, source_code, edit).unwrap();
    let new_tree = parser.parse(source_code, Some(tree)).unwrap();
    let result = tree.changed_ranges(&new_tree).collect();
    *tree = new_tree;
    result
}
