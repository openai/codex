use tree_sitter::{Node, Parser, Point, Tree};
use tree_sitter_generate::load_grammar_file;

use super::{
    get_random_edit,
    helpers::fixtures::{fixtures_dir, get_language, get_test_language},
    Rand,
};
use crate::{
    parse::perform_edit,
    tests::{generate_parser, helpers::fixtures::get_test_fixture_language},
};

const JSON_EXAMPLE: &str = r#"

[
  123,
  false,
  {
    "x": null
  }
]
"#;

const GRAMMAR_WITH_ALIASES_AND_EXTRAS: &str = r#"{
  "name": "aliases_and_extras",

  "extras": [
    {"type": "PATTERN", "value": "\\s+"},
    {"type": "SYMBOL", "name": "comment"}
  ],

  "rules": {
    "a": {
      "type": "SEQ",
      "members": [
        {"type": "SYMBOL", "name": "b"},
        {
          "type": "ALIAS",
          "value": "B",
          "named": true,
          "content": {"type": "SYMBOL", "name": "b"}
        },
        {
          "type": "ALIAS",
          "value": "C",
          "named": true,
          "content": {"type": "SYMBOL", "name": "_c"}
        }
      ]
    },

    "b": {"type": "STRING", "value": "b"},

    "_c": {"type": "STRING", "value": "c"},

    "comment": {"type": "STRING", "value": "..."}
  }
}"#;

#[test]
fn test_node_child() {
    let tree = parse_json_example();
    let array_node = tree.root_node().child(0).unwrap();

    assert_eq!(array_node.kind(), "array");
    assert_eq!(array_node.named_child_count(), 3);
    assert_eq!(array_node.start_byte(), JSON_EXAMPLE.find('[').unwrap());
    assert_eq!(array_node.end_byte(), JSON_EXAMPLE.find(']').unwrap() + 1);
    assert_eq!(array_node.start_position(), Point::new(2, 0));
    assert_eq!(array_node.end_position(), Point::new(8, 1));
    assert_eq!(array_node.child_count(), 7);

    let left_bracket_node = array_node.child(0).unwrap();
    let number_node = array_node.child(1).unwrap();
    let comma_node1 = array_node.child(2).unwrap();
    let false_node = array_node.child(3).unwrap();
    let comma_node2 = array_node.child(4).unwrap();
    let object_node = array_node.child(5).unwrap();
    let right_bracket_node = array_node.child(6).unwrap();

    assert_eq!(left_bracket_node.kind(), "[");
    assert_eq!(number_node.kind(), "number");
    assert_eq!(comma_node1.kind(), ",");
    assert_eq!(false_node.kind(), "false");
    assert_eq!(comma_node2.kind(), ",");
    assert_eq!(object_node.kind(), "object");
    assert_eq!(right_bracket_node.kind(), "]");

    assert!(!left_bracket_node.is_named());
    assert!(number_node.is_named());
    assert!(!comma_node1.is_named());
    assert!(false_node.is_named());
    assert!(!comma_node2.is_named());
    assert!(object_node.is_named());
    assert!(!right_bracket_node.is_named());

    assert_eq!(number_node.start_byte(), JSON_EXAMPLE.find("123").unwrap());
    assert_eq!(
        number_node.end_byte(),
        JSON_EXAMPLE.find("123").unwrap() + 3
    );
    assert_eq!(number_node.start_position(), Point::new(3, 2));
    assert_eq!(number_node.end_position(), Point::new(3, 5));

    assert_eq!(false_node.start_byte(), JSON_EXAMPLE.find("false").unwrap());
    assert_eq!(
        false_node.end_byte(),
        JSON_EXAMPLE.find("false").unwrap() + 5
    );
    assert_eq!(false_node.start_position(), Point::new(4, 2));
    assert_eq!(false_node.end_position(), Point::new(4, 7));

    assert_eq!(object_node.start_byte(), JSON_EXAMPLE.find('{').unwrap());
    assert_eq!(object_node.start_position(), Point::new(5, 2));
    assert_eq!(object_node.end_position(), Point::new(7, 3));

    assert_eq!(object_node.child_count(), 3);
    let left_brace_node = object_node.child(0).unwrap();
    let pair_node = object_node.child(1).unwrap();
    let right_brace_node = object_node.child(2).unwrap();

    assert_eq!(left_brace_node.kind(), "{");
    assert_eq!(pair_node.kind(), "pair");
    assert_eq!(right_brace_node.kind(), "}");

    assert!(!left_brace_node.is_named());
    assert!(pair_node.is_named());
    assert!(!right_brace_node.is_named());

    assert_eq!(pair_node.start_byte(), JSON_EXAMPLE.find("\"x\"").unwrap());
    assert_eq!(pair_node.end_byte(), JSON_EXAMPLE.find("null").unwrap() + 4);
    assert_eq!(pair_node.start_position(), Point::new(6, 4));
    assert_eq!(pair_node.end_position(), Point::new(6, 13));

    assert_eq!(pair_node.child_count(), 3);
    let string_node = pair_node.child(0).unwrap();
    let colon_node = pair_node.child(1).unwrap();
    let null_node = pair_node.child(2).unwrap();

    assert_eq!(string_node.kind(), "string");
    assert_eq!(colon_node.kind(), ":");
    assert_eq!(null_node.kind(), "null");

    assert!(string_node.is_named());
    assert!(!colon_node.is_named());
    assert!(null_node.is_named());

    assert_eq!(
        string_node.start_byte(),
        JSON_EXAMPLE.find("\"x\"").unwrap()
    );
    assert_eq!(
        string_node.end_byte(),
        JSON_EXAMPLE.find("\"x\"").unwrap() + 3
    );
    assert_eq!(string_node.start_position(), Point::new(6, 4));
    assert_eq!(string_node.end_position(), Point::new(6, 7));

    assert_eq!(null_node.start_byte(), JSON_EXAMPLE.find("null").unwrap());
    assert_eq!(null_node.end_byte(), JSON_EXAMPLE.find("null").unwrap() + 4);
    assert_eq!(null_node.start_position(), Point::new(6, 9));
    assert_eq!(null_node.end_position(), Point::new(6, 13));

    assert_eq!(string_node.parent().unwrap(), pair_node);
    assert_eq!(null_node.parent().unwrap(), pair_node);
    assert_eq!(pair_node.parent().unwrap(), object_node);
    assert_eq!(number_node.parent().unwrap(), array_node);
    assert_eq!(false_node.parent().unwrap(), array_node);
    assert_eq!(object_node.parent().unwrap(), array_node);
    assert_eq!(array_node.parent().unwrap(), tree.root_node());
    assert_eq!(tree.root_node().parent(), None);

    assert_eq!(
        tree.root_node().child_with_descendant(null_node).unwrap(),
        array_node
    );
    assert_eq!(
        array_node.child_with_descendant(null_node).unwrap(),
        object_node
    );
    assert_eq!(
        object_node.child_with_descendant(null_node).unwrap(),
        pair_node
    );
    assert_eq!(
        pair_node.child_with_descendant(null_node).unwrap(),
        null_node
    );
    assert_eq!(null_node.child_with_descendant(null_node), None);
}

