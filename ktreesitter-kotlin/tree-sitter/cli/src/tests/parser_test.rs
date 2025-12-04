use std::{
    sync::atomic::{AtomicUsize, Ordering},
    thread, time,
};

use tree_sitter::{
    Decode, IncludedRangesError, InputEdit, LogType, ParseOptions, ParseState, Parser, Point, Range,
};
use tree_sitter_proc_macro::retry;

use super::helpers::{
    allocations,
    edits::ReadRecorder,
    fixtures::{get_language, get_test_language},
};
use crate::{
    fuzz::edits::Edit,
    parse::perform_edit,
    tests::{generate_parser, helpers::fixtures::get_test_fixture_language, invert_edit},
};

#[test]
fn test_parsing_simple_string() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let tree = parser
        .parse(
            "
        struct Stuff {}
        fn main() {}
    ",
            None,
        )
        .unwrap();

    let root_node = tree.root_node();
    assert_eq!(root_node.kind(), "source_file");

    assert_eq!(
        root_node.to_sexp(),
        concat!(
            "(source_file ",
            "(struct_item name: (type_identifier) body: (field_declaration_list)) ",
            "(function_item name: (identifier) parameters: (parameters) body: (block)))"
        )
    );

    let struct_node = root_node.child(0).unwrap();
    assert_eq!(struct_node.kind(), "struct_item");
}

#[test]
fn test_parsing_with_logging() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let mut messages = Vec::new();
    parser.set_logger(Some(Box::new(|log_type, message| {
        messages.push((log_type, message.to_string()));
    })));

    parser
        .parse(
            "
        struct Stuff {}
        fn main() {}
    ",
            None,
        )
        .unwrap();

    assert!(messages.contains(&(
        LogType::Parse,
        "reduce sym:struct_item, child_count:3".to_string()
    )));
    assert!(messages.contains(&(LogType::Lex, "skip character:' '".to_string())));

    let mut row_starts_from_0 = false;
    for (_, m) in &messages {
        if m.contains("row:0") {
            row_starts_from_0 = true;
            break;
        }
    }
    assert!(row_starts_from_0);
}

#[test]
#[cfg(unix)]
fn test_parsing_with_debug_graph_enabled() {
    use std::io::{BufRead, BufReader, Seek};

    let has_zero_indexed_row = |s: &str| s.contains("position: 0,");

    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();

    let mut debug_graph_file = tempfile::tempfile().unwrap();
    parser.print_dot_graphs(&debug_graph_file);
    parser.parse("const zero = 0", None).unwrap();

    debug_graph_file.rewind().unwrap();
    let log_reader = BufReader::new(debug_graph_file)
        .lines()
        .map(|l| l.expect("Failed to read line from graph log"));
    for line in log_reader {
        assert!(
            !has_zero_indexed_row(&line),
            "Graph log output includes zero-indexed row: {line}",
        );
    }
}

#[test]
fn test_parsing_with_custom_utf8_input() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let lines = &["pub fn foo() {", "  1", "}"];

    let tree = parser
        .parse_with_options(
            &mut |_, position| {
                let row = position.row;
                let column = position.column;
                if row < lines.len() {
                    if column < lines[row].len() {
                        &lines[row].as_bytes()[column..]
                    } else {
                        b"\n"
                    }
                } else {
                    &[]
                }
            },
            None,
            None,
        )
        .unwrap();

    let root = tree.root_node();
    assert_eq!(
        root.to_sexp(),
        concat!(
            "(source_file ",
            "(function_item ",
            "(visibility_modifier) ",
            "name: (identifier) ",
            "parameters: (parameters) ",
            "body: (block (integer_literal))))"
        )
    );
    assert_eq!(root.kind(), "source_file");
    assert!(!root.has_error());
    assert_eq!(root.child(0).unwrap().kind(), "function_item");
}

#[test]
fn test_parsing_with_custom_utf16le_input() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let lines = ["pub fn foo() {", "  1", "}"]
        .iter()
        .map(|s| s.encode_utf16().map(u16::to_le).collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let newline = [('\n' as u16).to_le()];

    let tree = parser
        .parse_utf16_le_with_options(
            &mut |_, position| {
                let row = position.row;
                let column = position.column;
                if row < lines.len() {
                    if column < lines[row].len() {
                        &lines[row][column..]
                    } else {
                        &newline
                    }
                } else {
                    &[]
                }
            },
            None,
            None,
        )
        .unwrap();

    let root = tree.root_node();
    assert_eq!(
        root.to_sexp(),
        "(source_file (function_item (visibility_modifier) name: (identifier) parameters: (parameters) body: (block (integer_literal))))"
    );
    assert_eq!(root.kind(), "source_file");
    assert!(!root.has_error());
    assert_eq!(root.child(0).unwrap().kind(), "function_item");
}

#[test]
fn test_parsing_with_custom_utf16_be_input() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let lines: Vec<Vec<u16>> = ["pub fn foo() {", "  1", "}"]
        .iter()
        .map(|s| s.encode_utf16().collect::<Vec<_>>())
        .map(|v| v.iter().map(|u| u.to_be()).collect())
        .collect();

    let newline = [('\n' as u16).to_be()];

    let tree = parser
        .parse_utf16_be_with_options(
            &mut |_, position| {
                let row = position.row;
                let column = position.column;
                if row < lines.len() {
                    if column < lines[row].len() {
                        &lines[row][column..]
                    } else {
                        &newline
                    }
                } else {
                    &[]
                }
            },
            None,
            None,
        )
        .unwrap();
    let root = tree.root_node();
    assert_eq!(
        root.to_sexp(),
        "(source_file (function_item (visibility_modifier) name: (identifier) parameters: (parameters) body: (block (integer_literal))))"
    );
    assert_eq!(root.kind(), "source_file");
    assert!(!root.has_error());
    assert_eq!(root.child(0).unwrap().kind(), "function_item");
}

#[test]
fn test_parsing_with_callback_returning_owned_strings() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let text = b"pub fn foo() { 1 }";

    let tree = parser
        .parse_with_options(
            &mut |i, _| String::from_utf8(text[i..].to_vec()).unwrap(),
            None,
            None,
        )
        .unwrap();

    let root = tree.root_node();
    assert_eq!(
        root.to_sexp(),
        "(source_file (function_item (visibility_modifier) name: (identifier) parameters: (parameters) body: (block (integer_literal))))"
    );
}

