use std::{collections::HashMap, env, fs, path::Path, sync::LazyLock};

use rand::Rng;
use regex::Regex;
use tree_sitter::{Language, Parser};

pub mod allocations;
pub mod corpus_test;
pub mod edits;
pub mod random;
pub mod scope_sequence;

use crate::{
    fuzz::{
        corpus_test::{
            check_changed_ranges, check_consistent_sizes, get_parser, set_included_ranges,
        },
        edits::{get_random_edit, invert_edit},
        random::Rand,
    },
    parse::perform_edit,
    test::{parse_tests, print_diff, print_diff_key, strip_sexp_fields, TestEntry},
};

pub static LOG_ENABLED: LazyLock<bool> = LazyLock::new(|| env::var("TREE_SITTER_LOG").is_ok());

pub static LOG_GRAPH_ENABLED: LazyLock<bool> =
    LazyLock::new(|| env::var("TREE_SITTER_LOG_GRAPHS").is_ok());

pub static LANGUAGE_FILTER: LazyLock<Option<String>> =
    LazyLock::new(|| env::var("TREE_SITTER_LANGUAGE").ok());

pub static EXAMPLE_INCLUDE: LazyLock<Option<Regex>> =
    LazyLock::new(|| regex_env_var("TREE_SITTER_EXAMPLE_INCLUDE"));

pub static EXAMPLE_EXCLUDE: LazyLock<Option<Regex>> =
    LazyLock::new(|| regex_env_var("TREE_SITTER_EXAMPLE_EXCLUDE"));

pub static START_SEED: LazyLock<usize> = LazyLock::new(new_seed);

pub static EDIT_COUNT: LazyLock<usize> =
    LazyLock::new(|| int_env_var("TREE_SITTER_EDITS").unwrap_or(3));

pub static ITERATION_COUNT: LazyLock<usize> =
    LazyLock::new(|| int_env_var("TREE_SITTER_ITERATIONS").unwrap_or(10));

fn int_env_var(name: &'static str) -> Option<usize> {
    env::var(name).ok().and_then(|e| e.parse().ok())
}

fn regex_env_var(name: &'static str) -> Option<Regex> {
    env::var(name).ok().and_then(|e| Regex::new(&e).ok())
}

#[must_use]
pub fn new_seed() -> usize {
    int_env_var("TREE_SITTER_SEED").unwrap_or_else(|| {
        let mut rng = rand::thread_rng();
        let seed = rng.gen::<usize>();
        eprintln!("Seed: {seed}");
        seed
    })
}

pub struct FuzzOptions {
    pub skipped: Option<Vec<String>>,
    pub subdir: Option<String>,
    pub edits: usize,
    pub iterations: usize,
    pub include: Option<Regex>,
    pub exclude: Option<Regex>,
    pub log_graphs: bool,
    pub log: bool,
}

