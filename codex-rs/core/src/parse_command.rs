use crate::bash::try_parse_bash;
use crate::bash::try_parse_word_only_commands_sequence;
use serde::Deserialize;
use serde::Serialize;
use shlex::split as shlex_split;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum ParsedCommand {
    Read {
        cmd: Vec<String>,
        name: String,
    },
    ListFiles {
        cmd: Vec<String>,
        path: Option<String>,
    },
    Search {
        cmd: Vec<String>,
        query: Option<String>,
        path: Option<String>,
    },
    Format {
        cmd: Vec<String>,
        tool: Option<String>,
        targets: Option<Vec<String>>,
    },
    Test {
        cmd: Vec<String>,
    },
    Lint {
        cmd: Vec<String>,
        tool: Option<String>,
        targets: Option<Vec<String>>,
    },
    Unknown {
        cmd: Vec<String>,
    },
}

/// DO NOT REVIEW THIS CODE BY HAND
/// This parsing code is quite complex and not easy to hand-modify.
/// The easiest way to iterate is to add unit tests and have Codex fix the implementation.
/// To encourage this, the tests have been put directly below this function rather than at the bottom of the
///
/// Parses metadata out of an arbitrary command.
/// These commands are model driven and could include just about anything.
/// The parsing is slightly lossy due to the ~infinite expressiveness of an arbitrary command.
/// The goal of the parsed metadata is to be able to provide the user with a human readable gis
/// of what it is doing.
pub fn parse_command(command: &[String]) -> Vec<ParsedCommand> {
    // Parse and then collapse consecutive duplicate commands to avoid redundant summaries.
    let parsed = parse_command_impl(command);
    let mut deduped: Vec<ParsedCommand> = Vec::with_capacity(parsed.len());
    for cmd in parsed.into_iter() {
        if deduped.last().is_some_and(|prev| prev == &cmd) {
            continue;
        }
        deduped.push(cmd);
    }
    deduped
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
/// Tests are at the top to encourage using TDD + Codex to fix the implementation.
mod tests {
    use super::*;

    fn shlex_split_safe(s: &str) -> Vec<String> {
        shlex_split(s).unwrap_or_else(|| s.split_whitespace().map(|s| s.to_string()).collect())
    }

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    fn assert_parsed(args: &[String], expected: Vec<ParsedCommand>) {
        let out = parse_command(args);
        assert_eq!(out, expected);
    }

    #[test]
    fn git_status_is_unknown() {
        assert_parsed(
            &vec_str(&["git", "status"]),
            vec![ParsedCommand::Unknown {
                cmd: vec_str(&["git", "status"]),
            }],
        );
    }

    #[test]
    fn handles_complex_bash_command_head() {
        let inner =
            "rg --version && node -v && pnpm -v && rg --files | wc -l && rg --files | head -n 40";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![
                // Expect commands in left-to-right execution order
                ParsedCommand::Search {
                    cmd: vec_str(&["rg", "--version"]),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: vec_str(&["node", "-v"]),
                },
                ParsedCommand::Unknown {
                    cmd: vec_str(&["pnpm", "-v"]),
                },
                ParsedCommand::Search {
                    cmd: vec_str(&["rg", "--files"]),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: vec_str(&["head", "-n", "40"]),
                },
            ],
        );
    }

    #[test]
    fn supports_searching_for_navigate_to_route() -> anyhow::Result<()> {
        let inner = "rg -n \"navigate-to-route\" -S";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe(inner),
                query: Some("navigate-to-route".to_string()),
                path: None,
            }],
        );
        Ok(())
    }

    #[test]
    fn handles_complex_bash_command() {
        let inner = "rg -n \"BUG|FIXME|TODO|XXX|HACK\" -S | head -n 200";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![
                ParsedCommand::Search {
                    cmd: vec_str(&["rg", "-n", "BUG|FIXME|TODO|XXX|HACK", "-S"]),
                    query: Some("BUG|FIXME|TODO|XXX|HACK".to_string()),
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: vec_str(&["head", "-n", "200"]),
                },
            ],
        );
    }

    #[test]
    fn supports_rg_files_with_path_and_pipe() {
        let inner = "rg --files webview/src | sed -n";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Search {
                cmd: vec_str(&["rg", "--files", "webview/src"]),
                query: None,
                path: Some("webview".to_string()),
            }],
        );
    }

    #[test]
    fn supports_rg_files_then_head() {
        let inner = "rg --files | head -n 50";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![
                ParsedCommand::Search {
                    cmd: vec_str(&["rg", "--files"]),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: vec_str(&["head", "-n", "50"]),
                },
            ],
        );
    }

    #[test]
    fn supports_cat() {
        let inner = "cat webview/README.md";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(inner),
                name: "README.md".to_string(),
            }],
        );
    }

    #[test]
    fn supports_ls_with_pipe() {
        let inner = "ls -la | sed -n '1,120p'";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::ListFiles {
                cmd: vec_str(&["ls", "-la"]),
                path: None,
            }],
        );
    }

    #[test]
    fn supports_head_n() {
        let inner = "head -n 50 Cargo.toml";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(inner),
                name: "Cargo.toml".to_string(),
            }],
        );
    }

    #[test]
    fn supports_cat_sed_n() {
        let inner = "cat tui/Cargo.toml | sed -n '1,200p'";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(inner),
                name: "Cargo.toml".to_string(),
            }],
        );
    }

    #[test]
    fn supports_tail_n_plus() {
        let inner = "tail -n +522 README.md";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(inner),
                name: "README.md".to_string(),
            }],
        );
    }

    #[test]
    fn supports_tail_n_last_lines() {
        let inner = "tail -n 30 README.md";
        let out = parse_command(&vec_str(&["bash", "-lc", inner]));
        assert_eq!(
            out,
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(inner),
                name: "README.md".to_string(),
            }]
        );
    }

    #[test]
    fn supports_npm_run_build_is_unknown() {
        assert_parsed(
            &vec_str(&["npm", "run", "build"]),
            vec![ParsedCommand::Unknown {
                cmd: vec_str(&["npm", "run", "build"]),
            }],
        );
    }

    #[test]
    fn supports_npm_run_with_forwarded_args() {
        assert_parsed(
            &vec_str(&[
                "npm",
                "run",
                "lint",
                "--",
                "--max-warnings",
                "0",
                "--format",
                "json",
            ]),
            vec![ParsedCommand::Lint {
                cmd: vec_str(&[
                    "npm",
                    "run",
                    "lint",
                    "--",
                    "--max-warnings",
                    "0",
                    "--format",
                    "json",
                ]),
                tool: Some("npm-script:lint".to_string()),
                targets: None,
            }],
        );
    }

    #[test]
    fn supports_grep_recursive_current_dir() {
        assert_parsed(
            &vec_str(&["grep", "-R", "CODEX_SANDBOX_ENV_VAR", "-n", "."]),
            vec![ParsedCommand::Search {
                cmd: vec_str(&["grep", "-R", "CODEX_SANDBOX_ENV_VAR", "-n", "."]),
                query: Some("CODEX_SANDBOX_ENV_VAR".to_string()),
                path: Some(".".to_string()),
            }],
        );
    }

    #[test]
    fn supports_grep_recursive_specific_file() {
        assert_parsed(
            &vec_str(&[
                "grep",
                "-R",
                "CODEX_SANDBOX_ENV_VAR",
                "-n",
                "core/src/spawn.rs",
            ]),
            vec![ParsedCommand::Search {
                cmd: vec_str(&[
                    "grep",
                    "-R",
                    "CODEX_SANDBOX_ENV_VAR",
                    "-n",
                    "core/src/spawn.rs",
                ]),
                query: Some("CODEX_SANDBOX_ENV_VAR".to_string()),
                path: Some("spawn.rs".to_string()),
            }],
        );
    }

    #[test]
    fn supports_grep_query_with_slashes_not_shortened() {
        // Query strings may contain slashes and should not be shortened to the basename.
        // Previously, grep queries were passed through short_display_path, which is incorrect.
        assert_parsed(
            &shlex_split_safe("grep -R src/main.rs -n ."),
            vec![ParsedCommand::Search {
                cmd: vec_str(&["grep", "-R", "src/main.rs", "-n", "."]),
                query: Some("src/main.rs".to_string()),
                path: Some(".".to_string()),
            }],
        );
    }

    #[test]
    fn supports_grep_weird_backtick_in_query() {
        assert_parsed(
            &shlex_split_safe("grep -R COD`EX_SANDBOX -n"),
            vec![ParsedCommand::Search {
                cmd: vec_str(&["grep", "-R", "COD`EX_SANDBOX", "-n"]),
                query: Some("COD`EX_SANDBOX".to_string()),
                path: None,
            }],
        );
    }

    #[test]
    fn supports_cd_and_rg_files() {
        assert_parsed(
            &shlex_split_safe("cd codex-rs && rg --files"),
            vec![
                ParsedCommand::Unknown {
                    cmd: vec_str(&["cd", "codex-rs"]),
                },
                ParsedCommand::Search {
                    cmd: vec_str(&["rg", "--files"]),
                    query: None,
                    path: None,
                },
            ],
        );
    }

    #[test]
    fn echo_then_cargo_test_sequence() {
        assert_parsed(
            &shlex_split_safe("echo Running tests... && cargo test --all-features --quiet"),
            vec![ParsedCommand::Test {
                cmd: vec_str(&["cargo", "test", "--all-features", "--quiet"]),
            }],
        );
    }

    #[test]
    fn supports_cargo_fmt_and_test_with_config() {
        assert_parsed(
            &shlex_split_safe(
                "cargo fmt -- --config imports_granularity=Item && cargo test -p core --all-features",
            ),
            vec![
                ParsedCommand::Format {
                    cmd: shlex_split_safe("cargo fmt -- --config imports_granularity=Item"),
                    tool: Some("cargo fmt".to_string()),
                    targets: None,
                },
                ParsedCommand::Test {
                    cmd: vec_str(&["cargo", "test", "-p", "core", "--all-features"]),
                },
            ],
        );
    }

    #[test]
    fn recognizes_rustfmt_and_clippy() {
        assert_parsed(
            &shlex_split_safe("rustfmt src/main.rs"),
            vec![ParsedCommand::Format {
                cmd: vec_str(&["rustfmt", "src/main.rs"]),
                tool: Some("rustfmt".to_string()),
                targets: Some(vec!["src/main.rs".to_string()]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("cargo clippy -p core --all-features -- -D warnings"),
            vec![ParsedCommand::Lint {
                cmd: vec_str(&[
                    "cargo",
                    "clippy",
                    "-p",
                    "core",
                    "--all-features",
                    "--",
                    "-D",
                    "warnings",
                ]),
                tool: Some("cargo clippy".to_string()),
                targets: None,
            }],
        );
    }

    #[test]
    fn recognizes_pytest_go_and_tools() {
        assert_parsed(
            &shlex_split_safe(
                "pytest -k 'Login and not slow' tests/test_login.py::TestLogin::test_ok",
            ),
            vec![ParsedCommand::Test {
                cmd: vec_str(&[
                    "pytest",
                    "-k",
                    "Login and not slow",
                    "tests/test_login.py::TestLogin::test_ok",
                ]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("go fmt ./..."),
            vec![ParsedCommand::Format {
                cmd: vec_str(&["go", "fmt", "./..."]),
                tool: Some("go fmt".to_string()),
                targets: Some(vec!["./...".to_string()]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("go test ./pkg -run TestThing"),
            vec![ParsedCommand::Test {
                cmd: vec_str(&["go", "test", "./pkg", "-run", "TestThing"]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("eslint . --max-warnings 0"),
            vec![ParsedCommand::Lint {
                cmd: vec_str(&["eslint", ".", "--max-warnings", "0"]),
                tool: Some("eslint".to_string()),
                targets: Some(vec![".".to_string()]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("prettier -w ."),
            vec![ParsedCommand::Format {
                cmd: vec_str(&["prettier", "-w", "."]),
                tool: Some("prettier".to_string()),
                targets: Some(vec![".".to_string()]),
            }],
        );
    }

    #[test]
    fn recognizes_jest_and_vitest_filters() {
        assert_parsed(
            &shlex_split_safe("jest -t 'should work' src/foo.test.ts"),
            vec![ParsedCommand::Test {
                cmd: vec_str(&["jest", "-t", "should work", "src/foo.test.ts"]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("vitest -t 'runs' src/foo.test.tsx"),
            vec![ParsedCommand::Test {
                cmd: vec_str(&["vitest", "-t", "runs", "src/foo.test.tsx"]),
            }],
        );
    }

    #[test]
    fn recognizes_npx_and_scripts() {
        assert_parsed(
            &shlex_split_safe("npx eslint src"),
            vec![ParsedCommand::Lint {
                cmd: vec_str(&["npx", "eslint", "src"]),
                tool: Some("eslint".to_string()),
                targets: Some(vec!["src".to_string()]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("npx prettier -c ."),
            vec![ParsedCommand::Format {
                cmd: vec_str(&["npx", "prettier", "-c", "."]),
                tool: Some("prettier".to_string()),
                targets: Some(vec![".".to_string()]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("pnpm run lint -- --max-warnings 0"),
            vec![ParsedCommand::Lint {
                cmd: vec_str(&["pnpm", "run", "lint", "--", "--max-warnings", "0"]),
                tool: Some("pnpm-script:lint".to_string()),
                targets: None,
            }],
        );

        assert_parsed(
            &shlex_split_safe("npm test"),
            vec![ParsedCommand::Test {
                cmd: vec_str(&["npm", "test"]),
            }],
        );

        assert_parsed(
            &shlex_split_safe("yarn test"),
            vec![ParsedCommand::Test {
                cmd: vec_str(&["yarn", "test"]),
            }],
        );
    }

    // ---- is_small_formatting_command unit tests ----
    #[test]
    fn small_formatting_always_true_commands() {
        for cmd in [
            "wc", "tr", "cut", "sort", "uniq", "xargs", "tee", "column", "awk",
        ] {
            assert!(is_small_formatting_command(&shlex_split_safe(cmd)));
            assert!(is_small_formatting_command(&shlex_split_safe(&format!(
                "{cmd} -x"
            ))));
        }
    }

    #[test]
    fn head_behavior() {
        // No args -> small formatting
        assert!(is_small_formatting_command(&vec_str(&["head"])));
        // Numeric count only -> not considered small formatting by implementation
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "head -n 40"
        )));
        // With explicit file -> not small formatting
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "head -n 40 file.txt"
        )));
        // File only (no count) -> treated as small formatting by implementation
        assert!(is_small_formatting_command(&vec_str(&["head", "file.txt"])));
    }

    #[test]
    fn tail_behavior() {
        // No args -> small formatting
        assert!(is_small_formatting_command(&vec_str(&["tail"])));
        // Numeric with plus offset -> not small formatting
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "tail -n +10"
        )));
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "tail -n +10 file.txt"
        )));
        // Numeric count
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "tail -n 30"
        )));
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "tail -n 30 file.txt"
        )));
        // File only -> small formatting by implementation
        assert!(is_small_formatting_command(&vec_str(&["tail", "file.txt"])));
    }

    #[test]
    fn sed_behavior() {
        // Plain sed -> small formatting
        assert!(is_small_formatting_command(&vec_str(&["sed"])));
        // sed -n <range> (no file) -> still small formatting
        assert!(is_small_formatting_command(&vec_str(&["sed", "-n", "10p"])));
        // Valid range with file -> not small formatting
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "sed -n 10p file.txt"
        )));
        assert!(!is_small_formatting_command(&shlex_split_safe(
            "sed -n 1,200p file.txt"
        )));
        // Invalid ranges with file -> small formatting
        assert!(is_small_formatting_command(&shlex_split_safe(
            "sed -n p file.txt"
        )));
        assert!(is_small_formatting_command(&shlex_split_safe(
            "sed -n +10p file.txt"
        )));
    }

    #[test]
    fn empty_tokens_is_not_small() {
        let empty: Vec<String> = Vec::new();
        assert!(!is_small_formatting_command(&empty));
    }

    #[test]
    fn supports_nl_then_sed_reading() {
        let inner = "nl -ba core/src/parse_command.rs | sed -n '1200,1720p'";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(inner),
                name: "parse_command.rs".to_string(),
            }],
        );
    }

    #[test]
    fn supports_sed_n() {
        let inner = "sed -n '2000,2200p' tui/src/history_cell.rs";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(inner),
                name: "history_cell.rs".to_string(),
            }],
        );
    }

    #[test]
    fn filters_out_printf() {
        let inner =
            r#"printf "\n===== ansi-escape/Cargo.toml =====\n"; cat -- ansi-escape/Cargo.toml"#;
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe("cat -- ansi-escape/Cargo.toml"),
                name: "Cargo.toml".to_string(),
            }],
        );
    }

    #[test]
    fn drops_yes_in_pipelines() {
        // Inside bash -lc, `yes | rg --files` should focus on the primary command.
        let inner = "yes | rg --files";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Search {
                cmd: vec_str(&["rg", "--files"]),
                query: None,
                path: None,
            }],
        );
    }

    #[test]
    fn supports_sed_n_then_nl_as_search() {
        // Ensure `sed -n '<range>' <file> | nl -ba` is summarized as a search for that file.
        let args = shlex_split_safe(
            "sed -n '260,640p' exec/src/event_processor_with_human_output.rs | nl -ba",
        );
        assert_parsed(
            &args,
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(
                    "sed -n '260,640p' exec/src/event_processor_with_human_output.rs",
                ),
                name: "event_processor_with_human_output.rs".to_string(),
            }],
        );
    }

    #[test]
    fn preserves_rg_with_spaces() {
        assert_parsed(
            &shlex_split_safe("yes | rg -n 'foo bar' -S"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("rg -n 'foo bar' -S"),
                query: Some("foo bar".to_string()),
                path: None,
            }],
        );
    }

    #[test]
    fn ls_with_glob() {
        assert_parsed(
            &shlex_split_safe("ls -I '*.test.js'"),
            vec![ParsedCommand::ListFiles {
                cmd: shlex_split_safe("ls -I '*.test.js'"),
                path: None,
            }],
        );
    }

    #[test]
    fn trim_on_semicolon() {
        assert_parsed(
            &shlex_split_safe("rg foo ; echo done"),
            vec![
                ParsedCommand::Search {
                    cmd: shlex_split_safe("rg foo"),
                    query: Some("foo".to_string()),
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: shlex_split_safe("echo done"),
                },
            ],
        );
    }

    #[test]
    fn split_on_or_connector() {
        // Ensure we split commands on the logical OR operator as well.
        assert_parsed(
            &shlex_split_safe("rg foo || echo done"),
            vec![
                ParsedCommand::Search {
                    cmd: shlex_split_safe("rg foo"),
                    query: Some("foo".to_string()),
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: shlex_split_safe("echo done"),
                },
            ],
        );
    }

    #[test]
    fn strips_true_in_sequence() {
        // `true` should be dropped from parsed sequences
        assert_parsed(
            &shlex_split_safe("true && rg --files"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("rg --files"),
                query: None,
                path: None,
            }],
        );

        assert_parsed(
            &shlex_split_safe("rg --files && true"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("rg --files"),
                query: None,
                path: None,
            }],
        );
    }

    #[test]
    fn strips_true_inside_bash_lc() {
        let inner = "true && rg --files";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("rg --files"),
                query: None,
                path: None,
            }],
        );

        let inner2 = "rg --files || true";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner2]),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("rg --files"),
                query: None,
                path: None,
            }],
        );
    }

    #[test]
    fn shorten_path_on_windows() {
        assert_parsed(
            &shlex_split_safe(r#"cat "pkg\src\main.rs""#),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe(r#"cat "pkg\src\main.rs""#),
                name: "main.rs".to_string(),
            }],
        );
    }

    #[test]
    fn head_with_no_space() {
        assert_parsed(
            &shlex_split_safe("bash -lc 'head -n50 Cargo.toml'"),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe("head -n50 Cargo.toml"),
                name: "Cargo.toml".to_string(),
            }],
        );
    }

    #[test]
    fn bash_dash_c_pipeline_parsing() {
        // Ensure -c is handled similarly to -lc by normalization
        let inner = "rg --files | head -n 1";
        assert_parsed(
            &shlex_split_safe(inner),
            vec![
                ParsedCommand::Search {
                    cmd: shlex_split_safe("rg --files"),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: shlex_split_safe("head -n 1"),
                },
            ],
        );
    }

    #[test]
    fn tail_with_no_space() {
        assert_parsed(
            &shlex_split_safe("bash -lc 'tail -n+10 README.md'"),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe("tail -n+10 README.md"),
                name: "README.md".to_string(),
            }],
        );
    }

    #[test]
    fn pnpm_test_is_parsed_as_test() {
        assert_parsed(
            &shlex_split_safe("pnpm test"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("pnpm test"),
            }],
        );
    }

    #[test]
    fn pnpm_exec_vitest_is_unknown() {
        // From commands_combined: cd codex-cli && pnpm exec vitest run tests/... --threads=false --passWithNoTests
        let inner = "cd codex-cli && pnpm exec vitest run tests/file-tag-utils.test.ts --threads=false --passWithNoTests";
        assert_parsed(
            &shlex_split_safe(inner),
            vec![
                ParsedCommand::Unknown {
                    cmd: shlex_split_safe("cd codex-cli"),
                },
                ParsedCommand::Unknown {
                    cmd: shlex_split_safe(
                        "pnpm exec vitest run tests/file-tag-utils.test.ts --threads=false --passWithNoTests",
                    ),
                },
            ],
        );
    }

    #[test]
    fn cargo_test_with_crate() {
        assert_parsed(
            &shlex_split_safe("cargo test -p codex-core parse_command::"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("cargo test -p codex-core parse_command::"),
            }],
        );
    }

    #[test]
    fn cargo_test_with_crate_2() {
        assert_parsed(
            &shlex_split_safe(
                "cd core && cargo test -q parse_command::tests::bash_dash_c_pipeline_parsing parse_command::tests::fd_file_finder_variants",
            ),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe(
                    "cargo test -q parse_command::tests::bash_dash_c_pipeline_parsing parse_command::tests::fd_file_finder_variants",
                ),
            }],
        );
    }

    #[test]
    fn cargo_test_with_crate_3() {
        assert_parsed(
            &shlex_split_safe("cd core && cargo test -q parse_command::tests"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("cargo test -q parse_command::tests"),
            }],
        );
    }

    #[test]
    fn cargo_test_with_crate_4() {
        assert_parsed(
            &shlex_split_safe("cd core && cargo test --all-features parse_command -- --nocapture"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("cargo test --all-features parse_command -- --nocapture"),
            }],
        );
    }

    // Additional coverage for other common tools/frameworks
    #[test]
    fn recognizes_black_and_ruff() {
        // black formats Python code
        assert_parsed(
            &shlex_split_safe("black src"),
            vec![ParsedCommand::Format {
                cmd: shlex_split_safe("black src"),
                tool: Some("black".to_string()),
                targets: Some(vec!["src".to_string()]),
            }],
        );

        // ruff check is a linter; ensure we collect targets
        assert_parsed(
            &shlex_split_safe("ruff check ."),
            vec![ParsedCommand::Lint {
                cmd: shlex_split_safe("ruff check ."),
                tool: Some("ruff".to_string()),
                targets: Some(vec![".".to_string()]),
            }],
        );

        // ruff format is a formatter
        assert_parsed(
            &shlex_split_safe("ruff format pkg/"),
            vec![ParsedCommand::Format {
                cmd: shlex_split_safe("ruff format pkg/"),
                tool: Some("ruff".to_string()),
                targets: Some(vec!["pkg/".to_string()]),
            }],
        );
    }

    #[test]
    fn recognizes_pnpm_monorepo_test_and_npm_format_script() {
        // pnpm -r test in a monorepo should still parse as a test action
        assert_parsed(
            &shlex_split_safe("pnpm -r test"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("pnpm -r test"),
            }],
        );

        // npm run format should be recognized as a format action
        assert_parsed(
            &shlex_split_safe("npm run format -- -w ."),
            vec![ParsedCommand::Format {
                cmd: shlex_split_safe("npm run format -- -w ."),
                tool: Some("npm-script:format".to_string()),
                targets: None,
            }],
        );
    }

    #[test]
    fn yarn_test_is_parsed_as_test() {
        assert_parsed(
            &shlex_split_safe("yarn test"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("yarn test"),
            }],
        );
    }

    #[test]
    fn pytest_file_only_and_go_run_regex() {
        // pytest invoked with a file path should be captured as a filter
        assert_parsed(
            &shlex_split_safe("pytest tests/test_example.py"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("pytest tests/test_example.py"),
            }],
        );

        // go test with -run regex should capture the filter
        assert_parsed(
            &shlex_split_safe("go test ./... -run '^TestFoo$'"),
            vec![ParsedCommand::Test {
                cmd: shlex_split_safe("go test ./... -run '^TestFoo$'"),
            }],
        );
    }

    #[test]
    fn grep_with_query_and_path() {
        assert_parsed(
            &shlex_split_safe("grep -R TODO src"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("grep -R TODO src"),
                query: Some("TODO".to_string()),
                path: Some("src".to_string()),
            }],
        );
    }

    #[test]
    fn rg_with_equals_style_flags() {
        assert_parsed(
            &shlex_split_safe("rg --colors=never -n foo src"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("rg --colors=never -n foo src"),
                query: Some("foo".to_string()),
                path: Some("src".to_string()),
            }],
        );
    }

    #[test]
    fn cat_with_double_dash_and_sed_ranges() {
        // cat -- <file> should be treated as a read of that file
        assert_parsed(
            &shlex_split_safe("cat -- ./-strange-file-name"),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe("cat -- ./-strange-file-name"),
                name: "-strange-file-name".to_string(),
            }],
        );

        // sed -n <range> <file> should be treated as a read of <file>
        assert_parsed(
            &shlex_split_safe("sed -n '12,20p' Cargo.toml"),
            vec![ParsedCommand::Read {
                cmd: shlex_split_safe("sed -n '12,20p' Cargo.toml"),
                name: "Cargo.toml".to_string(),
            }],
        );
    }

    #[test]
    fn drop_trailing_nl_in_pipeline() {
        // When an `nl` stage has only flags, it should be dropped from the summary
        assert_parsed(
            &shlex_split_safe("rg --files | nl -ba"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("rg --files"),
                query: None,
                path: None,
            }],
        );
    }

    #[test]
    fn ls_with_time_style_and_path() {
        assert_parsed(
            &shlex_split_safe("ls --time-style=long-iso ./dist"),
            vec![ParsedCommand::ListFiles {
                cmd: shlex_split_safe("ls --time-style=long-iso ./dist"),
                // short_display_path drops "dist" and shows "." as the last useful segment
                path: Some(".".to_string()),
            }],
        );
    }

    #[test]
    fn eslint_with_config_path_and_target() {
        assert_parsed(
            &shlex_split_safe("eslint -c .eslintrc.json src"),
            vec![ParsedCommand::Lint {
                cmd: shlex_split_safe("eslint -c .eslintrc.json src"),
                tool: Some("eslint".to_string()),
                targets: Some(vec!["src".to_string()]),
            }],
        );
    }

    #[test]
    fn npx_eslint_with_config_path_and_target() {
        assert_parsed(
            &shlex_split_safe("npx eslint -c .eslintrc src"),
            vec![ParsedCommand::Lint {
                cmd: shlex_split_safe("npx eslint -c .eslintrc src"),
                tool: Some("eslint".to_string()),
                targets: Some(vec!["src".to_string()]),
            }],
        );
    }

    #[test]
    fn fd_file_finder_variants() {
        assert_parsed(
            &shlex_split_safe("fd -t f src/"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("fd -t f src/"),
                query: None,
                path: Some("src".to_string()),
            }],
        );

        // fd with query and path should capture both
        assert_parsed(
            &shlex_split_safe("fd main src"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("fd main src"),
                query: Some("main".to_string()),
                path: Some("src".to_string()),
            }],
        );
    }

    #[test]
    fn find_basic_name_filter() {
        assert_parsed(
            &shlex_split_safe("find . -name '*.rs'"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("find . -name '*.rs'"),
                query: Some("*.rs".to_string()),
                path: Some(".".to_string()),
            }],
        );
    }

    #[test]
    fn find_type_only_path() {
        assert_parsed(
            &shlex_split_safe("find src -type f"),
            vec![ParsedCommand::Search {
                cmd: shlex_split_safe("find src -type f"),
                query: None,
                path: Some("src".to_string()),
            }],
        );
    }
}

