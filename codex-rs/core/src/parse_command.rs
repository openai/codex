use crate::bash::Command;
use crate::bash::TidiedPartsParam;
use crate::bash::try_parse_bash;
use crate::define_bash_commands;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum ParsedCommand {
    Read {
        cmd: String,
        name: String,
    },
    ListFiles {
        cmd: String,
        path: Option<String>,
    },
    Search {
        cmd: String,
        query: Option<String>,
        path: Option<String>,
    },
    Unknown {
        cmd: String,
    },
}

// Convert core's parsed command enum into the protocol's simplified type so
// events can carry the canonical representation across process boundaries.
impl From<ParsedCommand> for codex_protocol::parse_command::ParsedCommand {
    fn from(v: ParsedCommand) -> Self {
        use codex_protocol::parse_command::ParsedCommand as P;
        match v {
            ParsedCommand::Read { cmd, name } => P::Read { cmd, name },
            ParsedCommand::ListFiles { cmd, path } => P::ListFiles { cmd, path },
            ParsedCommand::Search { cmd, query, path } => P::Search { cmd, query, path },
            ParsedCommand::Unknown { cmd } => P::Unknown { cmd },
        }
    }
}

/// Parses metadata out of an arbitrary command.
/// These commands are model driven and could include just about anything.
/// The parsing is slightly lossy due to the ~infinite expressiveness of an arbitrary command.
/// The goal of the parsed metadata is to be able to provide the user with a human readable gis
/// of what it is doing.
pub fn parse_command(command: &[String]) -> Vec<ParsedCommand> {
    let script = if command.len() >= 3 && command[0] == "bash" && command[1] == "-lc" {
        &command[2]
    } else if !command.is_empty() {
        &Command::shlex_join(&command[0..])
    } else {
        return Vec::new();
    };

    let mut p = BashCommandParser::new();
    if let Some(parsed) = p.parse(script) {
        let mut deduped: Vec<ParsedCommand> = Vec::with_capacity(parsed.len());
        for cmd in parsed.into_iter() {
            if deduped.last().is_some_and(|prev| prev == &cmd) {
                continue;
            }
            deduped.push(cmd);
        }
        deduped
    } else {
        // return empty vec to maintain the interface
        Vec::new()
    }
}

/// Returns flattened string vectors
pub fn parse_command_as_tokens(original: &[String]) -> Option<Vec<Vec<String>>> {
    let script = if original.len() >= 3 && original[0] == "bash" && original[1] == "-lc" {
        &original[2]
    } else if !original.is_empty() {
        &Command::shlex_join(&original[0..])
    } else {
        return None;
    };

    let mut p = BashCommandParser::new();
    p.parse(script);
    let commands = p.get_origin_commands();
    Some(
        commands
            .iter()
            .filter_map(|c| shlex::split(&c.original_cmd))
            .collect::<Vec<Vec<String>>>(),
    )
}

#[rustfmt::skip]
define_bash_commands!(
    Unknown, 
    Ls, 
    Rg, 
    Fd, 
    Find, 
    Sed, 
    Grep, 
    Cd,
    Echo,
    Nl,
    Head,
    Tail,
    Cat,
    Wc,
    Tr,
    Cut,
    Sort,
    Uniq,
    Xargs,
    Tree,
    Column,
    Awk,
    Yes,
    Printf,
    True,
);

#[derive(Debug)]
struct BashCommandParser {
    /// The source command text to parse from
    src: Option<String>,
    /// Reject the whole command or script
    reject_whole: bool,
    /// Skip the rest command for list and pipeline
    skip_rest: bool,
    /// Save the prased command
    parsed_commands: Vec<ParsedCommand>,
    /// Save the original command
    origin_commands: Vec<Command>,
}

impl BashCommandParser {
    pub fn new() -> Self {
        Self {
            src: None,
            reject_whole: false,
            skip_rest: false,
            parsed_commands: Vec::new(),
            origin_commands: Vec::new(),
        }
    }