#[test]
fn test_node_children() {
    let tree = parse_json_example();
    let mut cursor = tree.walk();
    let array_node = tree.root_node().child(0).unwrap();
    assert_eq!(
        array_node
            .children(&mut cursor)
            .map(|n| n.kind())
            .collect::<Vec<_>>(),
        &["[", "number", ",", "false", ",", "object", "]",]
    );
    assert_eq!(
        array_node
            .named_children(&mut cursor)
            .map(|n| n.kind())
            .collect::<Vec<_>>(),
        &["number", "false", "object"]
    );
    let object_node = array_node
        .named_children(&mut cursor)
        .find(|n| n.kind() == "object")
        .unwrap();
    assert_eq!(
        object_node
            .children(&mut cursor)
            .map(|n| n.kind())
            .collect::<Vec<_>>(),
        &["{", "pair", "}",]
    );
}

#[test]
fn test_node_children_by_field_name() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("python")).unwrap();
    let source = "
        if one:
            a()
        elif two:
            b()
        elif three:
            c()
        elif four:
            d()
    ";

    let tree = parser.parse(source, None).unwrap();
    let node = tree.root_node().child(0).unwrap();
    assert_eq!(node.kind(), "if_statement");
    let mut cursor = tree.walk();
    let alternatives = node.children_by_field_name("alternative", &mut cursor);
    let alternative_texts =
        alternatives.map(|n| &source[n.child_by_field_name("condition").unwrap().byte_range()]);
    assert_eq!(
        alternative_texts.collect::<Vec<_>>(),
        &["two", "three", "four",]
    );
}

#[test]
fn test_node_parent_of_child_by_field_name() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("foo(a().b[0].c.d.e())", None).unwrap();
    let call_node = tree
        .root_node()
        .named_child(0)
        .unwrap()
        .named_child(0)
        .unwrap();
    assert_eq!(call_node.kind(), "call_expression");

    // Regression test - when a field points to a hidden node (in this case, `_expression`)
    // the hidden node should not be added to the node parent cache.
    assert_eq!(
        call_node.child_by_field_name("function").unwrap().parent(),
        Some(call_node)
    );
}

#[test]
fn test_parent_of_zero_width_node() {
    let code = "def dupa(foo):";

    let mut parser = Parser::new();
    parser.set_language(&get_language("python")).unwrap();

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();
    let function_definition = root.child(0).unwrap();
    let block = function_definition.child(4).unwrap();
    let block_parent = block.parent().unwrap();

    assert_eq!(block.to_string(), "(block)");
    assert_eq!(block_parent.kind(), "function_definition");
    assert_eq!(block_parent.to_string(), "(function_definition name: (identifier) parameters: (parameters (identifier)) body: (block))");

    assert_eq!(
        root.child_with_descendant(block).unwrap(),
        function_definition
    );
    assert_eq!(
        function_definition.child_with_descendant(block).unwrap(),
        block
    );
    assert_eq!(block.child_with_descendant(block), None);

    let code = "<script></script>";
    parser.set_language(&get_language("html")).unwrap();

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();
    let script_element = root.child(0).unwrap();
    let raw_text = script_element.child(1).unwrap();
    let parent = raw_text.parent().unwrap();
    assert_eq!(parent, script_element);
}