pub fn parse_command_impl(command: &[String]) -> Vec<ParsedCommand> {
    let normalized = normalize_tokens(command);

    if let Some(commands) = parse_bash_lc_commands(command, &normalized) {
        return commands;
    }

    let parts = if contains_connectors(&normalized) {
        split_on_connectors(&normalized)
    } else {
        vec![normalized.clone()]
    };

    // Preserve left-to-right execution order for all commands, including bash -c/-lc
    // so summaries reflect the order they will run.

    // Map each pipeline segment to its parsed summary.
    let mut parsed: Vec<ParsedCommand> = parts
        .iter()
        .map(|tokens| summarize_main_tokens(tokens))
        .collect();

    // If a pipeline ends with `nl` using only flags (e.g., `| nl -ba`), drop it so the
    // main action (e.g., a sed range over a file) is surfaced cleanly.
    if parsed.len() >= 2 {
        let has_and_and = normalized.iter().any(|t| t == "&&");
        let contains_test = parsed
            .iter()
            .any(|pc| matches!(pc, ParsedCommand::Test { .. }));
        parsed.retain(|pc| match pc {
            ParsedCommand::Unknown { cmd } => {
                if let Some(first) = cmd.first() {
                    // Drop cosmetic echo segments in chained commands
                    if has_and_and && first == "echo" {
                        return false;
                    }
                    // In non-bash chained commands, ignore directory changes like `cd foo`
                    // when the sequence includes a recognized test command. Preserve `cd`
                    // for other sequences (e.g., followed by a search command).
                    if has_and_and && contains_test && first == "cd" {
                        return false;
                    }
                    // Drop no-op commands like `true`
                    if cmd.len() == 1 && first == "true" {
                        return false;
                    }
                    if first == "nl" {
                        // Treat `nl` without an explicit file operand as formatting-only.
                        return cmd.iter().skip(1).any(|a| !a.starts_with('-'));
                    }
                }
                true
            }
            _ => true,
        });
    }

    // Also drop standalone `true` commands when not part of a chained `&&` context above
    parsed.retain(|pc| match pc {
        ParsedCommand::Unknown { cmd } => {
            !(cmd.len() == 1 && cmd.first().is_some_and(|s| s == "true"))
        }
        _ => true,
    });

    parsed
}

