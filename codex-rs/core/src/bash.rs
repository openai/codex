use std::path::PathBuf;

use tree_sitter::Node;
use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;

use crate::shell::ShellType;
use crate::shell::detect_shell_type;

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
    if !matches!(flag.as_str(), "-lc" | "-c")
        || !matches!(
            detect_shell_type(&PathBuf::from(shell)),
            Some(ShellType::Zsh) | Some(ShellType::Bash) | Some(ShellType::Sh)
        )
    {
        return None;
    }
    Some((shell, script))
}

/// Returns the sequence of plain commands within a `bash -lc "..."` or
/// `zsh -lc "..."` invocation when the script only contains word-only commands
/// joined by safe operators.
pub fn parse_shell_lc_plain_commands(command: &[String]) -> Option<Vec<Vec<String>>> {
    let (_, script) = extract_bash_command(command)?;

    let tree = try_parse_shell(script)?;
    try_parse_word_only_commands_sequence(&tree, script)
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
}