#[test]
fn test_next_sibling_of_zero_width_node() {
    let mut parser = Parser::new();
    let language = get_test_fixture_language("next_sibling_from_zwt");
    parser.set_language(&language).unwrap();

    let tree = parser.parse("abdef", None).unwrap();

    let root_node = tree.root_node();
    let missing_c = root_node.child(2).unwrap();
    assert!(missing_c.is_missing());
    assert_eq!(missing_c.kind(), "c");
    let node_d = root_node.child(3).unwrap();
    assert_eq!(missing_c.next_sibling().unwrap(), node_d);

    let prev_sibling = node_d.prev_sibling().unwrap();
    assert_eq!(prev_sibling, missing_c);
}

#[test]
fn test_first_child_for_offset() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("x10 + 100", None).unwrap();
    let sum_node = tree.root_node().child(0).unwrap().child(0).unwrap();

    assert_eq!(
        sum_node.first_child_for_byte(0).unwrap().kind(),
        "identifier"
    );
    assert_eq!(
        sum_node.first_child_for_byte(1).unwrap().kind(),
        "identifier"
    );
    assert_eq!(sum_node.first_child_for_byte(3).unwrap().kind(), "+");
    assert_eq!(sum_node.first_child_for_byte(5).unwrap().kind(), "number");
}

#[test]
fn test_first_named_child_for_offset() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("x10 + 100", None).unwrap();
    let sum_node = tree.root_node().child(0).unwrap().child(0).unwrap();

    assert_eq!(
        sum_node.first_named_child_for_byte(0).unwrap().kind(),
        "identifier"
    );
    assert_eq!(
        sum_node.first_named_child_for_byte(1).unwrap().kind(),
        "identifier"
    );
    assert_eq!(
        sum_node.first_named_child_for_byte(3).unwrap().kind(),
        "number"
    );
}

#[test]
fn test_node_field_name_for_child() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("c")).unwrap();
    let tree = parser
        .parse("int w = x + /* y is special! */ y;", None)
        .unwrap();
    let translation_unit_node = tree.root_node();
    let declaration_node = translation_unit_node.named_child(0).unwrap();

    let binary_expression_node = declaration_node
        .child_by_field_name("declarator")
        .unwrap()
        .child_by_field_name("value")
        .unwrap();

    // -------------------
    // left: (identifier)  0
    // operator: "+"       1 <--- (not a named child)
    // (comment)           2 <--- (is an extra)
    // right: (identifier) 3
    // -------------------

    assert_eq!(binary_expression_node.field_name_for_child(0), Some("left"));
    assert_eq!(
        binary_expression_node.field_name_for_child(1),
        Some("operator")
    );
    // The comment should not have a field name, as it's just an extra
    assert_eq!(binary_expression_node.field_name_for_child(2), None);
    assert_eq!(
        binary_expression_node.field_name_for_child(3),
        Some("right")
    );
    // Negative test - Not a valid child index
    assert_eq!(binary_expression_node.field_name_for_child(4), None);
}

#[test]
fn test_node_field_name_for_named_child() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("c")).unwrap();
    let tree = parser
        .parse("int w = x + /* y is special! */ y;", None)
        .unwrap();
    let translation_unit_node = tree.root_node();
    let declaration_node = translation_unit_node.named_child(0).unwrap();

    let binary_expression_node = declaration_node
        .child_by_field_name("declarator")
        .unwrap()
        .child_by_field_name("value")
        .unwrap();

    // -------------------
    // left: (identifier)  0
    // operator: "+"       _ <--- (not a named child)
    // (comment)           1 <--- (is an extra)
    // right: (identifier) 2
    // -------------------

    assert_eq!(
        binary_expression_node.field_name_for_named_child(0),
        Some("left")
    );
    // The comment should not have a field name, as it's just an extra
    assert_eq!(binary_expression_node.field_name_for_named_child(1), None);
    // The operator is not a named child, so the named child at index 2 is the right child
    assert_eq!(
        binary_expression_node.field_name_for_named_child(2),
        Some("right")
    );
    // Negative test - Not a valid child index
    assert_eq!(binary_expression_node.field_name_for_named_child(3), None);
}

#[test]
fn test_node_child_by_field_name_with_extra_hidden_children() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("python")).unwrap();

    // In the Python grammar, some fields are applied to `suite` nodes,
    // which consist of an invisible `indent` token followed by a block.
    // Check that when searching for a child with a field name, we don't
    //
    let tree = parser.parse("while a:\n  pass", None).unwrap();
    let while_node = tree.root_node().child(0).unwrap();
    assert_eq!(while_node.kind(), "while_statement");
    assert_eq!(
        while_node.child_by_field_name("body").unwrap(),
        while_node.child(3).unwrap(),
    );
}