#[test]
fn test_parsing_text_with_byte_order_mark() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    // Parse UTF16 text with a BOM
    let tree = parser
        .parse_utf16_le(
            "\u{FEFF}fn a() {}"
                .encode_utf16()
                .map(u16::to_le)
                .collect::<Vec<_>>(),
            None,
        )
        .unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (function_item name: (identifier) parameters: (parameters) body: (block)))"
    );
    assert_eq!(tree.root_node().start_byte(), 2);

    // Parse UTF8 text with a BOM
    let mut tree = parser.parse("\u{FEFF}fn a() {}", None).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (function_item name: (identifier) parameters: (parameters) body: (block)))"
    );
    assert_eq!(tree.root_node().start_byte(), 3);

    // Edit the text, inserting a character before the BOM. The BOM is now an error.
    tree.edit(&InputEdit {
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 1,
        start_position: Point::new(0, 0),
        old_end_position: Point::new(0, 0),
        new_end_position: Point::new(0, 1),
    });
    let mut tree = parser.parse(" \u{FEFF}fn a() {}", Some(&tree)).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (ERROR (UNEXPECTED 65279)) (function_item name: (identifier) parameters: (parameters) body: (block)))"
    );
    assert_eq!(tree.root_node().start_byte(), 1);

    // Edit the text again, putting the BOM back at the beginning.
    tree.edit(&InputEdit {
        start_byte: 0,
        old_end_byte: 1,
        new_end_byte: 0,
        start_position: Point::new(0, 0),
        old_end_position: Point::new(0, 1),
        new_end_position: Point::new(0, 0),
    });
    let tree = parser.parse("\u{FEFF}fn a() {}", Some(&tree)).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (function_item name: (identifier) parameters: (parameters) body: (block)))"
    );
    assert_eq!(tree.root_node().start_byte(), 3);
}

#[test]
fn test_parsing_invalid_chars_at_eof() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("json")).unwrap();
    let tree = parser.parse(b"\xdf", None).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        "(document (ERROR (UNEXPECTED INVALID)))"
    );
}

#[test]
fn test_parsing_unexpected_null_characters_within_source() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let tree = parser.parse(b"var \0 something;", None).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        "(program (variable_declaration (ERROR (UNEXPECTED '\\0')) (variable_declarator name: (identifier))))"
    );
}

#[test]
fn test_parsing_ends_when_input_callback_returns_empty() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let mut i = 0;
    let source = b"abcdefghijklmnoqrs";
    let tree = parser
        .parse_with_options(
            &mut |offset, _| {
                i += 1;
                if offset >= 6 {
                    b""
                } else {
                    &source[offset..usize::min(source.len(), offset + 3)]
                }
            },
            None,
            None,
        )
        .unwrap();
    assert_eq!(tree.root_node().end_byte(), 6);
}

// Incremental parsing

#[test]
fn test_parsing_after_editing_beginning_of_code() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();

    let mut code = b"123 + 456 * (10 + x);".to_vec();
    let mut tree = parser.parse(&code, None).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(program (expression_statement (binary_expression ",
            "left: (number) ",
            "right: (binary_expression left: (number) right: (parenthesized_expression ",
            "(binary_expression left: (number) right: (identifier)))))))",
        )
    );

    perform_edit(
        &mut tree,
        &mut code,
        &Edit {
            position: 3,
            deleted_length: 0,
            inserted_text: b" || 5".to_vec(),
        },
    )
    .unwrap();

    let mut recorder = ReadRecorder::new(&code);
    let tree = parser
        .parse_with_options(&mut |i, _| recorder.read(i), Some(&tree), None)
        .unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(program (expression_statement (binary_expression ",
            "left: (number) ",
            "right: (binary_expression ",
            "left: (number) ",
            "right: (binary_expression ",
            "left: (number) ",
            "right: (parenthesized_expression (binary_expression left: (number) right: (identifier))))))))",
        )
    );

    assert_eq!(recorder.strings_read(), vec!["123 || 5 "]);
}

#[test]
fn test_parsing_after_editing_end_of_code() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();

    let mut code = b"x * (100 + abc);".to_vec();
    let mut tree = parser.parse(&code, None).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(program (expression_statement (binary_expression ",
            "left: (identifier) ",
            "right: (parenthesized_expression (binary_expression left: (number) right: (identifier))))))",
        )
    );

    let position = code.len() - 2;
    perform_edit(
        &mut tree,
        &mut code,
        &Edit {
            position,
            deleted_length: 0,
            inserted_text: b".d".to_vec(),
        },
    )
    .unwrap();

    let mut recorder = ReadRecorder::new(&code);
    let tree = parser
        .parse_with_options(&mut |i, _| recorder.read(i), Some(&tree), None)
        .unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(program (expression_statement (binary_expression ",
            "left: (identifier) ",
            "right: (parenthesized_expression (binary_expression ",
            "left: (number) ",
            "right: (member_expression ",
            "object: (identifier) ",
            "property: (property_identifier)))))))"
        )
    );

    assert_eq!(recorder.strings_read(), vec![" * ", "abc.d)",]);
}

#[test]
fn test_parsing_empty_file_with_reused_tree() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let tree = parser.parse("", None);
    parser.parse("", tree.as_ref());

    let tree = parser.parse("\n  ", None);
    parser.parse("\n  ", tree.as_ref());
}

#[test]
fn test_parsing_after_editing_tree_that_depends_on_column_values() {
    let mut parser = Parser::new();
    parser
        .set_language(&get_test_fixture_language("uses_current_column"))
        .unwrap();

    let mut code = b"
a = b
c = do d
       e + f
       g
h + i
    "
    .to_vec();
    let mut tree = parser.parse(&code, None).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(block ",
            "(binary_expression (identifier) (identifier)) ",
            "(binary_expression (identifier) (do_expression (block (identifier) (binary_expression (identifier) (identifier)) (identifier)))) ",
            "(binary_expression (identifier) (identifier)))",
        )
    );

    perform_edit(
        &mut tree,
        &mut code,
        &Edit {
            position: 8,
            deleted_length: 0,
            inserted_text: b"1234".to_vec(),
        },
    )
    .unwrap();

    assert_eq!(
        code,
        b"
a = b
c1234 = do d
       e + f
       g
h + i
    "
    );

    let mut recorder = ReadRecorder::new(&code);
    let tree = parser
        .parse_with_options(&mut |i, _| recorder.read(i), Some(&tree), None)
        .unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(block ",
            "(binary_expression (identifier) (identifier)) ",
            "(binary_expression (identifier) (do_expression (block (identifier)))) ",
            "(binary_expression (identifier) (identifier)) ",
            "(identifier) ",
            "(binary_expression (identifier) (identifier)))",
        )
    );

    assert_eq!(
        recorder.strings_read(),
        vec!["\nc1234 = do d\n       e + f\n       g\n"]
    );
}

