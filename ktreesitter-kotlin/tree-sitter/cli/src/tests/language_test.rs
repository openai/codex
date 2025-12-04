use tree_sitter::{self, Parser};

use super::helpers::fixtures::get_language;

#[test]
fn test_lookahead_iterator() {
    let mut parser = Parser::new();
    let language = get_language("rust");
    parser.set_language(&language).unwrap();

    let tree = parser.parse("struct Stuff {}", None).unwrap();

    let mut cursor = tree.walk();

    assert!(cursor.goto_first_child()); // struct
    assert!(cursor.goto_first_child()); // struct keyword

    let next_state = cursor.node().next_parse_state();
    assert_ne!(next_state, 0);
    assert_eq!(
        next_state,
        language.next_state(cursor.node().parse_state(), cursor.node().grammar_id())
    );
    assert!((next_state as usize) < language.parse_state_count());
    assert!(cursor.goto_next_sibling()); // type_identifier
    assert_eq!(next_state, cursor.node().parse_state());
    assert_eq!(cursor.node().grammar_name(), "identifier");
    assert_ne!(cursor.node().grammar_id(), cursor.node().kind_id());

    let expected_symbols = ["//", "/*", "identifier", "line_comment", "block_comment"];
    let mut lookahead = language.lookahead_iterator(next_state).unwrap();
    assert_eq!(*lookahead.language(), language);
    assert!(lookahead.iter_names().eq(expected_symbols));

    lookahead.reset_state(next_state);
    assert!(lookahead.iter_names().eq(expected_symbols));

    lookahead.reset(&language, next_state);
    assert!(lookahead
        .map(|s| language.node_kind_for_id(s).unwrap())
        .eq(expected_symbols));
}

#[test]
fn test_lookahead_iterator_modifiable_only_by_mut() {
    let mut parser = Parser::new();
    let language = get_language("rust");
    parser.set_language(&language).unwrap();

    let tree = parser.parse("struct Stuff {}", None).unwrap();

    let mut cursor = tree.walk();

    assert!(cursor.goto_first_child()); // struct
    assert!(cursor.goto_first_child()); // struct keyword

    let next_state = cursor.node().next_parse_state();
    assert_ne!(next_state, 0);

    let mut lookahead = language.lookahead_iterator(next_state).unwrap();
    let _ = lookahead.next();

    let mut names = lookahead.iter_names();
    let _ = names.next();
}

#[test]
fn test_symbol_metadata_checks() {
    let language = get_language("rust");
    for i in 0..language.node_kind_count() {
        let sym = i as u16;
        let name = language.node_kind_for_id(sym).unwrap();
        match name {
            "_type"
            | "_expression"
            | "_pattern"
            | "_literal"
            | "_literal_pattern"
            | "_declaration_statement" => assert!(language.node_kind_is_supertype(sym)),

            "_raw_string_literal_start"
            | "_raw_string_literal_end"
            | "_line_doc_comment"
            | "_error_sentinel" => assert!(!language.node_kind_is_supertype(sym)),

            "enum_item" | "struct_item" | "type_item" => {
                assert!(language.node_kind_is_named(sym));
            }

            "=>" | "[" | "]" | "(" | ")" | "{" | "}" => {
                assert!(language.node_kind_is_visible(sym));
            }

            _ => {}
        }
    }
}

#[test]
fn test_supertypes() {
    let language = get_language("rust");
    let supertypes = language.supertypes();

    if language.abi_version() < 15 {
        return;
    }

    assert_eq!(supertypes.len(), 5);
    assert_eq!(
        supertypes
            .iter()
            .filter_map(|&s| language.node_kind_for_id(s))
            .map(|s| s.to_string())
            .collect::<Vec<String>>(),
        vec![
            "_expression",
            "_literal",
            "_literal_pattern",
            "_pattern",
            "_type"
        ]
    );

    for &supertype in supertypes {
        let mut subtypes = language
            .subtypes_for_supertype(supertype)
            .iter()
            .filter_map(|symbol| language.node_kind_for_id(*symbol))
            .collect::<Vec<&str>>();
        subtypes.sort_unstable();
        subtypes.dedup();

        match language.node_kind_for_id(supertype) {
            Some("_literal") => {
                assert_eq!(
                    subtypes,
                    &[
                        "boolean_literal",
                        "char_literal",
                        "float_literal",
                        "integer_literal",
                        "raw_string_literal",
                        "string_literal"
                    ]
                );
            }
            Some("_pattern") => {
                assert_eq!(
                    subtypes,
                    &[
                        "_",
                        "_literal_pattern",
                        "captured_pattern",
                        "const_block",
                        "generic_pattern",
                        "identifier",
                        "macro_invocation",
                        "mut_pattern",
                        "or_pattern",
                        "range_pattern",
                        "ref_pattern",
                        "reference_pattern",
                        "remaining_field_pattern",
                        "scoped_identifier",
                        "slice_pattern",
                        "struct_pattern",
                        "tuple_pattern",
                        "tuple_struct_pattern",
                    ]
                );
            }
            Some("_type") => {
                assert_eq!(
                    subtypes,
                    &[
                        "abstract_type",
                        "array_type",
                        "bounded_type",
                        "dynamic_type",
                        "function_type",
                        "generic_type",
                        "macro_invocation",
                        "metavariable",
                        "never_type",
                        "pointer_type",
                        "primitive_type",
                        "reference_type",
                        "removed_trait_bound",
                        "scoped_type_identifier",
                        "tuple_type",
                        "type_identifier",
                        "unit_type"
                    ]
                );
            }
            _ => {}
        }
    }
}
