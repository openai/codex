use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;

pub fn is_known_safe_command(command: &[String]) -> bool {
    if is_safe_to_call_with_exec(command) {
        return true;
    }

    // TODO(mbolin): Also support safe commands that are piped together such
    // as `cat foo | wc -l`.
    matches!(
        command,
        [bash, flag, script]
            if bash == "bash"
            && flag == "-lc"
            && try_parse_bash(script).and_then(|tree|
                try_parse_single_word_only_command(&tree, script)).is_some_and(|parsed_bash_command| is_safe_to_call_with_exec(&parsed_bash_command))
    )
}

fn is_safe_to_call_with_exec(command: &[String]) -> bool {
    let cmd0 = command.first().map(String::as_str);

    match cmd0 {
        Some(
            "cat" | "cd" | "echo" | "grep" | "head" | "ls" | "pwd" | "rg" | "tail" | "wc" | "which",
        ) => true,

        Some("find") => {
            // Certain options to `find` can delete files, write to files, or
            // execute arbitrary commands, so we cannot auto-approve the
            // invocation of `find` in such cases.
            #[rustfmt::skip]
            const UNSAFE_FIND_OPTIONS: &[&str] = &[
                // Options that can execute arbitrary commands.
                "-exec", "-execdir", "-ok", "-okdir",
                // Option that deletes matching files.
                "-delete",
                // Options that write pathnames to a file.
                "-fls", "-fprint", "-fprint0", "-fprintf",
            ];

            !command
                .iter()
                .any(|arg| UNSAFE_FIND_OPTIONS.contains(&arg.as_str()))
        }

        // Git
        Some("git") => matches!(
            command.get(1).map(String::as_str),
            Some("branch" | "status" | "log" | "diff" | "show")
        ),

        // Rust
        Some("cargo") if command.get(1).map(String::as_str) == Some("check") => true,

        // Special-case `sed -n {N|M,N}p FILE`
        Some("sed")
            if {
                command.len() == 4
                    && command.get(1).map(String::as_str) == Some("-n")
                    && is_valid_sed_n_arg(command.get(2).map(String::as_str))
                    && command.get(3).map(String::is_empty) == Some(false)
            } =>
        {
            true
        }

        // ── anything else ─────────────────────────────────────────────────
        _ => false,
    }
}

fn try_parse_bash(bash_lc_arg: &str) -> Option<Tree> {
    let lang = BASH.into();
    let mut parser = Parser::new();
    parser.set_language(&lang).expect("load bash grammar");

    let old_tree: Option<&Tree> = None;
    parser.parse(bash_lc_arg, old_tree)
}

