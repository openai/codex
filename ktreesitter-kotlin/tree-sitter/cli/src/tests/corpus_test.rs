use std::{collections::HashMap, env, fs};

use tree_sitter::Parser;
use tree_sitter_proc_macro::test_with_seed;

use crate::{
    fuzz::{
        corpus_test::{
            check_changed_ranges, check_consistent_sizes, get_parser, set_included_ranges,
        },
        edits::{get_random_edit, invert_edit},
        flatten_tests, new_seed,
        random::Rand,
        EDIT_COUNT, EXAMPLE_EXCLUDE, EXAMPLE_INCLUDE, ITERATION_COUNT, LANGUAGE_FILTER,
        LOG_GRAPH_ENABLED, START_SEED,
    },
    parse::perform_edit,
    test::{parse_tests, print_diff, print_diff_key, strip_sexp_fields},
    tests::{
        allocations,
        helpers::fixtures::{fixtures_dir, get_language, get_test_language, SCRATCH_BASE_DIR},
    },
};

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_bash_language(seed: usize) {
    test_language_corpus(
        "bash",
        seed,
        Some(&[
            // Fragile tests where edit customization changes
            // lead to significant parse tree structure changes.
            "bash - corpus - commands - Nested Heredocs",
            "bash - corpus - commands - Quoted Heredocs",
            "bash - corpus - commands - Heredocs with weird characters",
        ]),
        None,
    );
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_c_language(seed: usize) {
    test_language_corpus("c", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_cpp_language(seed: usize) {
    test_language_corpus("cpp", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_embedded_template_language(seed: usize) {
    test_language_corpus("embedded-template", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_go_language(seed: usize) {
    test_language_corpus("go", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_html_language(seed: usize) {
    test_language_corpus("html", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_java_language(seed: usize) {
    test_language_corpus(
        "java",
        seed,
        Some(&["java - corpus - expressions - switch with unnamed pattern variable"]),
        None,
    );
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_javascript_language(seed: usize) {
    test_language_corpus("javascript", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_json_language(seed: usize) {
    test_language_corpus("json", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_php_language(seed: usize) {
    test_language_corpus("php", seed, None, Some("php"));
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_python_language(seed: usize) {
    test_language_corpus("python", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_ruby_language(seed: usize) {
    test_language_corpus("ruby", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_rust_language(seed: usize) {
    test_language_corpus("rust", seed, None, None);
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_typescript_language(seed: usize) {
    test_language_corpus("typescript", seed, None, Some("typescript"));
}

#[test_with_seed(retry=10, seed=*START_SEED, seed_fn=new_seed)]
fn test_corpus_for_tsx_language(seed: usize) {
    test_language_corpus("typescript", seed, None, Some("tsx"));
}

pub fn test_language_corpus(
    language_name: &str,
    start_seed: usize,
    skipped: Option<&[&str]>,
    language_dir: Option<&str>,
) {
    if let Some(filter) = LANGUAGE_FILTER.as_ref() {
        if language_name != filter {
            return;
        }
    }

    let language_dir = language_dir.unwrap_or_default();

    let grammars_dir = fixtures_dir().join("grammars");
    let error_corpus_dir = fixtures_dir().join("error_corpus");
    let template_corpus_dir = fixtures_dir().join("template_corpus");
    let corpus_dir = grammars_dir.join(language_name).join("test").join("corpus");

    println!("Testing {language_name} corpus @ {}", corpus_dir.display());

    let error_corpus_file = error_corpus_dir.join(format!("{language_name}_errors.txt"));
    let template_corpus_file = template_corpus_dir.join(format!("{language_name}_templates.txt"));
    let main_tests = parse_tests(&corpus_dir).unwrap();
    let error_tests = parse_tests(&error_corpus_file).unwrap_or_default();
    let template_tests = parse_tests(&template_corpus_file).unwrap_or_default();
    let mut tests = flatten_tests(
        main_tests,
        EXAMPLE_INCLUDE.as_ref(),
        EXAMPLE_EXCLUDE.as_ref(),
    );
    tests.extend(flatten_tests(
        error_tests,
        EXAMPLE_INCLUDE.as_ref(),
        EXAMPLE_EXCLUDE.as_ref(),
    ));
    tests.extend(
        flatten_tests(
            template_tests,
            EXAMPLE_INCLUDE.as_ref(),
            EXAMPLE_EXCLUDE.as_ref(),
        )
        .into_iter()
        .map(|mut t| {
            t.template_delimiters = Some(("<%", "%>"));
            t
        }),
    );

    tests.retain(|t| t.languages[0].is_empty() || t.languages.contains(&Box::from(language_dir)));

    let mut skipped = skipped.map(|x| x.iter().map(|x| (*x, 0)).collect::<HashMap<&str, usize>>());

    let language_path = if language_dir.is_empty() {
        language_name.to_string()
    } else {
        format!("{language_name}/{language_dir}")
    };
    let language = get_language(&language_path);
    let mut failure_count = 0;

    let log_seed = env::var("TREE_SITTER_LOG_SEED").is_ok();
    let dump_edits = env::var("TREE_SITTER_DUMP_EDITS").is_ok();

    if log_seed {
        println!("  start seed: {start_seed}");
    }

    println!();
    for (test_index, test) in tests.iter().enumerate() {
        let test_name = format!("{language_name} - {}", test.name);
        if let Some(skipped) = skipped.as_mut() {
            if let Some(counter) = skipped.get_mut(test_name.as_str()) {
                println!("  {test_index}. {test_name} - SKIPPED");
                *counter += 1;
                continue;
            }
        }

        println!("  {test_index}. {test_name}");

        let passed = allocations::record(|| {
            let mut log_session = None;
            let mut parser = get_parser(&mut log_session, "log.html");
            parser.set_language(&language).unwrap();
            set_included_ranges(&mut parser, &test.input, test.template_delimiters);

            let tree = parser.parse(&test.input, None).unwrap();
            let mut actual_output = tree.root_node().to_sexp();
            if !test.has_fields {
                actual_output = strip_sexp_fields(&actual_output);
            }

            if actual_output != test.output {
                println!("Incorrect initial parse for {test_name}");
                print_diff_key();
                print_diff(&actual_output, &test.output, true);
                println!();
                return false;
            }

            true
        })
        .unwrap();

        if !passed {
            failure_count += 1;
            continue;
        }

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&test.input, None).unwrap();
        drop(parser);

        for trial in 0..*ITERATION_COUNT {
            let seed = start_seed + trial;
            let passed = allocations::record(|| {
                let mut rand = Rand::new(seed);
                let mut log_session = None;
                let mut parser = get_parser(&mut log_session, "log.html");
                parser.set_language(&language).unwrap();
                let mut tree = tree.clone();
                let mut input = test.input.clone();

                if *LOG_GRAPH_ENABLED {
                    eprintln!("{}\n", String::from_utf8_lossy(&input));
                }

                // Perform a random series of edits and reparse.
                let edit_count = rand.unsigned(*EDIT_COUNT);
                let mut undo_stack = Vec::with_capacity(edit_count);
                for _ in 0..=edit_count {
                    let edit = get_random_edit(&mut rand, &input);
                    undo_stack.push(invert_edit(&input, &edit));
                    perform_edit(&mut tree, &mut input, &edit).unwrap();
                }

                if log_seed {
                    println!("   {test_index}.{trial:<2} seed: {seed}");
                }

                if dump_edits {
                    fs::write(
                        SCRATCH_BASE_DIR
                            .join(format!("edit.{seed}.{test_index}.{trial} {test_name}")),
                        &input,
                    )
                    .unwrap();
                }

                if *LOG_GRAPH_ENABLED {
                    eprintln!("{}\n", String::from_utf8_lossy(&input));
                }

                set_included_ranges(&mut parser, &input, test.template_delimiters);
                let mut tree2 = parser.parse(&input, Some(&tree)).unwrap();

                // Check that the new tree is consistent.
                check_consistent_sizes(&tree2, &input);
                if let Err(message) = check_changed_ranges(&tree, &tree2, &input) {
                    println!("\nUnexpected scope change in seed {seed} with start seed {start_seed}\n{message}\n\n",);
                    return false;
                }

                // Undo all of the edits and re-parse again.
                while let Some(edit) = undo_stack.pop() {
                    perform_edit(&mut tree2, &mut input, &edit).unwrap();
                }
                if *LOG_GRAPH_ENABLED {
                    eprintln!("{}\n", String::from_utf8_lossy(&input));
                }

                set_included_ranges(&mut parser, &test.input, test.template_delimiters);
                let tree3 = parser.parse(&input, Some(&tree2)).unwrap();

                // Verify that the final tree matches the expectation from the corpus.
                let mut actual_output = tree3.root_node().to_sexp();
                if !test.has_fields {
                    actual_output = strip_sexp_fields(&actual_output);
                }

                if actual_output != test.output {
                    println!("Incorrect parse for {test_name} - seed {seed}");
                    print_diff_key();
                    print_diff(&actual_output, &test.output, true);
                    println!();
                    return false;
                }

                // Check that the edited tree is consistent.
                check_consistent_sizes(&tree3, &input);
                if let Err(message) = check_changed_ranges(&tree2, &tree3, &input) {
                    println!("Unexpected scope change in seed {seed} with start seed {start_seed}\n{message}\n\n");
                    return false;
                }

                true
            }).unwrap();

            if !passed {
                failure_count += 1;
                break;
            }
        }
    }

    assert!(
        failure_count == 0,
        "{failure_count} {language_name} corpus tests failed"
    );

    if let Some(skipped) = skipped.as_mut() {
        skipped.retain(|_, v| *v == 0);

        if !skipped.is_empty() {
            println!("Non matchable skip definitions:");
            for k in skipped.keys() {
                println!("  {k}");
            }
            panic!("Non matchable skip definitions needs to be removed");
        }
    }
}

#[test]
fn test_feature_corpus_files() {
    let test_grammars_dir = fixtures_dir().join("test_grammars");

    let mut failure_count = 0;
    for entry in fs::read_dir(test_grammars_dir).unwrap() {
        let entry = entry.unwrap();
        if !entry.metadata().unwrap().is_dir() {
            continue;
        }
        let language_name = entry.file_name();
        let language_name = language_name.to_str().unwrap();

        if let Some(filter) = LANGUAGE_FILTER.as_ref() {
            if language_name != filter {
                continue;
            }
        }

        let test_path = entry.path();
        let mut grammar_path = test_path.join("grammar.js");
        if !grammar_path.exists() {
            grammar_path = test_path.join("grammar.json");
        }
        let error_message_path = test_path.join("expected_error.txt");
        let grammar_json = tree_sitter_generate::load_grammar_file(&grammar_path, None).unwrap();
        let generate_result =
            tree_sitter_generate::generate_parser_for_grammar(&grammar_json, Some((0, 0, 0)));

        if error_message_path.exists() {
            if EXAMPLE_INCLUDE.is_some() || EXAMPLE_EXCLUDE.is_some() {
                continue;
            }

            eprintln!("test language: {language_name:?}");

            let expected_message = fs::read_to_string(&error_message_path)
                .unwrap()
                .replace("\r\n", "\n");
            if let Err(e) = generate_result {
                let actual_message = e.to_string().replace("\r\n", "\n");
                if expected_message != actual_message {
                    eprintln!(
                        "Unexpected error message.\n\nExpected:\n\n{expected_message}\nActual:\n\n{actual_message}\n",
                    );
                    failure_count += 1;
                }
            } else {
                eprintln!("Expected error message but got none for test grammar '{language_name}'",);
                failure_count += 1;
            }
        } else {
            if let Err(e) = &generate_result {
                eprintln!("Unexpected error for test grammar '{language_name}':\n{e}",);
                failure_count += 1;
                continue;
            }

            let corpus_path = test_path.join("corpus.txt");
            let c_code = generate_result.unwrap().1;
            let language = get_test_language(language_name, &c_code, Some(&test_path));
            let test = parse_tests(&corpus_path).unwrap();
            let tests = flatten_tests(test, EXAMPLE_INCLUDE.as_ref(), EXAMPLE_EXCLUDE.as_ref());

            if !tests.is_empty() {
                eprintln!("test language: {language_name:?}");
            }

            for test in tests {
                eprintln!("  example: {:?}", test.name);

                let passed = allocations::record(|| {
                    let mut log_session = None;
                    let mut parser = get_parser(&mut log_session, "log.html");
                    parser.set_language(&language).unwrap();
                    let tree = parser.parse(&test.input, None).unwrap();
                    let mut actual_output = tree.root_node().to_sexp();
                    if !test.has_fields {
                        actual_output = strip_sexp_fields(&actual_output);
                    }
                    if actual_output == test.output {
                        true
                    } else {
                        print_diff_key();
                        print_diff(&actual_output, &test.output, true);
                        println!();
                        false
                    }
                })
                .unwrap();

                if !passed {
                    failure_count += 1;
                }
            }
        }
    }

    assert!(failure_count == 0, "{failure_count} corpus tests failed");
}