    pub fn parse(&mut self, src: &str) -> Option<Vec<ParsedCommand>> {
        self.src = Some(src.to_owned());
        let tree = try_parse_bash(src);
        if let Some(tree) = tree {
            let root = tree.root_node();
            self.visit(root);
        }
        if self.reject_whole {
            None
        } else {
            Some(self.parsed_commands.clone())
        }
    }

    pub fn get_origin_commands(&self) -> Vec<Command> {
        if self.reject_whole {
            Vec::new()
        } else {
            self.origin_commands.clone()
        }
    }

    fn reject_node(&mut self, node: Node) {
        if self
            .find_child_by_kind(node, "variable_assignment")
            .is_some()
        {
            self.reject_whole = true;
        }

        if self
            .find_child_by_kind(node, "command_substitution")
            .is_some()
        {
            self.reject_whole = true;
        }

        if self.find_child_by_kind(node, "simple_expansion").is_some() {
            self.reject_whole = true;
        }
    }
}

#[allow(unused_mut)]
impl<'a> NodeVisitor<'a> for BashCommandParser {
    fn source_code(&self) -> &str {
        // SAFE unwrap
        self.src.as_deref().unwrap_or("")
    }

    fn visit_subshell(&mut self, _node: Node<'a>) {
        self.reject_whole = true;
    }

    fn visit_enter_command(&mut self, node: Node<'a>) {
        self.reject_node(node);
    }

    fn visit_redirected_statement(&mut self, node: Node<'a>) {
        self.parsed_commands.push(ParsedCommand::Unknown {
            cmd: self.source_code()[node.start_byte()..node.end_byte()].to_owned(),
        });
    }

