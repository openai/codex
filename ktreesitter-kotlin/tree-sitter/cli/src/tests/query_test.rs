use std::{env, fmt::Write, sync::LazyLock};

use indoc::indoc;
use rand::{prelude::StdRng, SeedableRng};
use streaming_iterator::StreamingIterator;
use tree_sitter::{
    CaptureQuantifier, InputEdit, Language, Node, Parser, Point, Query, QueryCursor,
    QueryCursorOptions, QueryError, QueryErrorKind, QueryPredicate, QueryPredicateArg,
    QueryProperty, Range,
};
use unindent::Unindent;

use super::helpers::{
    allocations,
    fixtures::{get_language, get_test_language},
    query_helpers::{assert_query_matches, Match, Pattern},
};
use crate::tests::{
    generate_parser,
    helpers::query_helpers::{collect_captures, collect_matches},
    ITERATION_COUNT,
};

static EXAMPLE_FILTER: LazyLock<Option<String>> =
    LazyLock::new(|| env::var("TREE_SITTER_TEST_EXAMPLE_FILTER").ok());

#[test]
fn test_query_errors_on_invalid_syntax() {
    allocations::record(|| {
        let language = get_language("javascript");

        assert!(Query::new(&language, "(if_statement)").is_ok());
        assert!(Query::new(
            &language,
            "(if_statement condition:(parenthesized_expression (identifier)))"
        )
        .is_ok());

        // Mismatched parens
        assert_eq!(
            Query::new(&language, "(if_statement").unwrap_err().message,
            [
                "(if_statement", //
                "             ^",
            ]
            .join("\n")
        );
        assert_eq!(
            Query::new(&language, "; comment 1\n; comment 2\n  (if_statement))")
                .unwrap_err()
                .message,
            [
                "  (if_statement))", //
                "                ^",
            ]
            .join("\n")
        );

        // Return an error at the *beginning* of a bare identifier not followed a colon.
        // If there's a colon but no pattern, return an error at the end of the colon.
        assert_eq!(
            Query::new(&language, "(if_statement identifier)")
                .unwrap_err()
                .message,
            [
                "(if_statement identifier)", //
                "              ^",
            ]
            .join("\n")
        );
        assert_eq!(
            Query::new(&language, "(if_statement condition:)")
                .unwrap_err()
                .message,
            [
                "(if_statement condition:)", //
                "                        ^",
            ]
            .join("\n")
        );

        // Return an error at the beginning of an unterminated string.
        assert_eq!(
            Query::new(&language, r#"(identifier) "h "#)
                .unwrap_err()
                .message,
            [
                r#"(identifier) "h "#, //
                r"             ^",
            ]
            .join("\n")
        );

        // Empty tree pattern
        assert_eq!(
            Query::new(&language, r"((identifier) ()")
                .unwrap_err()
                .message,
            [
                "((identifier) ()", //
                "               ^",
            ]
            .join("\n")
        );

        // Empty alternation
        assert_eq!(
            Query::new(&language, r"((identifier) [])")
                .unwrap_err()
                .message,
            [
                "((identifier) [])", //
                "               ^",
            ]
            .join("\n")
        );

        // Unclosed sibling expression with predicate
        assert_eq!(
            Query::new(&language, r"((identifier) (#a)")
                .unwrap_err()
                .message,
            [
                "((identifier) (#a)", //
                "                  ^",
            ]
            .join("\n")
        );

        // Unclosed predicate
        assert_eq!(
            Query::new(&language, r"((identifier) @x (#eq? @x a")
                .unwrap_err()
                .message,
            [
                r"((identifier) @x (#eq? @x a",
                r"                           ^",
            ]
            .join("\n")
        );

        // Need at least one child node for a child anchor
        assert_eq!(
            Query::new(&language, r"(statement_block .)")
                .unwrap_err()
                .message,
            [
                //
                r"(statement_block .)",
                r"                  ^"
            ]
            .join("\n")
        );

        // Need a field name after a negated field operator
        assert_eq!(
            Query::new(&language, r"(statement_block ! (if_statement))")
                .unwrap_err()
                .message,
            [
                r"(statement_block ! (if_statement))",
                r"                   ^"
            ]
            .join("\n")
        );

        // Unclosed alternation within a tree
        // tree-sitter/tree-sitter/issues/968
        assert_eq!(
            Query::new(&get_language("c"), r#"(parameter_list [ ")" @foo)"#)
                .unwrap_err()
                .message,
            [
                r#"(parameter_list [ ")" @foo)"#,
                r"                          ^"
            ]
            .join("\n")
        );

        // Unclosed tree within an alternation
        // tree-sitter/tree-sitter/issues/1436
        assert_eq!(
            Query::new(
                &get_language("python"),
                r"[(unary_operator (_) @operand) (not_operator (_) @operand]"
            )
            .unwrap_err()
            .message,
            [
                r"[(unary_operator (_) @operand) (not_operator (_) @operand]",
                r"                                                         ^"
            ]
            .join("\n")
        );

        // MISSING keyword with full pattern
        assert_eq!(
            Query::new(
                &get_language("c"),
                r"(MISSING (function_declarator (identifier))) "
            )
            .unwrap_err()
            .message,
            [
                r"(MISSING (function_declarator (identifier))) ",
                r"         ^",
            ]
            .join("\n")
        );

        // MISSING keyword with multiple identifiers
        assert_eq!(
            Query::new(
                &get_language("c"),
                r"(MISSING function_declarator function_declarator) "
            )
            .unwrap_err()
            .message,
            [
                r"(MISSING function_declarator function_declarator) ",
                r"                             ^",
            ]
            .join("\n")
        );
    });
}

#[test]
fn test_query_errors_on_invalid_symbols() {
    allocations::record(|| {
        let language = get_language("javascript");

        assert_eq!(
            Query::new(&language, "\">>>>\"").unwrap_err(),
            QueryError {
                row: 0,
                offset: 1,
                column: 1,
                kind: QueryErrorKind::NodeType,
                message: ">>>>".to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "\"te\\\"st\"").unwrap_err(),
            QueryError {
                row: 0,
                offset: 1,
                column: 1,
                kind: QueryErrorKind::NodeType,
                message: "te\\\"st".to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "\"\\\\\" @cap").unwrap_err(),
            QueryError {
                row: 0,
                offset: 1,
                column: 1,
                kind: QueryErrorKind::NodeType,
                message: "\\\\".to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "(clas)").unwrap_err(),
            QueryError {
                row: 0,
                offset: 1,
                column: 1,
                kind: QueryErrorKind::NodeType,
                message: "clas".to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "(if_statement (arrayyyyy))").unwrap_err(),
            QueryError {
                row: 0,
                offset: 15,
                column: 15,
                kind: QueryErrorKind::NodeType,
                message: "arrayyyyy".to_string()
            },
        );
        assert_eq!(
            Query::new(&language, "(if_statement condition: (non_existent3))").unwrap_err(),
            QueryError {
                row: 0,
                offset: 26,
                column: 26,
                kind: QueryErrorKind::NodeType,
                message: "non_existent3".to_string()
            },
        );
        assert_eq!(
            Query::new(&language, "(if_statement condit: (identifier))").unwrap_err(),
            QueryError {
                row: 0,
                offset: 14,
                column: 14,
                kind: QueryErrorKind::Field,
                message: "condit".to_string()
            },
        );
        assert_eq!(
            Query::new(&language, "(if_statement conditioning: (identifier))").unwrap_err(),
            QueryError {
                row: 0,
                offset: 14,
                column: 14,
                kind: QueryErrorKind::Field,
                message: "conditioning".to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "(if_statement !alternativ)").unwrap_err(),
            QueryError {
                row: 0,
                offset: 15,
                column: 15,
                kind: QueryErrorKind::Field,
                message: "alternativ".to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "(if_statement !alternatives)").unwrap_err(),
            QueryError {
                row: 0,
                offset: 15,
                column: 15,
                kind: QueryErrorKind::Field,
                message: "alternatives".to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "fakefield: (identifier)").unwrap_err(),
            QueryError {
                row: 0,
                offset: 0,
                column: 0,
                kind: QueryErrorKind::Field,
                message: "fakefield".to_string()
            }
        );
    });
}

#[test]
fn test_query_errors_on_invalid_predicates() {
    allocations::record(|| {
        let language = get_language("javascript");

        assert_eq!(
            Query::new(&language, "((identifier) @id (@id))").unwrap_err(),
            QueryError {
                kind: QueryErrorKind::Syntax,
                row: 0,
                column: 19,
                offset: 19,
                message: [
                    "((identifier) @id (@id))", //
                    "                   ^"
                ]
                .join("\n")
            }
        );
        assert_eq!(
            Query::new(&language, "((identifier) @id (#eq? @id))").unwrap_err(),
            QueryError {
                kind: QueryErrorKind::Predicate,
                row: 0,
                column: 0,
                offset: 0,
                message: "Wrong number of arguments to #eq? predicate. Expected 2, got 1."
                    .to_string()
            }
        );
        assert_eq!(
            Query::new(&language, "((identifier) @id (#eq? @id @ok))").unwrap_err(),
            QueryError {
                kind: QueryErrorKind::Capture,
                row: 0,
                column: 29,
                offset: 29,
                message: "ok".to_string(),
            }
        );
    });
}

#[test]
fn test_query_errors_on_impossible_patterns() {
    let js_lang = get_language("javascript");
    let rb_lang = get_language("ruby");

    allocations::record(|| {
        assert_eq!(
            Query::new(
                &js_lang,
                "(binary_expression left: (expression (identifier)) left: (expression (identifier)))"
            ),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 0,
                offset: 51,
                column: 51,
                message: [
                    "(binary_expression left: (expression (identifier)) left: (expression (identifier)))",
                    "                                                   ^",
                ]
                .join("\n"),
            })
        );

        Query::new(
            &js_lang,
            "(function_declaration name: (identifier) (statement_block))",
        )
        .unwrap();
        assert_eq!(
            Query::new(&js_lang, "(function_declaration name: (statement_block))"),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 0,
                offset: 22,
                column: 22,
                message: [
                    "(function_declaration name: (statement_block))",
                    "                      ^",
                ]
                .join("\n")
            })
        );

        Query::new(&rb_lang, "(call receiver:(call))").unwrap();
        assert_eq!(
            Query::new(&rb_lang, "(call receiver:(binary))"),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 0,
                offset: 6,
                column: 6,
                message: [
                    "(call receiver:(binary))", //
                    "      ^",
                ]
                .join("\n")
            })
        );

        Query::new(
            &js_lang,
            "[
                (function_expression (identifier))
                (function_declaration (identifier))
                (generator_function_declaration (identifier))
            ]",
        )
        .unwrap();
        assert_eq!(
            Query::new(
                &js_lang,
                "[
                    (function_expression (identifier))
                    (function_declaration (object))
                    (generator_function_declaration (identifier))
                ]",
            ),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 2,
                offset: 99,
                column: 42,
                message: [
                    "                    (function_declaration (object))", //
                    "                                          ^",
                ]
                .join("\n")
            })
        );

        assert_eq!(
            Query::new(&js_lang, "(identifier (identifier))",),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 0,
                offset: 12,
                column: 12,
                message: [
                    "(identifier (identifier))", //
                    "            ^",
                ]
                .join("\n")
            })
        );
        assert_eq!(
            Query::new(&js_lang, "(true (true))",),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 0,
                offset: 6,
                column: 6,
                message: [
                    "(true (true))", //
                    "      ^",
                ]
                .join("\n")
            })
        );

        Query::new(
            &js_lang,
            "(if_statement
                condition: (parenthesized_expression (expression) @cond))",
        )
        .unwrap();

        assert_eq!(
            Query::new(&js_lang, "(if_statement condition: (expression))"),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 0,
                offset: 14,
                column: 14,
                message: [
                    "(if_statement condition: (expression))", //
                    "              ^",
                ]
                .join("\n")
            })
        );
        assert_eq!(
            Query::new(&js_lang, "(identifier/identifier)").unwrap_err(),
            QueryError {
                row: 0,
                offset: 0,
                column: 0,
                kind: QueryErrorKind::Structure,
                message: [
                    "(identifier/identifier)", //
                    "^"
                ]
                .join("\n")
            }
        );

        if js_lang.abi_version() >= 15 {
            assert_eq!(
                Query::new(&js_lang, "(statement/identifier)").unwrap_err(),
                QueryError {
                    row: 0,
                    offset: 0,
                    column: 0,
                    kind: QueryErrorKind::Structure,
                    message: [
                        "(statement/identifier)", //
                        "^"
                    ]
                    .join("\n")
                }
            );
            assert_eq!(
                Query::new(&js_lang, "(statement/pattern)").unwrap_err(),
                QueryError {
                    row: 0,
                    offset: 0,
                    column: 0,
                    kind: QueryErrorKind::Structure,
                    message: [
                        "(statement/pattern)", //
                        "^"
                    ]
                    .join("\n")
                }
            );
        }
    });
}

#[test]
fn test_query_verifies_possible_patterns_with_aliased_parent_nodes() {
    allocations::record(|| {
        let language = get_language("ruby");

        Query::new(&language, "(destructured_parameter (identifier))").unwrap();

        assert_eq!(
            Query::new(&language, "(destructured_parameter (string))",),
            Err(QueryError {
                kind: QueryErrorKind::Structure,
                row: 0,
                offset: 24,
                column: 24,
                message: [
                    "(destructured_parameter (string))", //
                    "                        ^",
                ]
                .join("\n")
            })
        );
    });
}

