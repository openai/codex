// For some reasons `Command::spawn` doesn't work in CI env for many exotic arches.
#![cfg(all(any(target_arch = "x86_64", target_arch = "x86"), not(sanitizing)))]

use std::{
    env::VarError,
    process::{Command, Stdio},
};

use tree_sitter::Parser;
use tree_sitter_generate::load_grammar_file;

use super::generate_parser;
use crate::tests::helpers::fixtures::{fixtures_dir, get_test_language};

// The `sanitizing` cfg is required to don't run tests under specific sunitizer
// because they don't work well with subprocesses _(it's an assumption)_.
//
// Below are two alternative examples of how to disable tests for some arches
// if a way with excluding the whole mod from compilation wouldn't work well.
//
// XXX: Also may be it makes sense to keep such tests as ignored by default
//      to omit surprises and enable them on CI by passing an extra option explicitly:
//
//        > cargo test -- --include-ignored
//
// #[cfg(all(any(target_arch = "x86_64", target_arch = "x86"), not(sanitizing)))]
// #[cfg_attr(not(all(any(target_arch = "x86_64", target_arch = "x86"), not(sanitizing))), ignore)]
//
#[test]
fn test_grammar_that_should_hang_and_not_segfault() {
    let parent_sleep_millis = 1000;
    let test_name = "test_grammar_that_should_hang_and_not_segfault";
    let test_var = "CARGO_HANG_TEST";

    eprintln!("  {test_name}");

    let tests_exec_path = std::env::args()
        .next()
        .expect("Failed to get tests executable path");

    match std::env::var(test_var) {
        Ok(v) if v == test_name => {
            eprintln!("    child process id {}", std::process::id());
            hang_test();
        }

        Err(VarError::NotPresent) => {
            eprintln!("    parent process id {}", std::process::id());
            let mut command = Command::new(tests_exec_path);
            command.arg(test_name).env(test_var, test_name);

            if std::env::args().any(|x| x == "--nocapture") {
                command.arg("--nocapture");
            } else {
                command.stdout(Stdio::null()).stderr(Stdio::null());
            }

            match command.spawn() {
                Ok(mut child) => {
                    std::thread::sleep(std::time::Duration::from_millis(parent_sleep_millis));
                    match child.try_wait() {
                        Ok(Some(status)) if status.success() => {
                            panic!("Child didn't hang and exited successfully")
                        }
                        Ok(Some(status)) => panic!(
                            "Child didn't hang and exited with status code: {:?}",
                            status.code()
                        ),
                        _ => (),
                    }
                    if let Err(e) = child.kill() {
                        eprintln!(
                            "Failed to kill hang test's process id: {}, error: {e}",
                            child.id()
                        );
                    }
                }
                Err(e) => panic!("{e}"),
            }
        }

        Err(e) => panic!("Env var error: {e}"),

        _ => unreachable!(),
    }
}

fn hang_test() {
    let test_grammar_dir = fixtures_dir()
        .join("test_grammars")
        .join("get_col_should_hang_not_crash");

    let grammar_json = load_grammar_file(&test_grammar_dir.join("grammar.js"), None).unwrap();
    let (parser_name, parser_code) = generate_parser(grammar_json.as_str()).unwrap();

    let language = get_test_language(&parser_name, &parser_code, Some(test_grammar_dir.as_path()));

    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();

    let code_that_should_hang = "\nHello";

    parser.parse(code_that_should_hang, None).unwrap();
}
