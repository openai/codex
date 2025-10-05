use tree_sitter::Node;
use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;
use indexmap::IndexMap;
use shlex::try_quote;

/// Parse the provided bash source using tree-sitter-bash, returning a Tree on
/// success or None if parsing failed.
pub fn try_parse_bash(bash_lc_arg: &str) -> Option<Tree> {
    let lang = BASH.into();
    let mut parser = Parser::new();
    #[expect(clippy::expect_used)]
    parser.set_language(&lang).expect("load bash grammar");
    let old_tree: Option<&Tree> = None;
    let tree = parser.parse(bash_lc_arg, old_tree);
    if let Some(ref t) = tree {
        print_node(t.root_node(), bash_lc_arg, 2);
    }
    tree
}

pub fn print_node(node: Node, source: &str, indent: usize) {
    let prefix = "  ".repeat(indent);
    print!("{}{}", prefix, node.kind());

    // Print text for leaf nodes
    if node.child_count() == 0 {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            println!(": '{}'", text);
        } else {
            println!();
        }
    } else {
        println!();
    }

    // Recursively print children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_node(child, source, indent + 1);
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Command {
    pub original_cmd: String,
    pub name: String,
    pub options: IndexMap<String, Vec<String>>,
    pub arguments: Vec<String>,
    pub extra: IndexMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TidiedPathParams {
    pub include_name: bool,
    pub include_options_key: bool,
    pub include_options_val: bool,
}

impl Command {
    pub(crate) fn len(&self) -> usize {
        let options_count: usize = self.options.iter()
            .map(|(key, vals)| {
                if vals.is_empty() {
                    1  
                } else {
                    vals.len() * 2                  }
            })
            .sum();
        
        self.arguments.len()
            .saturating_add(options_count)
            .saturating_add(if self.name.is_empty() { 0 } else { 1 })
    }
    
    /// Get the first value for any of the given keys
    pub(crate) fn find_option_val(&self, keys: &[&str]) -> Option<String> {
        keys.iter()
            .find_map(|key| {
                self.options.get(*key)
                    .and_then(|vals| vals.first())
                    .cloned()
            })
    }
    
    /// Get all values for any of the given keys
    pub(crate) fn find_option_vals(&self, keys: &[&str]) -> Vec<String> {
        keys.iter()
            .find_map(|key| self.options.get(*key).cloned())
            .unwrap_or_default()
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
        let keys_to_remove: Vec<String> = self.options.keys()
            .filter(|key| flags_with_vals.contains(&key.as_str()))
            .cloned()
            .collect();
        
        for key in keys_to_remove {
            self.options.remove(&key);
        }
    }
    
    pub(crate) fn get_original_cmd(&self) -> String {
        self.original_cmd.clone()
    }
    
    /// Get the tidied parts
    pub(crate) fn tidied_parts(&self, param: TidiedPathParams) -> Vec<String> {
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
                        // Already in --flag=value form
                        if param.include_options_key && param.include_options_val {
                            parts.push(format!("{}={}", key, val));
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
        
        // Positional arguments
        for arg in &self.arguments {
            parts.push(arg.clone());
        }
        
        parts
    }

    pub(crate) fn shlex_join(tokens: &[String]) -> String {
        tokens
            .iter()
            .map(|t| match t.as_str() {
                // Keep shell connectors as-is
                "|" | "&&" | "||" | ";" => t.clone(),
                // Safely quote everything else
                _ => try_quote(t)
                    .unwrap_or_else(|_| "<command included NUL byte>".into()).to_string(),
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
                !p.is_empty() && *p != "build" && *p != "dist" && *p != "node_modules" && *p != "src"
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
        use tree_sitter::Tree;
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
                    Some(BashNodeKind::Command) => {
                        if let Some(node) = self.parse_to_command(node) {
                            if let Ok(ty) = BashCommands::from_str(&node.name) {
                                match ty {
                                    $(
                                        BashCommands::$cmd => {
                                            paste::paste! {
                                                println!("visiting {}", &node.name);
                                                self.[<visit_command_ $cmd:lower>](node);
                                            }
                                        },
                                    )*
                                }
                            } else {
                                self.visit_command_unknown(node);
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

            fn get_unescaped_text(&self, node: Node) -> String {
                let text = &self.source_code()[node.start_byte()..node.end_byte()];
                
                // Try to parse as shell token, fall back to raw text
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
                let mut extra = IndexMap::new();
                let mut children = node.children(&mut cursor).peekable();
                let mut seen_double_dash = false;
                
                while let Some(child) = children.next() {
                    // Skip command name
                    if child.kind() == "command_name" {
                        continue;
                    }
                    
                    // Get the properly unescaped text
                    let text = self.get_unescaped_text(child);
                    println!("EEEEEEEEEEEEEE: {}", text);
                    
                    if seen_double_dash {
                        arguments.push(text);
                        continue;
                    }
                    
                    if text == "--" {
                        seen_double_dash = true;
                        // arguments.push(text);
                        continue;
                    }
                    
                    // Handle --flag=value
                    if text.starts_with("--") && text.contains('=') {
                        let mut parts = text.splitn(2, '=');
                        let key = parts.next().unwrap_or_default().to_string();
                        let val = parts.next().unwrap_or_default().to_string();
                        options.entry(key).or_insert_with(Vec::new).push(val);
                        continue;
                    }
                    
                    // Handle options with separate values
                    if text.starts_with('-') {
                        if let Some(next) = children.peek() {
                            let next_text = self.get_unescaped_text(*next);
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
                println!("XXXXXXXXX {}", cmd_text);
                let original_cmd: Vec<String> = match shlex::split(&cmd_text) {
                    Some(parts) => parts,
                    None => vec![cmd_text.to_string()],
                };
                
                Some(Command {
                    original_cmd: Command::shlex_join(&original_cmd),
                    name,
                    options,
                    arguments,
                    extra,
                })
            }

            fn visit_children(&mut self, node: Node<'tree>) {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.visit(child);
                }
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn parse_seq(src: &str) -> Option<Vec<Vec<String>>> {
//         let tree = try_parse_bash(src)?;
//         try_parse_word_only_commands_sequence(&tree, src)
//     }

//     #[test]
//     fn accepts_single_simple_command() {
//         let cmds = parse_seq("ls -1").unwrap();
//         assert_eq!(cmds, vec![vec!["ls".to_string(), "-1".to_string()]]);
//     }

//     #[test]
//     fn accepts_multiple_commands_with_allowed_operators() {
//         let src = "ls && pwd; echo 'hi there' | wc -l";
//         let cmds = parse_seq(src).unwrap();
//         let expected: Vec<Vec<String>> = vec![
//             vec!["ls".to_string()],
//             vec!["pwd".to_string()],
//             vec!["echo".to_string(), "hi there".to_string()],
//             vec!["wc".to_string(), "-l".to_string()],
//         ];
//         assert_eq!(cmds, expected);
//     }

//     #[test]
//     fn extracts_double_and_single_quoted_strings() {
//         let cmds = parse_seq("echo \"hello world\"").unwrap();
//         assert_eq!(
//             cmds,
//             vec![vec!["echo".to_string(), "hello world".to_string()]]
//         );

//         let cmds2 = parse_seq("echo 'hi there'").unwrap();
//         assert_eq!(
//             cmds2,
//             vec![vec!["echo".to_string(), "hi there".to_string()]]
//         );
//     }

//     #[test]
//     fn accepts_numbers_as_words() {
//         let cmds = parse_seq("echo 123 456").unwrap();
//         assert_eq!(
//             cmds,
//             vec![vec![
//                 "echo".to_string(),
//                 "123".to_string(),
//                 "456".to_string()
//             ]]
//         );
//     }

//     #[test]
//     fn rejects_parentheses_and_subshells() {
//         assert!(parse_seq("(ls)").is_none());
//         assert!(parse_seq("ls || (pwd && echo hi)").is_none());
//     }

//     #[test]
//     fn rejects_redirections_and_unsupported_operators() {
//         assert!(parse_seq("ls > out.txt").is_none());
//         assert!(parse_seq("echo hi & echo bye").is_none());
//     }

//     #[test]
//     fn rejects_command_and_process_substitutions_and_expansions() {
//         assert!(parse_seq("echo $(pwd)").is_none());
//         assert!(parse_seq("echo `pwd`").is_none());
//         assert!(parse_seq("echo $HOME").is_none());
//         assert!(parse_seq("echo \"hi $USER\"").is_none());
//     }

//     #[test]
//     fn rejects_variable_assignment_prefix() {
//         assert!(parse_seq("FOO=bar ls").is_none());
//     }

//     #[test]
//     fn rejects_trailing_operator_parse_error() {
//         assert!(parse_seq("ls &&").is_none());
//     }
// }