#[test]
fn test_node_named_child() {
    let tree = parse_json_example();
    let array_node = tree.root_node().child(0).unwrap();

    let number_node = array_node.named_child(0).unwrap();
    let false_node = array_node.named_child(1).unwrap();
    let object_node = array_node.named_child(2).unwrap();

    assert_eq!(number_node.kind(), "number");
    assert_eq!(number_node.start_byte(), JSON_EXAMPLE.find("123").unwrap());
    assert_eq!(
        number_node.end_byte(),
        JSON_EXAMPLE.find("123").unwrap() + 3
    );
    assert_eq!(number_node.start_position(), Point::new(3, 2));
    assert_eq!(number_node.end_position(), Point::new(3, 5));

    assert_eq!(false_node.kind(), "false");
    assert_eq!(false_node.start_byte(), JSON_EXAMPLE.find("false").unwrap());
    assert_eq!(
        false_node.end_byte(),
        JSON_EXAMPLE.find("false").unwrap() + 5
    );
    assert_eq!(false_node.start_position(), Point::new(4, 2));
    assert_eq!(false_node.end_position(), Point::new(4, 7));

    assert_eq!(object_node.kind(), "object");
    assert_eq!(object_node.start_byte(), JSON_EXAMPLE.find('{').unwrap());
    assert_eq!(object_node.start_position(), Point::new(5, 2));
    assert_eq!(object_node.end_position(), Point::new(7, 3));

    assert_eq!(object_node.named_child_count(), 1);

    let pair_node = object_node.named_child(0).unwrap();
    assert_eq!(pair_node.kind(), "pair");
    assert_eq!(pair_node.start_byte(), JSON_EXAMPLE.find("\"x\"").unwrap());
    assert_eq!(pair_node.end_byte(), JSON_EXAMPLE.find("null").unwrap() + 4);
    assert_eq!(pair_node.start_position(), Point::new(6, 4));
    assert_eq!(pair_node.end_position(), Point::new(6, 13));

    let string_node = pair_node.named_child(0).unwrap();
    let null_node = pair_node.named_child(1).unwrap();

    assert_eq!(string_node.kind(), "string");
    assert_eq!(null_node.kind(), "null");

    assert_eq!(
        string_node.start_byte(),
        JSON_EXAMPLE.find("\"x\"").unwrap()
    );
    assert_eq!(
        string_node.end_byte(),
        JSON_EXAMPLE.find("\"x\"").unwrap() + 3
    );
    assert_eq!(string_node.start_position(), Point::new(6, 4));
    assert_eq!(string_node.end_position(), Point::new(6, 7));

    assert_eq!(null_node.start_byte(), JSON_EXAMPLE.find("null").unwrap());
    assert_eq!(null_node.end_byte(), JSON_EXAMPLE.find("null").unwrap() + 4);
    assert_eq!(null_node.start_position(), Point::new(6, 9));
    assert_eq!(null_node.end_position(), Point::new(6, 13));

    assert_eq!(string_node.parent().unwrap(), pair_node);
    assert_eq!(null_node.parent().unwrap(), pair_node);
    assert_eq!(pair_node.parent().unwrap(), object_node);
    assert_eq!(number_node.parent().unwrap(), array_node);
    assert_eq!(false_node.parent().unwrap(), array_node);
    assert_eq!(object_node.parent().unwrap(), array_node);
    assert_eq!(array_node.parent().unwrap(), tree.root_node());
    assert_eq!(tree.root_node().parent(), None);

    assert_eq!(
        tree.root_node().child_with_descendant(null_node).unwrap(),
        array_node
    );
    assert_eq!(
        array_node.child_with_descendant(null_node).unwrap(),
        object_node
    );
    assert_eq!(
        object_node.child_with_descendant(null_node).unwrap(),
        pair_node
    );
    assert_eq!(
        pair_node.child_with_descendant(null_node).unwrap(),
        null_node
    );
    assert_eq!(null_node.child_with_descendant(null_node), None);
}

#[test]
fn test_node_named_child_with_aliases_and_extras() {
    let (parser_name, parser_code) = generate_parser(GRAMMAR_WITH_ALIASES_AND_EXTRAS).unwrap();

    let mut parser = Parser::new();
    parser
        .set_language(&get_test_language(&parser_name, &parser_code, None))
        .unwrap();

    let tree = parser.parse("b ... b ... c", None).unwrap();
    let root = tree.root_node();
    assert_eq!(root.to_sexp(), "(a (b) (comment) (B) (comment) (C))");
    assert_eq!(root.named_child_count(), 5);
    assert_eq!(root.named_child(0).unwrap().kind(), "b");
    assert_eq!(root.named_child(1).unwrap().kind(), "comment");
    assert_eq!(root.named_child(2).unwrap().kind(), "B");
    assert_eq!(root.named_child(3).unwrap().kind(), "comment");
    assert_eq!(root.named_child(4).unwrap().kind(), "C");
}