#[test]
fn test_parsing_after_editing_tree_that_depends_on_column_position() {
    let mut parser = Parser::new();
    parser
        .set_language(&get_test_fixture_language("depends_on_column"))
        .unwrap();

    let mut code = b"\n x".to_vec();
    let mut tree = parser.parse(&code, None).unwrap();
    assert_eq!(tree.root_node().to_sexp(), "(x_is_at (odd_column))");

    perform_edit(
        &mut tree,
        &mut code,
        &Edit {
            position: 1,
            deleted_length: 0,
            inserted_text: b" ".to_vec(),
        },
    )
    .unwrap();

    assert_eq!(code, b"\n  x");

    let mut recorder = ReadRecorder::new(&code);
    let mut tree = parser
        .parse_with_options(&mut |i, _| recorder.read(i), Some(&tree), None)
        .unwrap();

    assert_eq!(tree.root_node().to_sexp(), "(x_is_at (even_column))",);
    assert_eq!(recorder.strings_read(), vec!["\n  x"]);

    perform_edit(
        &mut tree,
        &mut code,
        &Edit {
            position: 1,
            deleted_length: 0,
            inserted_text: b"\n".to_vec(),
        },
    )
    .unwrap();

    assert_eq!(code, b"\n\n  x");

    let mut recorder = ReadRecorder::new(&code);
    let tree = parser
        .parse_with_options(&mut |i, _| recorder.read(i), Some(&tree), None)
        .unwrap();

    assert_eq!(tree.root_node().to_sexp(), "(x_is_at (even_column))",);
    assert_eq!(recorder.strings_read(), vec!["\n\n  x"]);
}

#[test]
fn test_parsing_after_detecting_error_in_the_middle_of_a_string_token() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("python")).unwrap();

    let mut source = b"a = b, 'c, d'".to_vec();
    let tree = parser.parse(&source, None).unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        "(module (expression_statement (assignment left: (identifier) right: (expression_list (identifier) (string (string_start) (string_content) (string_end))))))"
    );

    // Delete a suffix of the source code, starting in the middle of the string
    // literal, after some whitespace. With this deletion, the remaining string
    // content: "c, " looks like two valid python tokens: an identifier and a comma.
    // When this edit is undone, in order correctly recover the original tree, the
    // parser needs to remember that before matching the `c` as an identifier, it
    // lookahead ahead several bytes, trying to find the closing quotation mark in
    // order to match the "string content" node.
    let edit_ix = std::str::from_utf8(&source).unwrap().find("d'").unwrap();
    let edit = Edit {
        position: edit_ix,
        deleted_length: source.len() - edit_ix,
        inserted_text: Vec::new(),
    };
    let undo = invert_edit(&source, &edit);

    let mut tree2 = tree.clone();
    perform_edit(&mut tree2, &mut source, &edit).unwrap();
    tree2 = parser.parse(&source, Some(&tree2)).unwrap();
    assert!(tree2.root_node().has_error());

    let mut tree3 = tree2.clone();
    perform_edit(&mut tree3, &mut source, &undo).unwrap();
    tree3 = parser.parse(&source, Some(&tree3)).unwrap();
    assert_eq!(tree3.root_node().to_sexp(), tree.root_node().to_sexp(),);
}

// Thread safety

#[test]
fn test_parsing_on_multiple_threads() {
    // Parse this source file so that each thread has a non-trivial amount of
    // work to do.
    let this_file_source = include_str!("parser_test.rs");

    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();
    let tree = parser.parse(this_file_source, None).unwrap();

    let mut parse_threads = Vec::new();
    for thread_id in 1..5 {
        let mut tree_clone = tree.clone();
        parse_threads.push(thread::spawn(move || {
            // For each thread, prepend a different number of declarations to the
            // source code.
            let mut prepend_line_count = 0;
            let mut prepended_source = String::new();
            for _ in 0..thread_id {
                prepend_line_count += 2;
                prepended_source += "struct X {}\n\n";
            }

            tree_clone.edit(&InputEdit {
                start_byte: 0,
                old_end_byte: 0,
                new_end_byte: prepended_source.len(),
                start_position: Point::new(0, 0),
                old_end_position: Point::new(0, 0),
                new_end_position: Point::new(prepend_line_count, 0),
            });
            prepended_source += this_file_source;

            // Reparse using the old tree as a starting point.
            let mut parser = Parser::new();
            parser.set_language(&get_language("rust")).unwrap();
            parser.parse(&prepended_source, Some(&tree_clone)).unwrap()
        }));
    }

    // Check that the trees have the expected relationship to one another.
    let trees = parse_threads
        .into_iter()
        .map(|thread| thread.join().unwrap());
    let child_count_differences = trees
        .map(|t| t.root_node().child_count() - tree.root_node().child_count())
        .collect::<Vec<_>>();

    assert_eq!(child_count_differences, &[1, 2, 3, 4]);
}

#[test]
fn test_parsing_cancelled_by_another_thread() {
    let cancellation_flag = std::sync::Arc::new(AtomicUsize::new(0));
    let flag = cancellation_flag.clone();
    let callback = &mut |_: &ParseState| cancellation_flag.load(Ordering::SeqCst) != 0;

    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();

    // Long input - parsing succeeds
    let tree = parser.parse_with_options(
        &mut |offset, _| {
            if offset == 0 {
                " [".as_bytes()
            } else if offset >= 20000 {
                "".as_bytes()
            } else {
                "0,".as_bytes()
            }
        },
        None,
        Some(ParseOptions::new().progress_callback(callback)),
    );
    assert!(tree.is_some());

    let cancel_thread = thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(100));
        flag.store(1, Ordering::SeqCst);
    });

    // Infinite input
    let tree = parser.parse_with_options(
        &mut |offset, _| {
            thread::yield_now();
            thread::sleep(time::Duration::from_millis(10));
            if offset == 0 {
                b" ["
            } else {
                b"0,"
            }
        },
        None,
        Some(ParseOptions::new().progress_callback(callback)),
    );

    // Parsing returns None because it was cancelled.
    cancel_thread.join().unwrap();
    assert!(tree.is_none());
}

// Timeouts

#[test]
#[retry(10)]
fn test_parsing_with_a_timeout() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("json")).unwrap();

    // Parse an infinitely-long array, but pause after 1ms of processing.
    let start_time = time::Instant::now();
    let tree = parser.parse_with_options(
        &mut |offset, _| {
            if offset == 0 {
                b" ["
            } else {
                b",0"
            }
        },
        None,
        Some(
            ParseOptions::new().progress_callback(&mut |_| start_time.elapsed().as_micros() > 1000),
        ),
    );
    assert!(tree.is_none());
    assert!(start_time.elapsed().as_micros() < 2000);

    // Continue parsing, but pause after 1 ms of processing.
    let start_time = time::Instant::now();
    let tree = parser.parse_with_options(
        &mut |offset, _| {
            if offset == 0 {
                b" ["
            } else {
                b",0"
            }
        },
        None,
        Some(
            ParseOptions::new().progress_callback(&mut |_| start_time.elapsed().as_micros() > 5000),
        ),
    );
    assert!(tree.is_none());
    assert!(start_time.elapsed().as_micros() > 100);
    assert!(start_time.elapsed().as_micros() < 10000);

    // Finish parsing
    let tree = parser
        .parse_with_options(
            &mut |offset, _| match offset {
                5001.. => "".as_bytes(),
                5000 => "]".as_bytes(),
                _ => ",0".as_bytes(),
            },
            None,
            None,
        )
        .unwrap();
    assert_eq!(tree.root_node().child(0).unwrap().kind(), "array");
}

