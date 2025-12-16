use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use tree_sitter::Parser;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::StreamingIterator;
use tree_sitter_bash::LANGUAGE as BASH;

use crate::ApplyPatchAction;
use crate::ApplyPatchArgs;
use crate::ApplyPatchError;
use crate::ApplyPatchFileChange;
use crate::ApplyPatchFileUpdate;
use crate::IoError;
use crate::MaybeApplyPatchVerified;
use crate::parser::Hunk;
use crate::parser::ParseError;
use crate::parser::parse_patch;
use crate::unified_diff_from_chunks;
use std::str::Utf8Error;
use tree_sitter::LanguageError;

const APPLY_PATCH_COMMANDS: [&str; 2] = ["apply_patch", "applypatch"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyPatchShell {
    Unix,
    PowerShell,
    Cmd,
}

#[derive(Debug, PartialEq)]
pub enum MaybeApplyPatch {
    Body(ApplyPatchArgs),
    ShellParseError(ExtractHeredocError),
    PatchParseError(ParseError),
    NotApplyPatch,
}

#[derive(Debug, PartialEq)]
pub enum ExtractHeredocError {
    CommandDidNotStartWithApplyPatch,
    FailedToLoadBashGrammar(LanguageError),
    HeredocNotUtf8(Utf8Error),
    FailedToParsePatchIntoAst,
    FailedToFindHeredocBody,
}

fn classify_shell_name(shell: &str) -> Option<String> {
    std::path::Path::new(shell)
        .file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase)
}

fn classify_shell(shell: &str, flag: &str) -> Option<ApplyPatchShell> {
    classify_shell_name(shell).and_then(|name| match name.as_str() {
        "bash" | "zsh" | "sh" if matches!(flag, "-lc" | "-c") => Some(ApplyPatchShell::Unix),
        "pwsh" | "powershell" if flag.eq_ignore_ascii_case("-command") => {
            Some(ApplyPatchShell::PowerShell)
        }
        "cmd" if flag.eq_ignore_ascii_case("/c") => Some(ApplyPatchShell::Cmd),
        _ => None,
    })
}

fn can_skip_flag(shell: &str, flag: &str) -> bool {
    classify_shell_name(shell).is_some_and(|name| {
        matches!(name.as_str(), "pwsh" | "powershell") && flag.eq_ignore_ascii_case("-noprofile")
    })
}

fn parse_shell_script(argv: &[String]) -> Option<(ApplyPatchShell, &str)> {
    match argv {
        [shell, flag, script] => classify_shell(shell, flag).map(|shell_type| {
            let script = script.as_str();
            (shell_type, script)
        }),
        [shell, skip_flag, flag, script] if can_skip_flag(shell, skip_flag) => {
            classify_shell(shell, flag).map(|shell_type| {
                let script = script.as_str();
                (shell_type, script)
            })
        }
        _ => None,
    }
}

fn extract_apply_patch_from_shell(
    shell: ApplyPatchShell,
    script: &str,
) -> std::result::Result<(String, Option<String>), ExtractHeredocError> {
    match shell {
        ApplyPatchShell::Unix | ApplyPatchShell::PowerShell | ApplyPatchShell::Cmd => {
            extract_apply_patch_from_bash(script)
        }
    }
}

// TODO: make private once we remove tests in lib.rs
pub fn maybe_parse_apply_patch(argv: &[String]) -> MaybeApplyPatch {
    match argv {
        // Direct invocation: apply_patch <patch>
        [cmd, body] if APPLY_PATCH_COMMANDS.contains(&cmd.as_str()) => match parse_patch(body) {
            Ok(source) => MaybeApplyPatch::Body(source),
            Err(e) => MaybeApplyPatch::PatchParseError(e),
        },
        // Shell heredoc form: (optional `cd <path> &&`) apply_patch <<'EOF' ...
        _ => match parse_shell_script(argv) {
            Some((shell, script)) => match extract_apply_patch_from_shell(shell, script) {
                Ok((body, workdir)) => match parse_patch(&body) {
                    Ok(mut source) => {
                        source.workdir = workdir;
                        MaybeApplyPatch::Body(source)
                    }
                    Err(e) => MaybeApplyPatch::PatchParseError(e),
                },
                Err(ExtractHeredocError::CommandDidNotStartWithApplyPatch) => {
                    MaybeApplyPatch::NotApplyPatch
                }
                Err(e) => MaybeApplyPatch::ShellParseError(e),
            },
            None => MaybeApplyPatch::NotApplyPatch,
        },
    }
}