/// If `tree` represents a single Bash command whose name and every argument is
/// an ordinary `word`, return those words in order; otherwise, return `None`.
///
/// `src` must be the exact source string that was parsed into `tree`, so we can
/// extract the text for every node.
pub fn try_parse_single_word_only_command(tree: &Tree, src: &str) -> Option<Vec<String>> {
    // Any parse error is an immediate rejection.
    if tree.root_node().has_error() {
        return None;
    }

    // (program …) with exactly one statement
    let root = tree.root_node();
    if root.kind() != "program" || root.named_child_count() != 1 {
        return None;
    }

    let cmd = root.named_child(0)?; // (command …)
    if cmd.kind() != "command" {
        return None;
    }

    let mut words = Vec::new();
    let mut cursor = cmd.walk();

    for child in cmd.named_children(&mut cursor) {
        match child.kind() {
            // The command name node wraps one `word` child.
            "command_name" => {
                let word_node = child.named_child(0)?; // make sure it's only a word
                if word_node.kind() != "word" {
                    return None;
                }
                words.push(word_node.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            // Positional‑argument word (allowed).
            "word" | "number" => {
                words.push(child.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "string" => {
                if child.child_count() == 3
                    && child.child(0)?.kind() == "\""
                    && child.child(1)?.kind() == "string_content"
                    && child.child(2)?.kind() == "\""
                {
                    words.push(child.child(1)?.utf8_text(src.as_bytes()).ok()?.to_owned());
                } else {
                    // Anything else means the command is *not* plain words.
                    return None;
                }
            }
            "concatenation" => {
                // TODO: Consider things like `'ab\'a'`.
                return None;
            }
            "raw_string" => {
                // Raw string is a single word, but we need to strip the quotes.
                let raw_string = child.utf8_text(src.as_bytes()).ok()?;
                let stripped = raw_string
                    .strip_prefix('\'')
                    .and_then(|s| s.strip_suffix('\''));
                if let Some(stripped) = stripped {
                    words.push(stripped.to_owned());
                } else {
                    return None;
                }
            }
            // Anything else means the command is *not* plain words.
            _ => return None,
        }
    }

    Some(words)
}

/* ----------------------------------------------------------
Example
---------------------------------------------------------- */

/// Returns true if `arg` matches /^(\d+,)?\d+p$/
fn is_valid_sed_n_arg(arg: Option<&str>) -> bool {
    // unwrap or bail
    let s = match arg {
        Some(s) => s,
        None => return false,
    };

    // must end with 'p', strip it
    let core = match s.strip_suffix('p') {
        Some(rest) => rest,
        None => return false,
    };

    // split on ',' and ensure 1 or 2 numeric parts
    let parts: Vec<&str> = core.split(',').collect();
    match parts.as_slice() {
        // single number, e.g. "10"
        [num] => !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()),

        // two numbers, e.g. "1,5"
        [a, b] => {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }

        // anything else (more than one comma) is invalid
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn known_safe_examples() {
        assert!(is_safe_to_call_with_exec(&vec_str(&["ls"])));
        assert!(is_safe_to_call_with_exec(&vec_str(&["git", "status"])));
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "sed", "-n", "1,5p", "file.txt"
        ])));

        // Safe `find` command (no unsafe options).
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "find", ".", "-name", "file.txt"
        ])));
    }

    #[test]
    fn unknown_or_partial() {
        assert!(!is_safe_to_call_with_exec(&vec_str(&["foo"])));
        assert!(!is_safe_to_call_with_exec(&vec_str(&["git", "fetch"])));
        assert!(!is_safe_to_call_with_exec(&vec_str(&[
            "sed", "-n", "xp", "file.txt"
        ])));

        // Unsafe `find` commands.
        for args in [
            vec_str(&["find", ".", "-name", "file.txt", "-exec", "rm", "{}", ";"]),
            vec_str(&[
                "find", ".", "-name", "*.py", "-execdir", "python3", "{}", ";",
            ]),
            vec_str(&["find", ".", "-name", "file.txt", "-ok", "rm", "{}", ";"]),
            vec_str(&["find", ".", "-name", "*.py", "-okdir", "python3", "{}", ";"]),
            vec_str(&["find", ".", "-delete", "-name", "file.txt"]),
            vec_str(&["find", ".", "-fls", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprint", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprint0", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprintf", "/root/suid.txt", "%#m %u %p\n"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {:?} to be unsafe",
                args
            );
        }
    }

    #[test]
    fn bash_lc_safe_examples() {
        assert!(is_known_safe_command(&vec_str(&["bash", "-lc", "ls"])));
        assert!(is_known_safe_command(&vec_str(&["bash", "-lc", "ls -1"])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git status"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "grep -R \"Cargo.toml\" -n"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "sed -n 1,5p file.txt"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "sed -n '1,5p' file.txt"
        ])));

        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "find . -name file.txt"
        ])));
    }

    #[test]
    fn bash_lc_unsafe_examples() {
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "git", "status"])),
            "Four arg version is not known to be safe."
        );
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "'git status'"])),
            "The extra quoting around 'git status' makes it a program named 'git status' and is therefore unsafe."
        );

        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "find . -name file.txt -delete"])),
            "Unsafe find option should not be auto‑approved."
        );
    }

    #[test]
    fn test_try_parse_single_word_only_command() {
        let script_with_single_quoted_string = "sed -n '1,5p' file.txt";
        let parsed_words = try_parse_bash(script_with_single_quoted_string)
            .and_then(|tree| {
                try_parse_single_word_only_command(&tree, script_with_single_quoted_string)
            })
            .unwrap();
        assert_eq!(
            vec![
                "sed".to_string(),
                "-n".to_string(),
                // Ensure the single quotes are properly removed.
                "1,5p".to_string(),
                "file.txt".to_string()
            ],
            parsed_words,
        );

        let script_with_number_arg = "ls -1";
        let parsed_words = try_parse_bash(script_with_number_arg)
            .and_then(|tree| try_parse_single_word_only_command(&tree, script_with_number_arg))
            .unwrap();
        assert_eq!(vec!["ls", "-1"], parsed_words,);

        let script_with_double_quoted_string_with_no_funny_stuff_arg = "grep -R \"Cargo.toml\" -n";
        let parsed_words = try_parse_bash(script_with_double_quoted_string_with_no_funny_stuff_arg)
            .and_then(|tree| {
                try_parse_single_word_only_command(
                    &tree,
                    script_with_double_quoted_string_with_no_funny_stuff_arg,
                )
            })
            .unwrap();
        assert_eq!(vec!["grep", "-R", "Cargo.toml", "-n"], parsed_words);
    }
}