/// Validates that this is a `sed -n 123,123p` command.
fn is_valid_sed_n_arg(arg: Option<&str>) -> bool {
    let s = match arg {
        Some(s) => s,
        None => return false,
    };
    let core = match s.strip_suffix('p') {
        Some(rest) => rest,
        None => return false,
    };
    let parts: Vec<&str> = core.split(',').collect();
    match parts.as_slice() {
        [num] => !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()),
        [a, b] => {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

/// Normalize a command by:
/// - Removing `yes`/`no`/`bash -c`/`bash -lc` prefixes.
/// - Splitting on `|` and `&&`/`||`/`;
fn normalize_tokens(cmd: &[String]) -> Vec<String> {
    match cmd {
        [first, pipe, rest @ ..] if (first == "yes" || first == "y") && pipe == "|" => {
            // Do not re-shlex already-tokenized input; just drop the prefix.
            rest.to_vec()
        }
        [first, pipe, rest @ ..] if (first == "no" || first == "n") && pipe == "|" => {
            // Do not re-shlex already-tokenized input; just drop the prefix.
            rest.to_vec()
        }
        [bash, flag, script] if bash == "bash" && (flag == "-c" || flag == "-lc") => {
            shlex_split(script)
                .unwrap_or_else(|| vec!["bash".to_string(), flag.clone(), script.clone()])
        }
        _ => cmd.to_vec(),
    }
}

fn contains_connectors(tokens: &[String]) -> bool {
    tokens
        .iter()
        .any(|t| t == "&&" || t == "||" || t == "|" || t == ";")
}

fn split_on_connectors(tokens: &[String]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for t in tokens {
        if t == "&&" || t == "||" || t == "|" || t == ";" {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(t.clone());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn trim_at_connector(tokens: &[String]) -> Vec<String> {
    let idx = tokens
        .iter()
        .position(|t| t == "|" || t == "&&" || t == "||" || t == ";")
        .unwrap_or(tokens.len());
    tokens[..idx].to_vec()
}

/// Shorten a path to the last component, excluding `build`/`dist`/`node_modules`/`src`.
/// It also pulls out a useful path from a directory such as:
/// - webview/src -> webview
/// - foo/src/ -> foo
/// - packages/app/node_modules/ -> app
fn short_display_path(path: &str) -> String {
    // Normalize separators and drop any trailing slash for display.
    let normalized = path.replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    let mut parts = trimmed.split('/').rev().filter(|p| {
        !p.is_empty() && *p != "build" && *p != "dist" && *p != "node_modules" && *p != "src"
    });
    parts
        .next()
        .map(|s| s.to_string())
        .unwrap_or_else(|| trimmed.to_string())
}

// Skip values consumed by specific flags and ignore --flag=value style arguments.
fn skip_flag_values<'a>(args: &'a [String], flags_with_vals: &[&str]) -> Vec<&'a String> {
    let mut out: Vec<&'a String> = Vec::new();
    let mut skip_next = false;
    for (i, a) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if a == "--" {
            // From here on, everything is positional operands; push the rest and break.
            for rest in &args[i + 1..] {
                out.push(rest);
            }
            break;
        }
        if a.starts_with("--") && a.contains('=') {
            // --flag=value form: treat as a flag taking a value; skip entirely.
            continue;
        }
        if flags_with_vals.contains(&a.as_str()) {
            // This flag consumes the next argument as its value.
            if i + 1 < args.len() {
                skip_next = true;
            }
            continue;
        }
        out.push(a);
    }
    out
}

/// Common flags for ESLint that take a following value and should not be
/// considered positional targets.
const ESLINT_FLAGS_WITH_VALUES: &[&str] = &[
    "-c",
    "--config",
    "--parser",
    "--parser-options",
    "--rulesdir",
    "--plugin",
    "--max-warnings",
    "--format",
];

fn collect_non_flag_targets(args: &[String]) -> Option<Vec<String>> {
    let mut targets = Vec::new();
    let mut skip_next = false;
    for (i, a) in args.iter().enumerate() {
        if a == "--" {
            break;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if a == "-p"
            || a == "--package"
            || a == "--features"
            || a == "-C"
            || a == "--config"
            || a == "--config-path"
            || a == "--out-dir"
            || a == "-o"
            || a == "--run"
            || a == "--max-warnings"
            || a == "--format"
        {
            if i + 1 < args.len() {
                skip_next = true;
            }
            continue;
        }
        if a.starts_with('-') {
            continue;
        }
        targets.push(a.clone());
    }
    if targets.is_empty() {
        None
    } else {
        Some(targets)
    }
}

fn collect_non_flag_targets_with_flags(
    args: &[String],
    flags_with_vals: &[&str],
) -> Option<Vec<String>> {
    let targets: Vec<String> = skip_flag_values(args, flags_with_vals)
        .into_iter()
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect();
    if targets.is_empty() {
        None
    } else {
        Some(targets)
    }
}

fn is_pathish(s: &str) -> bool {
    s == "."
        || s == ".."
        || s.starts_with("./")
        || s.starts_with("../")
        || s.contains('/')
        || s.contains('\\')
}

fn parse_fd_query_and_path(tail: &[String]) -> (Option<String>, Option<String>) {
    let args_no_connector = trim_at_connector(tail);
    // fd has several flags that take values (e.g., -t/--type, -e/--extension).
    // Skip those values when extracting positional operands.
    let candidates = skip_flag_values(
        &args_no_connector,
        &[
            "-t",
            "--type",
            "-e",
            "--extension",
            "-E",
            "--exclude",
            "--search-path",
        ],
    );
    let non_flags: Vec<&String> = candidates
        .into_iter()
        .filter(|p| !p.starts_with('-'))
        .collect();
    match non_flags.as_slice() {
        [one] => {
            if is_pathish(one) {
                (None, Some(short_display_path(one)))
            } else {
                (Some((*one).clone()), None)
            }
        }
        [q, p, ..] => (Some((*q).clone()), Some(short_display_path(p))),
        _ => (None, None),
    }
}

fn parse_find_query_and_path(tail: &[String]) -> (Option<String>, Option<String>) {
    let args_no_connector = trim_at_connector(tail);
    // First positional argument (excluding common unary operators) is the root path
    let mut path: Option<String> = None;
    for a in &args_no_connector {
        if !a.starts_with('-') && *a != "!" && *a != "(" && *a != ")" {
            path = Some(short_display_path(a));
            break;
        }
    }
    // Extract a common name/path/regex pattern if present
    let mut query: Option<String> = None;
    let mut i = 0;
    while i < args_no_connector.len() {
        let a = &args_no_connector[i];
        if a == "-name" || a == "-iname" || a == "-path" || a == "-regex" {
            if i + 1 < args_no_connector.len() {
                query = Some(args_no_connector[i + 1].clone());
            }
            break;
        }
        i += 1;
    }
    (query, path)
}

fn classify_npm_like(tool: &str, tail: &[String], full_cmd: &[String]) -> Option<ParsedCommand> {
    let mut r = tail;
    if tool == "pnpm" && r.first().map(|s| s.as_str()) == Some("-r") {
        r = &r[1..];
    }
    let mut script_name: Option<String> = None;
    if r.first().map(|s| s.as_str()) == Some("run") {
        script_name = r.get(1).cloned();
    } else {
        let is_test_cmd = (tool == "npm" && r.first().map(|s| s.as_str()) == Some("t"))
            || ((tool == "npm" || tool == "pnpm" || tool == "yarn")
                && r.first().map(|s| s.as_str()) == Some("test"));
        if is_test_cmd {
            script_name = Some("test".to_string());
        }
    }
    if let Some(name) = script_name {
        let lname = name.to_lowercase();
        if lname == "test" || lname == "unit" || lname == "jest" || lname == "vitest" {
            return Some(ParsedCommand::Test {
                cmd: full_cmd.to_vec(),
            });
        }
        if lname == "lint" || lname == "eslint" {
            return Some(ParsedCommand::Lint {
                cmd: full_cmd.to_vec(),
                tool: Some(format!("{tool}-script:{name}")),
                targets: None,
            });
        }
        if lname == "format" || lname == "fmt" || lname == "prettier" {
            return Some(ParsedCommand::Format {
                cmd: full_cmd.to_vec(),
                tool: Some(format!("{tool}-script:{name}")),
                targets: None,
            });
        }
    }
    None
}

fn parse_bash_lc_commands(
    original: &[String],
    normalized: &[String],
) -> Option<Vec<ParsedCommand>> {
    let [bash, flag, script] = original else {
        return None;
    };
    if bash != "bash" || flag != "-lc" {
        return None;
    }
    if let Some(tree) = try_parse_bash(script) {
        if let Some(all_commands) = try_parse_word_only_commands_sequence(&tree, script) {
            if !all_commands.is_empty() {
                let script_tokens = shlex_split(script)
                    .unwrap_or_else(|| vec!["bash".to_string(), flag.clone(), script.clone()]);
                // Strip small formatting helpers (e.g., head/tail/awk/wc/etc) so we
                // bias toward the primary command when pipelines are present.
                // First, drop obvious small formatting helpers (e.g., wc/awk/etc).
                let had_multiple_commands = all_commands.len() > 1;
                // The bash AST walker yields commands in right-to-left order for
                // connector/pipeline sequences. Reverse to reflect actual execution order.
                let mut filtered_commands = drop_small_formatting_commands(all_commands);
                filtered_commands.reverse();
                if filtered_commands.is_empty() {
                    return Some(vec![ParsedCommand::Unknown {
                        cmd: normalized.to_vec(),
                    }]);
                }
                let mut commands: Vec<ParsedCommand> = filtered_commands
                    .into_iter()
                    .map(|tokens| summarize_main_tokens(&tokens))
                    .collect();
                // Drop no-op `true` commands
                commands.retain(|pc| match pc {
                    ParsedCommand::Unknown { cmd } => {
                        !(cmd.len() == 1 && cmd.first().is_some_and(|s| s == "true"))
                    }
                    _ => true,
                });
                commands = maybe_collapse_cat_sed(commands, &script_tokens);
                if commands.len() == 1 {
                    // If we reduced to a single command, attribute the full original script
                    // for clearer UX in file-reading and listing scenarios, or when there were
                    // no connectors in the original script. For search commands that came from
                    // a pipeline (e.g. `rg --files | sed -n`), keep only the primary command.
                    let had_connectors = had_multiple_commands
                        || script_tokens
                            .iter()
                            .any(|t| t == "|" || t == "&&" || t == "||" || t == ";");
                    commands = commands
                        .into_iter()
                        .map(|pc| match pc {
                            ParsedCommand::Read { name, cmd } => {
                                if had_connectors {
                                    let has_pipe = script_tokens.iter().any(|t| t == "|");
                                    let has_sed_n = script_tokens.windows(2).any(|w| {
                                        w.first().map(|s| s.as_str()) == Some("sed")
                                            && w.get(1).map(|s| s.as_str()) == Some("-n")
                                    });
                                    if has_pipe && has_sed_n {
                                        ParsedCommand::Read {
                                            cmd: script_tokens.clone(),
                                            name,
                                        }
                                    } else {
                                        ParsedCommand::Read { cmd, name }
                                    }
                                } else {
                                    ParsedCommand::Read {
                                        cmd: script_tokens.clone(),
                                        name,
                                    }
                                }
                            }
                            ParsedCommand::ListFiles { path, cmd } => {
                                if had_connectors {
                                    ParsedCommand::ListFiles { cmd, path }
                                } else {
                                    ParsedCommand::ListFiles {
                                        cmd: script_tokens.clone(),
                                        path,
                                    }
                                }
                            }
                            ParsedCommand::Search { cmd, query, path } => {
                                if had_connectors {
                                    ParsedCommand::Search { cmd, query, path }
                                } else {
                                    ParsedCommand::Search {
                                        cmd: script_tokens.clone(),
                                        query,
                                        path,
                                    }
                                }
                            }
                            ParsedCommand::Format { tool, targets, .. } => ParsedCommand::Format {
                                cmd: script_tokens.clone(),
                                tool,
                                targets,
                            },
                            ParsedCommand::Test { .. } => ParsedCommand::Test {
                                cmd: script_tokens.clone(),
                            },
                            ParsedCommand::Lint { tool, targets, .. } => ParsedCommand::Lint {
                                cmd: script_tokens.clone(),
                                tool,
                                targets,
                            },
                            ParsedCommand::Unknown { .. } => ParsedCommand::Unknown {
                                cmd: script_tokens.clone(),
                            },
                        })
                        .collect();
                }
                return Some(commands);
            }
        }
    }
    Some(vec![ParsedCommand::Unknown {
        cmd: normalized.to_vec(),
    }])
}

/// Return true if this looks like a small formatting helper in a pipeline.
/// Examples: `head -n 40`, `tail -n +10`, `wc -l`, `awk ...`, `cut ...`, `tr ...`.
/// We try to keep variants that clearly include a file path (e.g. `tail -n 30 file`).
fn is_small_formatting_command(tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let cmd = tokens[0].as_str();
    match cmd {
        // Always formatting; typically used in pipes.
        // `nl` is special-cased below to allow `nl <file>` to be treated as a read command.
        "wc" | "tr" | "cut" | "sort" | "uniq" | "xargs" | "tee" | "column" | "awk" | "yes"
        | "printf" => true,
        "head" => {
            // Treat as formatting when no explicit file operand is present.
            // Common forms: `head -n 40`, `head -c 100`.
            // Keep cases like `head -n 40 file`.
            tokens.len() < 3
        }
        "tail" => {
            // Treat as formatting when no explicit file operand is present.
            // Common forms: `tail -n +10`, `tail -n 30`.
            // Keep cases like `tail -n 30 file`.
            tokens.len() < 3
        }
        "sed" => {
            // Keep `sed -n <range> file` (treated as a file read elsewhere);
            // otherwise consider it a formatting helper in a pipeline.
            tokens.len() < 4
                || !(tokens[1] == "-n" && is_valid_sed_n_arg(tokens.get(2).map(|s| s.as_str())))
        }
        _ => false,
    }
}

fn drop_small_formatting_commands(mut commands: Vec<Vec<String>>) -> Vec<Vec<String>> {
    commands.retain(|tokens| !is_small_formatting_command(tokens));
    commands
}

fn maybe_collapse_cat_sed(
    commands: Vec<ParsedCommand>,
    script_tokens: &[String],
) -> Vec<ParsedCommand> {
    if commands.len() < 2 {
        return commands;
    }
    let drop_leading_sed = match (&commands[0], &commands[1]) {
        (ParsedCommand::Unknown { cmd: sed_cmd }, ParsedCommand::Read { cmd: cat_cmd, .. }) => {
            let is_sed_n = sed_cmd.first().map(|s| s.as_str()) == Some("sed")
                && sed_cmd.get(1).map(|s| s.as_str()) == Some("-n")
                && is_valid_sed_n_arg(sed_cmd.get(2).map(|s| s.as_str()))
                && sed_cmd.len() == 3;
            let is_cat_file =
                cat_cmd.first().map(|s| s.as_str()) == Some("cat") && cat_cmd.len() == 2;
            is_sed_n && is_cat_file
        }
        _ => false,
    };
    if drop_leading_sed {
        if let ParsedCommand::Read { name, .. } = &commands[1] {
            return vec![ParsedCommand::Read {
                cmd: script_tokens.to_vec(),
                name: name.clone(),
            }];
        }
    }
    commands
}

fn summarize_main_tokens(main_cmd: &[String]) -> ParsedCommand {
    match main_cmd.split_first() {
        // (sed-specific logic handled below in dedicated arm returning Read)
        Some((head, tail))
            if head == "cargo" && tail.first().map(|s| s.as_str()) == Some("fmt") =>
        {
            ParsedCommand::Format {
                cmd: main_cmd.to_vec(),
                tool: Some("cargo fmt".to_string()),
                targets: collect_non_flag_targets(&tail[1..]),
            }
        }
        Some((head, tail))
            if head == "cargo" && tail.first().map(|s| s.as_str()) == Some("clippy") =>
        {
            ParsedCommand::Lint {
                cmd: main_cmd.to_vec(),
                tool: Some("cargo clippy".to_string()),
                targets: collect_non_flag_targets(&tail[1..]),
            }
        }
        Some((head, tail))
            if head == "cargo" && tail.first().map(|s| s.as_str()) == Some("test") =>
        {
            ParsedCommand::Test {
                cmd: main_cmd.to_vec(),
            }
        }
        Some((head, tail)) if head == "rustfmt" => ParsedCommand::Format {
            cmd: main_cmd.to_vec(),
            tool: Some("rustfmt".to_string()),
            targets: collect_non_flag_targets(tail),
        },
        Some((head, tail)) if head == "go" && tail.first().map(|s| s.as_str()) == Some("fmt") => {
            ParsedCommand::Format {
                cmd: main_cmd.to_vec(),
                tool: Some("go fmt".to_string()),
                targets: collect_non_flag_targets(&tail[1..]),
            }
        }
        Some((head, tail)) if head == "go" && tail.first().map(|s| s.as_str()) == Some("test") => {
            ParsedCommand::Test {
                cmd: main_cmd.to_vec(),
            }
        }
        Some((head, _)) if head == "pytest" => ParsedCommand::Test {
            cmd: main_cmd.to_vec(),
        },
        Some((head, tail)) if head == "eslint" => {
            // Treat configuration flags with values (e.g. `-c .eslintrc`) as non-targets.
            let targets = collect_non_flag_targets_with_flags(tail, ESLINT_FLAGS_WITH_VALUES);
            ParsedCommand::Lint {
                cmd: main_cmd.to_vec(),
                tool: Some("eslint".to_string()),
                targets,
            }
        }
        Some((head, tail)) if head == "prettier" => ParsedCommand::Format {
            cmd: main_cmd.to_vec(),
            tool: Some("prettier".to_string()),
            targets: collect_non_flag_targets(tail),
        },
        Some((head, tail)) if head == "black" => ParsedCommand::Format {
            cmd: main_cmd.to_vec(),
            tool: Some("black".to_string()),
            targets: collect_non_flag_targets(tail),
        },
        Some((head, tail))
            if head == "ruff" && tail.first().map(|s| s.as_str()) == Some("check") =>
        {
            ParsedCommand::Lint {
                cmd: main_cmd.to_vec(),
                tool: Some("ruff".to_string()),
                targets: collect_non_flag_targets(&tail[1..]),
            }
        }
        Some((head, tail))
            if head == "ruff" && tail.first().map(|s| s.as_str()) == Some("format") =>
        {
            ParsedCommand::Format {
                cmd: main_cmd.to_vec(),
                tool: Some("ruff".to_string()),
                targets: collect_non_flag_targets(&tail[1..]),
            }
        }
        Some((head, _)) if (head == "jest" || head == "vitest") => ParsedCommand::Test {
            cmd: main_cmd.to_vec(),
        },
        Some((head, tail))
            if head == "npx" && tail.first().map(|s| s.as_str()) == Some("eslint") =>
        {
            let targets = collect_non_flag_targets_with_flags(&tail[1..], ESLINT_FLAGS_WITH_VALUES);
            ParsedCommand::Lint {
                cmd: main_cmd.to_vec(),
                tool: Some("eslint".to_string()),
                targets,
            }
        }
        Some((head, tail))
            if head == "npx" && tail.first().map(|s| s.as_str()) == Some("prettier") =>
        {
            ParsedCommand::Format {
                cmd: main_cmd.to_vec(),
                tool: Some("prettier".to_string()),
                targets: collect_non_flag_targets(&tail[1..]),
            }
        }
        // NPM-like scripts including yarn
        Some((tool, tail)) if (tool == "pnpm" || tool == "npm" || tool == "yarn") => {
            if let Some(cmd) = classify_npm_like(tool, tail, main_cmd) {
                cmd
            } else {
                ParsedCommand::Unknown {
                    cmd: main_cmd.to_vec(),
                }
            }
        }
        Some((head, tail)) if head == "ls" => {
            // Avoid treating option values as paths (e.g., ls -I "*.test.js").
            let candidates = skip_flag_values(
                tail,
                &[
                    "-I",
                    "-w",
                    "--block-size",
                    "--format",
                    "--time-style",
                    "--color",
                    "--quoting-style",
                ],
            );
            let path = candidates
                .into_iter()
                .find(|p| !p.starts_with('-'))
                .map(|p| short_display_path(p));
            ParsedCommand::ListFiles {
                cmd: main_cmd.to_vec(),
                path,
            }
        }
        Some((head, tail)) if head == "rg" => {
            let args_no_connector = trim_at_connector(tail);
            let has_files_flag = args_no_connector.iter().any(|a| a == "--files");
            let non_flags: Vec<&String> = args_no_connector
                .iter()
                .filter(|p| !p.starts_with('-'))
                .collect();
            let (query, path) = if has_files_flag {
                (None, non_flags.first().map(|s| short_display_path(s)))
            } else {
                (
                    non_flags.first().cloned().map(|s| s.to_string()),
                    non_flags.get(1).map(|s| short_display_path(s)),
                )
            };
            ParsedCommand::Search {
                cmd: main_cmd.to_vec(),
                query,
                path,
            }
        }
        Some((head, tail)) if head == "fd" => {
            let (query, path) = parse_fd_query_and_path(tail);
            ParsedCommand::Search {
                cmd: main_cmd.to_vec(),
                query,
                path,
            }
        }
        Some((head, tail)) if head == "find" => {
            // Basic find support: capture path and common name filter
            let (query, path) = parse_find_query_and_path(tail);
            ParsedCommand::Search {
                cmd: main_cmd.to_vec(),
                query,
                path,
            }
        }
        Some((head, tail)) if head == "grep" => {
            let args_no_connector = trim_at_connector(tail);
            let non_flags: Vec<&String> = args_no_connector
                .iter()
                .filter(|p| !p.starts_with('-'))
                .collect();
            // Do not shorten the query: grep patterns may legitimately contain slashes
            // and should be preserved verbatim. Only paths should be shortened.
            let query = non_flags.first().cloned().map(|s| s.to_string());
            let path = non_flags.get(1).map(|s| short_display_path(s));
            ParsedCommand::Search {
                cmd: main_cmd.to_vec(),
                query,
                path,
            }
        }
        Some((head, tail)) if head == "cat" => {
            // Support both `cat <file>` and `cat -- <file>` forms.
            let effective_tail: &[String] = if tail.first().map(|s| s.as_str()) == Some("--") {
                &tail[1..]
            } else {
                tail
            };
            if effective_tail.len() == 1 {
                let name = short_display_path(&effective_tail[0]);
                ParsedCommand::Read {
                    cmd: main_cmd.to_vec(),
                    name,
                }
            } else {
                ParsedCommand::Unknown {
                    cmd: main_cmd.to_vec(),
                }
            }
        }
        Some((head, tail)) if head == "head" => {
            // Support `head -n 50 file` and `head -n50 file` forms.
            let has_valid_n = match tail.split_first() {
                Some((first, rest)) if first == "-n" => rest
                    .first()
                    .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit())),
                Some((first, _)) if first.starts_with("-n") => {
                    first[2..].chars().all(|c| c.is_ascii_digit())
                }
                _ => false,
            };
            if has_valid_n {
                // Build candidates skipping the numeric value consumed by `-n` when separated.
                let mut candidates: Vec<&String> = Vec::new();
                let mut i = 0;
                while i < tail.len() {
                    if i == 0 && tail[i] == "-n" && i + 1 < tail.len() {
                        let n = &tail[i + 1];
                        if n.chars().all(|c| c.is_ascii_digit()) {
                            i += 2;
                            continue;
                        }
                    }
                    candidates.push(&tail[i]);
                    i += 1;
                }
                if let Some(p) = candidates.into_iter().find(|p| !p.starts_with('-')) {
                    let name = short_display_path(p);
                    return ParsedCommand::Read {
                        cmd: main_cmd.to_vec(),
                        name,
                    };
                }
            }
            ParsedCommand::Unknown {
                cmd: main_cmd.to_vec(),
            }
        }
        Some((head, tail)) if head == "tail" => {
            // Support `tail -n +10 file` and `tail -n+10 file` forms.
            let has_valid_n = match tail.split_first() {
                Some((first, rest)) if first == "-n" => rest.first().is_some_and(|n| {
                    let s = n.strip_prefix('+').unwrap_or(n);
                    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
                }),
                Some((first, _)) if first.starts_with("-n") => {
                    let v = &first[2..];
                    let s = v.strip_prefix('+').unwrap_or(v);
                    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
                }
                _ => false,
            };
            if has_valid_n {
                // Build candidates skipping the numeric value consumed by `-n` when separated.
                let mut candidates: Vec<&String> = Vec::new();
                let mut i = 0;
                while i < tail.len() {
                    if i == 0 && tail[i] == "-n" && i + 1 < tail.len() {
                        let n = &tail[i + 1];
                        let s = n.strip_prefix('+').unwrap_or(n);
                        if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
                            i += 2;
                            continue;
                        }
                    }
                    candidates.push(&tail[i]);
                    i += 1;
                }
                if let Some(p) = candidates.into_iter().find(|p| !p.starts_with('-')) {
                    let name = short_display_path(p);
                    return ParsedCommand::Read {
                        cmd: main_cmd.to_vec(),
                        name,
                    };
                }
            }
            ParsedCommand::Unknown {
                cmd: main_cmd.to_vec(),
            }
        }
        Some((head, tail)) if head == "nl" => {
            // Avoid treating option values as paths (e.g., nl -s "  ").
            let candidates = skip_flag_values(tail, &["-s", "-w", "-v", "-i", "-b"]);
            if let Some(p) = candidates.into_iter().find(|p| !p.starts_with('-')) {
                let name = short_display_path(p);
                ParsedCommand::Read {
                    cmd: main_cmd.to_vec(),
                    name,
                }
            } else {
                ParsedCommand::Unknown {
                    cmd: main_cmd.to_vec(),
                }
            }
        }
        Some((head, tail))
            if head == "sed"
                && tail.len() >= 3
                && tail[0] == "-n"
                && is_valid_sed_n_arg(tail.get(1).map(|s| s.as_str())) =>
        {
            if let Some(path) = tail.get(2) {
                let name = short_display_path(path);
                ParsedCommand::Read {
                    cmd: main_cmd.to_vec(),
                    name,
                }
            } else {
                ParsedCommand::Unknown {
                    cmd: main_cmd.to_vec(),
                }
            }
        }
        // Other commands
        _ => ParsedCommand::Unknown {
            cmd: main_cmd.to_vec(),
        },
    }
}