#[test]
#[retry(10)]
fn test_parsing_with_a_timeout_and_a_reset() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("json")).unwrap();

    let start_time = time::Instant::now();
    let code = "[\"ok\", 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]";
    let tree = parser.parse_with_options(
        &mut |offset, _| {
            if offset >= code.len() {
                &[]
            } else {
                &code.as_bytes()[offset..]
            }
        },
        None,
        Some(ParseOptions::new().progress_callback(&mut |_| start_time.elapsed().as_micros() > 5)),
    );
    assert!(tree.is_none());

    // Without calling reset, the parser continues from where it left off, so
    // it does not see the changes to the beginning of the source code.
    let tree = parser.parse(
        "[null, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]",
        None,
    ).unwrap();
    assert_eq!(
        tree.root_node()
            .named_child(0)
            .unwrap()
            .named_child(0)
            .unwrap()
            .kind(),
        "string"
    );

    let start_time = time::Instant::now();
    let code = "[\"ok\", 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]";
    let tree = parser.parse_with_options(
        &mut |offset, _| {
            if offset >= code.len() {
                &[]
            } else {
                &code.as_bytes()[offset..]
            }
        },
        None,
        Some(ParseOptions::new().progress_callback(&mut |_| start_time.elapsed().as_micros() > 5)),
    );
    assert!(tree.is_none());

    // By calling reset, we force the parser to start over from scratch so
    // that it sees the changes to the beginning of the source code.
    parser.reset();
    let tree = parser.parse(
        "[null, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]",
        None,
    ).unwrap();
    assert_eq!(
        tree.root_node()
            .named_child(0)
            .unwrap()
            .named_child(0)
            .unwrap()
            .kind(),
        "null"
    );
}

#[test]
#[retry(10)]
fn test_parsing_with_a_timeout_and_implicit_reset() {
    allocations::record(|| {
        let mut parser = Parser::new();
        parser.set_language(&get_language("javascript")).unwrap();

        let code = "[\"ok\", 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]";
        let start_time = time::Instant::now();
        let tree = parser.parse_with_options(
            &mut |offset, _| {
                if offset >= code.len() {
                    &[]
                } else {
                    &code.as_bytes()[offset..]
                }
            },
            None,
            Some(
                ParseOptions::new()
                    .progress_callback(&mut |_| start_time.elapsed().as_micros() > 5),
            ),
        );
        assert!(tree.is_none());

        // Changing the parser's language implicitly resets, discarding
        // the previous partial parse.
        parser.set_language(&get_language("json")).unwrap();
        let tree = parser.parse(
            "[null, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]",
            None,
        ).unwrap();
        assert_eq!(
            tree.root_node()
                .named_child(0)
                .unwrap()
                .named_child(0)
                .unwrap()
                .kind(),
            "null"
        );
    });
}

#[test]
#[retry(10)]
fn test_parsing_with_timeout_and_no_completion() {
    allocations::record(|| {
        let mut parser = Parser::new();
        parser.set_language(&get_language("javascript")).unwrap();

        let code = "[\"ok\", 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]";
        let start_time = time::Instant::now();
        let tree = parser.parse_with_options(
            &mut |offset, _| {
                if offset >= code.len() {
                    &[]
                } else {
                    &code.as_bytes()[offset..]
                }
            },
            None,
            Some(
                ParseOptions::new()
                    .progress_callback(&mut |_| start_time.elapsed().as_micros() > 5),
            ),
        );
        assert!(tree.is_none());

        // drop the parser when it has an unfinished parse
    });
}

#[test]
fn test_parsing_with_timeout_during_balancing() {
    allocations::record(|| {
        let mut parser = Parser::new();
        parser.set_language(&get_language("javascript")).unwrap();

        let function_count = 100;

        let code = "function() {}\n".repeat(function_count);
        let mut current_byte_offset = 0;
        let mut in_balancing = false;
        let tree = parser.parse_with_options(
            &mut |offset, _| {
                if offset >= code.len() {
                    &[]
                } else {
                    &code.as_bytes()[offset..]
                }
            },
            None,
            Some(ParseOptions::new().progress_callback(&mut |state| {
                // The parser will call the progress_callback during parsing, and at the very end
                // during tree-balancing. For very large trees, this balancing act can take quite
                // some time, so we want to verify that timing out during this operation is
                // possible.
                //
                // We verify this by checking the current byte offset, as this number will *not* be
                // updated during tree balancing. If we see the same offset twice, we know that we
                // are in the balancing phase.
                if state.current_byte_offset() != current_byte_offset {
                    current_byte_offset = state.current_byte_offset();
                    false
                } else {
                    in_balancing = true;
                    true
                }
            })),
        );

        assert!(tree.is_none());
        assert!(in_balancing);

        // This should not cause an assertion failure.
        parser.reset();
        let tree = parser.parse_with_options(
            &mut |offset, _| {
                if offset >= code.len() {
                    &[]
                } else {
                    &code.as_bytes()[offset..]
                }
            },
            None,
            Some(ParseOptions::new().progress_callback(&mut |state| {
                if state.current_byte_offset() != current_byte_offset {
                    current_byte_offset = state.current_byte_offset();
                    false
                } else {
                    in_balancing = true;
                    true
                }
            })),
        );

        assert!(tree.is_none());
        assert!(in_balancing);

        // If we resume parsing (implying we didn't call `parser.reset()`), we should be able to
        // finish parsing the tree, continuing from where we left off.
        let tree = parser
            .parse_with_options(
                &mut |offset, _| {
                    if offset >= code.len() {
                        &[]
                    } else {
                        &code.as_bytes()[offset..]
                    }
                },
                None,
                Some(ParseOptions::new().progress_callback(&mut |state| {
                    // Because we've already finished parsing, we should only be resuming the
                    // balancing phase.
                    assert!(state.current_byte_offset() == current_byte_offset);
                    false
                })),
            )
            .unwrap();
        assert!(!tree.root_node().has_error());
        assert_eq!(tree.root_node().child_count(), function_count);
    });
}