/// cwd must be an absolute path so that we can resolve relative paths in the
/// patch.
pub fn maybe_parse_apply_patch_verified(argv: &[String], cwd: &Path) -> MaybeApplyPatchVerified {
    // Detect a raw patch body passed directly as the command or as the body of a shell
    // script. In these cases, report an explicit error rather than applying the patch.
    if let [body] = argv
        && parse_patch(body).is_ok()
    {
        return MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation);
    }
    if let Some((_, script)) = parse_shell_script(argv)
        && parse_patch(script).is_ok()
    {
        return MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation);
    }

    match maybe_parse_apply_patch(argv) {
        MaybeApplyPatch::Body(ApplyPatchArgs {
            patch,
            hunks,
            workdir,
        }) => {
            let effective_cwd = workdir
                .as_ref()
                .map(|dir| {
                    let path = Path::new(dir);
                    if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        cwd.join(path)
                    }
                })
                .unwrap_or_else(|| cwd.to_path_buf());
            let mut changes = HashMap::new();
            for hunk in hunks {
                let path = hunk.resolve_path(&effective_cwd);
                match hunk {
                    Hunk::AddFile { contents, .. } => {
                        changes.insert(path, ApplyPatchFileChange::Add { content: contents });
                    }
                    Hunk::DeleteFile { .. } => {
                        let content = match std::fs::read_to_string(&path) {
                            Ok(content) => content,
                            Err(e) => {
                                return MaybeApplyPatchVerified::CorrectnessError(
                                    ApplyPatchError::IoError(IoError {
                                        context: format!("Failed to read {}", path.display()),
                                        source: e,
                                    }),
                                );
                            }
                        };
                        changes.insert(path, ApplyPatchFileChange::Delete { content });
                    }
                    Hunk::UpdateFile {
                        move_path, chunks, ..
                    } => {
                        let ApplyPatchFileUpdate {
                            unified_diff,
                            content: contents,
                        } = match unified_diff_from_chunks(&path, &chunks) {
                            Ok(diff) => diff,
                            Err(e) => {
                                return MaybeApplyPatchVerified::CorrectnessError(e);
                            }
                        };
                        changes.insert(
                            path,
                            ApplyPatchFileChange::Update {
                                unified_diff,
                                move_path: move_path.map(|p| effective_cwd.join(p)),
                                new_content: contents,
                            },
                        );
                    }
                }
            }
            MaybeApplyPatchVerified::Body(ApplyPatchAction {
                changes,
                patch,
                cwd: effective_cwd,
            })
        }
        MaybeApplyPatch::ShellParseError(e) => MaybeApplyPatchVerified::ShellParseError(e),
        MaybeApplyPatch::PatchParseError(e) => MaybeApplyPatchVerified::CorrectnessError(e.into()),
        MaybeApplyPatch::NotApplyPatch => MaybeApplyPatchVerified::NotApplyPatch,
    }
}

