use indexmap::IndexMap;
use shlex::try_quote;
use tracing::debug;
use tree_sitter::Node;
use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;

const PRINT_TREE_DEBUG: bool = false;

/// Parse the provided bash source using tree-sitter-bash, returning a Tree on
/// success or None if parsing failed.
pub fn try_parse_bash(bash_lc_arg: &str) -> Option<Tree> {
    let lang = BASH.into();
    let mut parser = Parser::new();
    #[expect(clippy::expect_used)]
    parser.set_language(&lang).expect("load bash grammar");
    let old_tree: Option<&Tree> = None;
    let tree = parser.parse(bash_lc_arg, old_tree);
    if PRINT_TREE_DEBUG && let Some(ref t) = tree {
        print_node(t.root_node(), bash_lc_arg, 2);
    }
    tree
}

fn print_node(node: Node, source: &str, indent: usize) {
    let prefix = "  ".repeat(indent);

    let suffix = if node.child_count() == 0 {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            format!(": '{}'", text.replace('\n', "\\n").replace('\t', "\\t"))
        } else {
            ": <UTF8 ERROR>".to_string()
        }
    } else {
        String::new()
    };

    debug!("{}{}{}", prefix, node.kind(), suffix);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_node(child, source, indent + 1);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Command {
    pub original_cmd: String,
    pub name: String,
    pub options: IndexMap<String, Vec<String>>,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TidiedPartsParam {
    pub include_name: bool,
    pub include_options_key: bool,
    pub include_options_val: bool,
    pub include_options_assign_key: bool,
    pub include_options_assign_value: bool,
    pub include_argument: bool,
}

impl Command {
    pub(crate) fn len(&self) -> usize {
        let options_count: usize = self
            .options
            .iter()
            .map(
                |(_key, vals)| {
                    if vals.is_empty() { 1 } else { vals.len() * 2 }
                },
            )
            .sum();

        self.arguments
            .len()
            .saturating_add(options_count)
            .saturating_add(if self.name.is_empty() { 0 } else { 1 })
    }

    /// Get the first value for any of the given keys
    pub(crate) fn find_option_val(&self, keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| {
            self.options
                .get(*key)
                .and_then(|vals| vals.first())
                .cloned()
        })
    }

    /// Similar to `find_option_val`, but also supports combined short flags
    /// like `-n50` (no space between key and value).
    pub(crate) fn find_option_val_strip_key(&self, keys: &[&str]) -> Option<String> {
        if let Some(val) = self.find_option_val(keys) {
            return Some(val);
        }

        for opt_key in self.options.keys() {
            for &key in keys {
                if opt_key.starts_with(key) && opt_key.len() > key.len() {
                    // e.g. "-n50" starts with "-n"
                    let stripped_val = opt_key[key.len()..].to_string();
                    return Some(stripped_val);
                }
            }
        }
        None
    }

    /// Remove options whose keys are in `flags_with_vals`.
    pub(crate) fn skip_flag_values(&mut self, flags_with_vals: &[&str]) {
        let keys_to_remove: Vec<String> = self
            .options
            .keys()
            .filter(|key| flags_with_vals.contains(&key.as_str()))
            .cloned()
            .collect();

        for key in keys_to_remove {
            self.options.shift_remove(&key);
        }
    }

    pub(crate) fn get_original_cmd(&self) -> String {
        self.original_cmd.clone()
    }

    /// Get the tidied parts
    pub(crate) fn get_tidied_parts(&self, param: TidiedPartsParam) -> Vec<String> {
        let mut parts = Vec::new();

        if param.include_name {
            parts.push(self.name.clone());
        }

        for (key, vals) in &self.options {
            if vals.is_empty() {
                if param.include_options_key {
                    parts.push(key.clone());
                }
            } else {
                for val in vals {
                    if key.starts_with("--") && key.contains('=') {
                        if param.include_options_assign_key && param.include_options_assign_value {
                            parts.push(format!("{key}={val}"));
                        }
                    } else {
                        if param.include_options_key {
                            parts.push(key.clone());
                        }
                        if param.include_options_val {
                            parts.push(val.clone());
                        }
                    }
                }
            }
        }

        if param.include_argument {
            for arg in &self.arguments {
                parts.push(arg.clone());
            }
        }

        parts
    }

    pub(crate) fn shlex_join(tokens: &[String]) -> String {
        tokens
            .iter()
            .map(|t| match t.as_str() {
                // Keep shell connectors as-is
                "|" | "&&" | "||" | ";" | "&" => t.clone(),
                // Safely quote everything else
                _ => try_quote(t)
                    .unwrap_or_else(|_| "<command included NUL byte>".into())
                    .to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Shorten a path to the last component, excluding `build`/`dist`/`node_modules`/`src`.
    /// It also pulls out a useful path from a directory such as:
    /// - webview/src -> webview
    /// - foo/src/ -> foo
    /// - packages/app/node_modules/ -> app
    pub(crate) fn short_display_path(&self, path: Option<String>) -> Option<String> {
        path.map(|p| {
            // Normalize separators and drop any trailing slash for display.
            let normalized = p.replace('\\', "/");
            let trimmed = normalized.trim_end_matches('/');
            let mut parts = trimmed.split('/').rev().filter(|p| {
                !p.is_empty()
                    && *p != "build"
                    && *p != "dist"
                    && *p != "node_modules"
                    && *p != "src"
            });
            parts
                .next()
                .map(str::to_string)
                .unwrap_or_else(|| trimmed.to_string())
        })
    }
}

/// Macro to define bash commands and generate visitor methods
#[macro_export]
macro_rules! define_bash_commands {
    ($($cmd:ident),* $(,)?) => {
        use strum_macros::Display;
        use strum_macros::EnumString;
        use std::str::FromStr;
        use tree_sitter::Node;
        use indexmap::IndexMap;

        /// All supported bash AST node kinds.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
        #[strum(serialize_all = "snake_case")]
        enum BashNodeKind {
            Program,
            List,
            Pipeline,
            Command,
            Subshell,
            RedirectedStatement,
        }

        impl BashNodeKind {
            /// Try to parse a node kind from a tree-sitter node.
            fn from_node(node: Node) -> Option<Self> {
                Self::from_str(node.kind()).ok()
            }
        }

        /// Enum representing all supported bash commands
        #[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
        #[strum(serialize_all = "snake_case")]
        enum BashCommands {
            $($cmd),*
        }

        /// Core visitor trait for traversing tree-sitter AST nodes.
        trait NodeVisitor<'tree> {
            /// Main entry point for visiting a node
            fn visit(&mut self, node: Node<'tree>) {
                match BashNodeKind::from_node(node) {
                    Some(BashNodeKind::Program) => self.visit_program(node),
                    Some(BashNodeKind::List) => self.visit_list(node),
                    Some(BashNodeKind::Pipeline) => self.visit_pipeline(node),
                    Some(BashNodeKind::Subshell) => self.visit_subshell(node),
                    Some(BashNodeKind::RedirectedStatement) => self.visit_redirected_statement(node),
                    Some(BashNodeKind::Command) => {
                        if let Some(parsed_cmd) = self.parse_to_command(node) {
                            if let Ok(ty) = BashCommands::from_str(&parsed_cmd.name) {
                                match ty {
                                    $(
                                        BashCommands::$cmd => {
                                            paste::paste! {
                                                self.visit_enter_command(node);
                                                self.[<visit_command_ $cmd:lower>](parsed_cmd);
                                                self.visit_leave_command(node);
                                                self.visit_children(node);
                                            }
                                        },
                                    )*
                                }
                            } else {
                                self.visit_enter_command(node);
                                self.visit_command_unknown(parsed_cmd);
                                self.visit_leave_command(node);
                                self.visit_children(node);
                            }
                        } else {
                            println!("failed to parse command: {}", self.get_text(node))
                        }
                    }
                    _ => self.visit_children(node),
                }
            }

            fn source_code(&self) -> &str;

            fn get_text(&self, node: Node<'tree>) -> &str {
                let start = node.start_byte();
                let end = node.end_byte();
                &self.source_code()[start..end]
            }

            /// Try to parse as shell token, fall back to raw text
            fn get_unescaped_token(&self, node: Node) -> String {
                let text = &self.source_code()[node.start_byte()..node.end_byte()];

                shlex::split(text)
                    .and_then(|parts| parts.into_iter().next())
                    .unwrap_or_else(|| text.to_string())
            }

            fn get_command_name(&self, node: Node<'tree>) -> Option<String> {
                let name_node = self.find_child_by_kind(node, "command_name")?;

                let text = &self.get_text(name_node);
                Some(text.to_string())
            }

            fn find_child_by_kind(&self, node: Node<'tree>, expected_kind: &str) -> Option<Node<'tree>> {
                let mut cursor = node.walk();
                node.children(&mut cursor)
                    .find(|child| child.kind() == expected_kind)
            }

            fn parse_to_command(&self, node: Node<'tree>) -> Option<Command> {
                let mut cursor = node.walk();
                let name = self.get_command_name(node)?;
                let mut options: IndexMap<String, Vec<String>> = IndexMap::new();
                let mut arguments = Vec::new();
                let mut children = node.children(&mut cursor).peekable();
                let mut seen_double_dash = false;

                while let Some(child) = children.next() {
                    // Skip command name
                    if child.kind() == "command_name" {
                        continue;
                    }

                    // Get the properly unescaped token
                    let text = self.get_unescaped_token(child);

                    if seen_double_dash {
                        arguments.push(text);
                        continue;
                    }

                    if text == "--" {
                        seen_double_dash = true;
                        continue;
                    }

                    // Handle --flag=value
                    if text.starts_with("--") && text.contains('=') {
                        let key = text;
                        options.entry(key).or_insert_with(Vec::new).push("".into());
                        continue;
                    }

                    // Handle options with separate values
                    if text.starts_with('-') {
                        if let Some(next) = children.peek() {
                            let next_text = self.get_unescaped_token(*next);
                            if !next_text.starts_with('-') {
                                options.entry(text).or_insert_with(Vec::new).push(next_text);
                                children.next();
                                continue;
                            }
                        }
                        options.entry(text).or_insert_with(Vec::new);
                    } else {
                        arguments.push(text);
                    }
                }

                let cmd_text = self.get_text(node);
                let original_cmd: Vec<String> = match shlex::split(&cmd_text) {
                    Some(parts) => parts,
                    None => vec![cmd_text.to_string()],
                };

                Some(Command {
                    original_cmd: Command::shlex_join(&original_cmd),
                    name,
                    options,
                    arguments,
                })
            }

            fn visit_children(&mut self, node: Node<'tree>) {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.visit(child);
                }
            }

            fn visit_enter_command(&mut self, _node: Node<'tree>) {
            }

            fn visit_leave_command(&mut self, _node: Node<'tree>) {
            }

            fn visit_redirected_statement(&mut self, node: Node<'tree>) {
                self.visit_children(node)
            }

            fn visit_program(&mut self, node: Node<'tree>) {
                self.visit_children(node)
            }

            fn visit_list(&mut self, node: Node<'tree>) {
                self.visit_children(node)
            }

            fn visit_pipeline(&mut self, node: Node<'tree>) {
                self.visit_children(node)
            }

            fn visit_subshell(&mut self, _node: Node<'tree>) {}

            $(
                paste::paste! {
                    fn [<visit_command_ $cmd:lower>](&mut self, _cmd: Command) {}
                }
            )*
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    define_bash_commands!(
        Unknown,
    );

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    struct TestParser {
        source: String,
        command: Vec<Command>,
    }

    impl TestParser {
        fn new() -> Self {
            Default::default()
        }
    }

    impl<'tree> NodeVisitor<'tree> for TestParser {
        fn source_code(&self) -> &str {
            &self.source
        }

        fn visit_command_unknown(&mut self, cmd: Command) {
            self.command.push(cmd);
        }
    }

    fn parse_command(source: &str) -> Vec<Command> {
        let tree = try_parse_bash(source).unwrap();
        let root = tree.root_node();

        let mut p = TestParser::new();
        p.source = source.to_owned();
        p.visit(root);
        p.command
    }

    #[test]
    fn test_parse_simple_command_name_only() {
        let cmd = parse_command("ls");

        assert_eq!(cmd[0].name, "ls");
        assert!(cmd[0].options.is_empty());
        assert!(cmd[0].arguments.is_empty());
    }

    #[test]
    fn test_parse_command_with_single_flag() {
        let cmd = parse_command("ls -l");

        assert_eq!(cmd[0].name, "ls");
        assert_eq!(cmd[0].options.len(), 1);
        assert!(cmd[0].options.contains_key("-l"));
        assert!(cmd[0].options["-l"].is_empty());
    }

    #[test]
    fn test_parse_command_with_multiple_separate_flags() {
        let cmd = parse_command("ls -l -a -h");

        assert_eq!(cmd[0].name, "ls");
        assert_eq!(cmd[0].options.len(), 3);
        assert!(cmd[0].options.contains_key("-l"));
        assert!(cmd[0].options.contains_key("-a"));
        assert!(cmd[0].options.contains_key("-h"));
        assert!(cmd[0].options["-l"].is_empty());
        assert!(cmd[0].options["-a"].is_empty());
        assert!(cmd[0].options["-h"].is_empty());
    }

    #[test]
    fn test_parse_command_with_combined_flags() {
        let cmd = parse_command("ls -la");

        assert_eq!(cmd[0].name, "ls");
        assert!(cmd[0].options.contains_key("-la"));
    }

    #[test]
    fn test_parse_command_with_single_positional_arg() {
        let cmd = parse_command("cat file.txt");

        assert_eq!(cmd[0].name, "cat");
        assert_eq!(cmd[0].arguments, vec!["file.txt"]);
    }

    #[test]
    fn test_parse_command_with_multiple_positional_args() {
        let cmd = parse_command("cat file1.txt file2.txt file3.txt");

        assert_eq!(cmd[0].name, "cat");
        assert_eq!(
            cmd[0].arguments,
            vec!["file1.txt", "file2.txt", "file3.txt"]
        );
    }

    #[test]
    fn test_parse_command_with_option_and_value() {
        let cmd = parse_command("grep -n pattern");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-n"], vec!["pattern"]);
    }

    #[test]
    fn test_parse_command_with_multiple_options_and_values() {
        let cmd = parse_command("grep -n pattern -A 5 -B 3");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-n"], vec!["pattern"]);
        assert_eq!(cmd[0].options["-A"], vec!["5"]);
        assert_eq!(cmd[0].options["-B"], vec!["3"]);
    }

    #[test]
    fn test_parse_command_with_repeated_option() {
        let cmd = parse_command("grep -n foo -n bar -n baz");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options.len(), 1);
        assert_eq!(cmd[0].options["-n"], vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn test_parse_command_with_repeated_option_same_value() {
        let cmd = parse_command("grep -e pattern -e pattern");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-e"], vec!["pattern", "pattern"]);
    }

    #[test]
    fn test_parse_command_option_with_numeric_value() {
        let cmd = parse_command("head -n 100");

        assert_eq!(cmd[0].name, "head");
        assert_eq!(cmd[0].options["-n"], vec!["100"]);
    }

    #[test]
    fn test_parse_command_with_long_option_no_value() {
        let cmd = parse_command("ls --all");

        assert_eq!(cmd[0].name, "ls");
        assert!(cmd[0].options.contains_key("--all"));
        assert!(cmd[0].options["--all"].is_empty());
    }

    #[test]
    fn test_parse_command_with_long_option_equals_syntax() {
        let cmd = parse_command("grep --color=auto");

        assert_eq!(cmd[0].name, "grep");
        assert!(cmd[0].options.contains_key("--color=auto"));
        assert_eq!(cmd[0].options["--color=auto"], vec![""]);
    }

    #[test]
    fn test_parse_command_with_multiple_long_options_equals() {
        let cmd = parse_command("grep --color=auto --exclude=*.log");

        assert_eq!(cmd[0].name, "grep");
        assert!(cmd[0].options.contains_key("--color=auto"));
        assert!(cmd[0].options.contains_key("--exclude=*.log"));
    }

    #[test]
    fn test_parse_command_with_long_option_and_value() {
        let cmd = parse_command("find --name test.txt");

        assert_eq!(cmd[0].name, "find");
        assert_eq!(cmd[0].options["--name"], vec!["test.txt"]);
    }

    #[test]
    fn test_parse_command_mixed_short_and_long_options() {
        let cmd = parse_command("grep -n pattern --color=auto");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-n"], vec!["pattern"]);
        assert!(cmd[0].options.contains_key("--color=auto"));
    }

    #[test]
    fn test_parse_command_option_then_args() {
        let cmd = parse_command("grep -n pattern file.txt");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-n"], vec!["pattern"]);
        assert_eq!(cmd[0].arguments, vec!["file.txt"]);
    }

    #[test]
    fn test_parse_command_args_between_options() {
        let cmd = parse_command("find . -name test.txt");

        assert_eq!(cmd[0].name, "find");
        assert_eq!(cmd[0].arguments[0], ".");
        assert_eq!(cmd[0].options["-name"], vec!["test.txt"]);
    }

    #[test]
    fn test_parse_command_multiple_options_and_args() {
        let cmd = parse_command("grep -n -i pattern file1.txt file2.txt");

        assert_eq!(cmd[0].name, "grep");
        assert!(cmd[0].options.contains_key("-n"));
        assert_eq!(cmd[0].options["-i"], vec!["pattern"]);
        assert_eq!(cmd[0].arguments, vec!["file1.txt", "file2.txt"]);
    }

    #[test]
    fn test_parse_command_double_dash_basic() {
        let cmd = parse_command("grep pattern -- file.txt");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].arguments, vec!["pattern", "file.txt"]);
    }

    #[test]
    fn test_parse_command_double_dash_treats_flags_as_args() {
        let cmd = parse_command("echo -- -n -e test");

        assert_eq!(cmd[0].name, "echo");
        assert!(cmd[0].options.is_empty());
        assert_eq!(cmd[0].arguments, vec!["-n", "-e", "test"]);
    }

    #[test]
    fn test_parse_command_double_dash_with_options_before() {
        let cmd = parse_command("grep -i pattern -- --file.txt");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-i"], vec!["pattern"]);
        assert_eq!(cmd[0].arguments, vec!["--file.txt"]);
    }

    #[test]
    fn test_parse_command_double_dash_only() {
        let cmd = parse_command("ls --");

        assert_eq!(cmd[0].name, "ls");
        assert!(cmd[0].arguments.is_empty());
    }

    #[test]
    fn test_parse_command_double_quoted_arg() {
        let cmd = parse_command(r#"echo "hello world""#);

        assert_eq!(cmd[0].name, "echo");
        assert_eq!(cmd[0].arguments, vec!["hello world"]);
    }

    #[test]
    fn test_parse_command_single_quoted_arg() {
        let cmd = parse_command("echo 'hello world'");

        assert_eq!(cmd[0].name, "echo");
        assert_eq!(cmd[0].arguments, vec!["hello world"]);
    }

    #[test]
    fn test_parse_command_mixed_quotes() {
        let cmd = parse_command(r#"echo "double" 'single' unquoted"#);

        assert_eq!(cmd[0].name, "echo");
        assert_eq!(cmd[0].arguments.len(), 3);
        assert_eq!(cmd[0].arguments[0], "double");
        assert_eq!(cmd[0].arguments[1], "single");
        assert_eq!(cmd[0].arguments[2], "unquoted");
    }

    #[test]
    fn test_parse_command_quoted_option_value() {
        let cmd = parse_command(r#"grep -e "foo|bar|baz" file.txt"#);

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-e"], vec!["foo|bar|baz"]);
        assert_eq!(cmd[0].arguments, vec!["file.txt"]);
    }

    #[test]
    fn test_parse_command_escaped_quotes() {
        let cmd = parse_command(r#"echo \"test\""#);

        assert_eq!(cmd[0].name, "echo");
        // Shlex should unescape this
        assert!(cmd[0].arguments[0].contains("test"));
    }

    #[test]
    fn test_parse_command_empty_quotes() {
        let cmd = parse_command(r#"echo """#);

        assert_eq!(cmd[0].name, "echo");
        assert_eq!(cmd[0].arguments, vec![""]);
    }

    #[test]
    fn test_parse_command_quoted_empty_option_value() {
        let cmd = parse_command(r#"grep -e "" file.txt"#);

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-e"], vec![""]);
    }

    #[test]
    fn test_parse_command_with_pipes_in_quotes() {
        let cmd = parse_command(r#"grep "foo|bar|baz""#);

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].arguments, vec!["foo|bar|baz"]);
    }

    #[test]
    fn test_parse_command_with_glob_pattern() {
        let cmd = parse_command("ls *.txt");

        assert_eq!(cmd[0].name, "ls");
        assert_eq!(cmd[0].arguments, vec!["*.txt"]);
    }

    #[test]
    fn test_parse_command_with_path_separators() {
        let cmd = parse_command("cat /path/to/file.txt");

        assert_eq!(cmd[0].name, "cat");
        assert_eq!(cmd[0].arguments, vec!["/path/to/file.txt"]);
    }

    #[test]
    fn test_parse_command_with_equals_in_value() {
        let cmd = parse_command(r#"grep -e "a=b" file.txt"#);

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-e"], vec!["a=b"]);
    }

    #[test]
    fn test_parse_command_with_spaces_in_path() {
        let cmd = parse_command(r#"cat "my file.txt""#);

        assert_eq!(cmd[0].name, "cat");
        assert_eq!(cmd[0].arguments, vec!["my file.txt"]);
    }

    #[test]
    fn test_parse_command_with_special_chars() {
        let cmd = parse_command(r#"echo "!@#$%^&*()""#);

        assert_eq!(cmd[0].name, "echo");
        assert_eq!(cmd[0].arguments, vec!["!@#$%^&*()"]);
    }

    #[test]
    fn test_parse_command_flag_followed_by_flag() {
        let cmd = parse_command("grep -i -n pattern");

        assert_eq!(cmd[0].name, "grep");
        assert!(cmd[0].options["-i"].is_empty());
        assert_eq!(cmd[0].options["-n"], vec!["pattern"]);
    }

    #[test]
    fn test_parse_command_negative_number_as_arg() {
        let cmd = parse_command("echo -42");

        assert_eq!(cmd[0].name, "echo");
        // -42 might be treated as a flag
        assert!(
            cmd[0].options.contains_key("-42") || cmd[0].arguments.contains(&"-42".to_string())
        );
    }

    #[test]
    fn test_parse_command_with_extra_whitespace() {
        let cmd = parse_command("grep   -n    pattern     file.txt");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-n"], vec!["pattern"]);
        assert_eq!(cmd[0].arguments, vec!["file.txt"]);
    }

    #[test]
    fn test_parse_command_option_order_preserved() {
        let cmd = parse_command("grep -A 5 -B 3 -C 2");

        let keys: Vec<&String> = cmd[0].options.keys().collect();
        assert_eq!(
            keys,
            vec![&"-A".to_string(), &"-B".to_string(), &"-C".to_string()]
        );
    }

    #[test]
    fn test_parse_command_many_args() {
        let cmd = parse_command("echo one two three four five six seven eight");

        assert_eq!(cmd[0].name, "echo");
        assert_eq!(cmd[0].arguments.len(), 8);
    }

    #[test]
    fn test_parse_command_option_at_end() {
        let cmd = parse_command("grep pattern file.txt -i");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].arguments, vec!["pattern", "file.txt"]);
        assert!(cmd[0].options.contains_key("-i"));
    }

    #[test]
    fn test_original_cmd_is_set() {
        let cmd = parse_command("grep -n pattern file.txt");

        assert!(!cmd[0].original_cmd.is_empty());
        assert_eq!(cmd[0].original_cmd, "grep -n pattern file.txt");
    }

    #[test]
    fn test_original_cmd_with_quotes() {
        let cmd = parse_command(r#"echo "hello world""#);

        assert!(!cmd[0].original_cmd.is_empty());
        assert_eq!(cmd[0].original_cmd, r#"echo 'hello world'"#);
    }

    #[test]
    fn test_parse_command_complex_grep() {
        let cmd = parse_command(r#"grep -n -i -A 5 -B 3 "pattern" file1.txt file2.txt"#);

        assert_eq!(cmd[0].name, "grep");
        assert!(cmd[0].options.contains_key("-n"));
        assert!(cmd[0].options["-i"].is_empty());
        assert_eq!(cmd[0].options["-A"], vec!["5"]);
        assert_eq!(cmd[0].options["-B"], vec!["3"]);
        assert_eq!(cmd[0].arguments, vec!["pattern", "file1.txt", "file2.txt"]);
    }

    #[test]
    fn test_parse_command_complex_find() {
        let cmd = parse_command(r#"find . -name "*.rs" -type f"#);

        assert_eq!(cmd[0].name, "find");
        assert_eq!(cmd[0].arguments[0], ".");
        assert_eq!(cmd[0].options["-name"], vec!["*.rs"]);
        assert_eq!(cmd[0].options["-type"], vec!["f"]);
    }

    #[test]
    fn test_parse_command_all_features() {
        let cmd = parse_command(r#"cmd -a -b val1 --long=value arg1 "quoted arg" -- -flag-as-arg"#);

        assert_eq!(cmd[0].name, "cmd");
        assert!(cmd[0].options.contains_key("-a"));
        assert_eq!(cmd[0].options["-b"], vec!["val1"]);
        assert!(cmd[0].options.contains_key("--long=value"));
        assert!(cmd[0].arguments.contains(&"arg1".to_string()));
        assert!(cmd[0].arguments.contains(&"quoted arg".to_string()));
        assert!(cmd[0].arguments.contains(&"-flag-as-arg".to_string()));
    }

    #[test]
    fn test_parse_background_job_simple() {
        // note: & is a special operator not a charactor
        let cmd = parse_command(r"sleep 10 &");
        // Should parse the sleep command
        assert_eq!(cmd[0].name, "sleep");
        assert_eq!(cmd[0].arguments, vec!["10"]);
    }

    #[test]
    fn test_parse_multiple_commands_with_ampersand() {
        let cmd = parse_command("echo hi & echo bye");
        // Should have both commands
        assert_eq!(cmd.len(), 2);
        assert_eq!(cmd[0].name, "echo");
        assert_eq!(cmd[0].arguments, vec!["hi"]);
        assert_eq!(cmd[1].name, "echo");
        assert_eq!(cmd[1].arguments, vec!["bye"]);
    }

    #[test]
    fn test_parse_background_with_options() {
        let source = "grep -n pattern file.txt &";
        let cmd = parse_command(source);
        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-n"], vec!["pattern"]);
        assert_eq!(cmd[0].arguments, vec!["file.txt"]);
    }

    #[test]
    fn test_multiple_identical_options_different_values() {
        let cmd = parse_command("grep -e foo -e bar -e baz file.txt");

        assert_eq!(cmd[0].name, "grep");
        assert_eq!(cmd[0].options["-e"], vec!["foo", "bar", "baz"]);
        assert_eq!(cmd[0].arguments, vec!["file.txt"]);
    }
}