pub fn fuzz_language_corpus(
    language: &Language,
    language_name: &str,
    start_seed: usize,
    grammar_dir: &Path,
    options: &mut FuzzOptions,
) {
    fn retain(entry: &mut TestEntry, language_name: &str) -> bool {
        match entry {
            TestEntry::Example { attributes, .. } => {
                attributes.languages[0].is_empty()
                    || attributes
                        .languages
                        .iter()
                        .any(|lang| lang.as_ref() == language_name)
            }
            TestEntry::Group {
                ref mut children, ..
            } => {
                children.retain_mut(|child| retain(child, language_name));
                !children.is_empty()
            }
        }
    }

    let subdir = options.subdir.take().unwrap_or_default();

    let corpus_dir = grammar_dir.join(subdir).join("test").join("corpus");

    if !corpus_dir.exists() || !corpus_dir.is_dir() {
        eprintln!("No corpus directory found, ensure that you have a `test/corpus` directory in your grammar directory with at least one test file.");
        return;
    }

    if std::fs::read_dir(&corpus_dir).unwrap().count() == 0 {
        eprintln!("No corpus files found in `test/corpus`, ensure that you have at least one test file in your corpus directory.");
        return;
    }

    let mut main_tests = parse_tests(&corpus_dir).unwrap();
    match main_tests {
        TestEntry::Group {
            ref mut children, ..
        } => {
            children.retain_mut(|child| retain(child, language_name));
        }
        TestEntry::Example { .. } => unreachable!(),
    }
    let tests = flatten_tests(
        main_tests,
        options.include.as_ref(),
        options.exclude.as_ref(),
    );

    let get_test_name = |test: &FlattenedTest| format!("{language_name} - {}", test.name);

    let mut skipped = options
        .skipped
        .take()
        .unwrap_or_default()
        .into_iter()
        .chain(tests.iter().filter(|x| x.skip).map(get_test_name))
        .map(|x| (x, 0))
        .collect::<HashMap<String, usize>>();

    let mut failure_count = 0;

    let log_seed = env::var("TREE_SITTER_LOG_SEED").is_ok();
    let dump_edits = env::var("TREE_SITTER_DUMP_EDITS").is_ok();

    if log_seed {
        println!("  start seed: {start_seed}");
    }

    println!();
    for (test_index, test) in tests.iter().enumerate() {
        let test_name = get_test_name(test);
        if let Some(counter) = skipped.get_mut(test_name.as_str()) {
            println!("  {test_index}. {test_name} - SKIPPED");
            *counter += 1;
            continue;
        }

        println!("  {test_index}. {test_name}");

        let passed = allocations::record(|| {
            let mut log_session = None;
            let mut parser = get_parser(&mut log_session, "log.html");
            parser.set_language(language).unwrap();
            set_included_ranges(&mut parser, &test.input, test.template_delimiters);

            let tree = parser.parse(&test.input, None).unwrap();

            if test.error {
                return true;
            }

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
        .unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            false
        });

        if !passed {
            failure_count += 1;
            continue;
        }

        let mut parser = Parser::new();
        parser.set_language(language).unwrap();
        let tree = parser.parse(&test.input, None).unwrap();
        drop(parser);

        for trial in 0..options.iterations {
            let seed = start_seed + trial;
            let passed = allocations::record(|| {
                let mut rand = Rand::new(seed);
                let mut log_session = None;
                let mut parser = get_parser(&mut log_session, "log.html");
                parser.set_language(language).unwrap();
                let mut tree = tree.clone();
                let mut input = test.input.clone();

                if options.log_graphs {
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
                    fs::create_dir_all("fuzz").unwrap();
                    fs::write(
                        Path::new("fuzz")
                            .join(format!("edit.{seed}.{test_index}.{trial} {test_name}")),
                        &input,
                    )
                    .unwrap();
                }

                if options.log_graphs {
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
                if options.log_graphs {
                    eprintln!("{}\n", String::from_utf8_lossy(&input));
                }

                set_included_ranges(&mut parser, &test.input, test.template_delimiters);
                let tree3 = parser.parse(&input, Some(&tree2)).unwrap();

                // Verify that the final tree matches the expectation from the corpus.
                let mut actual_output = tree3.root_node().to_sexp();
                if !test.has_fields {
                    actual_output = strip_sexp_fields(&actual_output);
                }

                if actual_output != test.output && !test.error {
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
            }).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                false
            });

            if !passed {
                failure_count += 1;
                break;
            }
        }
    }

    if failure_count != 0 {
        eprintln!("{failure_count} {language_name} corpus tests failed fuzzing");
    }

    skipped.retain(|_, v| *v == 0);

    if !skipped.is_empty() {
        println!("Non matchable skip definitions:");
        for k in skipped.keys() {
            println!("  {k}");
        }
        panic!("Non matchable skip definitions needs to be removed");
    }
}

pub struct FlattenedTest {
    pub name: String,
    pub input: Vec<u8>,
    pub output: String,
    pub languages: Vec<Box<str>>,
    pub error: bool,
    pub skip: bool,
    pub has_fields: bool,
    pub template_delimiters: Option<(&'static str, &'static str)>,
}

#[must_use]
pub fn flatten_tests(
    test: TestEntry,
    include: Option<&Regex>,
    exclude: Option<&Regex>,
) -> Vec<FlattenedTest> {
    fn helper(
        test: TestEntry,
        include: Option<&Regex>,
        exclude: Option<&Regex>,
        is_root: bool,
        prefix: &str,
        result: &mut Vec<FlattenedTest>,
    ) {
        match test {
            TestEntry::Example {
                mut name,
                input,
                output,
                has_fields,
                attributes,
                ..
            } => {
                if !prefix.is_empty() {
                    name.insert_str(0, " - ");
                    name.insert_str(0, prefix);
                }

                if let Some(include) = include {
                    if !include.is_match(&name) {
                        return;
                    }
                } else if let Some(exclude) = exclude {
                    if exclude.is_match(&name) {
                        return;
                    }
                }

                result.push(FlattenedTest {
                    name,
                    input,
                    output,
                    has_fields,
                    languages: attributes.languages,
                    error: attributes.error,
                    skip: attributes.skip,
                    template_delimiters: None,
                });
            }
            TestEntry::Group {
                mut name, children, ..
            } => {
                if !is_root && !prefix.is_empty() {
                    name.insert_str(0, " - ");
                    name.insert_str(0, prefix);
                }
                for child in children {
                    helper(child, include, exclude, false, &name, result);
                }
            }
        }
    }
    let mut result = Vec::new();
    helper(test, include, exclude, true, "", &mut result);
    result
}