#[test]
fn test_parsing_with_timeout_when_error_detected() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("json")).unwrap();

    // Parse an infinitely-long array, but insert an error after 1000 characters.
    let mut offset = 0;
    let erroneous_code = "!,";
    let tree = parser.parse_with_options(
        &mut |i, _| match i {
            0 => "[",
            1..=1000 => "0,",
            _ => erroneous_code,
        },
        None,
        Some(ParseOptions::new().progress_callback(&mut |state| {
            offset = state.current_byte_offset();
            state.has_error()
        })),
    );

    // The callback is called at the end of parsing, however, what we're asserting here is that
    // parsing ends immediately as the error is detected. This is verified by checking the offset
    // of the last byte processed is the length of the erroneous code we inserted, aka, 1002, or
    // 1000 + the length of the erroneous code.
    assert_eq!(offset, 1000 + erroneous_code.len());
    assert!(tree.is_none());
}

// Included Ranges

#[test]
fn test_parsing_with_one_included_range() {
    let source_code = "<span>hi</span><script>console.log('sup');</script>";

    let mut parser = Parser::new();
    parser.set_language(&get_language("html")).unwrap();
    let html_tree = parser.parse(source_code, None).unwrap();
    let script_content_node = html_tree.root_node().child(1).unwrap().child(1).unwrap();
    assert_eq!(script_content_node.kind(), "raw_text");

    assert_eq!(
        parser.included_ranges(),
        &[Range {
            start_byte: 0,
            end_byte: u32::MAX as usize,
            start_point: Point::new(0, 0),
            end_point: Point::new(u32::MAX as usize, u32::MAX as usize),
        }]
    );
    parser
        .set_included_ranges(&[script_content_node.range()])
        .unwrap();
    assert_eq!(parser.included_ranges(), &[script_content_node.range()]);
    parser.set_language(&get_language("javascript")).unwrap();
    let js_tree = parser.parse(source_code, None).unwrap();

    assert_eq!(
        js_tree.root_node().to_sexp(),
        concat!(
            "(program (expression_statement (call_expression ",
            "function: (member_expression object: (identifier) property: (property_identifier)) ",
            "arguments: (arguments (string (string_fragment))))))",
        )
    );
    assert_eq!(
        js_tree.root_node().start_position(),
        Point::new(0, source_code.find("console").unwrap())
    );
    assert_eq!(js_tree.included_ranges(), &[script_content_node.range()]);
}

#[test]
fn test_parsing_with_multiple_included_ranges() {
    let source_code = "html `<div>Hello, ${name.toUpperCase()}, it's <b>${now()}</b>.</div>`";

    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    let js_tree = parser.parse(source_code, None).unwrap();
    let template_string_node = js_tree
        .root_node()
        .descendant_for_byte_range(
            source_code.find("`<").unwrap(),
            source_code.find(">`").unwrap(),
        )
        .unwrap();
    assert_eq!(template_string_node.kind(), "template_string");

    let open_quote_node = template_string_node.child(0).unwrap();
    let interpolation_node1 = template_string_node.child(2).unwrap();
    let interpolation_node2 = template_string_node.child(4).unwrap();
    let close_quote_node = template_string_node.child(6).unwrap();

    parser.set_language(&get_language("html")).unwrap();
    let html_ranges = &[
        Range {
            start_byte: open_quote_node.end_byte(),
            start_point: open_quote_node.end_position(),
            end_byte: interpolation_node1.start_byte(),
            end_point: interpolation_node1.start_position(),
        },
        Range {
            start_byte: interpolation_node1.end_byte(),
            start_point: interpolation_node1.end_position(),
            end_byte: interpolation_node2.start_byte(),
            end_point: interpolation_node2.start_position(),
        },
        Range {
            start_byte: interpolation_node2.end_byte(),
            start_point: interpolation_node2.end_position(),
            end_byte: close_quote_node.start_byte(),
            end_point: close_quote_node.start_position(),
        },
    ];
    parser.set_included_ranges(html_ranges).unwrap();
    let html_tree = parser.parse(source_code, None).unwrap();

    assert_eq!(
        html_tree.root_node().to_sexp(),
        concat!(
            "(document (element",
            " (start_tag (tag_name))",
            " (text)",
            " (element (start_tag (tag_name)) (end_tag (tag_name)))",
            " (text)",
            " (end_tag (tag_name))))",
        )
    );
    assert_eq!(html_tree.included_ranges(), html_ranges);

    let div_element_node = html_tree.root_node().child(0).unwrap();
    let hello_text_node = div_element_node.child(1).unwrap();
    let b_element_node = div_element_node.child(2).unwrap();
    let b_start_tag_node = b_element_node.child(0).unwrap();
    let b_end_tag_node = b_element_node.child(1).unwrap();

    assert_eq!(hello_text_node.kind(), "text");
    assert_eq!(
        hello_text_node.start_byte(),
        source_code.find("Hello").unwrap()
    );
    assert_eq!(
        hello_text_node.end_byte(),
        source_code.find(" <b>").unwrap()
    );

    assert_eq!(b_start_tag_node.kind(), "start_tag");
    assert_eq!(
        b_start_tag_node.start_byte(),
        source_code.find("<b>").unwrap()
    );
    assert_eq!(
        b_start_tag_node.end_byte(),
        source_code.find("${now()}").unwrap()
    );

    assert_eq!(b_end_tag_node.kind(), "end_tag");
    assert_eq!(
        b_end_tag_node.start_byte(),
        source_code.find("</b>").unwrap()
    );
    assert_eq!(
        b_end_tag_node.end_byte(),
        source_code.find(".</div>").unwrap()
    );
}

#[test]
fn test_parsing_with_included_range_containing_mismatched_positions() {
    let source_code = "<div>test</div>{_ignore_this_part_}";

    let mut parser = Parser::new();
    parser.set_language(&get_language("html")).unwrap();

    let end_byte = source_code.find("{_ignore_this_part_").unwrap();

    let range_to_parse = Range {
        start_byte: 0,
        start_point: Point {
            row: 10,
            column: 12,
        },
        end_byte,
        end_point: Point {
            row: 10,
            column: 12 + end_byte,
        },
    };

    parser.set_included_ranges(&[range_to_parse]).unwrap();

    let html_tree = parser
        .parse_with_options(&mut chunked_input(source_code, 3), None, None)
        .unwrap();

    assert_eq!(html_tree.root_node().range(), range_to_parse);

    assert_eq!(
        html_tree.root_node().to_sexp(),
        "(document (element (start_tag (tag_name)) (text) (end_tag (tag_name))))"
    );
}

