use std::path::PathBuf;

use tree_sitter::Node;
use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;

use crate::shell_detect::ShellType;
use crate::shell_detect::detect_shell_type;

/// Parse the provided bash source using tree-sitter-bash, returning a Tree on
/// success or None if parsing failed.
pub fn try_parse_shell(shell_lc_arg: &str) -> Option<Tree> {
    let lang = BASH.into();
    let mut parser = Parser::new();
    #[expect(clippy::expect_used)]
    parser.set_language(&lang).expect("load bash grammar");
    let old_tree: Option<&Tree> = None;
    parser.parse(shell_lc_arg, old_tree)
}

/// Parse a script which may contain multiple simple commands joined only by
/// the safe logical/pipe/sequencing operators: `&&`, `||`, `;`, `|`.
///
/// Returns `Some(Vec<command_words>)` if every command is a plain word‑only
/// command and the parse tree does not contain disallowed constructs
/// (parentheses, redirections, substitutions, control flow, etc.). Otherwise
/// returns `None`.
pub fn try_parse_word_only_commands_sequence(tree: &Tree, src: &str) -> Option<Vec<Vec<String>>> {
    if tree.root_node().has_error() {
        return None;
    }

    // List of allowed (named) node kinds for a "word only commands sequence".
    // If we encounter a named node that is not in this list we reject.
    const ALLOWED_KINDS: &[&str] = &[
        // top level containers
        "program",
        "list",
        "pipeline",
        // commands & words
        "command",
        "command_name",
        "word",
        "string",
        "string_content",
        "raw_string",
        "number",
        "concatenation",
    ];
    // Allow only safe punctuation / operator tokens; anything else causes reject.
    const ALLOWED_PUNCT_TOKENS: &[&str] = &["&&", "||", ";", "|", "\"", "'"];

    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut command_nodes = Vec::new();
    while let Some(node) = stack.pop() {
        let kind = node.kind();
        if node.is_named() {
            if !ALLOWED_KINDS.contains(&kind) {
                return None;
            }
            if kind == "command" {
                command_nodes.push(node);
            }
        } else {
            // Reject any punctuation / operator tokens that are not explicitly allowed.
            if kind.chars().any(|c| "&;|".contains(c)) && !ALLOWED_PUNCT_TOKENS.contains(&kind) {
                return None;
            }
            if !(ALLOWED_PUNCT_TOKENS.contains(&kind) || kind.trim().is_empty()) {
                // If it's a quote token or operator it's allowed above; we also allow whitespace tokens.
                // Any other punctuation like parentheses, braces, redirects, backticks, etc are rejected.
                return None;
            }
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    // Walk uses a stack (LIFO), so re-sort by position to restore source order.
    command_nodes.sort_by_key(Node::start_byte);

    let mut commands = Vec::new();
    for node in command_nodes {
        if let Some(words) = parse_plain_command_from_node(node, src) {
            commands.push(words);
        } else {
            return None;
        }
    }
    Some(commands)
}

pub fn extract_bash_command(command: &[String]) -> Option<(&str, &str)> {
    let [shell, flag, script] = command else {
        return None;
    };
    if !is_recognised_bash_lc_invocation(shell, flag) {
        return None;
    }
    Some((shell, script))
}

/// Single source of truth for "is `<shell> <flag>` a recognised bash-lc
/// invocation we want to strip the wrapper from?".
///
/// Shared by `extract_bash_command` and `extract_bash_command_joined` so the
/// acceptance rules (which flags, which shells) live in exactly one place.
fn is_recognised_bash_lc_invocation(shell: &str, flag: &str) -> bool {
    matches!(flag, "-lc" | "-c")
        && matches!(
            detect_shell_type(PathBuf::from(shell)),
            Some(ShellType::Zsh) | Some(ShellType::Bash) | Some(ShellType::Sh)
        )
}

/// Like `extract_bash_command`, but also recognises argv where the inner
/// script was flattened past `[shell, flag, script]` (e.g. the protocol
/// payload re-split an unquoted form into `[shell, flag, word, word, ...]`).
/// Returns an owned script reconstructed across the tail. Ordinary arguments
/// are shell-quoted, while standalone shell operator tokens remain operators.
///
/// Caveat: this assumes every token after the flag is script text, so it is
/// deliberately wrong for POSIX `sh -c <script> <arg0> <arg1>` where the tail
/// is `$0`/`$1`/… — `["bash","-c","echo $0","foo"]` yields `"echo $0 foo"`.
/// That mis-rendering is fine for display-only consumers (exec history,
/// unified-exec, tab status detail). Canonical-identity/approval matching in
/// `core/src/command_canonicalization.rs` stays strict; do not reuse here.
pub fn extract_bash_command_joined(command: &[String]) -> Option<(String, String)> {
    if let Some((shell, script)) = extract_bash_command(command) {
        return Some((shell.to_string(), script.to_string()));
    }
    let [shell, flag, rest @ ..] = command else {
        return None;
    };
    // `rest.len() < 2` rejects both the 3-element case (handled above) and
    // the impossible <3-element case at once.
    if rest.len() < 2 || !is_recognised_bash_lc_invocation(shell, flag) {
        return None;
    }
    Some((shell.clone(), join_flattened_shell_script(rest)))
}

fn join_flattened_shell_script(tokens: &[String]) -> String {
    let mut script = String::new();
    for token in tokens {
        if !script.is_empty() {
            script.push(' ');
        }
        if is_shell_operator_token(token) {
            script.push_str(token);
        } else {
            let Ok(quoted) = shlex::try_quote(token) else {
                return "<command included NUL byte>".to_string();
            };
            script.push_str(&quoted);
        }
    }
    script
}

fn is_shell_operator_token(token: &str) -> bool {
    token.chars().any(|ch| "<>|&;()".contains(ch))
        && token
            .chars()
            .all(|ch| ch.is_ascii_digit() || "<>|&;()".contains(ch))
}

/// Returns the sequence of plain commands within a `bash -lc "..."` or
/// `zsh -lc "..."` invocation when the script only contains word-only commands
/// joined by safe operators.
///
/// Uses the strict `extract_bash_command`, so flattened argv returns None
/// rather than being rich-parsed: disambiguating "tail is script" from "tail
/// is POSIX `$0`/`$1`" isn't safe here. Such argv instead lands in
/// `single_unknown_for_command`'s joined fallback as `Unknown { cmd }`.
pub fn parse_shell_lc_plain_commands(command: &[String]) -> Option<Vec<Vec<String>>> {
    let (_, script) = extract_bash_command(command)?;

    let tree = try_parse_shell(script)?;
    try_parse_word_only_commands_sequence(&tree, script)
}

/// Returns the parsed argv for a single shell command in a here-doc style
/// script (`<<`), as long as the script contains exactly one command node.
pub fn parse_shell_lc_single_command_prefix(command: &[String]) -> Option<Vec<String>> {
    let (_, script) = extract_bash_command(command)?;
    let tree = try_parse_shell(script)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    if !has_named_descendant_kind(root, "heredoc_redirect") {
        return None;
    }
    if has_named_descendant_kind(root, "file_redirect") {
        return None;
    }

    let command_node = find_single_command_node(root)?;
    parse_heredoc_command_words(command_node, script)
}

fn parse_plain_command_from_node(cmd: tree_sitter::Node, src: &str) -> Option<Vec<String>> {
    if cmd.kind() != "command" {
        return None;
    }
    let mut words = Vec::new();
    let mut cursor = cmd.walk();
    for child in cmd.named_children(&mut cursor) {
        match child.kind() {
            "command_name" => {
                let word_node = child.named_child(0)?;
                if word_node.kind() != "word" {
                    return None;
                }
                words.push(word_node.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "word" | "number" => {
                words.push(child.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "string" => {
                let parsed = parse_double_quoted_string(child, src)?;
                words.push(parsed);
            }
            "raw_string" => {
                let parsed = parse_raw_string(child, src)?;
                words.push(parsed);
            }
            "concatenation" => {
                // Handle concatenated arguments like -g"*.py"
                let mut concatenated = String::new();
                let mut concat_cursor = child.walk();
                for part in child.named_children(&mut concat_cursor) {
                    match part.kind() {
                        "word" | "number" => {
                            concatenated
                                .push_str(part.utf8_text(src.as_bytes()).ok()?.to_owned().as_str());
                        }
                        "string" => {
                            let parsed = parse_double_quoted_string(part, src)?;
                            concatenated.push_str(&parsed);
                        }
                        "raw_string" => {
                            let parsed = parse_raw_string(part, src)?;
                            concatenated.push_str(&parsed);
                        }
                        _ => return None,
                    }
                }
                if concatenated.is_empty() {
                    return None;
                }
                words.push(concatenated);
            }
            _ => return None,
        }
    }
    Some(words)
}

fn parse_heredoc_command_words(cmd: Node<'_>, src: &str) -> Option<Vec<String>> {
    if cmd.kind() != "command" {
        return None;
    }

    let mut words = Vec::new();
    let mut cursor = cmd.walk();
    for child in cmd.named_children(&mut cursor) {
        match child.kind() {
            "command_name" => {
                let word_node = child.named_child(0)?;
                if !matches!(word_node.kind(), "word" | "number")
                    || !is_literal_word_or_number(word_node)
                {
                    return None;
                }
                words.push(word_node.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "word" | "number" => {
                if !is_literal_word_or_number(child) {
                    return None;
                }
                words.push(child.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            // Allow heredoc constructs that attach stdin to a single command
            // without changing argv matching semantics for the executable
            // prefix. Other file redirects may write outside the sandbox and
            // must not be collapsed to the executable prefix for execpolicy.
            "comment" => {}
            kind if is_allowed_heredoc_attachment_kind(kind) => {}
            _ => return None,
        }
    }

    if words.is_empty() { None } else { Some(words) }
}

fn is_literal_word_or_number(node: Node<'_>) -> bool {
    if !matches!(node.kind(), "word" | "number") {
        return false;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next().is_none()
}

fn is_allowed_heredoc_attachment_kind(kind: &str) -> bool {
    matches!(
        kind,
        "heredoc_body"
            | "simple_heredoc_body"
            | "heredoc_redirect"
            | "herestring_redirect"
            | "redirected_statement"
    )
}

fn find_single_command_node(root: Node<'_>) -> Option<Node<'_>> {
    let mut stack = vec![root];
    let mut single_command = None;
    while let Some(node) = stack.pop() {
        if node.kind() == "command" {
            if single_command.is_some() {
                return None;
            }
            single_command = Some(node);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            stack.push(child);
        }
    }
    single_command
}

fn has_named_descendant_kind(node: Node<'_>, kind: &str) -> bool {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if current.kind() == kind {
            return true;
        }
        let mut cursor = current.walk();
        for child in current.named_children(&mut cursor) {
            stack.push(child);
        }
    }
    false
}

fn parse_double_quoted_string(node: Node, src: &str) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }

    let mut cursor = node.walk();
    for part in node.named_children(&mut cursor) {
        if part.kind() != "string_content" {
            return None;
        }
    }
    let raw = node.utf8_text(src.as_bytes()).ok()?;
    let stripped = raw
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))?;
    Some(stripped.to_string())
}

fn parse_raw_string(node: Node, src: &str) -> Option<String> {
    if node.kind() != "raw_string" {
        return None;
    }

    let raw_string = node.utf8_text(src.as_bytes()).ok()?;
    let stripped = raw_string
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''));
    stripped.map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn parse_seq(src: &str) -> Option<Vec<Vec<String>>> {
        let tree = try_parse_shell(src)?;
        try_parse_word_only_commands_sequence(&tree, src)
    }

    #[test]
    fn extract_bash_command_joined_accepts_flattened_argv() {
        // Unquoted `zsh -lc touch /tmp/foo` shlex-splits to 4 tokens, which
        // strict extract_bash_command rejects; the joined variant stitches it.
        let (shell, script) = extract_bash_command_joined(&[
            "zsh".to_string(),
            "-lc".to_string(),
            "touch".to_string(),
            "/tmp/foo".to_string(),
        ])
        .expect("known shell + -lc + tail should match");
        assert_eq!(shell, "zsh");
        assert_eq!(script, "touch /tmp/foo");
    }

    #[test]
    fn extract_bash_command_joined_preserves_shell_operators() {
        let (_, script) = extract_bash_command_joined(&[
            "zsh".to_string(),
            "-lc".to_string(),
            "rg".to_string(),
            "foo bar".to_string(),
            "|".to_string(),
            "head".to_string(),
            ">".to_string(),
            "/tmp/out".to_string(),
            "2>&1".to_string(),
            ";".to_string(),
            "echo".to_string(),
            "done".to_string(),
        ])
        .expect("known shell + -lc + tail should match");
        assert_eq!(script, "rg 'foo bar' | head > /tmp/out 2>&1 ; echo done");
    }

    #[test]
    fn extract_bash_command_joined_accepts_absolute_shell_path() {
        let (shell, script) = extract_bash_command_joined(&[
            "/opt/homebrew/bin/zsh".to_string(),
            "-lc".to_string(),
            "touch".to_string(),
            "/tmp/foo".to_string(),
        ])
        .expect("absolute path resolving to zsh should match");
        assert_eq!(shell, "/opt/homebrew/bin/zsh");
        assert_eq!(script, "touch /tmp/foo");
    }

    #[test]
    fn extract_bash_command_joined_handles_three_element_canonical_shape() {
        let (_, script) = extract_bash_command_joined(&[
            "bash".to_string(),
            "-c".to_string(),
            "echo hello".to_string(),
        ])
        .expect("canonical 3-element form should also match");
        // Single tail element comes back as-is (no re-shlex-join).
        assert_eq!(script, "echo hello");
    }

    #[test]
    fn extract_bash_command_joined_rejects_non_shell_first_arg() {
        assert!(
            extract_bash_command_joined(&[
                "python3".to_string(),
                "-c".to_string(),
                "print('hi')".to_string(),
            ])
            .is_none(),
            "python3 -c is not a recognised shell wrapper"
        );
    }

    #[test]
    fn extract_bash_command_joined_rejects_unknown_flag() {
        assert!(
            extract_bash_command_joined(&[
                "bash".to_string(),
                "-x".to_string(),
                "echo hi".to_string(),
            ])
            .is_none(),
            "only -lc and -c are recognised"
        );
    }

    #[test]
    fn extract_bash_command_joined_rejects_empty_tail() {
        assert!(
            extract_bash_command_joined(&["bash".to_string(), "-lc".to_string()]).is_none(),
            "no script tail means nothing to extract"
        );
    }

    #[test]
    fn extract_bash_command_joined_acceptance_tracks_strict_matcher() {
        // The 3-element and 4+-element arms now share
        // `is_recognised_bash_lc_invocation`; assert they agree across a
        // matrix of (shell, flag) pairs so the two can't silently diverge.
        let cases: &[(&str, &str)] = &[
            // Accepted.
            ("bash", "-lc"),
            ("bash", "-c"),
            ("zsh", "-lc"),
            ("zsh", "-c"),
            ("sh", "-lc"),
            ("/bin/bash", "-lc"),
            ("/opt/homebrew/bin/zsh", "-lc"),
            // Rejected.
            ("bash", "-x"),
            ("bash", "-ic"),
            ("bash", "-l"),
            ("python3", "-c"),
            ("fish", "-lc"),
            ("pwsh", "-c"),
        ];
        for &(shell, flag) in cases {
            let three: Vec<String> = vec![shell.into(), flag.into(), "echo hi".into()];
            let four: Vec<String> = vec![shell.into(), flag.into(), "echo".into(), "hi".into()];

            let strict_three = extract_bash_command(&three).is_some();
            let joined_three = extract_bash_command_joined(&three).is_some();
            let joined_four = extract_bash_command_joined(&four).is_some();

            assert_eq!(
                strict_three, joined_three,
                "3-element delegation must agree with strict for ({shell:?}, {flag:?})",
            );
            assert_eq!(
                strict_three, joined_four,
                "4+-element relaxation must accept iff strict accepts for ({shell:?}, {flag:?})",
            );
        }
    }

    #[test]
    fn accepts_single_simple_command() {
        let cmds = parse_seq("ls -1").unwrap();
        assert_eq!(cmds, vec![vec!["ls".to_string(), "-1".to_string()]]);
    }

    #[test]
    fn accepts_multiple_commands_with_allowed_operators() {
        let src = "ls && pwd; echo 'hi there' | wc -l";
        let cmds = parse_seq(src).unwrap();
        let expected: Vec<Vec<String>> = vec![
            vec!["ls".to_string()],
            vec!["pwd".to_string()],
            vec!["echo".to_string(), "hi there".to_string()],
            vec!["wc".to_string(), "-l".to_string()],
        ];
        assert_eq!(cmds, expected);
    }

    #[test]
    fn extracts_double_and_single_quoted_strings() {
        let cmds = parse_seq("echo \"hello world\"").unwrap();
        assert_eq!(
            cmds,
            vec![vec!["echo".to_string(), "hello world".to_string()]]
        );

        let cmds2 = parse_seq("echo 'hi there'").unwrap();
        assert_eq!(
            cmds2,
            vec![vec!["echo".to_string(), "hi there".to_string()]]
        );
    }

    #[test]
    fn accepts_double_quoted_strings_with_newlines() {
        let cmds = parse_seq("git commit -m \"line1\nline2\"").unwrap();
        assert_eq!(
            cmds,
            vec![vec![
                "git".to_string(),
                "commit".to_string(),
                "-m".to_string(),
                "line1\nline2".to_string(),
            ]]
        );
    }

    #[test]
    fn accepts_mixed_quote_concatenation() {
        assert_eq!(
            parse_seq(r#"echo "/usr"'/'"local"/bin"#).unwrap(),
            vec![vec!["echo".to_string(), "/usr/local/bin".to_string()]]
        );
        assert_eq!(
            parse_seq(r#"echo '/usr'"/"'local'/bin"#).unwrap(),
            vec![vec!["echo".to_string(), "/usr/local/bin".to_string()]]
        );
    }

    #[test]
    fn rejects_double_quoted_strings_with_expansions() {
        assert!(parse_seq(r#"echo "hi ${USER}""#).is_none());
        assert!(parse_seq(r#"echo "$HOME""#).is_none());
    }

    #[test]
    fn accepts_numbers_as_words() {
        let cmds = parse_seq("echo 123 456").unwrap();
        assert_eq!(
            cmds,
            vec![vec![
                "echo".to_string(),
                "123".to_string(),
                "456".to_string()
            ]]
        );
    }

    #[test]
    fn rejects_parentheses_and_subshells() {
        assert!(parse_seq("(ls)").is_none());
        assert!(parse_seq("ls || (pwd && echo hi)").is_none());
    }

    #[test]
    fn rejects_redirections_and_unsupported_operators() {
        assert!(parse_seq("ls > out.txt").is_none());
        assert!(parse_seq("echo hi & echo bye").is_none());
    }

    #[test]
    fn rejects_command_and_process_substitutions_and_expansions() {
        assert!(parse_seq("echo $(pwd)").is_none());
        assert!(parse_seq("echo `pwd`").is_none());
        assert!(parse_seq("echo $HOME").is_none());
        assert!(parse_seq("echo \"hi $USER\"").is_none());
    }

    #[test]
    fn rejects_variable_assignment_prefix() {
        assert!(parse_seq("FOO=bar ls").is_none());
    }

    #[test]
    fn rejects_trailing_operator_parse_error() {
        assert!(parse_seq("ls &&").is_none());
    }

    #[test]
    fn rejects_empty_command_position_with_leading_operator() {
        assert!(parse_seq("&& ls").is_none());
    }

    #[test]
    fn rejects_empty_command_position_with_double_separator() {
        assert!(parse_seq("ls ;; pwd").is_none());
    }

    #[test]
    fn rejects_empty_command_position_with_empty_pipeline_segment() {
        assert!(parse_seq("ls | | wc").is_none());
    }

    #[test]
    fn parse_zsh_lc_plain_commands() {
        let command = vec!["zsh".to_string(), "-lc".to_string(), "ls".to_string()];
        let parsed = parse_shell_lc_plain_commands(&command).unwrap();
        assert_eq!(parsed, vec![vec!["ls".to_string()]]);
    }

    #[test]
    fn accepts_concatenated_flag_and_value() {
        // Test case: -g"*.py" (flag directly concatenated with quoted value)
        let cmds = parse_seq("rg -n \"foo\" -g\"*.py\"").unwrap();
        assert_eq!(
            cmds,
            vec![vec![
                "rg".to_string(),
                "-n".to_string(),
                "foo".to_string(),
                "-g*.py".to_string(),
            ]]
        );
    }

    #[test]
    fn accepts_concatenated_flag_with_single_quotes() {
        let cmds = parse_seq("grep -n 'pattern' -g'*.txt'").unwrap();
        assert_eq!(
            cmds,
            vec![vec![
                "grep".to_string(),
                "-n".to_string(),
                "pattern".to_string(),
                "-g*.txt".to_string(),
            ]]
        );
    }

    #[test]
    fn rejects_concatenation_with_variable_substitution() {
        // Environment variables in concatenated strings should be rejected
        assert!(parse_seq("rg -g\"$VAR\" pattern").is_none());
        assert!(parse_seq("rg -g\"${VAR}\" pattern").is_none());
    }

    #[test]
    fn rejects_concatenation_with_command_substitution() {
        // Command substitution in concatenated strings should be rejected
        assert!(parse_seq("rg -g\"$(pwd)\" pattern").is_none());
        assert!(parse_seq("rg -g\"$(echo '*.py')\" pattern").is_none());
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_supports_heredoc() {
        let command = vec![
            "zsh".to_string(),
            "-lc".to_string(),
            "python3 <<'PY'\nprint('hello')\nPY".to_string(),
        ];
        let parsed = parse_shell_lc_single_command_prefix(&command);
        assert_eq!(parsed, Some(vec!["python3".to_string()]));

        let command_unquoted = vec![
            "zsh".to_string(),
            "-lc".to_string(),
            "python3 << PY\nprint('hello')\nPY".to_string(),
        ];
        let parsed_unquoted = parse_shell_lc_single_command_prefix(&command_unquoted);
        assert_eq!(parsed_unquoted, Some(vec!["python3".to_string()]));
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_multi_command_scripts() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "python3 <<'PY'\nprint('hello')\nPY\necho done".to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_non_heredoc_redirects() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo hello > /tmp/out.txt".to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_heredoc_with_extra_file_redirect() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "python3 <<'PY' > /tmp/out.txt\nprint('hello')\nPY".to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_heredoc_with_variable_assignment() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "PATH=/tmp/evil:$PATH cat <<'EOF'\nhello\nEOF".to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_herestring_with_chaining() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            r#"echo hello > /tmp/out.txt && cat /tmp/out.txt"#.to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_herestring_with_substitution() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            r#"python3 <<< "$(rm -rf /)""#.to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_arithmetic_shift_non_heredoc_script() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo $((1<<2))".to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }

    #[test]
    fn parse_shell_lc_single_command_prefix_rejects_heredoc_command_with_word_expansion() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "python3 $((1<<2)) <<'PY'\nprint('hello')\nPY".to_string(),
        ];
        assert_eq!(parse_shell_lc_single_command_prefix(&command), None);
    }
}