#[test]
fn test_node_descendant_count() {
    let tree = parse_json_example();
    let value_node = tree.root_node();
    let all_nodes = get_all_nodes(&tree);

    assert_eq!(value_node.descendant_count(), all_nodes.len());

    let mut cursor = value_node.walk();
    for (i, node) in all_nodes.iter().enumerate() {
        cursor.goto_descendant(i);
        assert_eq!(cursor.node(), *node, "index {i}");
    }

    for (i, node) in all_nodes.iter().enumerate().rev() {
        cursor.goto_descendant(i);
        assert_eq!(cursor.node(), *node, "rev index {i}");
    }
}

#[test]
fn test_descendant_count_single_node_tree() {
    let mut parser = Parser::new();
    parser
        .set_language(&get_language("embedded-template"))
        .unwrap();
    let tree = parser.parse("hello", None).unwrap();

    let nodes = get_all_nodes(&tree);
    assert_eq!(nodes.len(), 2);
    assert_eq!(tree.root_node().descendant_count(), 2);

    let mut cursor = tree.root_node().walk();

    cursor.goto_descendant(0);
    assert_eq!(cursor.depth(), 0);
    assert_eq!(cursor.node(), nodes[0]);
    cursor.goto_descendant(1);
    assert_eq!(cursor.depth(), 1);
    assert_eq!(cursor.node(), nodes[1]);
}

#[test]
fn test_node_descendant_for_range() {
    let tree = parse_json_example();
    let array_node = tree.root_node();

    // Leaf node exactly matches the given bounds - byte query
    let colon_index = JSON_EXAMPLE.find(':').unwrap();
    let colon_node = array_node
        .descendant_for_byte_range(colon_index, colon_index + 1)
        .unwrap();
    assert_eq!(colon_node.kind(), ":");
    assert_eq!(colon_node.start_byte(), colon_index);
    assert_eq!(colon_node.end_byte(), colon_index + 1);
    assert_eq!(colon_node.start_position(), Point::new(6, 7));
    assert_eq!(colon_node.end_position(), Point::new(6, 8));

    // Leaf node exactly matches the given bounds - point query
    let colon_node = array_node
        .descendant_for_point_range(Point::new(6, 7), Point::new(6, 8))
        .unwrap();
    assert_eq!(colon_node.kind(), ":");
    assert_eq!(colon_node.start_byte(), colon_index);
    assert_eq!(colon_node.end_byte(), colon_index + 1);
    assert_eq!(colon_node.start_position(), Point::new(6, 7));
    assert_eq!(colon_node.end_position(), Point::new(6, 8));

    // The given point is between two adjacent leaf nodes - byte query
    let colon_index = JSON_EXAMPLE.find(':').unwrap();
    let colon_node = array_node
        .descendant_for_byte_range(colon_index, colon_index)
        .unwrap();
    assert_eq!(colon_node.kind(), ":");
    assert_eq!(colon_node.start_byte(), colon_index);
    assert_eq!(colon_node.end_byte(), colon_index + 1);
    assert_eq!(colon_node.start_position(), Point::new(6, 7));
    assert_eq!(colon_node.end_position(), Point::new(6, 8));

    // The given point is between two adjacent leaf nodes - point query
    let colon_node = array_node
        .descendant_for_point_range(Point::new(6, 7), Point::new(6, 7))
        .unwrap();
    assert_eq!(colon_node.kind(), ":");
    assert_eq!(colon_node.start_byte(), colon_index);
    assert_eq!(colon_node.end_byte(), colon_index + 1);
    assert_eq!(colon_node.start_position(), Point::new(6, 7));
    assert_eq!(colon_node.end_position(), Point::new(6, 8));

    // Leaf node starts at the lower bound, ends after the upper bound - byte query
    let string_index = JSON_EXAMPLE.find("\"x\"").unwrap();
    let string_node = array_node
        .descendant_for_byte_range(string_index, string_index + 2)
        .unwrap();
    assert_eq!(string_node.kind(), "string");
    assert_eq!(string_node.start_byte(), string_index);
    assert_eq!(string_node.end_byte(), string_index + 3);
    assert_eq!(string_node.start_position(), Point::new(6, 4));
    assert_eq!(string_node.end_position(), Point::new(6, 7));

    // Leaf node starts at the lower bound, ends after the upper bound - point query
    let string_node = array_node
        .descendant_for_point_range(Point::new(6, 4), Point::new(6, 6))
        .unwrap();
    assert_eq!(string_node.kind(), "string");
    assert_eq!(string_node.start_byte(), string_index);
    assert_eq!(string_node.end_byte(), string_index + 3);
    assert_eq!(string_node.start_position(), Point::new(6, 4));
    assert_eq!(string_node.end_position(), Point::new(6, 7));

    // Leaf node starts before the lower bound, ends at the upper bound - byte query
    let null_index = JSON_EXAMPLE.find("null").unwrap();
    let null_node = array_node
        .descendant_for_byte_range(null_index + 1, null_index + 4)
        .unwrap();
    assert_eq!(null_node.kind(), "null");
    assert_eq!(null_node.start_byte(), null_index);
    assert_eq!(null_node.end_byte(), null_index + 4);
    assert_eq!(null_node.start_position(), Point::new(6, 9));
    assert_eq!(null_node.end_position(), Point::new(6, 13));

    // Leaf node starts before the lower bound, ends at the upper bound - point query
    let null_node = array_node
        .descendant_for_point_range(Point::new(6, 11), Point::new(6, 13))
        .unwrap();
    assert_eq!(null_node.kind(), "null");
    assert_eq!(null_node.start_byte(), null_index);
    assert_eq!(null_node.end_byte(), null_index + 4);
    assert_eq!(null_node.start_position(), Point::new(6, 9));
    assert_eq!(null_node.end_position(), Point::new(6, 13));

    // The bounds span multiple leaf nodes - return the smallest node that does span it.
    let pair_node = array_node
        .descendant_for_byte_range(string_index + 2, string_index + 4)
        .unwrap();
    assert_eq!(pair_node.kind(), "pair");
    assert_eq!(pair_node.start_byte(), string_index);
    assert_eq!(pair_node.end_byte(), string_index + 9);
    assert_eq!(pair_node.start_position(), Point::new(6, 4));
    assert_eq!(pair_node.end_position(), Point::new(6, 13));

    assert_eq!(colon_node.parent(), Some(pair_node));

    // no leaf spans the given range - return the smallest node that does span it.
    let pair_node = array_node
        .named_descendant_for_point_range(Point::new(6, 6), Point::new(6, 8))
        .unwrap();
    assert_eq!(pair_node.kind(), "pair");
    assert_eq!(pair_node.start_byte(), string_index);
    assert_eq!(pair_node.end_byte(), string_index + 9);
    assert_eq!(pair_node.start_position(), Point::new(6, 4));
    assert_eq!(pair_node.end_position(), Point::new(6, 13));

    // Zero-width token
    {
        let code = "<script></script>";
        let mut parser = Parser::new();
        parser.set_language(&get_language("html")).unwrap();

        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        let child = root
            .named_descendant_for_point_range(Point::new(0, 8), Point::new(0, 8))
            .unwrap();
        assert_eq!(child.kind(), "raw_text");

        let child2 = root.named_descendant_for_byte_range(8, 8).unwrap();
        assert_eq!(child2.kind(), "raw_text");

        assert_eq!(child, child2);
    }

    // Negative test, start > end
    assert_eq!(array_node.descendant_for_byte_range(1, 0), None);
    assert_eq!(
        array_node.descendant_for_point_range(Point::new(6, 8), Point::new(6, 7)),
        None
    );
}