#[test]
fn test_parsing_error_in_invalid_included_ranges() {
    let mut parser = Parser::new();

    // Ranges are not ordered
    let error = parser
        .set_included_ranges(&[
            Range {
                start_byte: 23,
                end_byte: 29,
                start_point: Point::new(0, 23),
                end_point: Point::new(0, 29),
            },
            Range {
                start_byte: 0,
                end_byte: 5,
                start_point: Point::new(0, 0),
                end_point: Point::new(0, 5),
            },
            Range {
                start_byte: 50,
                end_byte: 60,
                start_point: Point::new(0, 50),
                end_point: Point::new(0, 60),
            },
        ])
        .unwrap_err();
    assert_eq!(error, IncludedRangesError(1));

    // Range ends before it starts
    let error = parser
        .set_included_ranges(&[Range {
            start_byte: 10,
            end_byte: 5,
            start_point: Point::new(0, 10),
            end_point: Point::new(0, 5),
        }])
        .unwrap_err();
    assert_eq!(error, IncludedRangesError(0));
}

#[test]
fn test_parsing_utf16_code_with_errors_at_the_end_of_an_included_range() {
    let source_code = "<script>a.</script>";
    let utf16_source_code = source_code
        .encode_utf16()
        .map(u16::to_le)
        .collect::<Vec<_>>();

    let start_byte = 2 * source_code.find("a.").unwrap();
    let end_byte = 2 * source_code.find("</script>").unwrap();

    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    parser
        .set_included_ranges(&[Range {
            start_byte,
            end_byte,
            start_point: Point::new(0, start_byte),
            end_point: Point::new(0, end_byte),
        }])
        .unwrap();
    let tree = parser.parse_utf16_le(&utf16_source_code, None).unwrap();
    assert_eq!(tree.root_node().to_sexp(), "(program (ERROR (identifier)))");
}

#[test]
fn test_parsing_with_external_scanner_that_uses_included_range_boundaries() {
    let source_code = "a <%= b() %> c <% d() %>";
    let range1_start_byte = source_code.find(" b() ").unwrap();
    let range1_end_byte = range1_start_byte + " b() ".len();
    let range2_start_byte = source_code.find(" d() ").unwrap();
    let range2_end_byte = range2_start_byte + " d() ".len();

    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    parser
        .set_included_ranges(&[
            Range {
                start_byte: range1_start_byte,
                end_byte: range1_end_byte,
                start_point: Point::new(0, range1_start_byte),
                end_point: Point::new(0, range1_end_byte),
            },
            Range {
                start_byte: range2_start_byte,
                end_byte: range2_end_byte,
                start_point: Point::new(0, range2_start_byte),
                end_point: Point::new(0, range2_end_byte),
            },
        ])
        .unwrap();

    let tree = parser.parse(source_code, None).unwrap();
    let root = tree.root_node();
    let statement1 = root.child(0).unwrap();
    let statement2 = root.child(1).unwrap();

    assert_eq!(
        root.to_sexp(),
        concat!(
            "(program",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments)))",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments))))"
        )
    );

    assert_eq!(statement1.start_byte(), source_code.find("b()").unwrap());
    assert_eq!(statement1.end_byte(), source_code.find(" %> c").unwrap());
    assert_eq!(statement2.start_byte(), source_code.find("d()").unwrap());
    assert_eq!(statement2.end_byte(), source_code.len() - " %>".len());
}

#[test]
fn test_parsing_with_a_newly_excluded_range() {
    let mut source_code = String::from("<div><span><%= something %></span></div>");

    // Parse HTML including the template directive, which will cause an error
    let mut parser = Parser::new();
    parser.set_language(&get_language("html")).unwrap();
    let mut first_tree = parser
        .parse_with_options(&mut chunked_input(&source_code, 3), None, None)
        .unwrap();

    // Insert code at the beginning of the document.
    let prefix = "a very very long line of plain text. ";
    first_tree.edit(&InputEdit {
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: prefix.len(),
        start_position: Point::new(0, 0),
        old_end_position: Point::new(0, 0),
        new_end_position: Point::new(0, prefix.len()),
    });
    source_code.insert_str(0, prefix);

    // Parse the HTML again, this time *excluding* the template directive
    // (which has moved since the previous parse).
    let directive_start = source_code.find("<%=").unwrap();
    let directive_end = source_code.find("</span>").unwrap();
    let source_code_end = source_code.len();
    parser
        .set_included_ranges(&[
            Range {
                start_byte: 0,
                end_byte: directive_start,
                start_point: Point::new(0, 0),
                end_point: Point::new(0, directive_start),
            },
            Range {
                start_byte: directive_end,
                end_byte: source_code_end,
                start_point: Point::new(0, directive_end),
                end_point: Point::new(0, source_code_end),
            },
        ])
        .unwrap();
    let tree = parser
        .parse_with_options(&mut chunked_input(&source_code, 3), Some(&first_tree), None)
        .unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(document (text) (element",
            " (start_tag (tag_name))",
            " (element (start_tag (tag_name)) (end_tag (tag_name)))",
            " (end_tag (tag_name))))"
        )
    );

    assert_eq!(
        tree.changed_ranges(&first_tree).collect::<Vec<_>>(),
        vec![
            // The first range that has changed syntax is the range of the newly-inserted text.
            Range {
                start_byte: 0,
                end_byte: prefix.len(),
                start_point: Point::new(0, 0),
                end_point: Point::new(0, prefix.len()),
            },
            // Even though no edits were applied to the outer `div` element,
            // its contents have changed syntax because a range of text that
            // was previously included is now excluded.
            Range {
                start_byte: directive_start,
                end_byte: directive_end,
                start_point: Point::new(0, directive_start),
                end_point: Point::new(0, directive_end),
            },
        ]
    );
}

#[test]
fn test_parsing_with_a_newly_included_range() {
    let source_code = "<div><%= foo() %></div><span><%= bar() %></span><%= baz() %>";
    let range1_start = source_code.find(" foo").unwrap();
    let range2_start = source_code.find(" bar").unwrap();
    let range3_start = source_code.find(" baz").unwrap();
    let range1_end = range1_start + 7;
    let range2_end = range2_start + 7;
    let range3_end = range3_start + 7;

    // Parse only the first code directive as JavaScript
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();
    parser
        .set_included_ranges(&[simple_range(range1_start, range1_end)])
        .unwrap();
    let tree = parser
        .parse_with_options(&mut chunked_input(source_code, 3), None, None)
        .unwrap();
    assert_eq!(
        tree.root_node().to_sexp(),
        concat!(
            "(program",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments))))",
        )
    );

    // Parse both the first and third code directives as JavaScript, using the old tree as a
    // reference.
    parser
        .set_included_ranges(&[
            simple_range(range1_start, range1_end),
            simple_range(range3_start, range3_end),
        ])
        .unwrap();
    let tree2 = parser
        .parse_with_options(&mut chunked_input(source_code, 3), Some(&tree), None)
        .unwrap();
    assert_eq!(
        tree2.root_node().to_sexp(),
        concat!(
            "(program",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments)))",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments))))",
        )
    );
    assert_eq!(
        tree2.changed_ranges(&tree).collect::<Vec<_>>(),
        &[simple_range(range1_end, range3_end)]
    );

    // Parse all three code directives as JavaScript, using the old tree as a
    // reference.
    parser
        .set_included_ranges(&[
            simple_range(range1_start, range1_end),
            simple_range(range2_start, range2_end),
            simple_range(range3_start, range3_end),
        ])
        .unwrap();
    let tree3 = parser.parse(source_code, Some(&tree)).unwrap();
    assert_eq!(
        tree3.root_node().to_sexp(),
        concat!(
            "(program",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments)))",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments)))",
            " (expression_statement (call_expression function: (identifier) arguments: (arguments))))",
        )
    );
    assert_eq!(
        tree3.changed_ranges(&tree2).collect::<Vec<_>>(),
        &[simple_range(range2_start + 1, range2_end - 1)]
    );
}