    fn visit_children(&mut self, node: Node<'a>) {
        self.reject_node(node);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.reject_whole || self.skip_rest {
                return;
            }
            self.visit(child);
        }
    }

    fn visit_command_ls(&mut self, mut cmd: Command) {
        let main_cmd = cmd.get_original_cmd();
        cmd.skip_flag_values(&[
            "-I",
            "-w",
            "--block-size",
            "--format",
            "--time-style",
            "--color",
            "--quoting-style",
        ]);
        let non_flags_param = TidiedPartsParam {
            include_options_val: true,
            include_argument: true,
            ..TidiedPartsParam::default()
        };
        let non_flags = cmd.get_tidied_parts(non_flags_param);
        let path = non_flags.first().cloned();
        self.origin_commands.push(cmd.clone());
        self.parsed_commands.push(ParsedCommand::ListFiles {
            cmd: main_cmd,
            path: cmd.short_display_path(path),
        });
    }

    fn visit_command_rg(&mut self, mut cmd: Command) {
        let has_files_flag = cmd.options.contains_key("--files");
        let non_flags_param = TidiedPartsParam {
            include_options_val: true,
            include_argument: true,
            ..TidiedPartsParam::default()
        };
        let non_flags = cmd.get_tidied_parts(non_flags_param);
        let (query, path) = if has_files_flag {
            (None, non_flags.first().map(String::from))
        } else {
            (
                non_flags.first().map(String::from),
                non_flags.get(1).cloned(),
            )
        };
        self.origin_commands.push(cmd.clone());
        self.parsed_commands.push(ParsedCommand::Search {
            cmd: cmd.get_original_cmd(),
            query,
            path: cmd.short_display_path(path),
        });
    }

    fn visit_command_fd(&mut self, mut cmd: Command) {
        let main_cmd = cmd.get_original_cmd();
        cmd.skip_flag_values(&[
            "-t",
            "--type",
            "-e",
            "--extension",
            "-E",
            "--exclude",
            "--search-path",
        ]);
        let non_flags_param = TidiedPartsParam {
            include_options_val: true,
            include_argument: true,
            ..TidiedPartsParam::default()
        };
        let non_flags = cmd.get_tidied_parts(non_flags_param);
        fn is_pathish(s: &str) -> bool {
            s == "."
                || s == ".."
                || s.starts_with("./")
                || s.starts_with("../")
                || s.contains('/')
                || s.contains('\\')
        }
        let (query, path) = match non_flags.as_slice() {
            [one] => {
                if is_pathish(one) {
                    (None, Some(one.clone()))
                } else {
                    (Some((*one).clone()), None)
                }
            }
            [q, p, ..] => (Some((*q).clone()), Some(p.clone())),
            _ => (None, None),
        };
        self.origin_commands.push(cmd.clone());
        self.parsed_commands.push(ParsedCommand::Search {
            cmd: main_cmd,
            query,
            path: cmd.short_display_path(path),
        });
    }

    fn visit_command_find(&mut self, mut cmd: Command) {
        let main_cmd = cmd.get_original_cmd();
        let path = cmd.arguments.first().cloned();
        let query = cmd.find_option_val(&["-name", "-iname", "-path", "-regex"]);

        self.origin_commands.push(cmd.clone());
        self.parsed_commands.push(ParsedCommand::Search {
            cmd: main_cmd,
            query,
            path: cmd.short_display_path(path),
        });
    }

    fn visit_command_grep(&mut self, mut cmd: Command) {
        let main_cmd = cmd.get_original_cmd();
        let non_flags_param = TidiedPartsParam {
            include_options_val: true,
            include_argument: true,
            ..TidiedPartsParam::default()
        };
        let non_flags = cmd.get_tidied_parts(non_flags_param);
        let query = non_flags.first().cloned();
        let path = non_flags.get(1).cloned();
        self.origin_commands.push(cmd.clone());
        self.parsed_commands.push(ParsedCommand::Search {
            cmd: main_cmd,
            query,
            path: cmd.short_display_path(path),
        });
    }

    fn visit_command_cat(&mut self, mut cmd: Command) {
        let main_cmd = cmd.get_original_cmd();
        self.origin_commands.push(cmd.clone());
        // Support both `cat <file>` and `cat -- <file>` forms.
        if cmd.arguments.len() == 1 {
            let file = cmd.arguments.first().cloned();
            self.parsed_commands.push(ParsedCommand::Read {
                cmd: main_cmd,
                name: cmd.short_display_path(file).unwrap_or_default(),
            });
        } else {
            self.visit_command_unknown(cmd);
        }
    }

    fn visit_command_head(&mut self, mut cmd: Command) {
        self.origin_commands.push(cmd.clone());
        if cmd.len() < 3 {
            return;
        }
        let main_cmd = cmd.get_original_cmd();

        // Handle both `-n 50`, `-n+50`, `-n50`, and `--lines 50` forms
        let n_value = cmd.find_option_val_strip_key(&["-n", "--lines"]);

        let valid_n = n_value.as_ref().is_some_and(|n| {
            let s = n.strip_prefix('+').unwrap_or(n);
            !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
        });

        let non_flags_param = TidiedPartsParam {
            include_options_val: true,
            include_argument: true,
            ..TidiedPartsParam::default()
        };
        let non_flags = cmd.get_tidied_parts(non_flags_param);
        if valid_n 
            && let Some(p) = non_flags.last() 
            // assume a file shouldn't be named just with digits
            && !p.chars().all(|c| c.is_ascii_digit())
        {
            let name = Some((*p).clone());
            self.parsed_commands.push(ParsedCommand::Read {
                cmd: main_cmd,
                name: cmd.short_display_path(name).unwrap_or_default(),
            });
        } else {
            self.visit_command_unknown(cmd);
        }
    }

    fn visit_command_tail(&mut self, mut cmd: Command) {
        self.visit_command_head(cmd);
    }

    fn visit_command_nl(&mut self, mut cmd: Command) {
        let main_cmd = cmd.get_original_cmd();
        self.origin_commands.push(cmd.clone());

        cmd.skip_flag_values(&["-s", "-w", "-v", "-i", "-b"]);
        let non_flags_param = TidiedPartsParam {
            include_options_val: true,
            include_argument: true,
            ..TidiedPartsParam::default()
        };
        let non_flags = cmd.get_tidied_parts(non_flags_param);
        if !non_flags.is_empty() {
            let name = non_flags.first().cloned();
            self.parsed_commands.push(ParsedCommand::Read {
                cmd: main_cmd,
                name: cmd.short_display_path(name).unwrap_or_default(),
            });
        }
    }

    fn visit_command_sed(&mut self, mut cmd: Command) {
        self.origin_commands.push(cmd.clone());
        if cmd.len() < 4 {
            return;
        }

        // Look up the value of the -n option (e.g., "1,10p")
        let n_val = cmd.find_option_val_strip_key(&["-n"]);

        if let Some(val) = n_val
            && is_valid_sed_n_arg(Some(&val))
        {
            // The first argument is the target file
            let file_path = cmd.arguments.first().cloned();
            let name = cmd.short_display_path(file_path).unwrap_or_default();

            self.parsed_commands.push(ParsedCommand::Read {
                cmd: cmd.get_original_cmd(),
                name,
            });
            return;
        }

        self.visit_command_unknown(cmd);
    }

    fn visit_command_echo(&mut self, mut cmd: Command) {
        self.origin_commands.push(cmd.clone());
        self.parsed_commands.push(ParsedCommand::Unknown {
            cmd: cmd.get_original_cmd(),
        });
    }

    /// Any known commands(eg. cd) not listed here are ignored by default
    /// Any unknown command will go to this function
    fn visit_command_unknown(&mut self, mut cmd: Command) {
        self.origin_commands.push(cmd.clone());
        self.parsed_commands.push(ParsedCommand::Unknown {
            cmd: cmd.get_original_cmd(),
        });
    }
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
/// Tests are at the top to encourage using TDD + Codex to fix the implementation.
mod tests {
    use super::*;
    use crate::bash::Command;
    use pretty_assertions::assert_eq;
    use shlex::split as shlex_split;
    use std::string::ToString;