#[test]
fn test_node_edit() {
    let mut code = JSON_EXAMPLE.as_bytes().to_vec();
    let mut tree = parse_json_example();
    let mut rand = Rand::new(0);

    for _ in 0..10 {
        let mut nodes_before = get_all_nodes(&tree);

        let edit = get_random_edit(&mut rand, &code);
        let mut tree2 = tree.clone();
        let edit = perform_edit(&mut tree2, &mut code, &edit).unwrap();
        for node in &mut nodes_before {
            node.edit(&edit);
        }

        let nodes_after = get_all_nodes(&tree2);
        for (i, node) in nodes_before.into_iter().enumerate() {
            assert_eq!(
                (node.kind(), node.start_byte(), node.start_position()),
                (
                    nodes_after[i].kind(),
                    nodes_after[i].start_byte(),
                    nodes_after[i].start_position()
                ),
            );
        }

        tree = tree2;
    }
}

#[test]
fn test_root_node_with_offset() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("  if (a) b", None).unwrap();

    let node = tree.root_node_with_offset(6, Point::new(2, 2));
    assert_eq!(node.byte_range(), 8..16);
    assert_eq!(node.start_position(), Point::new(2, 4));
    assert_eq!(node.end_position(), Point::new(2, 12));

    let child = node.child(0).unwrap().child(2).unwrap();
    assert_eq!(child.kind(), "expression_statement");
    assert_eq!(child.byte_range(), 15..16);
    assert_eq!(child.start_position(), Point::new(2, 11));
    assert_eq!(child.end_position(), Point::new(2, 12));

    let mut cursor = node.walk();
    cursor.goto_first_child();
    cursor.goto_first_child();
    cursor.goto_next_sibling();
    let child = cursor.node();
    assert_eq!(child.kind(), "parenthesized_expression");
    assert_eq!(child.byte_range(), 11..14);
    assert_eq!(child.start_position(), Point::new(2, 7));
    assert_eq!(child.end_position(), Point::new(2, 10));
}

#[test]
fn test_node_is_extra() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("foo(/* hi */);", None).unwrap();

    let root_node = tree.root_node();
    let comment_node = root_node.descendant_for_byte_range(7, 7).unwrap();

    assert_eq!(root_node.kind(), "program");
    assert_eq!(comment_node.kind(), "comment");
    assert!(!root_node.is_extra());
    assert!(comment_node.is_extra());
}