#[test]
fn test_parsing_with_included_ranges_and_missing_tokens() {
    let (parser_name, parser_code) = generate_parser(
        r#"{
            "name": "test_leading_missing_token",
            "rules": {
                "program": {
                    "type": "SEQ",
                    "members": [
                        {"type": "SYMBOL", "name": "A"},
                        {"type": "SYMBOL", "name": "b"},
                        {"type": "SYMBOL", "name": "c"},
                        {"type": "SYMBOL", "name": "A"},
                        {"type": "SYMBOL", "name": "b"},
                        {"type": "SYMBOL", "name": "c"}
                    ]
                },
                "A": {"type": "SYMBOL", "name": "a"},
                "a": {"type": "STRING", "value": "a"},
                "b": {"type": "STRING", "value": "b"},
                "c": {"type": "STRING", "value": "c"}
            }
        }"#,
    )
    .unwrap();

    let mut parser = Parser::new();
    parser
        .set_language(&get_test_language(&parser_name, &parser_code, None))
        .unwrap();

    // There's a missing `a` token at the beginning of the code. It must be inserted
    // at the beginning of the first included range, not at {0, 0}.
    let source_code = "__bc__bc__";
    parser
        .set_included_ranges(&[
            Range {
                start_byte: 2,
                end_byte: 4,
                start_point: Point::new(0, 2),
                end_point: Point::new(0, 4),
            },
            Range {
                start_byte: 6,
                end_byte: 8,
                start_point: Point::new(0, 6),
                end_point: Point::new(0, 8),
            },
        ])
        .unwrap();

    let tree = parser.parse(source_code, None).unwrap();
    let root = tree.root_node();
    assert_eq!(
        root.to_sexp(),
        "(program (A (MISSING a)) (b) (c) (A (MISSING a)) (b) (c))"
    );
    assert_eq!(root.start_byte(), 2);
    assert_eq!(root.child(3).unwrap().start_byte(), 4);
}

#[test]
fn test_grammars_that_can_hang_on_eof() {
    let (parser_name, parser_code) = generate_parser(
        r#"
        {
            "name": "test_single_null_char_regex",
            "rules": {
                "source_file": {
                    "type": "SEQ",
                    "members": [
                        { "type": "STRING", "value": "\"" },
                        { "type": "PATTERN", "value": "[\\x00]*" },
                        { "type": "STRING", "value": "\"" }
                    ]
                }
            },
            "extras": [ { "type": "PATTERN", "value": "\\s" } ]
        }
        "#,
    )
    .unwrap();

    let mut parser = Parser::new();
    parser
        .set_language(&get_test_language(&parser_name, &parser_code, None))
        .unwrap();
    parser.parse("\"", None).unwrap();

    let (parser_name, parser_code) = generate_parser(
        r#"
        {
            "name": "test_null_char_with_next_char_regex",
            "rules": {
                "source_file": {
                    "type": "SEQ",
                    "members": [
                        { "type": "STRING", "value": "\"" },
                        { "type": "PATTERN", "value": "[\\x00-\\x01]*" },
                        { "type": "STRING", "value": "\"" }
                    ]
                }
            },
            "extras": [ { "type": "PATTERN", "value": "\\s" } ]
        }
        "#,
    )
    .unwrap();

    parser
        .set_language(&get_test_language(&parser_name, &parser_code, None))
        .unwrap();
    parser.parse("\"", None).unwrap();

    let (parser_name, parser_code) = generate_parser(
        r#"
        {
            "name": "test_null_char_with_range_regex",
            "rules": {
                "source_file": {
                    "type": "SEQ",
                    "members": [
                        { "type": "STRING", "value": "\"" },
                        { "type": "PATTERN", "value": "[\\x00-\\x7F]*" },
                        { "type": "STRING", "value": "\"" }
                    ]
                }
            },
            "extras": [ { "type": "PATTERN", "value": "\\s" } ]
        }
        "#,
    )
    .unwrap();

    parser
        .set_language(&get_test_language(&parser_name, &parser_code, None))
        .unwrap();
    parser.parse("\"", None).unwrap();
}

#[test]
fn test_parse_stack_recursive_merge_error_cost_calculation_bug() {
    let source_code = r"
fn main() {
  if n == 1 {
  } else if n == 2 {
  } else {
  }
}

let y = if x == 5 { 10 } else { 15 };

if foo && bar {}

if foo && bar || baz {}
";

    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let mut tree = parser.parse(source_code, None).unwrap();

    let edit = Edit {
        position: 60,
        deleted_length: 63,
        inserted_text: Vec::new(),
    };
    let mut input = source_code.as_bytes().to_vec();
    perform_edit(&mut tree, &mut input, &edit).unwrap();

    parser.parse(&input, Some(&tree)).unwrap();
}

#[test]
fn test_parsing_with_scanner_logging() {
    let mut parser = Parser::new();
    parser
        .set_language(&get_test_fixture_language("external_tokens"))
        .unwrap();

    let mut found = false;
    parser.set_logger(Some(Box::new(|log_type, message| {
        if log_type == LogType::Lex && message == "Found a percent string" {
            found = true;
        }
    })));

    let source_code = "x + %(sup (external) scanner?)";

    parser.parse(source_code, None).unwrap();
    assert!(found);
}

#[test]
fn test_parsing_get_column_at_eof() {
    let mut parser = Parser::new();
    parser
        .set_language(&get_test_fixture_language("get_col_eof"))
        .unwrap();

    parser.parse("a", None).unwrap();
}

#[test]
fn test_parsing_by_halting_at_offset() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("javascript")).unwrap();

    let source_code = "function foo() { return 1; }".repeat(1000);

    let mut seen_byte_offsets = vec![];

    parser
        .parse_with_options(
            &mut |offset, _| {
                if offset < source_code.len() {
                    &source_code.as_bytes()[offset..]
                } else {
                    &[]
                }
            },
            None,
            Some(ParseOptions::new().progress_callback(&mut |p| {
                seen_byte_offsets.push(p.current_byte_offset());
                false
            })),
        )
        .unwrap();

    assert!(seen_byte_offsets.len() > 100);
}