    fn shlex_join(tokens: &[String]) -> String {
        Command::shlex_join(tokens)
    }

    fn shlex_split_safe(s: &str) -> Vec<String> {
        shlex_split(s).unwrap_or_else(|| s.split_whitespace().map(ToString::to_string).collect())
    }

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    fn assert_parsed(args: &[String], expected: Vec<ParsedCommand>) {
        let out = parse_command(args);
        assert_eq!(out, expected);
    }

    fn assert_parsed_none(args: &[String]) {
        assert!(parse_command(args).is_empty())
    }

    #[test]
    fn git_status_is_unknown() {
        assert_parsed(
            &vec_str(&["git", "status"]),
            vec![ParsedCommand::Unknown {
                cmd: "git status".to_string(),
            }],
        );
    }

    #[test]
    fn handles_git_pipe_wc() {
        let inner = "git status | wc -l";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Unknown {
                cmd: "git status".to_string(),
            }],
        );
    }

    #[test]
    fn bash_lc_redirect_not_quoted() {
        let inner = "echo foo > bar";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Unknown {
                cmd: "echo foo > bar".to_string(),
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
                    cmd: "rg --version".to_string(),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: "node -v".to_string(),
                },
                ParsedCommand::Unknown {
                    cmd: "pnpm -v".to_string(),
                },
                ParsedCommand::Search {
                    cmd: "rg --files".to_string(),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: "head -n 40".to_string(),
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
                cmd: "rg -n navigate-to-route -S".to_string(),
                query: Some("navigate-to-route".to_string()),
                path: None,
            }],
        );
        Ok(())
    }

    #[test]
    fn handles_complex_bash_command_two() {
        let inner = "rg -n \"BUG|FIXME|TODO|XXX|HACK\" -S | head -n 200";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![
                ParsedCommand::Search {
                    cmd: "rg -n 'BUG|FIXME|TODO|XXX|HACK' -S".to_string(),
                    query: Some("BUG|FIXME|TODO|XXX|HACK".to_string()),
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: "head -n 200".to_string(),
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
                cmd: "rg --files webview/src".to_string(),
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
                    cmd: "rg --files".to_string(),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: "head -n 50".to_string(),
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
                cmd: inner.to_string(),
                name: "README.md".to_string(),
            }],
        );
    }

    #[test]
    fn cd_then_cat_is_single_read() {
        assert_parsed(
            &shlex_split_safe("cd foo && cat foo.txt"),
            vec![ParsedCommand::Read {
                cmd: "cat foo.txt".to_string(),
                name: "foo.txt".to_string(),
            }],
        );
    }

    #[test]
    fn bash_cd_then_bar_is_same_as_bar() {
        // Ensure a leading `cd` inside bash -lc is dropped when followed by another command.
        assert_parsed(
            &shlex_split_safe("bash -lc 'cd foo && bar'"),
            vec![ParsedCommand::Unknown {
                cmd: "bar".to_string(),
            }],
        );
    }

    #[test]
    fn bash_cd_then_cat_is_read() {
        assert_parsed(
            &shlex_split_safe("bash -lc 'cd foo && cat foo.txt'"),
            vec![ParsedCommand::Read {
                cmd: "cat foo.txt".to_string(),
                name: "foo.txt".to_string(),
            }],
        );
    }

    #[test]
    fn supports_ls_with_pipe() {
        let inner = "ls -la | sed -n '1,120p'";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::ListFiles {
                cmd: "ls -la".to_string(),
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
                cmd: inner.to_string(),
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
                cmd: "cat tui/Cargo.toml".to_string(),
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
                cmd: inner.to_string(),
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
                cmd: inner.to_string(),
                name: "README.md".to_string(),
            }]
        );
    }

    #[test]
    fn supports_npm_run_build_is_unknown() {
        assert_parsed(
            &vec_str(&["npm", "run", "build"]),
            vec![ParsedCommand::Unknown {
                cmd: "npm run build".to_string(),
            }],
        );
    }

    #[test]
    fn supports_grep_recursive_current_dir() {
        assert_parsed(
            &vec_str(&["grep", "-R", "CODEX_SANDBOX_ENV_VAR", "-n", "."]),
            vec![ParsedCommand::Search {
                cmd: "grep -R CODEX_SANDBOX_ENV_VAR -n .".to_string(),
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
                cmd: "grep -R CODEX_SANDBOX_ENV_VAR -n core/src/spawn.rs".to_string(),
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
                cmd: "grep -R src/main.rs -n .".to_string(),
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
                cmd: "grep -R 'COD`EX_SANDBOX' -n".to_string(),
                query: Some("COD`EX_SANDBOX".to_string()),
                path: None,
            }],
        );
    }

    #[test]
    fn supports_cd_and_rg_files() {
        assert_parsed(
            &shlex_split_safe("cd codex-rs && rg --files"),
            vec![ParsedCommand::Search {
                cmd: "rg --files".to_string(),
                query: None,
                path: None,
            }],
        );
    }

    #[test]
    fn supports_nl_then_sed_reading() {
        let inner = "nl -ba core/src/parse_command.rs | sed -n '1200,1720p'";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner]),
            vec![ParsedCommand::Read {
                cmd: "nl -ba core/src/parse_command.rs".to_string(),
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
                cmd: inner.to_string(),
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
                cmd: "cat -- ansi-escape/Cargo.toml".to_string(),
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
                cmd: "rg --files".to_string(),
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
                cmd: "sed -n '260,640p' exec/src/event_processor_with_human_output.rs".to_string(),
                name: "event_processor_with_human_output.rs".to_string(),
            }],
        );
    }

    #[test]
    fn preserves_rg_with_spaces() {
        assert_parsed(
            &shlex_split_safe("yes | rg -n 'foo bar' -S"),
            vec![ParsedCommand::Search {
                cmd: "rg -n 'foo bar' -S".to_string(),
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
                cmd: "ls -I '*.test.js'".to_string(),
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
                    cmd: "rg foo".to_string(),
                    query: Some("foo".to_string()),
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: "echo done".to_string(),
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
                    cmd: "rg foo".to_string(),
                    query: Some("foo".to_string()),
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: "echo done".to_string(),
                },
            ],
        );
    }

    #[test]
    fn parses_mixed_sequence_with_pipes_semicolons_and_or() {
        // Provided long command sequence combining sequencing, pipelines, and ORs.
        let inner = "pwd; ls -la; rg --files -g '!target' | wc -l; rg -n '^\\[workspace\\]' -n Cargo.toml || true; rg -n '^\\[package\\]' -n */Cargo.toml || true; cargo --version; rustc --version; cargo clippy --workspace --all-targets --all-features -q";
        let args = vec_str(&["bash", "-lc", inner]);

        let expected = vec![
            ParsedCommand::Unknown {
                cmd: "pwd".to_string(),
            },
            ParsedCommand::ListFiles {
                cmd: shlex_join(&shlex_split_safe("ls -la")),
                path: None,
            },
            ParsedCommand::Search {
                cmd: shlex_join(&shlex_split_safe("rg --files -g '!target'")),
                query: None,
                path: Some("!target".to_string()),
            },
            ParsedCommand::Search {
                cmd: shlex_join(&shlex_split_safe("rg -n '^\\[workspace\\]' -n Cargo.toml")),
                query: Some("^\\[workspace\\]".to_string()),
                path: Some("Cargo.toml".to_string()),
            },
            ParsedCommand::Search {
                cmd: shlex_join(&shlex_split_safe("rg -n '^\\[package\\]' -n */Cargo.toml")),
                query: Some("^\\[package\\]".to_string()),
                path: Some("Cargo.toml".to_string()),
            },
            ParsedCommand::Unknown {
                cmd: shlex_join(&shlex_split_safe("cargo --version")),
            },
            ParsedCommand::Unknown {
                cmd: shlex_join(&shlex_split_safe("rustc --version")),
            },
            ParsedCommand::Unknown {
                cmd: shlex_join(&shlex_split_safe(
                    "cargo clippy --workspace --all-targets --all-features -q",
                )),
            },
        ];

        assert_parsed(&args, expected);
    }

    #[test]
    fn strips_true_in_sequence() {
        // `true` should be dropped from parsed sequences
        assert_parsed(
            &shlex_split_safe("true && rg --files"),
            vec![ParsedCommand::Search {
                cmd: "rg --files".to_string(),
                query: None,
                path: None,
            }],
        );

        assert_parsed(
            &shlex_split_safe("rg --files && true"),
            vec![ParsedCommand::Search {
                cmd: "rg --files".to_string(),
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
                cmd: "rg --files".to_string(),
                query: None,
                path: None,
            }],
        );

        let inner2 = "rg --files || true";
        assert_parsed(
            &vec_str(&["bash", "-lc", inner2]),
            vec![ParsedCommand::Search {
                cmd: "rg --files".to_string(),
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
                cmd: r#"cat "pkg\\src\\main.rs""#.to_string(),
                name: "main.rs".to_string(),
            }],
        );
    }

    #[test]
    fn head_with_no_space() {
        assert_parsed(
            &shlex_split_safe("bash -lc 'head -n50 Cargo.toml'"),
            vec![ParsedCommand::Read {
                cmd: "head -n50 Cargo.toml".to_string(),
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
                    cmd: "rg --files".to_string(),
                    query: None,
                    path: None,
                },
                ParsedCommand::Unknown {
                    cmd: "head -n 1".to_string(),
                },
            ],
        );
    }

    #[test]
    fn tail_with_no_space() {
        assert_parsed(
            &shlex_split_safe("bash -lc 'tail -n+10 README.md'"),
            vec![ParsedCommand::Read {
                cmd: "tail -n+10 README.md".to_string(),
                name: "README.md".to_string(),
            }],
        );
    }

    #[test]
    fn grep_with_query_and_path() {
        assert_parsed(
            &shlex_split_safe("grep -R TODO src"),
            vec![ParsedCommand::Search {
                cmd: "grep -R TODO src".to_string(),
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
                cmd: "rg '--colors=never' -n foo src".to_string(),
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
                cmd: "cat -- ./-strange-file-name".to_string(),
                name: "-strange-file-name".to_string(),
            }],
        );

        // sed -n <range> <file> should be treated as a read of <file>
        assert_parsed(
            &shlex_split_safe("sed -n '12,20p' Cargo.toml"),
            vec![ParsedCommand::Read {
                cmd: "sed -n '12,20p' Cargo.toml".to_string(),
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
                cmd: "rg --files".to_string(),
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
                cmd: "ls '--time-style=long-iso' ./dist".to_string(),
                // short_display_path drops "dist" and shows "." as the last useful segment
                path: Some(".".to_string()),
            }],
        );
    }

    #[test]
    fn fd_file_finder_variants() {
        assert_parsed(
            &shlex_split_safe("fd -t f src/"),
            vec![ParsedCommand::Search {
                cmd: "fd -t f src/".to_string(),
                query: None,
                path: Some("src".to_string()),
            }],
        );

        // fd with query and path should capture both
        assert_parsed(
            &shlex_split_safe("fd main src"),
            vec![ParsedCommand::Search {
                cmd: "fd main src".to_string(),
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
                cmd: "find . -name '*.rs'".to_string(),
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
                cmd: "find src -type f".to_string(),
                query: None,
                path: Some("src".to_string()),
            }],
        );
    }

    #[test]
    fn cd_followed_by_git() {
        assert_parsed(
            &shlex_split_safe("cd codex && git rev-parse --show-toplevel"),
            vec![ParsedCommand::Unknown {
                cmd: "git rev-parse --show-toplevel".to_string(),
            }],
        );
    }

    #[test]
    fn extracts_double_and_single_quoted_strings() {
        assert_parsed(
            &shlex_split_safe("echo \"hello world\""),
            vec![ParsedCommand::Unknown {
                cmd: "echo 'hello world'".to_string(),
            }],
        );

        assert_parsed(
            &shlex_split_safe("echo 'hello there'"),
            vec![ParsedCommand::Unknown {
                cmd: "echo 'hello there'".to_string(),
            }],
        );
    }

    #[test]
    fn rejects_parentheses_and_subshells() {
        // NOTE: shlex split not work on parenthes (), it can not correctly
        // convert to a Vec<string> such that can be converted to AST
        //
        // so this will not work:
        // let inner = "ls || (pwd && echo hi)";
        // assert_parsed_none(
        //     &&shlex_split_safe(inner)
        // );

        let inner = "ls || (pwd && echo hi)";
        assert_parsed_none(&vec_str(&["bash", "-lc", inner]));

        let inner = "(ls)";
        assert_parsed_none(&vec_str(&["bash", "-lc", inner]));
    }

    #[test]
    fn rejects_command_and_process_substitutions_and_expansions() {
        assert_parsed_none(&vec_str(&["bash", "-lc", "echo $(pwd)"]));

        assert_parsed_none(&vec_str(&["bash", "-lc", "echo `pwd`"]));
        assert_parsed_none(&vec_str(&["bash", "-lc", "echo $HOME"]));
        assert_parsed_none(&vec_str(&["bash", "-lc", r#"echo "hi $USER""#]));
    }

    #[test]
    fn rejects_variable_assignment_prefix() {
        assert_parsed_none(&vec_str(&["bash", "-lc", "FOO=bar ls"]));
    }
}