#[test]
fn test_node_is_error() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("foo(", None).unwrap();
    let root_node = tree.root_node();
    assert_eq!(root_node.kind(), "program");
    assert!(root_node.has_error());

    let child = root_node.child(0).unwrap();
    assert_eq!(child.kind(), "ERROR");
    assert!(child.is_error());
}

#[test]
fn test_node_sexp() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse("if (a) b", None).unwrap();
    let root_node = tree.root_node();
    let if_node = root_node.descendant_for_byte_range(0, 0).unwrap();
    let paren_node = root_node.descendant_for_byte_range(3, 3).unwrap();
    let identifier_node = root_node.descendant_for_byte_range(4, 4).unwrap();
    assert_eq!(if_node.kind(), "if");
    assert_eq!(if_node.to_sexp(), "(\"if\")");
    assert_eq!(paren_node.kind(), "(");
    assert_eq!(paren_node.to_sexp(), "(\"(\")");
    assert_eq!(identifier_node.kind(), "identifier");
    assert_eq!(identifier_node.to_sexp(), "(identifier)");
}

#[test]
fn test_node_field_names() {
    let (parser_name, parser_code) = generate_parser(
        r#"
        {
            "name": "test_grammar_with_fields",
            "extras": [
                {"type": "PATTERN", "value": "\\s+"}
            ],
            "rules": {
                "rule_a": {
                    "type": "SEQ",
                    "members": [
                        {
                            "type": "FIELD",
                            "name": "field_1",
                            "content": {"type": "STRING", "value": "child-0"}
                        },
                        {
                            "type": "CHOICE",
                            "members": [
                                {"type": "STRING", "value": "child-1"},
                                {"type": "BLANK"},

                                // This isn't used in the test, but prevents `_hidden_rule1`
                                // from being eliminated as a unit reduction.
                                {
                                    "type": "ALIAS",
                                    "value": "x",
                                    "named": true,
                                    "content": {
                                        "type": "SYMBOL",
                                        "name": "_hidden_rule1"
                                    }
                                }
                            ]
                        },
                        {
                            "type": "FIELD",
                            "name": "field_2",
                            "content": {"type": "SYMBOL", "name": "_hidden_rule1"}
                        },
                        {"type": "SYMBOL", "name": "_hidden_rule2"}
                    ]
                },

                // Fields pointing to hidden nodes with a single child resolve to the child.
                "_hidden_rule1": {
                    "type": "CHOICE",
                    "members": [
                        {"type": "STRING", "value": "child-2"},
                        {"type": "STRING", "value": "child-2.5"}
                    ]
                },

                // Fields within hidden nodes can be referenced through the parent node.
                "_hidden_rule2": {
                    "type": "SEQ",
                    "members": [
                        {"type": "STRING", "value": "child-3"},
                        {
                            "type": "FIELD",
                            "name": "field_3",
                            "content": {"type": "STRING", "value": "child-4"}
                        }
                    ]
                }
            }
        }
    "#,
    )
    .unwrap();

    let mut parser = Parser::new();
    let language = get_test_language(&parser_name, &parser_code, None);
    parser.set_language(&language).unwrap();

    let tree = parser
        .parse("child-0 child-1 child-2 child-3 child-4", None)
        .unwrap();
    let root_node = tree.root_node();

    assert_eq!(root_node.child_by_field_name("field_1"), root_node.child(0));
    assert_eq!(root_node.child_by_field_name("field_2"), root_node.child(2));
    assert_eq!(root_node.child_by_field_name("field_3"), root_node.child(4));
    assert_eq!(
        root_node.child(0).unwrap().child_by_field_name("field_1"),
        None
    );
    assert_eq!(root_node.child_by_field_name("not_a_real_field"), None);

    let mut cursor = root_node.walk();
    assert_eq!(cursor.field_name(), None);
    cursor.goto_first_child();
    assert_eq!(cursor.node().kind(), "child-0");
    assert_eq!(cursor.field_name(), Some("field_1"));
    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "child-1");
    assert_eq!(cursor.field_name(), None);
    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "child-2");
    assert_eq!(cursor.field_name(), Some("field_2"));
    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "child-3");
    assert_eq!(cursor.field_name(), None);
    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind(), "child-4");
    assert_eq!(cursor.field_name(), Some("field_3"));
}

#[test]
fn test_node_field_calls_in_language_without_fields() {
    let (parser_name, parser_code) = generate_parser(
        r#"
        {
            "name": "test_grammar_with_no_fields",
            "extras": [
                {"type": "PATTERN", "value": "\\s+"}
            ],
            "rules": {
                "a": {
                    "type": "SEQ",
                    "members": [
                        {
                            "type": "STRING",
                            "value": "b"
                        },
                        {
                            "type": "STRING",
                            "value": "c"
                        },
                        {
                            "type": "STRING",
                            "value": "d"
                        }
                    ]
                }
            }
        }
    "#,
    )
    .unwrap();

    let mut parser = Parser::new();
    let language = get_test_language(&parser_name, &parser_code, None);
    parser.set_language(&language).unwrap();

    let tree = parser.parse("b c d", None).unwrap();

    let root_node = tree.root_node();
    assert_eq!(root_node.kind(), "a");
    assert_eq!(root_node.child_by_field_name("something"), None);

    let mut cursor = root_node.walk();
    assert_eq!(cursor.field_name(), None);
    assert!(cursor.goto_first_child());
    assert_eq!(cursor.field_name(), None);
}

