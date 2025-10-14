use crate::approval::ast::CommandAst;
use crate::approval::ast::SimpleAst;
use crate::approval::rules_index;
use crate::approval::shell_parser;
use std::path::Path;

/// Parse argv into an AST:
/// - Strip repeated leading `sudo`
/// - Try to unwrap shell scripts (any tool with `-c` or `-lc` pattern)
/// - Otherwise produce a single SimpleAst
pub(crate) fn parse_to_ast(argv: &[String]) -> CommandAst {
    if argv.is_empty() {
        return CommandAst::Unknown(vec![]);
    }

    // Normalize away leading `sudo` (possibly repeated)
    let mut i = 0;
    while i < argv.len() && argv[i] == "sudo" {
        i += 1;
    }
    let argv = &argv[i..];

    if argv.is_empty() {
        return CommandAst::Unknown(vec![]);
    }

    // Try to unwrap shell scripts of the form `<tool> -c|-lc <script>`. The shell parser
    // accepts executables whose basename ends with `sh` and only allows word-only
    // commands joined by safe operators. Any other construct is treated as a single,
    // opaque command.
    if argv.len() == 3
        && matches!(argv[1].as_str(), "-c" | "-lc")
        && let Some(simple_vecs) = shell_parser::parse_shell_script_commands(argv)
    {
        let simples = simple_vecs.into_iter().map(normalize_simple).collect();
        return CommandAst::Sequence(simples);
    }

    // Default: single simple command
    CommandAst::Sequence(vec![normalize_simple(argv.to_vec())])
}

/// Normalize a single argv vector into SimpleAst:
/// - Basename tool
/// - Split flags vs operands at `--`
/// - Subcommand = first non-flag token before `--`
pub fn normalize_simple(argv: Vec<String>) -> SimpleAst {
    let tool_full = argv.first().map(std::string::String::as_str).unwrap_or("");
    let tool = Path::new(tool_full)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(tool_full)
        .to_string();

    // Split flags vs operands at `--`
    let term = argv.iter().position(|s| s == "--").unwrap_or(argv.len());
    let before = &argv[1..term];

    let mut flags = Vec::new();
    let mut subcommand: Option<String> = None;
    let mut positional_operands: Vec<String> = Vec::new();

    const MAX_SHORT_FLAG_CLUSTER: usize = 4;

    for token in before {
        if token.starts_with("--") && token.len() > 2 {
            flags.push(token.clone());
            continue;
        }

        if token.starts_with('-') && token.len() > 1 {
            flags.push(token.clone());

            let tail = &token[1..];
            if tail.len() > 1
                && tail.len() <= MAX_SHORT_FLAG_CLUSTER
                && tail.chars().all(|c| c.is_ascii_alphabetic())
            {
                for c in tail.chars() {
                    flags.push(format!("-{c}"));
                }
            }
            continue;
        }

        if subcommand.is_none() && rules_index::tool_uses_subcommand(tool.as_str()) {
            subcommand = Some(token.clone());
        } else {
            positional_operands.push(token.clone());
        }
    }

    let mut operands = positional_operands;
    if term < argv.len() {
        operands.extend(argv[(term + 1)..].to_vec());
    }
    SimpleAst {
        tool,
        subcommand,
        flags,
        operands,
        raw: argv,
    }
}