/// Extract the heredoc body (and optional `cd` workdir) from a `bash -lc` script
/// that invokes the apply_patch tool using a heredoc.
///
/// Supported top‑level forms (must be the only top‑level statement):
/// - `apply_patch <<'EOF'\n...\nEOF`
/// - `cd <path> && apply_patch <<'EOF'\n...\nEOF`
///
/// Notes about matching:
/// - Parsed with Tree‑sitter Bash and a strict query that uses anchors so the
///   heredoc‑redirected statement is the only top‑level statement.
/// - The connector between `cd` and `apply_patch` must be `&&` (not `|` or `||`).
/// - Exactly one positional `word` argument is allowed for `cd` (no flags, no quoted
///   strings, no second argument).
/// - The apply command is validated in‑query via `#any-of?` to allow `apply_patch`
///   or `applypatch`.
/// - Preceding or trailing commands (e.g., `echo ...;` or `... && echo done`) do not match.
///
/// Returns `(heredoc_body, Some(path))` when the `cd` variant matches, or
/// `(heredoc_body, None)` for the direct form. Errors are returned if the script
/// cannot be parsed or does not match the allowed patterns.
fn extract_apply_patch_from_bash(
    src: &str,
) -> std::result::Result<(String, Option<String>), ExtractHeredocError> {
    // This function uses a Tree-sitter query to recognize one of two
    // whole-script forms, each expressed as a single top-level statement:
    //
    // 1. apply_patch <<'EOF'\n...\nEOF
    // 2. cd <path> && apply_patch <<'EOF'\n...\nEOF
    //
    // Key ideas when reading the query:
    // - dots (`.`) between named nodes enforces adjacency among named children and
    //   anchor to the start/end of the expression.
    // - we match a single redirected_statement directly under program with leading
    //   and trailing anchors (`.`). This ensures it is the only top-level statement
    //   (so prefixes like `echo ...;` or suffixes like `... && echo done` do not match).
    //
    // Overall, we want to be conservative and only match the intended forms, as other
    // forms are likely to be model errors, or incorrectly interpreted by later code.
    //
    // If you're editing this query, it's helpful to start by creating a debugging binary
    // which will let you see the AST of an arbitrary bash script passed in, and optionally
    // also run an arbitrary query against the AST. This is useful for understanding
    // how tree-sitter parses the script and whether the query syntax is correct. Be sure
    // to test both positive and negative cases.
    static APPLY_PATCH_QUERY: LazyLock<Query> = LazyLock::new(|| {
        let language = BASH.into();
        #[expect(clippy::expect_used)]
        Query::new(
            &language,
            r#"
            (
              program
                . (redirected_statement
                    body: (command
                            name: (command_name (word) @apply_name) .)
                    (#any-of? @apply_name "apply_patch" "applypatch")
                    redirect: (heredoc_redirect
                                . (heredoc_start)
                                . (heredoc_body) @heredoc
                                . (heredoc_end)
                                .))
                .)

            (
              program
                . (redirected_statement
                    body: (list
                            . (command
                                name: (command_name (word) @cd_name) .
                                argument: [
                                  (word) @cd_path
                                  (string (string_content) @cd_path)
                                  (raw_string) @cd_raw_string
                                ] .)
                            "&&"
                            . (command
                                name: (command_name (word) @apply_name))
                            .)
                    (#eq? @cd_name "cd")
                    (#any-of? @apply_name "apply_patch" "applypatch")
                    redirect: (heredoc_redirect
                                . (heredoc_start)
                                . (heredoc_body) @heredoc
                                . (heredoc_end)
                                .))
                .)
            "#,
        )
        .expect("valid bash query")
    });

    let lang = BASH.into();
    let mut parser = Parser::new();
    parser
        .set_language(&lang)
        .map_err(ExtractHeredocError::FailedToLoadBashGrammar)?;
    let tree = parser
        .parse(src, None)
        .ok_or(ExtractHeredocError::FailedToParsePatchIntoAst)?;

    let bytes = src.as_bytes();
    let root = tree.root_node();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&APPLY_PATCH_QUERY, root, bytes);
    while let Some(m) = matches.next() {
        let mut heredoc_text: Option<String> = None;
        let mut cd_path: Option<String> = None;

        for capture in m.captures.iter() {
            let name = APPLY_PATCH_QUERY.capture_names()[capture.index as usize];
            match name {
                "heredoc" => {
                    let text = capture
                        .node
                        .utf8_text(bytes)
                        .map_err(ExtractHeredocError::HeredocNotUtf8)?
                        .trim_end_matches('\n')
                        .to_string();
                    heredoc_text = Some(text);
                }
                "cd_path" => {
                    let text = capture
                        .node
                        .utf8_text(bytes)
                        .map_err(ExtractHeredocError::HeredocNotUtf8)?
                        .to_string();
                    cd_path = Some(text);
                }
                "cd_raw_string" => {
                    let raw = capture
                        .node
                        .utf8_text(bytes)
                        .map_err(ExtractHeredocError::HeredocNotUtf8)?;
                    let trimmed = raw
                        .strip_prefix('\'')
                        .and_then(|s| s.strip_suffix('\''))
                        .unwrap_or(raw);
                    cd_path = Some(trimmed.to_string());
                }
                _ => {}
            }
        }

        if let Some(heredoc) = heredoc_text {
            return Ok((heredoc, cd_path));
        }
    }

    Err(ExtractHeredocError::CommandDidNotStartWithApplyPatch)
}