#[test]
fn test_node_is_named_but_aliased_as_anonymous() {
    let grammar_json = load_grammar_file(
        &fixtures_dir()
            .join("test_grammars")
            .join("named_rule_aliased_as_anonymous")
            .join("grammar.js"),
        None,
    )
    .unwrap();

    let (parser_name, parser_code) = generate_parser(&grammar_json).unwrap();

    let mut parser = Parser::new();
    let language = get_test_language(&parser_name, &parser_code, None);
    parser.set_language(&language).unwrap();

    let tree = parser.parse("B C B", None).unwrap();

    let root_node = tree.root_node();
    assert!(!root_node.has_error());
    assert_eq!(root_node.child_count(), 3);
    assert_eq!(root_node.named_child_count(), 2);

    let aliased = root_node.child(0).unwrap();
    assert!(!aliased.is_named());
    assert_eq!(aliased.kind(), "the-alias");

    assert_eq!(root_node.named_child(0).unwrap().kind(), "c");
}

#[test]
fn test_node_numeric_symbols_respect_simple_aliases() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("python")).unwrap();

    // Example 1:
    // Python argument lists can contain "splat" arguments, which are not allowed
    // within other expressions. This includes `parenthesized_list_splat` nodes
    // like `(*b)`. These `parenthesized_list_splat` nodes are aliased as
    // `parenthesized_expression`. Their numeric `symbol`, aka `kind_id` should
    // match that of a normal `parenthesized_expression`.
    let tree = parser.parse("(a((*b)))", None).unwrap();
    let root = tree.root_node();
    assert_eq!(
        root.to_sexp(),
        "(module (expression_statement (parenthesized_expression (call function: (identifier) arguments: (argument_list (parenthesized_expression (list_splat (identifier))))))))",
    );

    let outer_expr_node = root.child(0).unwrap().child(0).unwrap();
    assert_eq!(outer_expr_node.kind(), "parenthesized_expression");

    let inner_expr_node = outer_expr_node
        .named_child(0)
        .unwrap()
        .child_by_field_name("arguments")
        .unwrap()
        .named_child(0)
        .unwrap();
    assert_eq!(inner_expr_node.kind(), "parenthesized_expression");
    assert_eq!(inner_expr_node.kind_id(), outer_expr_node.kind_id());

    // Example 2:
    // Ruby handles the unary (negative) and binary (minus) `-` operators using two
    // different tokens. One or more of these is an external token that's
    // aliased as `-`. Their numeric kind ids should match.
    parser.set_language(&get_language("ruby")).unwrap();
    let tree = parser.parse("-a - b", None).unwrap();
    let root = tree.root_node();
    assert_eq!(
        root.to_sexp(),
        "(program (binary left: (unary operand: (identifier)) right: (identifier)))",
    );

    let binary_node = root.child(0).unwrap();
    assert_eq!(binary_node.kind(), "binary");

    let unary_minus_node = binary_node
        .child_by_field_name("left")
        .unwrap()
        .child(0)
        .unwrap();
    assert_eq!(unary_minus_node.kind(), "-");

    let binary_minus_node = binary_node.child_by_field_name("operator").unwrap();
    assert_eq!(binary_minus_node.kind(), "-");
    assert_eq!(unary_minus_node.kind_id(), binary_minus_node.kind_id());
}

#[test]
fn test_hidden_zero_width_node_with_visible_child() {
    let code = r"
class Foo {
  std::
private:
  std::string s;
};
";

    let mut parser = Parser::new();
    parser.set_language(&get_language("cpp")).unwrap();
    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    let class_specifier = root.child(0).unwrap();
    let field_decl_list = class_specifier.child_by_field_name("body").unwrap();
    let field_decl = field_decl_list.named_child(0).unwrap();
    let field_ident = field_decl.child_by_field_name("declarator").unwrap();
    assert_eq!(
        field_decl.child_with_descendant(field_ident).unwrap(),
        field_ident
    );
}

fn get_all_nodes(tree: &Tree) -> Vec<Node> {
    let mut result = Vec::new();
    let mut visited_children = false;
    let mut cursor = tree.walk();
    loop {
        if !visited_children {
            result.push(cursor.node());
            if !cursor.goto_first_child() {
                visited_children = true;
            }
        } else if cursor.goto_next_sibling() {
            visited_children = false;
        } else if !cursor.goto_parent() {
            break;
        }
    }
    result
}

fn parse_json_example() -> Tree {
    let mut parser = Parser::new();
    parser.set_language(&get_language("json")).unwrap();
    parser.parse(JSON_EXAMPLE, None).unwrap()
}