#[test]
fn test_query_matches_with_simple_pattern() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "(function_declaration name: (identifier) @fn-name)",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "function one() { two(); function three() {} }",
            &[
                (0, vec![("fn-name", "one")]),
                (0, vec![("fn-name", "three")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_multiple_on_same_root() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "(class_declaration
                name: (identifier) @the-class-name
                (class_body
                    (method_definition
                        name: (property_identifier) @the-method-name)))",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            class Person {
                // the constructor
                constructor(name) { this.name = name; }

                // the getter
                getFullName() { return this.name; }
            }
            ",
            &[
                (
                    0,
                    vec![
                        ("the-class-name", "Person"),
                        ("the-method-name", "constructor"),
                    ],
                ),
                (
                    0,
                    vec![
                        ("the-class-name", "Person"),
                        ("the-method-name", "getFullName"),
                    ],
                ),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_multiple_patterns_different_roots() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
                (function_declaration name:(identifier) @fn-def)
                (call_expression function:(identifier) @fn-ref)
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            function f1() {
                f2(f3());
            }
            ",
            &[
                (0, vec![("fn-def", "f1")]),
                (1, vec![("fn-ref", "f2")]),
                (1, vec![("fn-ref", "f3")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_multiple_patterns_same_root() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
              (pair
                key: (property_identifier) @method-def
                value: (function_expression))

              (pair
                key: (property_identifier) @method-def
                value: (arrow_function))
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            a = {
                b: () => { return c; },
                d: function() { return d; }
            };
            ",
            &[
                (1, vec![("method-def", "b")]),
                (0, vec![("method-def", "d")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_nesting_and_no_fields() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
                (array
                    (array
                        (identifier) @x1
                        (identifier) @x2))
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            [[a]];
            [[c, d], [e, f, g, h]];
            [[h], [i]];
            ",
            &[
                (0, vec![("x1", "c"), ("x2", "d")]),
                (0, vec![("x1", "e"), ("x2", "f")]),
                (0, vec![("x1", "e"), ("x2", "g")]),
                (0, vec![("x1", "f"), ("x2", "g")]),
                (0, vec![("x1", "e"), ("x2", "h")]),
                (0, vec![("x1", "f"), ("x2", "h")]),
                (0, vec![("x1", "g"), ("x2", "h")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_many_results() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(&language, "(array (identifier) @element)").unwrap();

        assert_query_matches(
            &language,
            &query,
            &"[hello];\n".repeat(50),
            &vec![(0, vec![("element", "hello")]); 50],
        );
    });
}

#[test]
fn test_query_matches_with_many_overlapping_results() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            (call_expression
                function: (member_expression
                    property: (property_identifier) @method))
            (call_expression
                function: (identifier) @function)
            ((identifier) @constant
             (#match? @constant "[A-Z\\d_]+"))
            "#,
        )
        .unwrap();

        let count = 1024;

        // Deeply nested chained function calls:
        // a
        //    .foo(bar(BAZ))
        //    .foo(bar(BAZ))
        //    .foo(bar(BAZ))
        //    ...
        let mut source = "a".to_string();
        source += &"\n  .foo(bar(BAZ))".repeat(count);

        assert_query_matches(
            &language,
            &query,
            &source,
            &[
                (0, vec![("method", "foo")]),
                (1, vec![("function", "bar")]),
                (2, vec![("constant", "BAZ")]),
            ]
            .iter()
            .cloned()
            .cycle()
            .take(3 * count)
            .collect::<Vec<_>>(),
        );
    });
}

#[test]
fn test_query_matches_capturing_error_nodes() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (ERROR (identifier) @the-error-identifier) @the-error
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "function a(b,, c, d :e:) {}",
            &[(0, vec![("the-error", ":e:"), ("the-error-identifier", "e")])],
        );
    });
}

#[test]
fn test_query_matches_capturing_missing_nodes() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            (MISSING
              ; Comments should be valid
            ) @missing
            (MISSING
              ; Comments should be valid
              ";"
              ; Comments should be valid
              ) @missing-semicolon
            "#,
        )
        .unwrap();

        // Missing anonymous nodes
        assert_query_matches(
            &language,
            &query,
            "
            x = function(a) { b; } function(c) { d; }
            //                    ^ MISSING semicolon here
            ",
            &[
                (0, vec![("missing", "")]),
                (1, vec![("missing-semicolon", "")]),
            ],
        );

        let language = get_language("c");
        let query = Query::new(
            &language,
            "(MISSING field_identifier) @missing-field-ident
            (MISSING identifier) @missing-ident
            (MISSING) @missing-anything",
        )
        .unwrap();

        // Missing named nodes
        assert_query_matches(
            &language,
            &query,
            "
            int main() {
              if (a.) {
              //    ^ MISSING field_identifier here
                b();
                c();

                if (*) d();
                //   ^ MISSING identifier here
              }
            }
            ",
            &[
                (0, vec![("missing-field-ident", "")]),
                (2, vec![("missing-anything", "")]),
                (1, vec![("missing-ident", "")]),
                (2, vec![("missing-anything", "")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_extra_children() {
    allocations::record(|| {
        let language = get_language("ruby");
        let query = Query::new(
            &language,
            "
            (program(comment) @top_level_comment)
            (argument_list (heredoc_body) @heredoc_in_args)
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            # top-level
            puts(
                # not-top-level
                <<-IN_ARGS, bar.baz
                HELLO
                IN_ARGS
            )

            puts <<-NOT_IN_ARGS
            NO
            NOT_IN_ARGS
            ",
            &[
                (0, vec![("top_level_comment", "# top-level")]),
                (
                    1,
                    vec![(
                        "heredoc_in_args",
                        "\n                HELLO\n                IN_ARGS",
                    )],
                ),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_named_wildcard() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (return_statement (_) @the-return-value)
            (binary_expression operator: _ @the-operator)
            ",
        )
        .unwrap();

        let source = "return a + b - c;";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (0, vec![("the-return-value", "a + b - c")]),
                (1, vec![("the-operator", "+")]),
                (1, vec![("the-operator", "-")]),
            ]
        );
    });
}

#[test]
fn test_query_matches_with_wildcard_at_the_root() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (_
                (comment) @doc
                .
                (function_declaration
                    name: (identifier) @name))
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "/* one */ var x; /* two */ function y() {} /* three */ class Z {}",
            &[(0, vec![("doc", "/* two */"), ("name", "y")])],
        );

        let query = Query::new(
            &language,
            "
                (_ (string) @a)
                (_ (number) @b)
                (_ (true) @c)
                (_ (false) @d)
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "['hi', x(true), {y: false}]",
            &[
                (0, vec![("a", "'hi'")]),
                (2, vec![("c", "true")]),
                (3, vec![("d", "false")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_wildcard_within_wildcard() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (_ (_) @child) @parent
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "/* a */ b; c;",
            &[
                (0, vec![("parent", "/* a */ b; c;"), ("child", "/* a */")]),
                (0, vec![("parent", "/* a */ b; c;"), ("child", "b;")]),
                (0, vec![("parent", "b;"), ("child", "b")]),
                (0, vec![("parent", "/* a */ b; c;"), ("child", "c;")]),
                (0, vec![("parent", "c;"), ("child", "c")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_immediate_siblings() {
    allocations::record(|| {
        let language = get_language("python");

        // The immediate child operator '.' can be used in three similar ways:
        // 1. Before the first child node in a pattern, it means that there cannot be any named
        //    siblings before that child node.
        // 2. After the last child node in a pattern, it means that there cannot be any named
        //    sibling after that child node.
        // 2. Between two child nodes in a pattern, it specifies that there cannot be any named
        //    siblings between those two child snodes.
        let query = Query::new(
            &language,
            "
            (dotted_name
                (identifier) @parent
                .
                (identifier) @child)
            (dotted_name
                (identifier) @last-child
                .)
            (list
                .
                (_) @first-element)
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "import a.b.c.d; return [w, [1, y], z]",
            &[
                (0, vec![("parent", "a"), ("child", "b")]),
                (0, vec![("parent", "b"), ("child", "c")]),
                (0, vec![("parent", "c"), ("child", "d")]),
                (1, vec![("last-child", "d")]),
                (2, vec![("first-element", "w")]),
                (2, vec![("first-element", "1")]),
            ],
        );

        let query = Query::new(
            &language,
            "
            (block . (_) @first-stmt)
            (block (_) @stmt)
            (block (_) @last-stmt .)
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            if a:
                b()
                c()
                if d(): e(); f()
                g()
            ",
            &[
                (0, vec![("first-stmt", "b()")]),
                (1, vec![("stmt", "b()")]),
                (1, vec![("stmt", "c()")]),
                (1, vec![("stmt", "if d(): e(); f()")]),
                (0, vec![("first-stmt", "e()")]),
                (1, vec![("stmt", "e()")]),
                (1, vec![("stmt", "f()")]),
                (2, vec![("last-stmt", "f()")]),
                (1, vec![("stmt", "g()")]),
                (2, vec![("last-stmt", "g()")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_last_named_child() {
    allocations::record(|| {
        let language = get_language("c");
        let query = Query::new(
            &language,
            "(compound_statement
                (_)
                (_)
                (expression_statement
                    (identifier) @last_id) .)",
        )
        .unwrap();
        assert_query_matches(
            &language,
            &query,
            "
            void one() { a; b; c; }
            void two() { d; e; }
            void three() { f; g; h; i; }
            ",
            &[(0, vec![("last_id", "c")]), (0, vec![("last_id", "i")])],
        );
    });
}

#[test]
fn test_query_matches_with_negated_fields() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (import_specifier
                !alias
                name: (identifier) @import_name)

            (export_specifier
                !alias
                name: (identifier) @export_name)

            (export_statement
                !decorator
                !source
                (_) @exported)

            ; This negated field list is an extension of a previous
            ; negated field list. The order of the children and negated
            ; fields doesn't matter.
            (export_statement
                !decorator
                !source
                (_) @exported_expr
                !declaration)

            ; This negated field list is a prefix of a previous
            ; negated field list.
            (export_statement
                !decorator
                (_) @export_child .)
            ",
        )
        .unwrap();
        assert_query_matches(
            &language,
            &query,
            "
            import {a as b, c} from 'p1';
            export {g, h as i} from 'p2';

            @foo
            export default 1;

            export var j = 1;

            export default k;
            ",
            &[
                (0, vec![("import_name", "c")]),
                (1, vec![("export_name", "g")]),
                (4, vec![("export_child", "'p2'")]),
                (2, vec![("exported", "var j = 1;")]),
                (4, vec![("export_child", "var j = 1;")]),
                (2, vec![("exported", "k")]),
                (3, vec![("exported_expr", "k")]),
                (4, vec![("export_child", "k")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_field_at_root() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(&language, "name: (identifier) @name").unwrap();
        assert_query_matches(
            &language,
            &query,
            "
            a();
            function b() {}
            class c extends d {}
            ",
            &[(0, vec![("name", "b")]), (0, vec![("name", "c")])],
        );
    });
}

#[test]
fn test_query_matches_with_repeated_leaf_nodes() {
    allocations::record(|| {
        let language = get_language("javascript");

        let query = Query::new(
            &language,
            "
            (
                (comment)+ @doc
                .
                (class_declaration
                    name: (identifier) @name)
            )

            (
                (comment)+ @doc
                .
                (function_declaration
                    name: (identifier) @name)
            )
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            // one
            // two
            a();

            // three
            {
                // four
                // five
                // six
                class B {}

                // seven
                c();

                // eight
                function d() {}
            }
            ",
            &[
                (
                    0,
                    vec![
                        ("doc", "// four"),
                        ("doc", "// five"),
                        ("doc", "// six"),
                        ("name", "B"),
                    ],
                ),
                (1, vec![("doc", "// eight"), ("name", "d")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_optional_nodes_inside_of_repetitions() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(&language, r#"(array (","? (number) @num)+)"#).unwrap();

        assert_query_matches(
            &language,
            &query,
            r"
            var a = [1, 2, 3, 4]
            ",
            &[(
                0,
                vec![("num", "1"), ("num", "2"), ("num", "3"), ("num", "4")],
            )],
        );
    });
}

#[test]
fn test_query_matches_with_top_level_repetitions() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r"
            (comment)+ @doc
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            r"
            // a
            // b
            // c

            d()

            // e
            ",
            &[
                (0, vec![("doc", "// a"), ("doc", "// b"), ("doc", "// c")]),
                (0, vec![("doc", "// e")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_non_terminal_repetitions_within_root() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(&language, "(_ (expression_statement (identifier) @id)+)").unwrap();

        assert_query_matches(
            &language,
            &query,
            r"
            function f() {
                d;
                e;
                f;
                g;
            }
            a;
            b;
            c;
            ",
            &[
                (0, vec![("id", "d"), ("id", "e"), ("id", "f"), ("id", "g")]),
                (0, vec![("id", "a"), ("id", "b"), ("id", "c")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_nested_repetitions() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            (variable_declaration
                (","? (variable_declarator name: (identifier) @x))+)+
            "#,
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            r"
            var a = b, c, d
            var e, f

            // more
            var g
            ",
            &[
                (
                    0,
                    vec![("x", "a"), ("x", "c"), ("x", "d"), ("x", "e"), ("x", "f")],
                ),
                (0, vec![("x", "g")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_multiple_repetition_patterns_that_intersect_other_pattern() {
    allocations::record(|| {
        let language = get_language("javascript");

        // When this query sees a comment, it must keep track of several potential
        // matches: up to two for each pattern that begins with a comment.
        let query = Query::new(
            &language,
            r"
            (call_expression
                function: (member_expression
                    property: (property_identifier) @name)) @ref.method

            ((comment)* @doc (function_declaration))
            ((comment)* @doc (generator_function_declaration))
            ((comment)* @doc (class_declaration))
            ((comment)* @doc (lexical_declaration))
            ((comment)* @doc (variable_declaration))
            ((comment)* @doc (method_definition))

            (comment) @comment
            ",
        )
        .unwrap();

        // Here, a series of comments occurs in the middle of a match of the first
        // pattern. To avoid exceeding the storage limits and discarding that outer
        // match, the comment-related matches need to be managed efficiently.
        let source = format!(
            "theObject\n{}\n.theMethod()",
            "  // the comment\n".repeat(64)
        );

        assert_query_matches(
            &language,
            &query,
            &source,
            &vec![(7, vec![("comment", "// the comment")]); 64]
                .into_iter()
                .chain(vec![(
                    0,
                    vec![("ref.method", source.as_str()), ("name", "theMethod")],
                )])
                .collect::<Vec<_>>(),
        );
    });
}

#[test]
fn test_query_matches_with_trailing_repetitions_of_last_child() {
    allocations::record(|| {
        let language = get_language("javascript");

        let query = Query::new(
            &language,
            "
            (unary_expression (primary_expression)+ @operand)
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            a = typeof (!b && ~c);
            ",
            &[
                (0, vec![("operand", "b")]),
                (0, vec![("operand", "c")]),
                (0, vec![("operand", "(!b && ~c)")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_leading_zero_or_more_repeated_leaf_nodes() {
    allocations::record(|| {
        let language = get_language("javascript");

        let query = Query::new(
            &language,
            "
            (
                (comment)* @doc
                .
                (function_declaration
                    name: (identifier) @name)
            )
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            function a() {
                // one
                var b;

                function c() {}

                // two
                // three
                var d;

                // four
                // five
                function e() {

                }
            }

            // six
            ",
            &[
                (0, vec![("name", "a")]),
                (0, vec![("name", "c")]),
                (
                    0,
                    vec![("doc", "// four"), ("doc", "// five"), ("name", "e")],
                ),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_trailing_optional_nodes() {
    allocations::record(|| {
        let language = get_language("javascript");

        let query = Query::new(
            &language,
            "
            (class_declaration
                name: (identifier) @class
                (class_heritage
                  (identifier) @superclass)?)
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "class A {}",
            &[(0, vec![("class", "A")])],
        );

        assert_query_matches(
            &language,
            &query,
            "
            class A {}
            class B extends C {}
            class D extends (E.F) {}
            ",
            &[
                (0, vec![("class", "A")]),
                (0, vec![("class", "B"), ("superclass", "C")]),
                (0, vec![("class", "D")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_nested_optional_nodes() {
    allocations::record(|| {
        let language = get_language("javascript");

        // A function call, optionally containing a function call, which optionally contains a
        // number
        let query = Query::new(
            &language,
            "
            (call_expression
                function: (identifier) @outer-fn
                arguments: (arguments
                    (call_expression
                        function: (identifier) @inner-fn
                        arguments: (arguments
                            (number)? @num))?))
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            r"
            a(b, c(), d(null, 1, 2))
            e()
            f(g())
            ",
            &[
                (0, vec![("outer-fn", "a"), ("inner-fn", "c")]),
                (0, vec![("outer-fn", "c")]),
                (0, vec![("outer-fn", "a"), ("inner-fn", "d"), ("num", "1")]),
                (0, vec![("outer-fn", "a"), ("inner-fn", "d"), ("num", "2")]),
                (0, vec![("outer-fn", "d")]),
                (0, vec![("outer-fn", "e")]),
                (0, vec![("outer-fn", "f"), ("inner-fn", "g")]),
                (0, vec![("outer-fn", "g")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_repeated_internal_nodes() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (_
                (method_definition
                    (decorator (identifier) @deco)+
                    name: (property_identifier) @name))
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            class A {
                @c
                @d
                e() {}
            }
            ",
            &[(0, vec![("deco", "c"), ("deco", "d"), ("name", "e")])],
        );
    });
}

#[test]
fn test_query_matches_with_simple_alternatives() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (pair
                key: [(property_identifier) (string)] @key
                value: [(function_expression) @val1 (arrow_function) @val2])
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            a = {
                b: c,
                'd': e => f,
                g: {
                    h: function i() {},
                    'x': null,
                    j: _ => k
                },
                'l': function m() {},
            };
            ",
            &[
                (0, vec![("key", "'d'"), ("val2", "e => f")]),
                (0, vec![("key", "h"), ("val1", "function i() {}")]),
                (0, vec![("key", "j"), ("val2", "_ => k")]),
                (0, vec![("key", "'l'"), ("val1", "function m() {}")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_alternatives_in_repetitions() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            (array
                [(identifier) (string)] @el
                .
                (
                    ","
                    .
                    [(identifier) (string)] @el
                )*)
            "#,
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            a = [b, 'c', d, 1, e, 'f', 'g', h];
            ",
            &[
                (0, vec![("el", "b"), ("el", "'c'"), ("el", "d")]),
                (
                    0,
                    vec![("el", "e"), ("el", "'f'"), ("el", "'g'"), ("el", "h")],
                ),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_alternatives_at_root() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            [
                "if"
                "else"
                "function"
                "throw"
                "return"
            ] @keyword
            "#,
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            function a(b, c, d) {
                if (b) {
                    return c;
                } else {
                    throw d;
                }
            }
            ",
            &[
                (0, vec![("keyword", "function")]),
                (0, vec![("keyword", "if")]),
                (0, vec![("keyword", "return")]),
                (0, vec![("keyword", "else")]),
                (0, vec![("keyword", "throw")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_alternatives_under_fields() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r"
            (assignment_expression
                left: [
                    (identifier) @variable
                    (member_expression property: (property_identifier) @variable)
                ])
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            a = b;
            b = c.d;
            e.f = g;
            h.i = j.k;
            ",
            &[
                (0, vec![("variable", "a")]),
                (0, vec![("variable", "b")]),
                (0, vec![("variable", "f")]),
                (0, vec![("variable", "i")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_in_language_with_simple_aliases() {
    allocations::record(|| {
        let language = get_language("html");

        // HTML uses different tokens to track start tags names, end
        // tag names, script tag names, and style tag names. All of
        // these tokens are aliased to `tag_name`.
        let query = Query::new(&language, "(tag_name) @tag").unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            <div>
                <script>hi</script>
                <style>hi</style>
            </div>
            ",
            &[
                (0, vec![("tag", "div")]),
                (0, vec![("tag", "script")]),
                (0, vec![("tag", "script")]),
                (0, vec![("tag", "style")]),
                (0, vec![("tag", "style")]),
                (0, vec![("tag", "div")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_different_tokens_with_the_same_string_value() {
    allocations::record(|| {
        // In Rust, there are two '<' tokens: one for the binary operator,
        // and one with higher precedence for generics.
        let language = get_language("rust");
        let query = Query::new(
            &language,
            r#"
                "<" @less
                ">" @greater
                "#,
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "const A: B<C> = d < e || f > g;",
            &[
                (0, vec![("less", "<")]),
                (1, vec![("greater", ">")]),
                (0, vec![("less", "<")]),
                (1, vec![("greater", ">")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_too_many_permutations_to_track() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (array (identifier) @pre (identifier) @post)
        ",
        )
        .unwrap();

        let mut source = "hello, ".repeat(50);
        source.insert(0, '[');
        source.push_str("];");

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&source, None).unwrap();
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(32);
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        // For this pathological query, some match permutations will be dropped.
        // Just check that a subset of the results are returned, and crash or
        // leak occurs.
        assert_eq!(
            collect_matches(matches, &query, source.as_str())[0],
            (0, vec![("pre", "hello"), ("post", "hello")]),
        );
        assert!(cursor.did_exceed_match_limit());
    });
}

#[test]
fn test_query_sibling_patterns_dont_match_children_of_an_error() {
    allocations::record(|| {
        let language = get_language("rust");
        let query = Query::new(
            &language,
            r#"
            ("{" @open "}" @close)

            [
              (line_comment)
              (block_comment)
            ] @comment

            ("<" @first "<" @second)
            "#,
        )
        .unwrap();

        // Most of the document will fail to parse, resulting in a
        // large number of tokens that are *direct* children of an
        // ERROR node.
        //
        // These children should still match, unless they are part
        // of a "non-rooted" pattern, in which there are multiple
        // top-level sibling nodes. Those patterns should not match
        // directly inside of an error node, because the contents of
        // an error node are not syntactically well-structured, so we
        // would get many spurious matches.
        let source = "
            fn a() {}

            <<<<<<<<<< add pub b fn () {}
            // comment 1
            pub fn b() {
            /* comment 2 */
            ==========
            pub fn c() {
            // comment 3
            >>>>>>>>>> add pub c fn () {}
            }
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (0, vec![("open", "{"), ("close", "}")]),
                (1, vec![("comment", "// comment 1")]),
                (1, vec![("comment", "/* comment 2 */")]),
                (1, vec![("comment", "// comment 3")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_alternatives_and_too_many_permutations_to_track() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
            (
                (comment) @doc
                ; not immediate
                (class_declaration) @class
            )

            (call_expression
                function: [
                    (identifier) @function
                    (member_expression property: (property_identifier) @method)
                ])
            ",
        )
        .unwrap();

        let source = "/* hi */ a.b(); ".repeat(50);

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&source, None).unwrap();
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(32);
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        assert_eq!(
            collect_matches(matches, &query, source.as_str()),
            vec![(1, vec![("method", "b")]); 50],
        );
        assert!(cursor.did_exceed_match_limit());
    });
}

#[test]
fn test_repetitions_before_with_alternatives() {
    allocations::record(|| {
        let language = get_language("rust");
        let query = Query::new(
            &language,
            r"
            (
                (line_comment)* @comment
                .
                [
                    (struct_item name: (_) @name)
                    (function_item name: (_) @name)
                    (enum_item name: (_) @name)
                    (impl_item type: (_) @name)
                ]
            )
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            r"
            // a
            // b
            fn c() {}

            // d
            // e
            impl F {}
            ",
            &[
                (
                    0,
                    vec![("comment", "// a"), ("comment", "// b"), ("name", "c")],
                ),
                (
                    0,
                    vec![("comment", "// d"), ("comment", "// e"), ("name", "F")],
                ),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_anonymous_tokens() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            ";" @punctuation
            "&&" @operator
            "\"" @quote
            "#,
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            r#"foo(a && "b");"#,
            &[
                (1, vec![("operator", "&&")]),
                (2, vec![("quote", "\"")]),
                (2, vec![("quote", "\"")]),
                (0, vec![("punctuation", ";")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_supertypes() {
    allocations::record(|| {
        let language = get_language("python");
        let query = Query::new(
            &language,
            r"
            (argument_list (expression) @arg)

            (keyword_argument
                value: (expression) @kw_arg)

            (assignment
              left: (identifier) @var_def)

            (primary_expression/identifier) @var_ref
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
                a = b.c(
                    [d],
                    # a comment
                    e=f
                )
            ",
            &[
                (2, vec![("var_def", "a")]),
                (3, vec![("var_ref", "b")]),
                (0, vec![("arg", "[d]")]),
                (3, vec![("var_ref", "d")]),
                (1, vec![("kw_arg", "f")]),
                (3, vec![("var_ref", "f")]),
            ],
        );
    });
}

#[test]
#[allow(clippy::reversed_empty_ranges)]
fn test_query_matches_within_byte_range() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(&language, "(identifier) @element").unwrap();

        let source = "[a, b, c, d, e, f, g]";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let mut cursor = QueryCursor::new();

        let matches =
            cursor
                .set_byte_range(0..8)
                .matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (0, vec![("element", "a")]),
                (0, vec![("element", "b")]),
                (0, vec![("element", "c")]),
            ]
        );

        let matches =
            cursor
                .set_byte_range(5..15)
                .matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (0, vec![("element", "c")]),
                (0, vec![("element", "d")]),
                (0, vec![("element", "e")]),
            ]
        );

        // An end byte of zero indicates there is no end
        let matches =
            cursor
                .set_byte_range(12..0)
                .matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (0, vec![("element", "e")]),
                (0, vec![("element", "f")]),
                (0, vec![("element", "g")]),
            ]
        );
    });
}

#[test]
fn test_query_matches_within_point_range() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(&language, "(identifier) @element").unwrap();

        let source = "
            [
              a, b,
              c, d,
              e, f,
              g, h,
              i, j,
              k, l,
            ]
        "
        .unindent();

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let matches = cursor
            .set_point_range(Point::new(1, 0)..Point::new(2, 3))
            .matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, &source),
            &[
                (0, vec![("element", "a")]),
                (0, vec![("element", "b")]),
                (0, vec![("element", "c")]),
            ]
        );

        let matches = cursor
            .set_point_range(Point::new(2, 0)..Point::new(3, 3))
            .matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, &source),
            &[
                (0, vec![("element", "c")]),
                (0, vec![("element", "d")]),
                (0, vec![("element", "e")]),
            ]
        );

        // Zero end point is treated like no end point.
        let matches = cursor
            .set_point_range(Point::new(4, 1)..Point::new(0, 0))
            .matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, &source),
            &[
                (0, vec![("element", "g")]),
                (0, vec![("element", "h")]),
                (0, vec![("element", "i")]),
                (0, vec![("element", "j")]),
                (0, vec![("element", "k")]),
                (0, vec![("element", "l")]),
            ]
        );
    });
}

#[test]
fn test_query_captures_within_byte_range() {
    allocations::record(|| {
        let language = get_language("c");
        let query = Query::new(
            &language,
            "
            (call_expression
                function: (identifier) @function
                arguments: (argument_list (string_literal) @string.arg))

            (string_literal) @string
           ",
        )
        .unwrap();

        let source = r#"DEFUN ("safe-length", Fsafe_length, Ssafe_length, 1, 1, 0)"#;

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let mut cursor = QueryCursor::new();
        let captures =
            cursor
                .set_byte_range(3..27)
                .captures(&query, tree.root_node(), source.as_bytes());

        assert_eq!(
            collect_captures(captures, &query, source),
            &[
                ("function", "DEFUN"),
                ("string.arg", "\"safe-length\""),
                ("string", "\"safe-length\""),
            ]
        );
    });
}

#[test]
fn test_query_cursor_next_capture_with_byte_range() {
    allocations::record(|| {
        let language = get_language("python");
        let query = Query::new(
            &language,
            "(function_definition name: (identifier) @function)
             (attribute attribute: (identifier) @property)
             ((identifier) @variable)",
        )
        .unwrap();

        let source = "def func():\n  foo.bar.baz()\n";
        //            ^            ^    ^          ^
        // byte_pos   0           12    17        27
        // point_pos (0,0)      (1,0)  (1,5)    (1,15)

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let mut cursor = QueryCursor::new();
        let captures =
            cursor
                .set_byte_range(12..17)
                .captures(&query, tree.root_node(), source.as_bytes());

        assert_eq!(
            collect_captures(captures, &query, source),
            &[("variable", "foo"),]
        );
    });
}

#[test]
fn test_query_cursor_next_capture_with_point_range() {
    allocations::record(|| {
        let language = get_language("python");
        let query = Query::new(
            &language,
            "(function_definition name: (identifier) @function)
             (attribute attribute: (identifier) @property)
             ((identifier) @variable)",
        )
        .unwrap();

        let source = "def func():\n  foo.bar.baz()\n";
        //            ^            ^    ^          ^
        // byte_pos   0           12    17        27
        // point_pos (0,0)      (1,0)  (1,5)    (1,15)

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let mut cursor = QueryCursor::new();
        let captures = cursor
            .set_point_range(Point::new(1, 0)..Point::new(1, 5))
            .captures(&query, tree.root_node(), source.as_bytes());

        assert_eq!(
            collect_captures(captures, &query, source),
            &[("variable", "foo"),]
        );
    });
}

#[test]
fn test_query_matches_with_unrooted_patterns_intersecting_byte_range() {
    allocations::record(|| {
        let language = get_language("rust");
        let query = Query::new(
            &language,
            r#"
            ("{" @left "}" @right)
            ("<" @left ">" @right)
            "#,
        )
        .unwrap();

        let source = "mod a { fn a<B: C, D: E>(f: B) { g(f) } }";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        // within the type parameter list
        let offset = source.find("D: E>").unwrap();
        let matches = cursor.set_byte_range(offset..offset).matches(
            &query,
            tree.root_node(),
            source.as_bytes(),
        );
        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (1, vec![("left", "<"), ("right", ">")]),
                (0, vec![("left", "{"), ("right", "}")]),
            ]
        );

        // from within the type parameter list to within the function body
        let start_offset = source.find("D: E>").unwrap();
        let end_offset = source.find("g(f)").unwrap();
        let matches = cursor.set_byte_range(start_offset..end_offset).matches(
            &query,
            tree.root_node(),
            source.as_bytes(),
        );
        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (1, vec![("left", "<"), ("right", ">")]),
                (0, vec![("left", "{"), ("right", "}")]),
                (0, vec![("left", "{"), ("right", "}")]),
            ]
        );
    });
}

#[test]
fn test_query_matches_with_wildcard_at_root_intersecting_byte_range() {
    allocations::record(|| {
        let language = get_language("python");
        let query = Query::new(
            &language,
            "
            [
                (_ body: (block))
                (_ consequence: (block))
            ] @indent
            ",
        )
        .unwrap();

        let source = "
            class A:
                def b():
                    if c:
                        d
                    else:
                        e
        "
        .trim();

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        // After the first line of the class definition
        let offset = source.find("A:").unwrap() + 2;
        let mut matches = Vec::new();
        let mut match_iter = cursor.set_byte_range(offset..offset).matches(
            &query,
            tree.root_node(),
            source.as_bytes(),
        );

        while let Some(mat) = match_iter.next() {
            if let Some(capture) = mat.captures.first() {
                matches.push(capture.node.kind());
            }
        }
        assert_eq!(matches, &["class_definition"]);

        // After the first line of the function definition
        let offset = source.find("b():").unwrap() + 4;
        let mut matches = Vec::new();
        let mut match_iter = cursor.set_byte_range(offset..offset).matches(
            &query,
            tree.root_node(),
            source.as_bytes(),
        );

        while let Some(mat) = match_iter.next() {
            if let Some(capture) = mat.captures.first() {
                matches.push(capture.node.kind());
            }
        }
        assert_eq!(matches, &["class_definition", "function_definition"]);

        // After the first line of the if statement
        let offset = source.find("c:").unwrap() + 2;
        let mut matches = Vec::new();
        let mut match_iter = cursor.set_byte_range(offset..offset).matches(
            &query,
            tree.root_node(),
            source.as_bytes(),
        );

        while let Some(mat) = match_iter.next() {
            if let Some(capture) = mat.captures.first() {
                matches.push(capture.node.kind());
            }
        }
        assert_eq!(
            matches,
            &["class_definition", "function_definition", "if_statement"]
        );
    });
}

#[test]
fn test_query_captures_within_byte_range_assigned_after_iterating() {
    allocations::record(|| {
        let language = get_language("rust");
        let query = Query::new(
            &language,
            r#"
            (function_item
                name: (identifier) @fn_name)

            (mod_item
                name: (identifier) @mod_name
                body: (declaration_list
                    "{" @lbrace
                    "}" @rbrace))

            ; functions that return Result<()>
            ((function_item
                return_type: (generic_type
                    type: (type_identifier) @result
                    type_arguments: (type_arguments
                        (unit_type)))
                body: _ @fallible_fn_body)
             (#eq? @result "Result"))
            "#,
        )
        .unwrap();
        let source = "
        mod m1 {
            mod m2 {
                fn f1() -> Option<()> { Some(()) }
            }
            fn f2() -> Result<()> { Ok(()) }
            fn f3() {}
        }
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let mut captures = cursor.captures(&query, tree.root_node(), source.as_bytes());

        // Retrieve some captures
        let mut results = Vec::new();
        let mut first_five = captures.by_ref().take(5);
        while let Some((mat, capture_ix)) = first_five.next() {
            let capture = mat.captures[*capture_ix];
            results.push((
                query.capture_names()[capture.index as usize],
                &source[capture.node.byte_range()],
            ));
        }
        assert_eq!(
            results,
            vec![
                ("mod_name", "m1"),
                ("lbrace", "{"),
                ("mod_name", "m2"),
                ("lbrace", "{"),
                ("fn_name", "f1"),
            ]
        );

        // Advance to a range that only partially intersects some matches.
        // Captures from these matches are reported, but only those that
        // intersect the range.
        results.clear();
        captures.set_byte_range(source.find("Ok").unwrap()..source.len());
        while let Some((mat, capture_ix)) = captures.next() {
            let capture = mat.captures[*capture_ix];
            results.push((
                query.capture_names()[capture.index as usize],
                &source[capture.node.byte_range()],
            ));
        }
        assert_eq!(
            results,
            vec![
                ("fallible_fn_body", "{ Ok(()) }"),
                ("fn_name", "f3"),
                ("rbrace", "}")
            ]
        );
    });
}

#[test]
fn test_query_matches_within_range_of_long_repetition() {
    allocations::record(|| {
        let language = get_language("rust");
        let query = Query::new(
            &language,
            "
            (function_item name: (identifier) @fn-name)
            ",
        )
        .unwrap();

        let source = "
            fn zero() {}
            fn one() {}
            fn two() {}
            fn three() {}
            fn four() {}
            fn five() {}
            fn six() {}
            fn seven() {}
            fn eight() {}
            fn nine() {}
            fn ten() {}
            fn eleven() {}
            fn twelve() {}
        "
        .unindent();

        let mut parser = Parser::new();
        let mut cursor = QueryCursor::new();

        parser.set_language(&language).unwrap();
        let tree = parser.parse(&source, None).unwrap();

        let matches = cursor
            .set_point_range(Point::new(8, 0)..Point::new(20, 0))
            .matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, &source),
            &[
                (0, vec![("fn-name", "eight")]),
                (0, vec![("fn-name", "nine")]),
                (0, vec![("fn-name", "ten")]),
                (0, vec![("fn-name", "eleven")]),
                (0, vec![("fn-name", "twelve")]),
            ]
        );
    });
}

#[test]
fn test_query_matches_different_queries_same_cursor() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query1 = Query::new(
            &language,
            "
            (array (identifier) @id1)
        ",
        )
        .unwrap();
        let query2 = Query::new(
            &language,
            "
            (array (identifier) @id1)
            (pair (identifier) @id2)
        ",
        )
        .unwrap();
        let query3 = Query::new(
            &language,
            "
            (array (identifier) @id1)
            (pair (identifier) @id2)
            (parenthesized_expression (identifier) @id3)
        ",
        )
        .unwrap();

        let source = "[a, {b: b}, (c)];";

        let mut parser = Parser::new();
        let mut cursor = QueryCursor::new();

        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let matches = cursor.matches(&query1, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query1, source),
            &[(0, vec![("id1", "a")]),]
        );

        let matches = cursor.matches(&query3, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query3, source),
            &[
                (0, vec![("id1", "a")]),
                (1, vec![("id2", "b")]),
                (2, vec![("id3", "c")]),
            ]
        );

        let matches = cursor.matches(&query2, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query2, source),
            &[(0, vec![("id1", "a")]), (1, vec![("id2", "b")]),]
        );
    });
}

#[test]
fn test_query_matches_with_multiple_captures_on_a_node() {
    allocations::record(|| {
        let language = get_language("javascript");
        let mut query = Query::new(
            &language,
            "(function_declaration
                (identifier) @name1 @name2 @name3
                (statement_block) @body1 @body2)",
        )
        .unwrap();

        let source = "function foo() { return 1; }";
        let mut parser = Parser::new();
        let mut cursor = QueryCursor::new();

        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[(
                0,
                vec![
                    ("name1", "foo"),
                    ("name2", "foo"),
                    ("name3", "foo"),
                    ("body1", "{ return 1; }"),
                    ("body2", "{ return 1; }"),
                ]
            ),]
        );

        // disabling captures still works when there are multiple captures on a
        // single node.
        query.disable_capture("name2");
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[(
                0,
                vec![
                    ("name1", "foo"),
                    ("name3", "foo"),
                    ("body1", "{ return 1; }"),
                    ("body2", "{ return 1; }"),
                ]
            ),]
        );
    });
}

#[test]
fn test_query_matches_with_captured_wildcard_at_root() {
    allocations::record(|| {
        let language = get_language("python");
        let query = Query::new(
            &language,
            "
            ; captured wildcard at the root
            (_ [
                (except_clause (block) @block)
                (finally_clause (block) @block)
            ]) @stmt

            [
                (while_statement (block) @block)
                (if_statement (block) @block)

                ; captured wildcard at the root within an alternation
                (_ [
                    (else_clause (block) @block)
                    (elif_clause (block) @block)
                ])

                (try_statement (block) @block)
                (for_statement (block) @block)
            ] @stmt
            ",
        )
        .unwrap();

        let source = "
        for i in j:
            while True:
                if a:
                    print b
                elif c:
                    print d
                else:
                    try:
                        print f
                    except:
                        print g
                    finally:
                        print h
            else:
                print i
        "
        .trim();

        let mut parser = Parser::new();
        let mut cursor = QueryCursor::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let mut match_capture_names_and_rows = Vec::new();
        let mut match_iter = cursor.matches(&query, tree.root_node(), source.as_bytes());

        while let Some(m) = match_iter.next() {
            let captures = m
                .captures
                .iter()
                .map(|c| {
                    (
                        query.capture_names()[c.index as usize],
                        c.node.kind(),
                        c.node.start_position().row,
                    )
                })
                .collect::<Vec<_>>();
            match_capture_names_and_rows.push(captures);
        }

        assert_eq!(
            match_capture_names_and_rows,
            &[
                vec![("stmt", "for_statement", 0), ("block", "block", 1)],
                vec![("stmt", "while_statement", 1), ("block", "block", 2)],
                vec![("stmt", "if_statement", 2), ("block", "block", 3)],
                vec![("stmt", "if_statement", 2), ("block", "block", 5)],
                vec![("stmt", "if_statement", 2), ("block", "block", 7)],
                vec![("stmt", "try_statement", 7), ("block", "block", 8)],
                vec![("stmt", "try_statement", 7), ("block", "block", 10)],
                vec![("stmt", "try_statement", 7), ("block", "block", 12)],
                vec![("stmt", "while_statement", 1), ("block", "block", 14)],
            ]
        );
    });
}

#[test]
fn test_query_matches_with_no_captures() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r"
            (identifier)
            (string) @s
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            a = 'hi';
            b = 'bye';
            ",
            &[
                (0, vec![]),
                (1, vec![("s", "'hi'")]),
                (0, vec![]),
                (1, vec![("s", "'bye'")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_repeated_fields() {
    allocations::record(|| {
        let language = get_language("c");
        let query = Query::new(
            &language,
            "(field_declaration declarator: (field_identifier) @field)",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            struct S {
                int a, b, c;
            };
            ",
            &[
                (0, vec![("field", "a")]),
                (0, vec![("field", "b")]),
                (0, vec![("field", "c")]),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_deeply_nested_patterns_with_fields() {
    allocations::record(|| {
        let language = get_language("python");
        let query = Query::new(
            &language,
            "
            (call
                function: (_) @func
                arguments: (_) @args)
            (call
                function: (attribute
                    object: (_) @receiver
                    attribute: (identifier) @method)
                arguments: (argument_list))

            ; These don't match anything, but they require additional
            ; states to keep track of their captures.
            (call
                function: (_) @fn
                arguments: (argument_list
                    (keyword_argument
                        name: (identifier) @name
                        value: (_) @val) @arg) @args) @call
            (call
                function: (identifier) @fn
                (#eq? @fn \"super\")) @super_call
            ",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            a(1).b(2).c(3).d(4).e(5).f(6).g(7).h(8)
            ",
            &[
                (0, vec![("func", "a"), ("args", "(1)")]),
                (0, vec![("func", "a(1).b"), ("args", "(2)")]),
                (1, vec![("receiver", "a(1)"), ("method", "b")]),
                (0, vec![("func", "a(1).b(2).c"), ("args", "(3)")]),
                (1, vec![("receiver", "a(1).b(2)"), ("method", "c")]),
                (0, vec![("func", "a(1).b(2).c(3).d"), ("args", "(4)")]),
                (1, vec![("receiver", "a(1).b(2).c(3)"), ("method", "d")]),
                (0, vec![("func", "a(1).b(2).c(3).d(4).e"), ("args", "(5)")]),
                (
                    1,
                    vec![("receiver", "a(1).b(2).c(3).d(4)"), ("method", "e")],
                ),
                (
                    0,
                    vec![("func", "a(1).b(2).c(3).d(4).e(5).f"), ("args", "(6)")],
                ),
                (
                    1,
                    vec![("receiver", "a(1).b(2).c(3).d(4).e(5)"), ("method", "f")],
                ),
                (
                    0,
                    vec![("func", "a(1).b(2).c(3).d(4).e(5).f(6).g"), ("args", "(7)")],
                ),
                (
                    1,
                    vec![
                        ("receiver", "a(1).b(2).c(3).d(4).e(5).f(6)"),
                        ("method", "g"),
                    ],
                ),
                (
                    0,
                    vec![
                        ("func", "a(1).b(2).c(3).d(4).e(5).f(6).g(7).h"),
                        ("args", "(8)"),
                    ],
                ),
                (
                    1,
                    vec![
                        ("receiver", "a(1).b(2).c(3).d(4).e(5).f(6).g(7)"),
                        ("method", "h"),
                    ],
                ),
            ],
        );
    });
}

#[test]
fn test_query_matches_with_indefinite_step_containing_no_captures() {
    allocations::record(|| {
        // This pattern depends on the field declarations within the
        // struct's body, but doesn't capture anything within the body.
        // It demonstrates that internally, state-splitting needs to occur
        // for each field declaration within the body, in order to avoid
        // prematurely failing if the first field does not match.
        //
        // https://github.com/tree-sitter/tree-sitter/issues/937
        let language = get_language("c");
        let query = Query::new(
            &language,
            "(struct_specifier
                name: (type_identifier) @name
                body: (field_declaration_list
                    (field_declaration
                        type: (union_specifier))))",
        )
        .unwrap();

        assert_query_matches(
            &language,
            &query,
            "
            struct LacksUnionField {
                int a;
                struct {
                    B c;
                } d;
                G *h;
            };

            struct HasUnionField {
                int a;
                struct {
                    B c;
                } d;
                union {
                    bool e;
                    float f;
                } g;
                G *h;
            };
            ",
            &[(0, vec![("name", "HasUnionField")])],
        );
    });
}

#[test]
fn test_query_captures_basic() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            (pair
              key: _ @method.def
              (function_expression
                name: (identifier) @method.alias))

            (variable_declarator
              name: _ @function.def
              value: (function_expression
                name: (identifier) @function.alias))

            ":" @delimiter
            "=" @operator
            "#,
        )
        .unwrap();

        let source = "
          a({
            bc: function de() {
              const fg = function hi() {}
            },
            jk: function lm() {
              const no = function pq() {}
            },
          });
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (2, vec![("delimiter", ":")]),
                (0, vec![("method.def", "bc"), ("method.alias", "de")]),
                (3, vec![("operator", "=")]),
                (1, vec![("function.def", "fg"), ("function.alias", "hi")]),
                (2, vec![("delimiter", ":")]),
                (0, vec![("method.def", "jk"), ("method.alias", "lm")]),
                (3, vec![("operator", "=")]),
                (1, vec![("function.def", "no"), ("function.alias", "pq")]),
            ],
        );

        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_captures(captures, &query, source),
            &[
                ("method.def", "bc"),
                ("delimiter", ":"),
                ("method.alias", "de"),
                ("function.def", "fg"),
                ("operator", "="),
                ("function.alias", "hi"),
                ("method.def", "jk"),
                ("delimiter", ":"),
                ("method.alias", "lm"),
                ("function.def", "no"),
                ("operator", "="),
                ("function.alias", "pq"),
            ]
        );
    });
}

#[test]
fn test_query_captures_with_text_conditions() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            ((identifier) @constant
             (#match? @constant "^[A-Z]{2,}$"))

             ((identifier) @constructor
              (#match? @constructor "^[A-Z]"))

            ((identifier) @function.builtin
             (#eq? @function.builtin "require"))

            ((identifier) @variable.builtin
              (#any-of? @variable.builtin
                        "arguments"
                        "module"
                        "console"
                        "window"
                        "document"))

            ((identifier) @variable
             (#not-match? @variable "^(lambda|load)$"))
            "#,
        )
        .unwrap();

        let source = "
          toad
          load
          panda
          lambda
          const ab = require('./ab');
          new Cd(EF);
          document;
          module;
          console;
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_captures(captures, &query, source),
            &[
                ("variable", "toad"),
                ("variable", "panda"),
                ("variable", "ab"),
                ("function.builtin", "require"),
                ("variable", "require"),
                ("constructor", "Cd"),
                ("variable", "Cd"),
                ("constant", "EF"),
                ("constructor", "EF"),
                ("variable", "EF"),
                ("variable.builtin", "document"),
                ("variable", "document"),
                ("variable.builtin", "module"),
                ("variable", "module"),
                ("variable.builtin", "console"),
                ("variable", "console"),
            ],
        );
    });
}

#[test]
fn test_query_captures_with_predicates() {
    allocations::record(|| {
        let language = get_language("javascript");

        let query = Query::new(
            &language,
            r"
            ((call_expression (identifier) @foo)
             (#set! name something)
             (#set! cool)
             (#something! @foo omg))

            ((property_identifier) @bar
             (#is? cool)
             (#is-not? name something))",
        )
        .unwrap();

        assert_eq!(
            query.property_settings(0),
            &[
                QueryProperty::new("name", Some("something"), None),
                QueryProperty::new("cool", None, None),
            ]
        );
        assert_eq!(
            query.general_predicates(0),
            &[QueryPredicate {
                operator: "something!".to_string().into_boxed_str(),
                args: vec![
                    QueryPredicateArg::Capture(0),
                    QueryPredicateArg::String("omg".to_string().into_boxed_str()),
                ]
                .into_boxed_slice(),
            },]
        );
        assert_eq!(query.property_settings(1), &[]);
        assert_eq!(query.property_predicates(0), &[]);
        assert_eq!(
            query.property_predicates(1),
            &[
                (QueryProperty::new("cool", None, None), true),
                (QueryProperty::new("name", Some("something"), None), false),
            ]
        );

        let source = "const a = window.b";
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let query = Query::new(
            &language,
            r#"((identifier) @variable.builtin
                (#match? @variable.builtin "^(arguments|module|console|window|document)$")
                (#is-not? local))
            "#,
        )
        .unwrap();

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        let matches = collect_matches(matches, &query, source);

        assert_eq!(matches, &[(0, vec![("variable.builtin", "window")])]);
    });
}

#[test]
fn test_query_captures_with_quoted_predicate_args() {
    allocations::record(|| {
        let language = get_language("javascript");

        // Double-quoted strings can contain:
        // * special escape sequences like \n and \r
        // * escaped double quotes with \*
        // * literal backslashes with \\
        let query = Query::new(
            &language,
            r#"
            ((call_expression (identifier) @foo)
             (#set! one "\"something\ngreat\""))

            ((identifier)
             (#set! two "\\s(\r?\n)*$"))

            ((function_declaration)
             (#set! three "\"something\ngreat\""))
            "#,
        )
        .unwrap();

        assert_eq!(
            query.property_settings(0),
            &[QueryProperty::new(
                "one",
                Some("\"something\ngreat\""),
                None
            )]
        );
        assert_eq!(
            query.property_settings(1),
            &[QueryProperty::new("two", Some("\\s(\r?\n)*$"), None)]
        );
        assert_eq!(
            query.property_settings(2),
            &[QueryProperty::new(
                "three",
                Some("\"something\ngreat\""),
                None
            )]
        );
    });
}

#[test]
fn test_query_captures_with_duplicates() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r"
            (variable_declarator
                name: (identifier) @function
                value: (function_expression))

            (identifier) @variable
            ",
        )
        .unwrap();

        let source = "
          var x = function() {};
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_captures(captures, &query, source),
            &[("function", "x"), ("variable", "x"),],
        );
    });
}

#[test]
fn test_query_captures_with_many_nested_results_without_fields() {
    allocations::record(|| {
        let language = get_language("javascript");

        // Search for key-value pairs whose values are anonymous functions.
        let query = Query::new(
            &language,
            r#"
            (pair
              key: _ @method-def
              (arrow_function))

            ":" @colon
            "," @comma
            "#,
        )
        .unwrap();

        // The `pair` node for key `y` does not match any pattern, but inside of
        // its value, it contains many other `pair` nodes that do match the pattern.
        // The match for the *outer* pair should be terminated *before* descending into
        // the object value, so that we can avoid needing to buffer all of the inner
        // matches.
        let method_count = 50;
        let mut source = "x = { y: {\n".to_owned();
        for i in 0..method_count {
            writeln!(&mut source, "    method{i}: $ => null,").unwrap();
        }
        source.push_str("}};\n");

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        let captures = collect_captures(captures, &query, &source);

        assert_eq!(
            &captures[0..13],
            &[
                ("colon", ":"),
                ("method-def", "method0"),
                ("colon", ":"),
                ("comma", ","),
                ("method-def", "method1"),
                ("colon", ":"),
                ("comma", ","),
                ("method-def", "method2"),
                ("colon", ":"),
                ("comma", ","),
                ("method-def", "method3"),
                ("colon", ":"),
                ("comma", ","),
            ]
        );

        // Ensure that we don't drop matches because of needing to buffer too many.
        assert_eq!(captures.len(), 1 + 3 * method_count);
    });
}

#[test]
fn test_query_captures_with_many_nested_results_with_fields() {
    allocations::record(|| {
        let language = get_language("javascript");

        // Search expressions like `a ? a.b : null`
        let query = Query::new(
            &language,
            r"
            ((ternary_expression
                condition: (identifier) @left
                consequence: (member_expression
                    object: (identifier) @right)
                alternative: (null))
             (#eq? @left @right))
            ",
        )
        .unwrap();

        // The outer expression does not match the pattern, but the consequence of the ternary
        // is an object that *does* contain many occurrences of the pattern.
        let count = 50;
        let mut source = "a ? {".to_owned();
        for i in 0..count {
            writeln!(&mut source, "  x: y{i} ? y{i}.z : null,").unwrap();
        }
        source.push_str("} : null;\n");

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        let captures = collect_captures(captures, &query, &source);

        assert_eq!(
            &captures[0..20],
            &[
                ("left", "y0"),
                ("right", "y0"),
                ("left", "y1"),
                ("right", "y1"),
                ("left", "y2"),
                ("right", "y2"),
                ("left", "y3"),
                ("right", "y3"),
                ("left", "y4"),
                ("right", "y4"),
                ("left", "y5"),
                ("right", "y5"),
                ("left", "y6"),
                ("right", "y6"),
                ("left", "y7"),
                ("right", "y7"),
                ("left", "y8"),
                ("right", "y8"),
                ("left", "y9"),
                ("right", "y9"),
            ]
        );

        // Ensure that we don't drop matches because of needing to buffer too many.
        assert_eq!(captures.len(), 2 * count);
    });
}

#[test]
fn test_query_captures_with_too_many_nested_results() {
    allocations::record(|| {
        let language = get_language("javascript");

        // Search for method calls in general, and also method calls with a template string
        // in place of an argument list (aka "tagged template strings") in particular.
        //
        // This second pattern, which looks for the tagged template strings, is expensive to
        // use with the `captures()` method, because:
        // 1. When calling `captures`, all of the captures must be returned in order of their
        //    appearance.
        // 2. This pattern captures the root `call_expression`.
        // 3. This pattern's result also depends on the final child (the template string).
        // 4. In between the `call_expression` and the possible `template_string`, there can be an
        //    arbitrarily deep subtree.
        //
        // This means that, if any patterns match *after* the initial `call_expression` is
        // captured, but before the final `template_string` is found, those matches must
        // be buffered, in order to prevent captures from being returned out-of-order.
        let query = Query::new(
            &language,
            r"
            ;; easy 👇
            (call_expression
              function: (member_expression
                property: (property_identifier) @method-name))

            ;; hard 👇
            (call_expression
              function: (member_expression
                property: (property_identifier) @template-tag)
              arguments: (template_string)) @template-call
            ",
        )
        .unwrap();

        // There are a *lot* of matches in between the beginning of the outer `call_expression`
        // (the call to `a(...).f`), which starts at the beginning of the file, and the final
        // template string, which occurs at the end of the file. The query algorithm imposes a
        // limit on the total number of matches which can be buffered at a time. But we don't
        // want to neglect the inner matches just because of the expensive outer match, so we
        // abandon the outer match (which would have captured `f` as a `template-tag`).
        let source = "
        a(b => {
            b.c0().d0 `😄`;
            b.c1().d1 `😄`;
            b.c2().d2 `😄`;
            b.c3().d3 `😄`;
            b.c4().d4 `😄`;
            b.c5().d5 `😄`;
            b.c6().d6 `😄`;
            b.c7().d7 `😄`;
            b.c8().d8 `😄`;
            b.c9().d9 `😄`;
        }).e().f ``;
        "
        .trim();

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(32);
        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        let captures = collect_captures(captures, &query, source);

        assert_eq!(
            &captures[0..4],
            &[
                ("template-call", "b.c0().d0 `😄`"),
                ("method-name", "c0"),
                ("method-name", "d0"),
                ("template-tag", "d0"),
            ]
        );
        assert_eq!(
            &captures[36..40],
            &[
                ("template-call", "b.c9().d9 `😄`"),
                ("method-name", "c9"),
                ("method-name", "d9"),
                ("template-tag", "d9"),
            ]
        );
        assert_eq!(
            &captures[40..],
            &[("method-name", "e"), ("method-name", "f"),]
        );
    });
}

#[test]
fn test_query_captures_with_definite_pattern_containing_many_nested_matches() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            (array
              "[" @l-bracket
              "]" @r-bracket)

            "." @dot
            "#,
        )
        .unwrap();

        // The '[' node must be returned before all of the '.' nodes,
        // even though its pattern does not finish until the ']' node
        // at the end of the document. But because the '[' is definite,
        // it can be returned before the pattern finishes matching.
        let source = "
        [
            a.b.c.d.e.f.g.h.i,
            a.b.c.d.e.f.g.h.i,
            a.b.c.d.e.f.g.h.i,
            a.b.c.d.e.f.g.h.i,
            a.b.c.d.e.f.g.h.i,
        ]
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_captures(captures, &query, source),
            std::iter::once(&("l-bracket", "["))
                .chain([("dot", "."); 40].iter())
                .chain(std::iter::once(&("r-bracket", "]")))
                .copied()
                .collect::<Vec<_>>(),
        );
    });
}

#[test]
fn test_query_captures_ordered_by_both_start_and_end_positions() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r"
            (call_expression) @call
            (member_expression) @member
            (identifier) @variable
            ",
        )
        .unwrap();

        let source = "
          a.b(c.d().e).f;
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_captures(captures, &query, source),
            &[
                ("member", "a.b(c.d().e).f"),
                ("call", "a.b(c.d().e)"),
                ("member", "a.b"),
                ("variable", "a"),
                ("member", "c.d().e"),
                ("call", "c.d()"),
                ("member", "c.d"),
                ("variable", "c"),
            ],
        );
    });
}

#[test]
fn test_query_captures_with_matches_removed() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r"
            (binary_expression
                left: (identifier) @left
                operator: _ @op
                right: (identifier) @right)
            ",
        )
        .unwrap();

        let source = "
          a === b && c > d && e < f;
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let mut captured_strings = Vec::new();

        let mut captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        while let Some((m, i)) = captures.next() {
            let capture = m.captures[*i];
            let text = capture.node.utf8_text(source.as_bytes()).unwrap();
            if text == "a" {
                m.remove();
                continue;
            }
            captured_strings.push(text);
        }

        assert_eq!(captured_strings, &["c", ">", "d", "e", "<", "f",]);
    });
}

#[test]
fn test_query_captures_with_matches_removed_before_they_finish() {
    allocations::record(|| {
        let language = get_language("javascript");
        // When Tree-sitter detects that a pattern is guaranteed to match,
        // it will start to eagerly return the captures that it has found,
        // even though it hasn't matched the entire pattern yet. A
        // namespace_import node always has "*", "as" and then an identifier
        // for children, so captures will be emitted eagerly for this pattern.
        let query = Query::new(
            &language,
            r#"
            (namespace_import
              "*" @star
              "as" @as
              (identifier) @identifier)
            "#,
        )
        .unwrap();

        let source = "
          import * as name from 'module-name';
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        let mut captured_strings = Vec::new();
        let mut captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
        while let Some((m, i)) = captures.next() {
            let capture = m.captures[*i];
            let text = capture.node.utf8_text(source.as_bytes()).unwrap();
            if text == "as" {
                m.remove();
                continue;
            }
            captured_strings.push(text);
        }

        // .remove() removes the match before it is finished. The identifier
        // "name" is part of this match, so we expect that removing the "as"
        // capture from the match should prevent "name" from matching:
        assert_eq!(captured_strings, &["*",]);
    });
}

#[test]
fn test_query_captures_and_matches_iterators_are_fused() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r"
            (comment) @comment
            ",
        )
        .unwrap();

        let source = "
          // one
          // two
          // three
          /* unfinished
        ";

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let mut captures = cursor.captures(&query, tree.root_node(), source.as_bytes());

        assert_eq!(captures.next().unwrap().0.captures[0].index, 0);
        assert_eq!(captures.next().unwrap().0.captures[0].index, 0);
        assert_eq!(captures.next().unwrap().0.captures[0].index, 0);
        assert!(captures.next().is_none());
        assert!(captures.next().is_none());
        assert!(captures.next().is_none());
        drop(captures);

        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(matches.next().unwrap().captures[0].index, 0);
        assert_eq!(matches.next().unwrap().captures[0].index, 0);
        assert_eq!(matches.next().unwrap().captures[0].index, 0);
        assert!(matches.next().is_none());
        assert!(matches.next().is_none());
        assert!(matches.next().is_none());
    });
}

#[test]
fn test_query_text_callback_returns_chunks() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            ((identifier) @leading_upper
             (#match? @leading_upper "^[A-Z][A-Z_]*[a-z]"))
            ((identifier) @all_upper
             (#match? @all_upper "^[A-Z][A-Z_]*$"))
            ((identifier) @all_lower
             (#match? @all_lower "^[a-z][a-z_]*$"))
            "#,
        )
        .unwrap();

        let source = "SOMETHING[a] = transform(AnotherThing[b].property[c], PARAMETER);";

        // Store the source code in chunks of 3 bytes, and expose it via
        // an iterator API.
        let source_chunks = source.as_bytes().chunks(3).collect::<Vec<_>>();
        let chunks_in_range = |range: std::ops::Range<usize>| {
            let mut offset = 0;
            source_chunks.iter().filter_map(move |chunk| {
                let end_offset = offset + chunk.len();
                if offset < range.end && range.start < end_offset {
                    let end_in_chunk = (range.end - offset).min(chunk.len());
                    let start_in_chunk = range.start.max(offset) - offset;
                    offset = end_offset;
                    Some(&chunk[start_in_chunk..end_in_chunk])
                } else {
                    offset = end_offset;
                    None
                }
            })
        };
        assert_eq!(
            chunks_in_range(0..9)
                .map(|c| std::str::from_utf8(c).unwrap())
                .collect::<String>(),
            "SOMETHING",
        );
        assert_eq!(
            chunks_in_range(15..24)
                .map(|c| std::str::from_utf8(c).unwrap())
                .collect::<String>(),
            "transform",
        );

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let captures = cursor.captures(&query, tree.root_node(), |node: Node| {
            chunks_in_range(node.byte_range())
        });

        assert_eq!(
            collect_captures(captures, &query, source),
            &[
                ("all_upper", "SOMETHING"),
                ("all_lower", "a"),
                ("all_lower", "transform"),
                ("leading_upper", "AnotherThing"),
                ("all_lower", "b"),
                ("all_lower", "c"),
                ("all_upper", "PARAMETER"),
            ]
        );
    });
}

#[test]
fn test_query_start_end_byte_for_pattern() {
    let language = get_language("javascript");

    let patterns_1 = indoc! {r#"
        "+" @operator
        "-" @operator
        "*" @operator
        "=" @operator
        "=>" @operator
    "#};

    let patterns_2 = indoc! {"
        (identifier) @a
        (string) @b
    "};

    let patterns_3 = indoc! {"
        ((identifier) @b (#match? @b i))
        (function_declaration name: (identifier) @c)
        (method_definition name: (property_identifier) @d)
    "};

    let mut source = String::new();
    source += patterns_1;
    source += patterns_2;
    source += patterns_3;

    let query = Query::new(&language, &source).unwrap();

    assert_eq!(query.start_byte_for_pattern(0), 0);
    assert_eq!(query.end_byte_for_pattern(0), "\"+\" @operator\n".len());
    assert_eq!(query.start_byte_for_pattern(5), patterns_1.len());
    assert_eq!(
        query.end_byte_for_pattern(5),
        patterns_1.len() + "(identifier) @a\n".len()
    );
    assert_eq!(
        query.start_byte_for_pattern(7),
        patterns_1.len() + patterns_2.len()
    );
    assert_eq!(
        query.end_byte_for_pattern(7),
        patterns_1.len() + patterns_2.len() + "((identifier) @b (#match? @b i))\n".len()
    );
}

#[test]
fn test_query_capture_names() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            r#"
            (if_statement
              condition: (parenthesized_expression (binary_expression
                left: _ @left-operand
                operator: "||"
                right: _ @right-operand))
              consequence: (statement_block) @body)

            (while_statement
              condition: _ @loop-condition)
            "#,
        )
        .unwrap();

        assert_eq!(
            query.capture_names(),
            ["left-operand", "right-operand", "body", "loop-condition"]
        );
    });
}

#[test]
fn test_query_lifetime_is_separate_from_nodes_lifetime() {
    allocations::record(|| {
        let query = r"(call_expression) @call";
        let source = "a(1); b(2);";

        let language = get_language("javascript");
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();

        fn take_first_node_from_captures<'tree>(
            source: &str,
            query: &str,
            node: Node<'tree>,
        ) -> Node<'tree> {
            // Following 2 lines are redundant but needed to demonstrate
            // more understandable compiler error message
            let language = get_language("javascript");
            let query = Query::new(&language, query).unwrap();
            let mut cursor = QueryCursor::new();
            let node = cursor
                .matches(&query, node, source.as_bytes())
                .next()
                .unwrap()
                .captures[0]
                .node;
            node
        }

        let node = take_first_node_from_captures(source, query, tree.root_node());
        assert_eq!(node.kind(), "call_expression");

        fn take_first_node_from_matches<'tree>(
            source: &str,
            query: &str,
            node: Node<'tree>,
        ) -> Node<'tree> {
            let language = get_language("javascript");
            let query = Query::new(&language, query).unwrap();
            let mut cursor = QueryCursor::new();
            let node = cursor
                .captures(&query, node, source.as_bytes())
                .next()
                .unwrap()
                .0
                .captures[0]
                .node;
            node
        }

        let node = take_first_node_from_matches(source, query, tree.root_node());
        assert_eq!(node.kind(), "call_expression");
    });
}

#[test]
fn test_query_with_no_patterns() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(&language, "").unwrap();
        assert!(query.capture_names().is_empty());
        assert_eq!(query.pattern_count(), 0);
    });
}

#[test]
fn test_query_comments() {
    allocations::record(|| {
        let language = get_language("javascript");
        let query = Query::new(
            &language,
            "
                ; this is my first comment
                ; i have two comments here
                (function_declaration
                    ; there is also a comment here
                    ; and here
                    name: (identifier) @fn-name)",
        )
        .unwrap();

        let source = "function one() { }";
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[(0, vec![("fn-name", "one")]),],
        );
    });
}

#[test]
fn test_query_disable_pattern() {
    allocations::record(|| {
        let language = get_language("javascript");
        let mut query = Query::new(
            &language,
            "
                (function_declaration
                    name: (identifier) @name)
                (function_declaration
                    body: (statement_block) @body)
                (class_declaration
                    name: (identifier) @name)
                (class_declaration
                    body: (class_body) @body)
            ",
        )
        .unwrap();

        // disable the patterns that match names
        query.disable_pattern(0);
        query.disable_pattern(2);

        let source = "class A { constructor() {} } function b() { return 1; }";
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        assert_eq!(
            collect_matches(matches, &query, source),
            &[
                (3, vec![("body", "{ constructor() {} }")]),
                (1, vec![("body", "{ return 1; }")]),
            ],
        );
    });
}

#[test]
fn test_query_alternative_predicate_prefix() {
    allocations::record(|| {
        let language = get_language("c");
        let query = Query::new(
            &language,
            r#"
            ((call_expression
              function: (identifier) @keyword
              arguments: (argument_list
                          (string_literal) @function))
             (.eq? @keyword "DEFUN"))
        "#,
        )
        .unwrap();
        let source = r#"
            DEFUN ("identity", Fidentity, Sidentity, 1, 1, 0,
                   doc: /* Return the argument unchanged.  */
                   attributes: const)
              (Lisp_Object arg)
            {
              return arg;
            }
        "#;
        assert_query_matches(
            &language,
            &query,
            source,
            &[(0, vec![("keyword", "DEFUN"), ("function", "\"identity\"")])],
        );
    });
}

#[test]
fn test_query_random() {
    use pretty_assertions::assert_eq;

    allocations::record(|| {
        let language = get_language("rust");
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(64);

        let pattern_tree = parser
            .parse(include_str!("helpers/query_helpers.rs"), None)
            .unwrap();
        let test_tree = parser
            .parse(include_str!("helpers/query_helpers.rs"), None)
            .unwrap();

        let start_seed = 0;
        let end_seed = start_seed + *ITERATION_COUNT;

        for seed in start_seed..(start_seed + end_seed) {
            let seed = seed as u64;
            let mut rand = StdRng::seed_from_u64(seed);
            let (pattern_ast, _) = Pattern::random_pattern_in_tree(&pattern_tree, &mut rand);
            let pattern = pattern_ast.to_string();
            let expected_matches = pattern_ast.matches_in_tree(&test_tree);

            let query = match Query::new(&language, &pattern) {
                Ok(query) => query,
                Err(e) => {
                    panic!("failed to build query for pattern {pattern} - {e}. seed: {seed}");
                }
            };
            let mut actual_matches = Vec::new();
            let mut match_iter = cursor.matches(
                &query,
                test_tree.root_node(),
                include_bytes!("parser_test.rs").as_ref(),
            );

            while let Some(mat) = match_iter.next() {
                let transformed_match = Match {
                    last_node: None,
                    captures: mat
                        .captures
                        .iter()
                        .map(|c| (query.capture_names()[c.index as usize], c.node))
                        .collect::<Vec<_>>(),
                };
                actual_matches.push(transformed_match);
            }

            // actual_matches.sort_unstable();
            actual_matches.dedup();

            if !cursor.did_exceed_match_limit() {
                assert_eq!(
                    actual_matches, expected_matches,
                    "seed: {}, pattern:\n{}",
                    seed, pattern
                );
            }
        }
    });
}

#[test]
fn test_query_is_pattern_guaranteed_at_step() {
    struct Row {
        language: Language,
        description: &'static str,
        pattern: &'static str,
        results_by_substring: &'static [(&'static str, bool)],
    }

    let rows = &[
        Row {
            description: "no guaranteed steps",
            language: get_language("python"),
            pattern: r"(expression_statement (string))",
            results_by_substring: &[("expression_statement", false), ("string", false)],
        },
        Row {
            description: "all guaranteed steps",
            language: get_language("javascript"),
            pattern: r#"(object "{" "}")"#,
            results_by_substring: &[("object", false), ("{", true), ("}", true)],
        },
        Row {
            description: "a fallible step that is optional",
            language: get_language("javascript"),
            pattern: r#"(object "{" (identifier)? @foo "}")"#,
            results_by_substring: &[
                ("object", false),
                ("{", true),
                ("(identifier)?", false),
                ("}", true),
            ],
        },
        Row {
            description: "multiple fallible steps that are optional",
            language: get_language("javascript"),
            pattern: r#"(object "{" (identifier)? @id1 ("," (identifier) @id2)? "}")"#,
            results_by_substring: &[
                ("object", false),
                ("{", true),
                ("(identifier)? @id1", false),
                ("\",\"", false),
                ("}", true),
            ],
        },
        Row {
            description: "guaranteed step after fallibe step",
            language: get_language("javascript"),
            pattern: r#"(pair (property_identifier) ":")"#,
            results_by_substring: &[("pair", false), ("property_identifier", false), (":", true)],
        },
        Row {
            description: "fallible step in between two guaranteed steps",
            language: get_language("javascript"),
            pattern: r#"(ternary_expression
                condition: (_)
                "?"
                consequence: (call_expression)
                ":"
                alternative: (_))"#,
            results_by_substring: &[
                ("condition:", false),
                ("\"?\"", false),
                ("consequence:", false),
                ("\":\"", true),
                ("alternative:", true),
            ],
        },
        Row {
            description: "one guaranteed step after a repetition",
            language: get_language("javascript"),
            pattern: r#"(object "{" (_) "}")"#,
            results_by_substring: &[("object", false), ("{", false), ("(_)", false), ("}", true)],
        },
        Row {
            description: "guaranteed steps after multiple repetitions",
            language: get_language("json"),
            pattern: r#"(object "{" (pair) "," (pair) "," (_) "}")"#,
            results_by_substring: &[
                ("object", false),
                ("{", false),
                ("(pair) \",\" (pair)", false),
                ("(pair) \",\" (_)", false),
                ("\",\" (_)", false),
                ("(_)", true),
                ("}", true),
            ],
        },
        Row {
            description: "a guaranteed step with a field",
            language: get_language("javascript"),
            pattern: r"(binary_expression left: (expression) right: (_))",
            results_by_substring: &[
                ("binary_expression", false),
                ("(expression)", false),
                ("(_)", true),
            ],
        },
        Row {
            description: "multiple guaranteed steps with fields",
            language: get_language("javascript"),
            pattern: r"(function_declaration name: (identifier) body: (statement_block))",
            results_by_substring: &[
                ("function_declaration", false),
                ("identifier", true),
                ("statement_block", true),
            ],
        },
        Row {
            description: "nesting, one guaranteed step",
            language: get_language("javascript"),
            pattern: r#"
                (function_declaration
                    name: (identifier)
                    body: (statement_block "{" (expression_statement) "}"))"#,
            results_by_substring: &[
                ("function_declaration", false),
                ("identifier", false),
                ("statement_block", false),
                ("{", false),
                ("expression_statement", false),
                ("}", true),
            ],
        },
        Row {
            description: "a guaranteed step after some deeply nested hidden nodes",
            language: get_language("ruby"),
            pattern: r#"
            (singleton_class
                value: (constant)
                "end")
            "#,
            results_by_substring: &[
                ("singleton_class", false),
                ("constant", false),
                ("end", true),
            ],
        },
        Row {
            description: "nesting, no guaranteed steps",
            language: get_language("javascript"),
            pattern: r"
            (call_expression
                function: (member_expression
                  property: (property_identifier) @template-tag)
                arguments: (template_string)) @template-call
            ",
            results_by_substring: &[("property_identifier", false), ("template_string", false)],
        },
        Row {
            description: "a guaranteed step after a nested node",
            language: get_language("javascript"),
            pattern: r#"
            (subscript_expression
                object: (member_expression
                    object: (identifier) @obj
                    property: (property_identifier) @prop)
                "[")
            "#,
            results_by_substring: &[
                ("identifier", false),
                ("property_identifier", false),
                ("[", true),
            ],
        },
        Row {
            description: "a step that is fallible due to a predicate",
            language: get_language("javascript"),
            pattern: r#"
            (subscript_expression
                object: (member_expression
                    object: (identifier) @obj
                    property: (property_identifier) @prop)
                "["
                (#match? @prop "foo"))
            "#,
            results_by_substring: &[
                ("identifier", false),
                ("property_identifier", false),
                ("[", true),
            ],
        },
        Row {
            description: "alternation where one branch has guaranteed steps",
            language: get_language("javascript"),
            pattern: r"
            [
                (unary_expression (identifier))
                (call_expression
                  function: (_)
                  arguments: (_))
                (binary_expression right: (call_expression))
            ]
            ",
            results_by_substring: &[
                ("identifier", false),
                ("right:", false),
                ("function:", true),
                ("arguments:", true),
            ],
        },
        Row {
            description: "guaranteed step at the end of an aliased parent node",
            language: get_language("ruby"),
            pattern: r#"
            (method_parameters "(" (identifier) @id")")
            "#,
            results_by_substring: &[("\"(\"", false), ("(identifier)", false), ("\")\"", true)],
        },
        Row {
            description: "long, but not too long to analyze",
            language: get_language("javascript"),
            pattern: r#"
            (object "{" (pair) (pair) (pair) (pair) "}")
            "#,
            results_by_substring: &[
                ("\"{\"", false),
                ("(pair)", false),
                ("(pair) \"}\"", false),
                ("\"}\"", true),
            ],
        },
        Row {
            description: "too long to analyze",
            language: get_language("javascript"),
            pattern: r#"
            (object "{" (pair) (pair) (pair) (pair) (pair) (pair) (pair) (pair) (pair) (pair) (pair) (pair) "}")
            "#,
            results_by_substring: &[
                ("\"{\"", false),
                ("(pair)", false),
                ("(pair) \"}\"", false),
                ("\"}\"", false),
            ],
        },
        Row {
            description: "hidden nodes that have several fields",
            language: get_language("java"),
            pattern: r"
            (method_declaration name: (identifier))
            ",
            results_by_substring: &[("name:", true)],
        },
        Row {
            description: "top-level non-terminal extra nodes",
            language: get_language("ruby"),
            pattern: r"
            (heredoc_body
                (interpolation)
                (heredoc_end) @end)
            ",
            results_by_substring: &[
                ("(heredoc_body", false),
                ("(interpolation)", false),
                ("(heredoc_end)", true),
            ],
        },
        // TODO: figure out why line comments, an extra, are no longer allowed *anywhere*
        // likely culprits are the fact that it's no longer a token itself or that it uses an
        // external token
        // Row {
        //     description: "multiple extra nodes",
        //     language: get_language("rust"),
        //     pattern: r"
        //     (call_expression
        //         (line_comment) @a
        //         (line_comment) @b
        //         (arguments))
        //     ",
        //     results_by_substring: &[
        //         ("(line_comment) @a", false),
        //         ("(line_comment) @b", false),
        //         ("(arguments)", true),
        //     ],
        // },
    ];

    allocations::record(|| {
        eprintln!();

        for row in rows {
            if let Some(filter) = EXAMPLE_FILTER.as_ref() {
                if !row.description.contains(filter.as_str()) {
                    continue;
                }
            }
            eprintln!("  query example: {:?}", row.description);
            let query = Query::new(&row.language, row.pattern).unwrap();
            for (substring, is_definite) in row.results_by_substring {
                let offset = row.pattern.find(substring).unwrap();
                assert_eq!(
                    query.is_pattern_guaranteed_at_step(offset),
                    *is_definite,
                    "Description: {}, Pattern: {:?}, substring: {:?}, expected is_definite to be {}",
                    row.description,
                    row.pattern
                        .split_ascii_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    substring,
                    is_definite,
                );
            }
        }
    });
}

#[test]
fn test_query_is_pattern_rooted() {
    struct Row {
        description: &'static str,
        pattern: &'static str,
        is_rooted: bool,
    }

    let rows = [
        Row {
            description: "simple token",
            pattern: r"(identifier)",
            is_rooted: true,
        },
        Row {
            description: "simple non-terminal",
            pattern: r"(function_definition name: (identifier))",
            is_rooted: true,
        },
        Row {
            description: "alternative of many tokens",
            pattern: r#"["if" "def" (identifier) (comment)]"#,
            is_rooted: true,
        },
        Row {
            description: "alternative of many non-terminals",
            pattern: r"[
                (function_definition name: (identifier))
                (class_definition name: (identifier))
                (block)
            ]",
            is_rooted: true,
        },
        Row {
            description: "two siblings",
            pattern: r#"("{" "}")"#,
            is_rooted: false,
        },
        Row {
            description: "top-level repetition",
            pattern: r"(comment)*",
            is_rooted: false,
        },
        Row {
            description: "alternative where one option has two siblings",
            pattern: r#"[
                (block)
                (class_definition)
                ("(" ")")
                (function_definition)
            ]"#,
            is_rooted: false,
        },
        Row {
            description: "alternative where one option has a top-level repetition",
            pattern: r"[
                (block)
                (class_definition)
                (comment)*
                (function_definition)
            ]",
            is_rooted: false,
        },
    ];

    allocations::record(|| {
        eprintln!();

        let language = get_language("python");
        for row in &rows {
            if let Some(filter) = EXAMPLE_FILTER.as_ref() {
                if !row.description.contains(filter.as_str()) {
                    continue;
                }
            }
            eprintln!("  query example: {:?}", row.description);
            let query = Query::new(&language, row.pattern).unwrap();
            assert_eq!(
                query.is_pattern_rooted(0),
                row.is_rooted,
                "Description: {}, Pattern: {:?}",
                row.description,
                row.pattern
                    .split_ascii_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
    });
}

#[test]
fn test_query_is_pattern_non_local() {
    struct Row {
        description: &'static str,
        pattern: &'static str,
        language: Language,
        is_non_local: bool,
    }

    let rows = [
        Row {
            description: "simple token",
            pattern: r"(identifier)",
            language: get_language("python"),
            is_non_local: false,
        },
        Row {
            description: "siblings that can occur in an argument list",
            pattern: r"((identifier) (identifier))",
            language: get_language("python"),
            is_non_local: true,
        },
        Row {
            description: "siblings that can occur in a statement block",
            pattern: r"((return_statement) (return_statement))",
            language: get_language("python"),
            is_non_local: true,
        },
        Row {
            description: "siblings that can occur in a source file",
            pattern: r"((function_definition) (class_definition))",
            language: get_language("python"),
            is_non_local: true,
        },
        Row {
            description: "siblings that can't occur in any repetition",
            pattern: r#"("{" "}")"#,
            language: get_language("python"),
            is_non_local: false,
        },
        Row {
            description: "siblings that can't occur in any repetition, wildcard root",
            pattern: r#"(_ "{" "}") @foo"#,
            language: get_language("javascript"),
            is_non_local: false,
        },
        Row {
            description: "siblings that can occur in a class body, wildcard root",
            pattern: r"(_ (method_definition) (method_definition)) @foo",
            language: get_language("javascript"),
            is_non_local: true,
        },
        Row {
            description: "top-level repetitions that can occur in a class body",
            pattern: r"(method_definition)+ @foo",
            language: get_language("javascript"),
            is_non_local: true,
        },
        Row {
            description: "top-level repetitions that can occur in a statement block",
            pattern: r"(return_statement)+ @foo",
            language: get_language("javascript"),
            is_non_local: true,
        },
        Row {
            description: "rooted pattern that can occur in a statement block",
            pattern: r"(return_statement) @foo",
            language: get_language("javascript"),
            is_non_local: false,
        },
    ];

    allocations::record(|| {
        eprintln!();

        for row in &rows {
            if let Some(filter) = EXAMPLE_FILTER.as_ref() {
                if !row.description.contains(filter.as_str()) {
                    continue;
                }
            }
            eprintln!("  query example: {:?}", row.description);
            let query = Query::new(&row.language, row.pattern).unwrap();
            assert_eq!(
                query.is_pattern_non_local(0),
                row.is_non_local,
                "Description: {}, Pattern: {:?}",
                row.description,
                row.pattern
                    .split_ascii_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
    });
}

#[test]
fn test_capture_quantifiers() {
    struct Row {
        description: &'static str,
        language: Language,
        pattern: &'static str,
        capture_quantifiers: &'static [(usize, &'static str, CaptureQuantifier)],
    }

    let rows = &[
        // Simple quantifiers
        Row {
            description: "Top level capture",
            language: get_language("python"),
            pattern: r"
                (module) @mod
            ",
            capture_quantifiers: &[(0, "mod", CaptureQuantifier::One)],
        },
        Row {
            description: "Nested list capture capture",
            language: get_language("javascript"),
            pattern: r"
                (array (_)* @elems) @array
            ",
            capture_quantifiers: &[
                (0, "array", CaptureQuantifier::One),
                (0, "elems", CaptureQuantifier::ZeroOrMore),
            ],
        },
        Row {
            description: "Nested non-empty list capture capture",
            language: get_language("javascript"),
            pattern: r"
                (array (_)+ @elems) @array
            ",
            capture_quantifiers: &[
                (0, "array", CaptureQuantifier::One),
                (0, "elems", CaptureQuantifier::OneOrMore),
            ],
        },
        // Nested quantifiers
        Row {
            description: "capture nested in optional pattern",
            language: get_language("javascript"),
            pattern: r"
                (array (call_expression (arguments (_) @arg))? @call) @array
            ",
            capture_quantifiers: &[
                (0, "array", CaptureQuantifier::One),
                (0, "call", CaptureQuantifier::ZeroOrOne),
                (0, "arg", CaptureQuantifier::ZeroOrOne),
            ],
        },
        Row {
            description: "optional capture nested in non-empty list pattern",
            language: get_language("javascript"),
            pattern: r"
                (array (call_expression (arguments (_)? @arg))+ @call) @array
            ",
            capture_quantifiers: &[
                (0, "array", CaptureQuantifier::One),
                (0, "call", CaptureQuantifier::OneOrMore),
                (0, "arg", CaptureQuantifier::ZeroOrMore),
            ],
        },
        Row {
            description: "non-empty list capture nested in optional pattern",
            language: get_language("javascript"),
            pattern: r"
                (array (call_expression (arguments (_)+ @args))? @call) @array
            ",
            capture_quantifiers: &[
                (0, "array", CaptureQuantifier::One),
                (0, "call", CaptureQuantifier::ZeroOrOne),
                (0, "args", CaptureQuantifier::ZeroOrMore),
            ],
        },
        // Quantifiers in alternations
        Row {
            description: "capture is the same in all alternatives",
            language: get_language("javascript"),
            pattern: r"[
                (function_declaration name:(identifier) @name)
                (call_expression function:(identifier) @name)
            ]",
            capture_quantifiers: &[(0, "name", CaptureQuantifier::One)],
        },
        Row {
            description: "capture appears in some alternatives",
            language: get_language("javascript"),
            pattern: r"[
                (function_declaration name:(identifier) @name)
                (function_expression)
            ] @fun",
            capture_quantifiers: &[
                (0, "fun", CaptureQuantifier::One),
                (0, "name", CaptureQuantifier::ZeroOrOne),
            ],
        },
        Row {
            description: "capture has different quantifiers in alternatives",
            language: get_language("javascript"),
            pattern: r"[
                (call_expression arguments: (arguments (_)+ @args))
                (new_expression  arguments: (arguments (_)? @args))
            ] @call",
            capture_quantifiers: &[
                (0, "call", CaptureQuantifier::One),
                (0, "args", CaptureQuantifier::ZeroOrMore),
            ],
        },
        // Quantifiers in siblings
        Row {
            description: "siblings have different captures with different quantifiers",
            language: get_language("javascript"),
            pattern: r"
                (call_expression (arguments (identifier)? @self (_)* @args)) @call
            ",
            capture_quantifiers: &[
                (0, "call", CaptureQuantifier::One),
                (0, "self", CaptureQuantifier::ZeroOrOne),
                (0, "args", CaptureQuantifier::ZeroOrMore),
            ],
        },
        Row {
            description: "siblings have same capture with different quantifiers",
            language: get_language("javascript"),
            pattern: r"
                (call_expression (arguments (identifier) @args (_)* @args)) @call
            ",
            capture_quantifiers: &[
                (0, "call", CaptureQuantifier::One),
                (0, "args", CaptureQuantifier::OneOrMore),
            ],
        },
        // Combined scenarios
        Row {
            description: "combined nesting, alternatives, and siblings",
            language: get_language("javascript"),
            pattern: r"
                (array
                    (call_expression
                        (arguments [
                            (identifier) @self
                            (_)+ @args
                        ])
                    )+ @call
                ) @array
            ",
            capture_quantifiers: &[
                (0, "array", CaptureQuantifier::One),
                (0, "call", CaptureQuantifier::OneOrMore),
                (0, "self", CaptureQuantifier::ZeroOrMore),
                (0, "args", CaptureQuantifier::ZeroOrMore),
            ],
        },
        // Multiple patterns
        Row {
            description: "multiple patterns",
            language: get_language("javascript"),
            pattern: r"
                (function_declaration name: (identifier) @x)
                (statement_identifier) @y
                (property_identifier)+ @z
                (array (identifier)* @x)
            ",
            capture_quantifiers: &[
                // x
                (0, "x", CaptureQuantifier::One),
                (1, "x", CaptureQuantifier::Zero),
                (2, "x", CaptureQuantifier::Zero),
                (3, "x", CaptureQuantifier::ZeroOrMore),
                // y
                (0, "y", CaptureQuantifier::Zero),
                (1, "y", CaptureQuantifier::One),
                (2, "y", CaptureQuantifier::Zero),
                (3, "y", CaptureQuantifier::Zero),
                // z
                (0, "z", CaptureQuantifier::Zero),
                (1, "z", CaptureQuantifier::Zero),
                (2, "z", CaptureQuantifier::OneOrMore),
                (3, "z", CaptureQuantifier::Zero),
            ],
        },
        Row {
            description: "multiple alternatives",
            language: get_language("javascript"),
            pattern: r"
            [
                (array (identifier) @x)
                (function_declaration name: (identifier)+ @x)
            ]
            [
                (array (identifier) @x)
                (function_declaration name: (identifier)+ @x)
            ]
            ",
            capture_quantifiers: &[
                (0, "x", CaptureQuantifier::OneOrMore),
                (1, "x", CaptureQuantifier::OneOrMore),
            ],
        },
    ];

    allocations::record(|| {
        eprintln!();

        for row in rows {
            if let Some(filter) = EXAMPLE_FILTER.as_ref() {
                if !row.description.contains(filter.as_str()) {
                    continue;
                }
            }
            eprintln!("  query example: {:?}", row.description);
            let query = Query::new(&row.language, row.pattern).unwrap();
            for (pattern, capture, expected_quantifier) in row.capture_quantifiers {
                let index = query.capture_index_for_name(capture).unwrap();
                let actual_quantifier = query.capture_quantifiers(*pattern)[index as usize];
                assert_eq!(
                    actual_quantifier,
                    *expected_quantifier,
                    "Description: {}, Pattern: {:?}, expected quantifier of @{} to be {:?} instead of {:?}",
                    row.description,
                    row.pattern
                        .split_ascii_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    capture,
                    *expected_quantifier,
                    actual_quantifier,
                );
            }
        }
    });
}

#[test]
fn test_query_quantified_captures() {
    struct Row {
        description: &'static str,
        language: Language,
        code: &'static str,
        pattern: &'static str,
        captures: &'static [(&'static str, &'static str)],
    }

    // #[rustfmt::skip]
    let rows = &[
        Row {
            description: "doc comments where all must match the prefix",
            language: get_language("c"),
            code: indoc! {"
            /// foo
            /// bar
            /// baz

            void main() {}

            /// qux
            /// quux
            // quuz
        "},
            pattern: r#"
                ((comment)+ @comment.documentation
                  (#match? @comment.documentation "^///"))
            "#,
            captures: &[
                ("comment.documentation", "/// foo"),
                ("comment.documentation", "/// bar"),
                ("comment.documentation", "/// baz"),
            ],
        },
        Row {
            description: "doc comments where one must match the prefix",
            language: get_language("c"),
            code: indoc! {"
            /// foo
            /// bar
            /// baz

            void main() {}

            /// qux
            /// quux
            // quuz
        "},
            pattern: r#"
                ((comment)+ @comment.documentation
                  (#any-match? @comment.documentation "^///"))
            "#,
            captures: &[
                ("comment.documentation", "/// foo"),
                ("comment.documentation", "/// bar"),
                ("comment.documentation", "/// baz"),
                ("comment.documentation", "/// qux"),
                ("comment.documentation", "/// quux"),
                ("comment.documentation", "// quuz"),
            ],
        },
    ];

    allocations::record(|| {
        for row in rows {
            eprintln!("  quantified query example: {:?}", row.description);

            let mut parser = Parser::new();
            parser.set_language(&row.language).unwrap();
            let tree = parser.parse(row.code, None).unwrap();

            let query = Query::new(&row.language, row.pattern).unwrap();

            let mut cursor = QueryCursor::new();
            let matches = cursor.captures(&query, tree.root_node(), row.code.as_bytes());

            assert_eq!(collect_captures(matches, &query, row.code), row.captures);
        }
    });
}

#[test]
fn test_query_max_start_depth() {
    struct Row {
        description: &'static str,
        pattern: &'static str,
        depth: u32,
        matches: &'static [(usize, &'static [(&'static str, &'static str)])],
    }

    let source = indoc! {"
        if (a1 && a2) {
            if (b1 && b2) { }
            if (c) { }
        }
        if (d) {
            if (e1 && e2) { }
            if (f) { }
        }
    "};

    #[rustfmt::skip]
    let rows = &[
        Row {
            description: "depth 0: match translation unit",
            depth: 0,
            pattern: r"
                (translation_unit) @capture
            ",
            matches: &[
                (0, &[("capture", "if (a1 && a2) {\n    if (b1 && b2) { }\n    if (c) { }\n}\nif (d) {\n    if (e1 && e2) { }\n    if (f) { }\n}\n")]),
            ]
        },
        Row {
            description: "depth 0: match none",
            depth: 0,
            pattern: r"
                (if_statement) @capture
            ",
            matches: &[]
        },
        Row {
            description: "depth 1: match 2 if statements at the top level",
            depth: 1,
            pattern: r"
                (if_statement) @capture
            ",
            matches : &[
                (0, &[("capture", "if (a1 && a2) {\n    if (b1 && b2) { }\n    if (c) { }\n}")]),
                (0, &[("capture", "if (d) {\n    if (e1 && e2) { }\n    if (f) { }\n}")]),
            ]
        },
        Row {
            description: "depth 1 with deep pattern: match the only the first if statement",
            depth: 1,
            pattern: r"
                (if_statement
                    condition: (parenthesized_expression
                        (binary_expression)
                    )
                ) @capture
            ",
            matches: &[
                (0, &[("capture", "if (a1 && a2) {\n    if (b1 && b2) { }\n    if (c) { }\n}")]),
            ]
        },
        Row {
            description: "depth 3 with deep pattern: match all if statements with a binexpr condition",
            depth: 3,
            pattern: r"
                (if_statement
                    condition: (parenthesized_expression
                        (binary_expression)
                    )
                ) @capture
            ",
            matches: &[
                (0, &[("capture", "if (a1 && a2) {\n    if (b1 && b2) { }\n    if (c) { }\n}")]),
                (0, &[("capture", "if (b1 && b2) { }")]),
                (0, &[("capture", "if (e1 && e2) { }")]),
            ]
        },
    ];

    allocations::record(|| {
        let language = get_language("c");
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();

        for row in rows {
            eprintln!("  query example: {:?}", row.description);

            let query = Query::new(&language, row.pattern).unwrap();
            cursor.set_max_start_depth(Some(row.depth));

            let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            let expected = row
                .matches
                .iter()
                .map(|x| (x.0, x.1.to_vec()))
                .collect::<Vec<_>>();

            assert_eq!(collect_matches(matches, &query, source), expected);
        }
    });
}

#[test]
fn test_query_error_does_not_oob() {
    let language = get_language("javascript");

    assert_eq!(
        Query::new(&language, "(clas").unwrap_err(),
        QueryError {
            row: 0,
            offset: 1,
            column: 1,
            kind: QueryErrorKind::NodeType,
            message: "clas".to_string()
        }
    );
}

#[test]
fn test_consecutive_zero_or_modifiers() {
    let language = get_language("javascript");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    let zero_source = "";
    let three_source = "/**/ /**/ /**/";

    let zero_tree = parser.parse(zero_source, None).unwrap();
    let three_tree = parser.parse(three_source, None).unwrap();

    let tests = [
        "(comment)*** @capture",
        "(comment)??? @capture",
        "(comment)*?* @capture",
        "(comment)?*? @capture",
    ];

    for test in tests {
        let query = Query::new(&language, test).unwrap();

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, zero_tree.root_node(), zero_source.as_bytes());
        assert!(matches.next().is_some());

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, three_tree.root_node(), three_source.as_bytes());

        let mut len_3 = false;
        let mut len_1 = false;

        while let Some(m) = matches.next() {
            if m.captures.len() == 3 {
                len_3 = true;
            }
            if m.captures.len() == 1 {
                len_1 = true;
            }
        }

        assert_eq!(len_3, test.contains('*'));
        assert_eq!(len_1, test.contains("???"));
    }
}

#[test]
fn test_query_max_start_depth_more() {
    struct Row {
        depth: u32,
        matches: &'static [(usize, &'static [(&'static str, &'static str)])],
    }

    let source = indoc! {"
        {
            { }
            {
                { }
            }
        }
    "};

    #[rustfmt::skip]
    let rows = &[
        Row {
            depth: 0,
            matches: &[
                (0, &[("capture", "{\n    { }\n    {\n        { }\n    }\n}")])
            ]
        },
        Row {
            depth: 1,
            matches: &[
                (0, &[("capture", "{\n    { }\n    {\n        { }\n    }\n}")]),
                (0, &[("capture", "{ }")]),
                (0, &[("capture", "{\n        { }\n    }")])
            ]
        },
        Row {
            depth: 2,
            matches: &[
                (0, &[("capture", "{\n    { }\n    {\n        { }\n    }\n}")]),
                (0, &[("capture", "{ }")]),
                (0, &[("capture", "{\n        { }\n    }")]),
                (0, &[("capture", "{ }")]),
            ]
        },
    ];

    allocations::record(|| {
        let language = get_language("c");
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let mut cursor = QueryCursor::new();
        let query = Query::new(&language, "(compound_statement) @capture").unwrap();

        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        let node = matches.next().unwrap().captures[0].node;
        assert_eq!(node.kind(), "compound_statement");

        for row in rows {
            eprintln!("  depth: {}", row.depth);

            cursor.set_max_start_depth(Some(row.depth));

            let matches = cursor.matches(&query, node, source.as_bytes());
            let expected = row
                .matches
                .iter()
                .map(|x| (x.0, x.1.to_vec()))
                .collect::<Vec<_>>();

            assert_eq!(collect_matches(matches, &query, source), expected);
        }
    });
}

#[test]
fn test_grammar_with_aliased_literal_query() {
    // module.exports = grammar({
    //   name: 'test',
    //
    //   rules: {
    //     source: $ => repeat(choice($.compound_statement, $.expansion)),
    //
    //     compound_statement: $ => seq(alias(token(prec(-1, '}')), '}')),
    //
    //     expansion: $ => seq('}'),
    //   },
    // });
    let (parser_name, parser_code) = generate_parser(
        r#"
        {
            "name": "test",
            "rules": {
                "source": {
                    "type": "REPEAT",
                    "content": {
                        "type": "CHOICE",
                        "members": [
                            {
                                "type": "SYMBOL",
                                "name": "compound_statement"
                            },
                            {
                                "type": "SYMBOL",
                                "name": "expansion"
                            }
                        ]
                    }
                },
                "compound_statement": {
                    "type": "SEQ",
                    "members": [
                        {
                            "type": "ALIAS",
                            "content": {
                                "type": "TOKEN",
                                "content": {
                                    "type": "PREC",
                                    "value": -1,
                                    "content": {
                                        "type": "STRING",
                                        "value": "}"
                                    }
                                }
                            },
                            "named": false,
                            "value": "}"
                        }
                    ]
                },
                "expansion": {
                    "type": "SEQ",
                    "members": [
                        {
                            "type": "STRING",
                            "value": "}"
                        }
                    ]
                }
            }
        }
        "#,
    )
    .unwrap();

    let language = get_test_language(&parser_name, &parser_code, None);

    let query = Query::new(
        &language,
        r#"
        (compound_statement "}" @bracket1)
        (expansion "}" @bracket2)
        "#,
    );

    assert!(query.is_ok());
}

#[test]
fn test_query_with_first_child_in_group_is_anchor() {
    let language = get_language("c");
    let source_code = r"void fun(int a, char b, int c) { };";
    let query = r#"
            (parameter_list
              .
              ((parameter_declaration) @constant
                (#match? @constant "^int")))"#;
    let query = Query::new(&language, query).unwrap();
    assert_query_matches(
        &language,
        &query,
        source_code,
        &[(0, vec![("constant", "int a")])],
    );
}

// This test needs be executed with UBSAN enabled to check for regressions:
// ```
// UBSAN_OPTIONS="halt_on_error=1" \
// CFLAGS="-fsanitize=undefined"   \
// RUSTFLAGS="-lubsan"             \
// cargo test --target $(rustc -vV | sed -nr 's/^host: //p') -- --test-threads 1
// ```
#[test]
fn test_query_compiler_oob_access() {
    let language = get_language("java");
    // UBSAN should not report any OOB access
    assert!(Query::new(&language, "(package_declaration _ (_) @name _)").is_ok());
}

#[test]
fn test_query_wildcard_with_immediate_first_child() {
    let language = get_language("javascript");
    let query = Query::new(&language, "(_ . (identifier) @firstChild)").unwrap();
    let source = "function name(one, two, three) { }";

    assert_query_matches(
        &language,
        &query,
        source,
        &[
            (0, vec![("firstChild", "name")]),
            (0, vec![("firstChild", "one")]),
        ],
    );
}

#[test]
fn test_query_on_empty_source_code() {
    let language = get_language("javascript");
    let source_code = "";
    let query = "(program) @program";
    let query = Query::new(&language, query).unwrap();
    assert_query_matches(
        &language,
        &query,
        source_code,
        &[(0, vec![("program", "")])],
    );
}

#[test]
fn test_query_execution_with_timeout() {
    let language = get_language("javascript");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    let source_code = "function foo() { while (true) { } }\n".repeat(1000);
    let tree = parser.parse(&source_code, None).unwrap();

    let query = Query::new(&language, "(function_declaration) @function").unwrap();
    let mut cursor = QueryCursor::new();

    let start_time = std::time::Instant::now();
    let matches = cursor
        .matches_with_options(
            &query,
            tree.root_node(),
            source_code.as_bytes(),
            QueryCursorOptions::new()
                .progress_callback(&mut |_| start_time.elapsed().as_micros() > 1000),
        )
        .count();
    assert!(matches < 1000);

    let matches = cursor
        .matches(&query, tree.root_node(), source_code.as_bytes())
        .count();
    assert_eq!(matches, 1000);
}

#[test]
fn test_query_execution_with_points_causing_underflow() {
    let language = get_language("rust");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    #[allow(clippy::literal_string_with_formatting_args)]
    let code = r#"fn main() {
    println!("{:?}", foo());
}"#;
    parser
        .set_included_ranges(&[Range {
            start_byte: 24,
            end_byte: 39,
            start_point: Point::new(0, 0), // 5, 12
            end_point: Point::new(0, 0),   // 5, 27
        }])
        .unwrap();

    let query = Query::new(&language, "(call_expression) @cap").unwrap();
    let mut cursor = QueryCursor::new();

    let mut tree = parser.parse(code, None).unwrap();

    let matches = {
        let root_node = tree.root_node();
        let matches = cursor.matches(&query, root_node, code.as_bytes());
        collect_matches(matches, &query, code)
            .into_iter()
            .map(|(i, m)| {
                (
                    i,
                    m.into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };

    tree.edit(&InputEdit {
        start_byte: 40,
        old_end_byte: 40,
        new_end_byte: 41,
        start_position: Point::new(1, 28),
        old_end_position: Point::new(1, 28),
        new_end_position: Point::new(2, 0),
    });

    let tree2 = parser.parse(code, Some(&tree)).unwrap();

    let matches2 = {
        let root_node = tree2.root_node();
        let matches = cursor.matches(&query, root_node, code.as_bytes());
        collect_matches(matches, &query, code)
            .into_iter()
            .map(|(i, m)| {
                (
                    i,
                    m.into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(matches, matches2);
}

#[test]
fn test_wildcard_behavior_before_anchor() {
    let language = get_language("python");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    let source = "
        (a, b)
        (c, d,)
    ";

    //  In this query, we're targeting any *named* node immediately before a closing parenthesis.
    let query = Query::new(&language, r#"(tuple (_) @last . ")" .) @match"#).unwrap();
    assert_query_matches(
        &language,
        &query,
        source,
        &[
            (0, vec![("match", "(a, b)"), ("last", "b")]),
            (0, vec![("match", "(c, d,)"), ("last", "d")]),
        ],
    );

    // In this query, we're targeting *any* node immediately before a closing
    // parenthesis.
    let query = Query::new(&language, r#"(tuple _ @last . ")" .) @match"#).unwrap();
    assert_query_matches(
        &language,
        &query,
        source,
        &[
            (0, vec![("match", "(a, b)"), ("last", "b")]),
            (0, vec![("match", "(c, d,)"), ("last", ",")]),
        ],
    );
}

#[test]
fn test_pattern_alternatives_follow_last_child_constraint() {
    let language = get_language("rust");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    let code = "
fn f() {
    if a {} // <- should NOT match
    if b {}
}";

    let tree = parser.parse(code, None).unwrap();
    let mut cursor = QueryCursor::new();

    let query = Query::new(
        &language,
        "(block
        [
            (type_cast_expression)
            (expression_statement)
        ] @last
        .
        )",
    )
    .unwrap();

    let matches = {
        let root_node = tree.root_node();
        let matches = cursor.matches(&query, root_node, code.as_bytes());
        collect_matches(matches, &query, code)
            .into_iter()
            .map(|(i, m)| {
                (
                    i,
                    m.into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };

    let flipped_query = Query::new(
        &language,
        "(block
        [
            (expression_statement)
            (type_cast_expression)
        ] @last
        .
        )",
    )
    .unwrap();

    let flipped_matches = {
        let root_node = tree.root_node();
        let matches = cursor.matches(&flipped_query, root_node, code.as_bytes());
        collect_matches(matches, &flipped_query, code)
            .into_iter()
            .map(|(i, m)| {
                (
                    i,
                    m.into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        matches,
        vec![(0, vec![(String::from("last"), String::from("if b {}"))])]
    );
    assert_eq!(matches, flipped_matches);
}

#[test]
fn test_wildcard_parent_allows_fallible_child_patterns() {
    let language = get_language("javascript");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    let source_code = r#"
function foo() {
    "bar"
}
    "#;

    let query = Query::new(
        &language,
        "(function_declaration
          (_
            (expression_statement)
          )
        ) @part",
    )
    .unwrap();

    assert_query_matches(
        &language,
        &query,
        source_code,
        &[(0, vec![("part", "function foo() {\n    \"bar\"\n}")])],
    );
}

#[test]
fn test_unfinished_captures_are_not_definite_with_pending_anchors() {
    let language = get_language("javascript");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    let source_code = "
const foo = [
  1, 2, 3
]
";

    let tree = parser.parse(source_code, None).unwrap();
    let query = Query::new(&language, r#"(array (_) @foo . "]")"#).unwrap();
    let mut matches_cursor = QueryCursor::new();
    let mut captures_cursor = QueryCursor::new();

    let captures = captures_cursor.captures(&query, tree.root_node(), source_code.as_bytes());
    let captures = collect_captures(captures, &query, source_code);

    let matches = matches_cursor.matches(&query, tree.root_node(), source_code.as_bytes());
    let matches = collect_matches(matches, &query, source_code);

    assert_eq!(captures, vec![("foo", "3")]);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].1, captures);
}

#[test]
fn test_query_with_predicate_causing_oob_access() {
    let language = get_language("rust");

    let query = "(call_expression
     function: (scoped_identifier
       path: (scoped_identifier (identifier) @_regex (#any-of? @_regex \"Regex\" \"RegexBuilder\") .))
     (#set! injection.language \"regex\"))";
    Query::new(&language, query).unwrap();
}