#[test]
fn test_decode_utf32() {
    use widestring::u32cstr;

    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let utf32_text = u32cstr!("pub fn foo() { println!(\"€50\"); }");
    let utf32_text = unsafe {
        std::slice::from_raw_parts(utf32_text.as_ptr().cast::<u8>(), utf32_text.len() * 4)
    };

    struct U32Decoder;

    impl Decode for U32Decoder {
        fn decode(bytes: &[u8]) -> (i32, u32) {
            if bytes.len() >= 4 {
                #[cfg(target_endian = "big")]
                {
                    (
                        i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                        4,
                    )
                }

                #[cfg(target_endian = "little")]
                {
                    (
                        i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                        4,
                    )
                }
            } else {
                (0, 0)
            }
        }
    }

    let tree = parser
        .parse_custom_encoding::<U32Decoder, _, _>(
            &mut |offset, _| {
                if offset < utf32_text.len() {
                    &utf32_text[offset..]
                } else {
                    &[]
                }
            },
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (function_item (visibility_modifier) name: (identifier) parameters: (parameters) body: (block (expression_statement (macro_invocation macro: (identifier) (token_tree (string_literal (string_content))))))))"
    );
}

#[test]
fn test_decode_cp1252() {
    use encoding_rs::WINDOWS_1252;

    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let windows_1252_text = WINDOWS_1252.encode("pub fn foo() { println!(\"€50\"); }").0;

    struct Cp1252Decoder;

    impl Decode for Cp1252Decoder {
        fn decode(bytes: &[u8]) -> (i32, u32) {
            if !bytes.is_empty() {
                let byte = bytes[0];
                (byte as i32, 1)
            } else {
                (0, 0)
            }
        }
    }

    let tree = parser
        .parse_custom_encoding::<Cp1252Decoder, _, _>(
            &mut |offset, _| &windows_1252_text[offset..],
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (function_item (visibility_modifier) name: (identifier) parameters: (parameters) body: (block (expression_statement (macro_invocation macro: (identifier) (token_tree (string_literal (string_content))))))))"
    );
}

#[test]
fn test_decode_macintosh() {
    use encoding_rs::MACINTOSH;

    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let macintosh_text = MACINTOSH.encode("pub fn foo() { println!(\"€50\"); }").0;

    struct MacintoshDecoder;

    impl Decode for MacintoshDecoder {
        fn decode(bytes: &[u8]) -> (i32, u32) {
            if !bytes.is_empty() {
                let byte = bytes[0];
                (byte as i32, 1)
            } else {
                (0, 0)
            }
        }
    }

    let tree = parser
        .parse_custom_encoding::<MacintoshDecoder, _, _>(
            &mut |offset, _| &macintosh_text[offset..],
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (function_item (visibility_modifier) name: (identifier) parameters: (parameters) body: (block (expression_statement (macro_invocation macro: (identifier) (token_tree (string_literal (string_content))))))))"
    );
}

#[test]
fn test_decode_utf24le() {
    let mut parser = Parser::new();
    parser.set_language(&get_language("rust")).unwrap();

    let mut utf24le_text = Vec::new();
    for c in "pub fn foo() { println!(\"€50\"); }".chars() {
        let code_point = c as u32;
        utf24le_text.push((code_point & 0xFF) as u8);
        utf24le_text.push(((code_point >> 8) & 0xFF) as u8);
        utf24le_text.push(((code_point >> 16) & 0xFF) as u8);
    }

    struct Utf24LeDecoder;

    impl Decode for Utf24LeDecoder {
        fn decode(bytes: &[u8]) -> (i32, u32) {
            if bytes.len() >= 3 {
                (i32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]), 3)
            } else {
                (0, 0)
            }
        }
    }

    let tree = parser
        .parse_custom_encoding::<Utf24LeDecoder, _, _>(
            &mut |offset, _| &utf24le_text[offset..],
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (function_item (visibility_modifier) name: (identifier) parameters: (parameters) body: (block (expression_statement (macro_invocation macro: (identifier) (token_tree (string_literal (string_content))))))))"
    );
}

#[test]
fn test_grammars_that_should_not_compile() {
    assert!(generate_parser(
        r#"
        {
            "name": "issue_1111",
            "rules": {
                "source_file": { "type": "STRING", "value": "" }
            },
        }
        "#
    )
    .is_err());

    assert!(generate_parser(
        r#"
        {
            "name": "issue_1271",
            "rules": {
                "source_file": { "type": "SYMBOL", "name": "identifier" },
                "identifier": {
                    "type": "TOKEN",
                    "content": {
                        "type": "REPEAT",
                        "content": { "type": "PATTERN", "value": "a" }
                    }
                }
            },
        }
        "#
    )
    .is_err());

    assert!(generate_parser(
        r#"
        {
            "name": "issue_1156_expl_1",
            "rules": {
                "source_file": {
                    "type": "TOKEN",
                    "content": {
                        "type": "REPEAT",
                        "content": { "type": "STRING", "value": "c" }
                    }
                }
            },
        }
        "#
    )
    .is_err());

    assert!(generate_parser(
        r#"
        {
            "name": "issue_1156_expl_2",
            "rules": {
                "source_file": {
                    "type": "TOKEN",
                    "content": {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": "e" },
                            { "type": "BLANK" }
                        ]
                    }
                }
            },
        }
        "#
    )
    .is_err());

    assert!(generate_parser(
        r#"
        {
            "name": "issue_1156_expl_3",
            "rules": {
                "source_file": {
                    "type": "IMMEDIATE_TOKEN",
                    "content": {
                        "type": "REPEAT",
                        "content": { "type": "STRING", "value": "p" }
                    }
                }
            },
        }
        "#
    )
    .is_err());

    assert!(generate_parser(
        r#"
        {
            "name": "issue_1156_expl_4",
            "rules": {
                "source_file": {
                    "type": "IMMEDIATE_TOKEN",
                    "content": {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": "r" },
                            { "type": "BLANK" }
                        ]
                    }
                }
            },
        }
        "#
    )
    .is_err());
}

const fn simple_range(start: usize, end: usize) -> Range {
    Range {
        start_byte: start,
        end_byte: end,
        start_point: Point::new(0, start),
        end_point: Point::new(0, end),
    }
}

fn chunked_input<'a>(text: &'a str, size: usize) -> impl FnMut(usize, Point) -> &'a [u8] {
    move |offset, _| &text.as_bytes()[offset..text.len().min(offset + size)]
}
