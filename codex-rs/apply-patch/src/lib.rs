mod parser;
mod seek_sequence;
mod standalone_executable;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::str::Utf8Error;
use std::sync::LazyLock;

use anyhow::Context;
use anyhow::Result;
pub use parser::Hunk;
pub use parser::ParseError;
use parser::ParseError::*;
use parser::UpdateFileChunk;
pub use parser::parse_patch;
use similar::TextDiff;
use thiserror::Error;
use tree_sitter::LanguageError;
use tree_sitter::Parser;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::StreamingIterator;
use tree_sitter_bash::LANGUAGE as BASH;

pub use standalone_executable::main;

/// Detailed instructions for gpt-4.1 on how to use the `apply_patch` tool.
pub const APPLY_PATCH_TOOL_INSTRUCTIONS: &str = include_str!("../apply_patch_tool_instructions.md");

const APPLY_PATCH_COMMANDS: [&str; 2] = ["apply_patch", "applypatch"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyPatchShell {
    Unix,
    PowerShell,
    Cmd,
}

#[derive(Debug, Error, PartialEq)]
pub enum ApplyPatchError {
    #[error(transparent)]
    ParseError(#[from] ParseError),
    #[error(transparent)]
    IoError(#[from] IoError),
    /// Error that occurs while computing replacements when applying patch chunks
    #[error("{0}")]
    ComputeReplacements(String),
    /// A raw patch body was provided without an explicit `apply_patch` invocation.
    #[error(
        "patch detected without explicit call to apply_patch. Rerun as [\"apply_patch\", \"<patch>\"]"
    )]
    ImplicitInvocation,
}

impl From<std::io::Error> for ApplyPatchError {
    fn from(err: std::io::Error) -> Self {
        ApplyPatchError::IoError(IoError {
            context: "I/O error".to_string(),
            source: err,
        })
    }
}

impl From<&std::io::Error> for ApplyPatchError {
    fn from(err: &std::io::Error) -> Self {
        ApplyPatchError::IoError(IoError {
            context: "I/O error".to_string(),
            source: std::io::Error::new(err.kind(), err.to_string()),
        })
    }
}

#[derive(Debug, Error)]
#[error("{context}: {source}")]
pub struct IoError {
    context: String,
    #[source]
    source: std::io::Error,
}

impl PartialEq for IoError {
    fn eq(&self, other: &Self) -> bool {
        self.context == other.context && self.source.to_string() == other.source.to_string()
    }
}

#[derive(Debug, PartialEq)]
pub enum MaybeApplyPatch {
    Body(ApplyPatchArgs),
    ShellParseError(ExtractHeredocError),
    PatchParseError(ParseError),
    NotApplyPatch,
}

/// Both the raw PATCH argument to `apply_patch` as well as the PATCH argument
/// parsed into hunks.
#[derive(Debug, PartialEq)]
pub struct ApplyPatchArgs {
    pub patch: String,
    pub hunks: Vec<Hunk>,
    pub workdir: Option<String>,
}

fn classify_shell_name(shell: &str) -> Option<String> {
    std::path::Path::new(shell)
        .file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase)
}

fn classify_shell(shell: &str, flag: &str) -> Option<ApplyPatchShell> {
    classify_shell_name(shell).and_then(|name| match name.as_str() {
        "bash" | "zsh" | "sh" if flag == "-lc" => Some(ApplyPatchShell::Unix),
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

#[derive(Debug, PartialEq)]
pub enum ApplyPatchFileChange {
    Add {
        content: String,
    },
    Delete {
        content: String,
    },
    Update {
        unified_diff: String,
        move_path: Option<PathBuf>,
        /// new_content that will result after the unified_diff is applied.
        new_content: String,
    },
}

#[derive(Debug, PartialEq)]
pub enum MaybeApplyPatchVerified {
    /// `argv` corresponded to an `apply_patch` invocation, and these are the
    /// resulting proposed file changes.
    Body(ApplyPatchAction),
    /// `argv` could not be parsed to determine whether it corresponds to an
    /// `apply_patch` invocation.
    ShellParseError(ExtractHeredocError),
    /// `argv` corresponded to an `apply_patch` invocation, but it could not
    /// be fulfilled due to the specified error.
    CorrectnessError(ApplyPatchError),
    /// `argv` decidedly did not correspond to an `apply_patch` invocation.
    NotApplyPatch,
}

/// ApplyPatchAction is the result of parsing an `apply_patch` command. By
/// construction, all paths should be absolute paths.
#[derive(Debug, PartialEq)]
pub struct ApplyPatchAction {
    changes: HashMap<PathBuf, ApplyPatchFileChange>,

    /// The raw patch argument that can be used with `apply_patch` as an exec
    /// call. i.e., if the original arg was parsed in "lenient" mode with a
    /// heredoc, this should be the value without the heredoc wrapper.
    pub patch: String,

    /// The working directory that was used to resolve relative paths in the patch.
    pub cwd: PathBuf,
}

impl ApplyPatchAction {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Returns the changes that would be made by applying the patch.
    pub fn changes(&self) -> &HashMap<PathBuf, ApplyPatchFileChange> {
        &self.changes
    }

    /// Should be used exclusively for testing. (Not worth the overhead of
    /// creating a feature flag for this.)
    pub fn new_add_for_test(path: &Path, content: String) -> Self {
        if !path.is_absolute() {
            panic!("path must be absolute");
        }

        #[expect(clippy::expect_used)]
        let filename = path
            .file_name()
            .expect("path should not be empty")
            .to_string_lossy();
        let patch = format!(
            r#"*** Begin Patch
*** Update File: {filename}
@@
+ {content}
*** End Patch"#,
        );
        let changes = HashMap::from([(path.to_path_buf(), ApplyPatchFileChange::Add { content })]);
        #[expect(clippy::expect_used)]
        Self {
            changes,
            cwd: path
                .parent()
                .expect("path should have parent")
                .to_path_buf(),
            patch,
        }
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

#[derive(Debug, PartialEq)]
pub enum ExtractHeredocError {
    CommandDidNotStartWithApplyPatch,
    FailedToLoadBashGrammar(LanguageError),
    HeredocNotUtf8(Utf8Error),
    FailedToParsePatchIntoAst,
    FailedToFindHeredocBody,
}

/// Applies the patch and prints the result to stdout/stderr.
pub fn apply_patch(
    patch: &str,
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> Result<(), ApplyPatchError> {
    let hunks = match parse_patch(patch) {
        Ok(source) => source.hunks,
        Err(e) => {
            match &e {
                InvalidPatchError(message) => {
                    writeln!(stderr, "Invalid patch: {message}").map_err(ApplyPatchError::from)?;
                }
                InvalidHunkError {
                    message,
                    line_number,
                } => {
                    writeln!(
                        stderr,
                        "Invalid patch hunk on line {line_number}: {message}"
                    )
                    .map_err(ApplyPatchError::from)?;
                }
            }
            return Err(ApplyPatchError::ParseError(e));
        }
    };

    apply_hunks(&hunks, stdout, stderr)?;

    Ok(())
}

/// Applies hunks and continues to update stdout/stderr
pub fn apply_hunks(
    hunks: &[Hunk],
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> Result<(), ApplyPatchError> {
    let _existing_paths: Vec<&Path> = hunks
        .iter()
        .filter_map(|hunk| match hunk {
            Hunk::AddFile { .. } => {
                // The file is being added, so it doesn't exist yet.
                None
            }
            Hunk::DeleteFile { path } => Some(path.as_path()),
            Hunk::UpdateFile {
                path, move_path, ..
            } => match move_path {
                Some(move_path) => {
                    if std::fs::metadata(move_path)
                        .map(|m| m.is_file())
                        .unwrap_or(false)
                    {
                        Some(move_path.as_path())
                    } else {
                        None
                    }
                }
                None => Some(path.as_path()),
            },
        })
        .collect::<Vec<&Path>>();

    // Delegate to a helper that applies each hunk to the filesystem.
    match apply_hunks_to_files(hunks) {
        Ok(affected) => {
            print_summary(&affected, stdout).map_err(ApplyPatchError::from)?;
            Ok(())
        }
        Err(err) => {
            let msg = err.to_string();
            writeln!(stderr, "{msg}").map_err(ApplyPatchError::from)?;
            if let Some(io) = err.downcast_ref::<std::io::Error>() {
                Err(ApplyPatchError::from(io))
            } else {
                Err(ApplyPatchError::IoError(IoError {
                    context: msg,
                    source: std::io::Error::other(err),
                }))
            }
        }
    }
}

/// Applies each parsed patch hunk to the filesystem.
/// Returns an error if any of the changes could not be applied.
/// Tracks file paths affected by applying a patch.
pub struct AffectedPaths {
    pub added: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

/// Apply the hunks to the filesystem, returning which files were added, modified, or deleted.
/// Returns an error if the patch could not be applied.
fn apply_hunks_to_files(hunks: &[Hunk]) -> anyhow::Result<AffectedPaths> {
    if hunks.is_empty() {
        anyhow::bail!("No files were modified.");
    }

    let mut added: Vec<PathBuf> = Vec::new();
    let mut modified: Vec<PathBuf> = Vec::new();
    let mut deleted: Vec<PathBuf> = Vec::new();
    for hunk in hunks {
        match hunk {
            Hunk::AddFile { path, contents } => {
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create parent directories for {}", path.display())
                    })?;
                }
                std::fs::write(path, contents)
                    .with_context(|| format!("Failed to write file {}", path.display()))?;
                added.push(path.clone());
            }
            Hunk::DeleteFile { path } => {
                std::fs::remove_file(path)
                    .with_context(|| format!("Failed to delete file {}", path.display()))?;
                deleted.push(path.clone());
            }
            Hunk::UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                let AppliedPatch { new_contents, .. } =
                    derive_new_contents_from_chunks(path, chunks)?;
                if let Some(dest) = move_path {
                    if let Some(parent) = dest.parent()
                        && !parent.as_os_str().is_empty()
                    {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("Failed to create parent directories for {}", dest.display())
                        })?;
                    }
                    std::fs::write(dest, new_contents)
                        .with_context(|| format!("Failed to write file {}", dest.display()))?;
                    std::fs::remove_file(path)
                        .with_context(|| format!("Failed to remove original {}", path.display()))?;
                    modified.push(dest.clone());
                } else {
                    std::fs::write(path, new_contents)
                        .with_context(|| format!("Failed to write file {}", path.display()))?;
                    modified.push(path.clone());
                }
            }
        }
    }
    Ok(AffectedPaths {
        added,
        modified,
        deleted,
    })
}

struct AppliedPatch {
    original_contents: String,
    new_contents: String,
}

/// Return *only* the new file contents (joined into a single `String`) after
/// applying the chunks to the file at `path`.
fn derive_new_contents_from_chunks(
    path: &Path,
    chunks: &[UpdateFileChunk],
) -> std::result::Result<AppliedPatch, ApplyPatchError> {
    let original_contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            return Err(ApplyPatchError::IoError(IoError {
                context: format!("Failed to read file to update {}", path.display()),
                source: err,
            }));
        }
    };

    let original_lines: Vec<String> = build_lines_from_contents(&original_contents);

    let replacements = compute_replacements(&original_lines, path, chunks)?;
    let new_lines = apply_replacements(original_lines, &replacements);
    let mut new_lines = new_lines;
    if !new_lines.last().is_some_and(String::is_empty) {
        new_lines.push(String::new());
    }
    let new_contents = build_contents_from_lines(&original_contents, &new_lines);
    Ok(AppliedPatch {
        original_contents,
        new_contents,
    })
}

// TODO(dylan-hurd-oai): I think we can migrate to just use `contents.lines()`
// across all platforms.
fn build_lines_from_contents(contents: &str) -> Vec<String> {
    if cfg!(windows) {
        contents.lines().map(String::from).collect()
    } else {
        let mut lines: Vec<String> = contents.split('\n').map(String::from).collect();

        // Drop the trailing empty element that results from the final newline so
        // that line counts match the behaviour of standard `diff`.
        if lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }

        lines
    }
}

fn build_contents_from_lines(original_contents: &str, lines: &[String]) -> String {
    if cfg!(windows) {
        // for now, only compute this if we're on Windows.
        let uses_crlf = contents_uses_crlf(original_contents);
        if uses_crlf {
            lines.join("\r\n")
        } else {
            lines.join("\n")
        }
    } else {
        lines.join("\n")
    }
}

/// Detects whether the source file uses Windows CRLF line endings consistently.
/// We only consider a file CRLF-formatted if every newline is part of a
/// CRLF sequence. This avoids rewriting an LF-formatted file that merely
/// contains embedded sequences of "\r\n".
///
/// Returns `true` if the file uses CRLF line endings, `false` otherwise.
fn contents_uses_crlf(contents: &str) -> bool {
    let bytes = contents.as_bytes();
    let mut n_newlines = 0usize;
    let mut n_crlf = 0usize;
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            n_newlines += 1;
            if i > 0 && bytes[i - 1] == b'\r' {
                n_crlf += 1;
            }
        }
    }

    n_newlines > 0 && n_crlf == n_newlines
}

/// Compute a list of replacements needed to transform `original_lines` into the
/// new lines, given the patch `chunks`. Each replacement is returned as
/// `(start_index, old_len, new_lines)`.
fn compute_replacements(
    original_lines: &[String],
    path: &Path,
    chunks: &[UpdateFileChunk],
) -> std::result::Result<Vec<(usize, usize, Vec<String>)>, ApplyPatchError> {
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut line_index: usize = 0;

    for chunk in chunks {
        // If a chunk has a `change_context`, we use seek_sequence to find it, then
        // adjust our `line_index` to continue from there.
        if let Some(ctx_line) = &chunk.change_context {
            if let Some(idx) = seek_sequence::seek_sequence(
                original_lines,
                std::slice::from_ref(ctx_line),
                line_index,
                false,
            ) {
                line_index = idx + 1;
            } else {
                return Err(ApplyPatchError::ComputeReplacements(format!(
                    "Failed to find context '{}' in {}",
                    ctx_line,
                    path.display()
                )));
            }
        }

        if chunk.old_lines.is_empty() {
            // Pure addition (no old lines). We'll add them at the end or just
            // before the final empty line if one exists.
            let insertion_idx = if original_lines.last().is_some_and(String::is_empty) {
                original_lines.len() - 1
            } else {
                original_lines.len()
            };
            replacements.push((insertion_idx, 0, chunk.new_lines.clone()));
            continue;
        }

        // Otherwise, try to match the existing lines in the file with the old lines
        // from the chunk. If found, schedule that region for replacement.
        // Attempt to locate the `old_lines` verbatim within the file.  In many
        // real‑world diffs the last element of `old_lines` is an *empty* string
        // representing the terminating newline of the region being replaced.
        // This sentinel is not present in `original_lines` because we strip the
        // trailing empty slice emitted by `split('\n')`.  If a direct search
        // fails and the pattern ends with an empty string, retry without that
        // final element so that modifications touching the end‑of‑file can be
        // located reliably.

        let mut pattern: &[String] = &chunk.old_lines;
        let mut found =
            seek_sequence::seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);

        let mut new_slice: &[String] = &chunk.new_lines;

        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            // Retry without the trailing empty line which represents the final
            // newline in the file.
            pattern = &pattern[..pattern.len() - 1];
            if new_slice.last().is_some_and(String::is_empty) {
                new_slice = &new_slice[..new_slice.len() - 1];
            }

            found = seek_sequence::seek_sequence(
                original_lines,
                pattern,
                line_index,
                chunk.is_end_of_file,
            );
        }

        if let Some(start_idx) = found {
            replacements.push((start_idx, pattern.len(), new_slice.to_vec()));
            line_index = start_idx + pattern.len();
        } else {
            return Err(ApplyPatchError::ComputeReplacements(format!(
                "Failed to find expected lines in {}:\n{}",
                path.display(),
                chunk.old_lines.join("\n"),
            )));
        }
    }

    replacements.sort_by(|(lhs_idx, _, _), (rhs_idx, _, _)| lhs_idx.cmp(rhs_idx));

    Ok(replacements)
}

/// Apply the `(start_index, old_len, new_lines)` replacements to `original_lines`,
/// returning the modified file contents as a vector of lines.
fn apply_replacements(
    mut lines: Vec<String>,
    replacements: &[(usize, usize, Vec<String>)],
) -> Vec<String> {
    // We must apply replacements in descending order so that earlier replacements
    // don't shift the positions of later ones.
    for (start_idx, old_len, new_segment) in replacements.iter().rev() {
        let start_idx = *start_idx;
        let old_len = *old_len;

        // Remove old lines.
        for _ in 0..old_len {
            if start_idx < lines.len() {
                lines.remove(start_idx);
            }
        }

        // Insert new lines.
        for (offset, new_line) in new_segment.iter().enumerate() {
            lines.insert(start_idx + offset, new_line.clone());
        }
    }

    lines
}

/// Intended result of a file update for apply_patch.
#[derive(Debug, Eq, PartialEq)]
pub struct ApplyPatchFileUpdate {
    unified_diff: String,
    content: String,
}

pub fn unified_diff_from_chunks(
    path: &Path,
    chunks: &[UpdateFileChunk],
) -> std::result::Result<ApplyPatchFileUpdate, ApplyPatchError> {
    unified_diff_from_chunks_with_context(path, chunks, 1)
}

pub fn unified_diff_from_chunks_with_context(
    path: &Path,
    chunks: &[UpdateFileChunk],
    context: usize,
) -> std::result::Result<ApplyPatchFileUpdate, ApplyPatchError> {
    let AppliedPatch {
        original_contents,
        new_contents,
    } = derive_new_contents_from_chunks(path, chunks)?;
    let text_diff = TextDiff::from_lines(&original_contents, &new_contents);
    let unified_diff = text_diff.unified_diff().context_radius(context).to_string();
    Ok(ApplyPatchFileUpdate {
        unified_diff,
        content: new_contents,
    })
}

/// Print the summary of changes in git-style format.
/// Write a summary of changes to the given writer.
pub fn print_summary(
    affected: &AffectedPaths,
    out: &mut impl std::io::Write,
) -> std::io::Result<()> {
    writeln!(out, "Success. Updated the following files:")?;
    for path in &affected.added {
        writeln!(out, "A {}", path.display())?;
    }
    for path in &affected.modified {
        writeln!(out, "M {}", path.display())?;
    }
    for path in &affected.deleted {
        writeln!(out, "D {}", path.display())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::string::ToString;
    use tempfile::tempdir;

    /// Helper to construct a patch with the given body.
    fn wrap_patch(body: &str) -> String {
        format!("*** Begin Patch\n{body}\n*** End Patch")
    }

    fn strs_to_strings(strs: &[&str]) -> Vec<String> {
        strs.iter().map(ToString::to_string).collect()
    }

    // Test helpers to reduce repetition when building bash -lc heredoc scripts
    fn args_bash(script: &str) -> Vec<String> {
        strs_to_strings(&["bash", "-lc", script])
    }

    fn args_powershell(script: &str) -> Vec<String> {
        strs_to_strings(&["powershell.exe", "-Command", script])
    }

    fn args_powershell_no_profile(script: &str) -> Vec<String> {
        strs_to_strings(&["powershell.exe", "-NoProfile", "-Command", script])
    }

    fn args_pwsh(script: &str) -> Vec<String> {
        strs_to_strings(&["pwsh", "-NoProfile", "-Command", script])
    }

    fn args_cmd(script: &str) -> Vec<String> {
        strs_to_strings(&["cmd.exe", "/c", script])
    }

    fn heredoc_script(prefix: &str) -> String {
        format!(
            "{prefix}apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch\nPATCH"
        )
    }

    fn heredoc_script_ps(prefix: &str, suffix: &str) -> String {
        format!(
            "{prefix}apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch\nPATCH{suffix}"
        )
    }

    fn expected_single_add() -> Vec<Hunk> {
        vec![Hunk::AddFile {
            path: PathBuf::from("foo"),
            contents: "hi\n".to_string(),
        }]
    }

    fn assert_match_args(args: Vec<String>, expected_workdir: Option<&str>) {
        match maybe_parse_apply_patch(&args) {
            MaybeApplyPatch::Body(ApplyPatchArgs { hunks, workdir, .. }) => {
                assert_eq!(workdir.as_deref(), expected_workdir);
                assert_eq!(hunks, expected_single_add());
            }
            result => panic!("expected MaybeApplyPatch::Body got {result:?}"),
        }
    }

    fn assert_match(script: &str, expected_workdir: Option<&str>) {
        let args = args_bash(script);
        assert_match_args(args, expected_workdir);
    }

    fn assert_not_match(script: &str) {
        let args = args_bash(script);
        assert_matches!(
            maybe_parse_apply_patch(&args),
            MaybeApplyPatch::NotApplyPatch
        );
    }

    #[test]
    fn test_implicit_patch_single_arg_is_error() {
        let patch = "*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch".to_string();
        let args = vec![patch];
        let dir = tempdir().unwrap();
        assert_matches!(
            maybe_parse_apply_patch_verified(&args, dir.path()),
            MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation)
        );
    }

    #[test]
    fn test_implicit_patch_bash_script_is_error() {
        let script = "*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch";
        let args = args_bash(script);
        let dir = tempdir().unwrap();
        assert_matches!(
            maybe_parse_apply_patch_verified(&args, dir.path()),
            MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation)
        );
    }

    #[test]
    fn test_literal() {
        let args = strs_to_strings(&[
            "apply_patch",
            r#"*** Begin Patch
*** Add File: foo
+hi
*** End Patch
"#,
        ]);

        match maybe_parse_apply_patch(&args) {
            MaybeApplyPatch::Body(ApplyPatchArgs { hunks, .. }) => {
                assert_eq!(
                    hunks,
                    vec![Hunk::AddFile {
                        path: PathBuf::from("foo"),
                        contents: "hi\n".to_string()
                    }]
                );
            }
            result => panic!("expected MaybeApplyPatch::Body got {result:?}"),
        }
    }

    #[test]
    fn test_literal_applypatch() {
        let args = strs_to_strings(&[
            "applypatch",
            r#"*** Begin Patch
*** Add File: foo
+hi
*** End Patch
"#,
        ]);

        match maybe_parse_apply_patch(&args) {
            MaybeApplyPatch::Body(ApplyPatchArgs { hunks, .. }) => {
                assert_eq!(
                    hunks,
                    vec![Hunk::AddFile {
                        path: PathBuf::from("foo"),
                        contents: "hi\n".to_string()
                    }]
                );
            }
            result => panic!("expected MaybeApplyPatch::Body got {result:?}"),
        }
    }

    #[test]
    fn test_heredoc() {
        assert_match(&heredoc_script(""), None);
    }

    #[test]
    fn test_heredoc_applypatch() {
        let args = strs_to_strings(&[
            "bash",
            "-lc",
            r#"applypatch <<'PATCH'
*** Begin Patch
*** Add File: foo
+hi
*** End Patch
PATCH"#,
        ]);

        match maybe_parse_apply_patch(&args) {
            MaybeApplyPatch::Body(ApplyPatchArgs { hunks, workdir, .. }) => {
                assert_eq!(workdir, None);
                assert_eq!(
                    hunks,
                    vec![Hunk::AddFile {
                        path: PathBuf::from("foo"),
                        contents: "hi\n".to_string()
                    }]
                );
            }
            result => panic!("expected MaybeApplyPatch::Body got {result:?}"),
        }
    }

    #[test]
    fn test_powershell_heredoc() {
        let script = heredoc_script("");
        assert_match_args(args_powershell(&script), None);
    }
    #[test]
    fn test_powershell_heredoc_no_profile() {
        let script = heredoc_script("");
        assert_match_args(args_powershell_no_profile(&script), None);
    }
    #[test]
    fn test_pwsh_heredoc() {
        let script = heredoc_script("");
        assert_match_args(args_pwsh(&script), None);
    }

    #[test]
    fn test_cmd_heredoc_with_cd() {
        let script = heredoc_script("cd foo && ");
        assert_match_args(args_cmd(&script), Some("foo"));
    }

    #[test]
    fn test_heredoc_with_leading_cd() {
        assert_match(&heredoc_script("cd foo && "), Some("foo"));
    }

    #[test]
    fn test_cd_with_semicolon_is_ignored() {
        assert_not_match(&heredoc_script("cd foo; "));
    }

    #[test]
    fn test_cd_or_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("cd bar || "));
    }

    #[test]
    fn test_cd_pipe_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("cd bar | "));
    }

    #[test]
    fn test_cd_single_quoted_path_with_spaces() {
        assert_match(&heredoc_script("cd 'foo bar' && "), Some("foo bar"));
    }

    #[test]
    fn test_cd_double_quoted_path_with_spaces() {
        assert_match(&heredoc_script("cd \"foo bar\" && "), Some("foo bar"));
    }

    #[test]
    fn test_echo_and_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("echo foo && "));
    }

    #[test]
    fn test_apply_patch_with_arg_is_ignored() {
        let script = "apply_patch foo <<'PATCH'\n*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch\nPATCH";
        assert_not_match(script);
    }

    #[test]
    fn test_double_cd_then_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("cd foo && cd bar && "));
    }

    #[test]
    fn test_cd_two_args_is_ignored() {
        assert_not_match(&heredoc_script("cd foo bar && "));
    }

    #[test]
    fn test_cd_then_apply_patch_then_extra_is_ignored() {
        let script = heredoc_script_ps("cd bar && ", " && echo done");
        assert_not_match(&script);
    }

    #[test]
    fn test_echo_then_cd_and_apply_patch_is_ignored() {
        // Ensure preceding commands before the `cd && apply_patch <<...` sequence do not match.
        assert_not_match(&heredoc_script("echo foo; cd bar && "));
    }

    #[test]
    fn test_add_file_hunk_creates_file_with_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("add.txt");
        let patch = wrap_patch(&format!(
            r#"*** Add File: {}
+ab
+cd"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        // Verify expected stdout and stderr outputs.
        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();
        let expected_out = format!(
            "Success. Updated the following files:\nA {}\n",
            path.display()
        );
        assert_eq!(stdout_str, expected_out);
        assert_eq!(stderr_str, "");
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents, "ab\ncd\n");
    }

    #[test]
    fn test_delete_file_hunk_removes_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("del.txt");
        fs::write(&path, "x").unwrap();
        let patch = wrap_patch(&format!("*** Delete File: {}", path.display()));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();
        let expected_out = format!(
            "Success. Updated the following files:\nD {}\n",
            path.display()
        );
        assert_eq!(stdout_str, expected_out);
        assert_eq!(stderr_str, "");
        assert!(!path.exists());
    }

    #[test]
    fn test_update_file_hunk_modifies_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("update.txt");
        fs::write(&path, "foo\nbar\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 foo
-bar
+baz"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        // Validate modified file contents and expected stdout/stderr.
        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();
        let expected_out = format!(
            "Success. Updated the following files:\nM {}\n",
            path.display()
        );
        assert_eq!(stdout_str, expected_out);
        assert_eq!(stderr_str, "");
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "foo\nbaz\n");
    }

    #[test]
    fn test_update_file_hunk_can_move_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dst.txt");
        fs::write(&src, "line\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
*** Move to: {}
@@
-line
+line2"#,
            src.display(),
            dest.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        // Validate move semantics and expected stdout/stderr.
        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();
        let expected_out = format!(
            "Success. Updated the following files:\nM {}\n",
            dest.display()
        );
        assert_eq!(stdout_str, expected_out);
        assert_eq!(stderr_str, "");
        assert!(!src.exists());
        let contents = fs::read_to_string(&dest).unwrap();
        assert_eq!(contents, "line2\n");
    }

    /// Verify that a single `Update File` hunk with multiple change chunks can update different
    /// parts of a file and that the file is listed only once in the summary.
    #[test]
    fn test_multiple_update_chunks_apply_to_single_file() {
        // Start with a file containing four lines.
        let dir = tempdir().unwrap();
        let path = dir.path().join("multi.txt");
        fs::write(&path, "foo\nbar\nbaz\nqux\n").unwrap();
        // Construct an update patch with two separate change chunks.
        // The first chunk uses the line `foo` as context and transforms `bar` into `BAR`.
        // The second chunk uses `baz` as context and transforms `qux` into `QUX`.
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 foo
-bar
+BAR
@@
 baz
-qux
+QUX"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();
        let expected_out = format!(
            "Success. Updated the following files:\nM {}\n",
            path.display()
        );
        assert_eq!(stdout_str, expected_out);
        assert_eq!(stderr_str, "");
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "foo\nBAR\nbaz\nQUX\n");
    }

    /// A more involved `Update File` hunk that exercises additions, deletions and
    /// replacements in separate chunks that appear in non‑adjacent parts of the
    /// file.  Verifies that all edits are applied and that the summary lists the
    /// file only once.
    #[test]
    fn test_update_file_hunk_interleaved_changes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("interleaved.txt");

        // Original file: six numbered lines.
        fs::write(&path, "a\nb\nc\nd\ne\nf\n").unwrap();

        // Patch performs:
        //  • Replace `b` → `B`
        //  • Replace `e` → `E` (using surrounding context)
        //  • Append new line `g` at the end‑of‑file
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 a
-b
+B
@@
 c
 d
-e
+E
@@
 f
+g
*** End of File"#,
            path.display()
        ));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();

        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();

        let expected_out = format!(
            "Success. Updated the following files:\nM {}\n",
            path.display()
        );
        assert_eq!(stdout_str, expected_out);
        assert_eq!(stderr_str, "");

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "a\nB\nc\nd\nE\nf\ng\n");
    }

    /// Ensure CRLF line endings are preserved for updated files on Windows‑style inputs.
    #[cfg(windows)]
    #[test]
    fn test_preserve_crlf_line_endings_on_update() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("crlf.txt");

        // Original file uses CRLF (\r\n) endings.
        std::fs::write(&path, b"a\r\nb\r\nc\r\n").unwrap();

        // Replace `b` -> `B` and append `d`.
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 a
-b
+B
@@
 c
+d
*** End of File"#,
            path.display()
        ));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();

        let out = std::fs::read(&path).unwrap();
        // Expect all CRLF endings; count occurrences of CRLF and ensure there are 4 lines.
        let content = String::from_utf8_lossy(&out);
        assert!(content.contains("\r\n"));
        // No bare LF occurrences immediately preceding a non-CR: the text should not contain "a\nb".
        assert!(!content.contains("a\nb"));
        // Validate exact content sequence with CRLF delimiters.
        assert_eq!(content, "a\r\nB\r\nc\r\nd\r\n");
    }

    /// Ensure CRLF inputs with embedded carriage returns in the content are preserved.
    #[cfg(windows)]
    #[test]
    fn test_preserve_crlf_embedded_carriage_returns_on_append() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("crlf_cr_content.txt");

        // Original file: first line has a literal '\r' in the content before the CRLF terminator.
        std::fs::write(&path, b"foo\r\r\nbar\r\n").unwrap();

        // Append a new line without modifying existing ones.
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
+BAZ
*** End of File"#,
            path.display()
        ));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();

        let out = std::fs::read(&path).unwrap();
        // CRLF endings must be preserved and the extra CR in "foo\r\r" must not be collapsed.
        assert_eq!(out.as_slice(), b"foo\r\r\nbar\r\nBAZ\r\n");
    }

    #[test]
    fn test_pure_addition_chunk_followed_by_removal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("panic.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
+after-context
+second-line
@@
 line1
-line2
-line3
+line2-replacement"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(
            contents,
            "line1\nline2-replacement\nafter-context\nsecond-line\n"
        );
    }

    /// Ensure that patches authored with ASCII characters can update lines that
    /// contain typographic Unicode punctuation (e.g. EN DASH, NON-BREAKING
    /// HYPHEN). Historically `git apply` succeeds in such scenarios but our
    /// internal matcher failed requiring an exact byte-for-byte match.  The
    /// fuzzy-matching pass that normalises common punctuation should now bridge
    /// the gap.
    #[test]
    fn test_update_line_with_unicode_dash() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("unicode.py");

        // Original line contains EN DASH (\u{2013}) and NON-BREAKING HYPHEN (\u{2011}).
        let original = "import asyncio  # local import \u{2013} avoids top\u{2011}level dep\n";
        std::fs::write(&path, original).unwrap();

        // Patch uses plain ASCII dash / hyphen.
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
-import asyncio  # local import - avoids top-level dep
+import asyncio  # HELLO"#,
            path.display()
        ));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();

        // File should now contain the replaced comment.
        let expected = "import asyncio  # HELLO\n";
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, expected);

        // Ensure success summary lists the file as modified.
        let stdout_str = String::from_utf8(stdout).unwrap();
        let expected_out = format!(
            "Success. Updated the following files:\nM {}\n",
            path.display()
        );
        assert_eq!(stdout_str, expected_out);

        // No stderr expected.
        assert_eq!(String::from_utf8(stderr).unwrap(), "");
    }

    #[test]
    fn test_unified_diff() {
        // Start with a file containing four lines.
        let dir = tempdir().unwrap();
        let path = dir.path().join("multi.txt");
        fs::write(&path, "foo\nbar\nbaz\nqux\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 foo
-bar
+BAR
@@
 baz
-qux
+QUX"#,
            path.display()
        ));
        let patch = parse_patch(&patch).unwrap();

        let update_file_chunks = match patch.hunks.as_slice() {
            [Hunk::UpdateFile { chunks, .. }] => chunks,
            _ => panic!("Expected a single UpdateFile hunk"),
        };
        let diff = unified_diff_from_chunks(&path, update_file_chunks).unwrap();
        let expected_diff = r#"@@ -1,4 +1,4 @@
 foo
-bar
+BAR
 baz
-qux
+QUX
"#;
        let expected = ApplyPatchFileUpdate {
            unified_diff: expected_diff.to_string(),
            content: "foo\nBAR\nbaz\nQUX\n".to_string(),
        };
        assert_eq!(expected, diff);
    }

    #[test]
    fn test_unified_diff_first_line_replacement() {
        // Replace the very first line of the file.
        let dir = tempdir().unwrap();
        let path = dir.path().join("first.txt");
        fs::write(&path, "foo\nbar\nbaz\n").unwrap();

        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
-foo
+FOO
 bar
"#,
            path.display()
        ));

        let patch = parse_patch(&patch).unwrap();
        let chunks = match patch.hunks.as_slice() {
            [Hunk::UpdateFile { chunks, .. }] => chunks,
            _ => panic!("Expected a single UpdateFile hunk"),
        };

        let diff = unified_diff_from_chunks(&path, chunks).unwrap();
        let expected_diff = r#"@@ -1,2 +1,2 @@
-foo
+FOO
 bar
"#;
        let expected = ApplyPatchFileUpdate {
            unified_diff: expected_diff.to_string(),
            content: "FOO\nbar\nbaz\n".to_string(),
        };
        assert_eq!(expected, diff);
    }

    #[test]
    fn test_unified_diff_last_line_replacement() {
        // Replace the very last line of the file.
        let dir = tempdir().unwrap();
        let path = dir.path().join("last.txt");
        fs::write(&path, "foo\nbar\nbaz\n").unwrap();

        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 foo
 bar
-baz
+BAZ
"#,
            path.display()
        ));

        let patch = parse_patch(&patch).unwrap();
        let chunks = match patch.hunks.as_slice() {
            [Hunk::UpdateFile { chunks, .. }] => chunks,
            _ => panic!("Expected a single UpdateFile hunk"),
        };

        let diff = unified_diff_from_chunks(&path, chunks).unwrap();
        let expected_diff = r#"@@ -2,2 +2,2 @@
 bar
-baz
+BAZ
"#;
        let expected = ApplyPatchFileUpdate {
            unified_diff: expected_diff.to_string(),
            content: "foo\nbar\nBAZ\n".to_string(),
        };
        assert_eq!(expected, diff);
    }

    /// For LF-only inputs with a trailing newline ensure that the helper used
    /// on Windows-style builds drops the synthetic trailing empty element so
    /// replacements behave like standard `diff` line numbering.
    #[test]
    fn test_derive_new_contents_lf_trailing_newline() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lf_trailing_newline.txt");
        fs::write(&path, "foo\nbar\n").unwrap();

        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 foo
-bar
+BAR
"#,
            path.display()
        ));

        let patch = parse_patch(&patch).unwrap();
        let chunks = match patch.hunks.as_slice() {
            [Hunk::UpdateFile { chunks, .. }] => chunks,
            _ => panic!("Expected a single UpdateFile hunk"),
        };

        let AppliedPatch { new_contents, .. } =
            derive_new_contents_from_chunks(&path, chunks).unwrap();

        assert_eq!(new_contents, "foo\nBAR\n");
    }

    #[test]
    fn test_unified_diff_insert_at_eof() {
        // Insert a new line at end‑of‑file.
        let dir = tempdir().unwrap();
        let path = dir.path().join("insert.txt");
        fs::write(&path, "foo\nbar\nbaz\n").unwrap();

        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
+quux
*** End of File
"#,
            path.display()
        ));

        let patch = parse_patch(&patch).unwrap();
        let chunks = match patch.hunks.as_slice() {
            [Hunk::UpdateFile { chunks, .. }] => chunks,
            _ => panic!("Expected a single UpdateFile hunk"),
        };

        let diff = unified_diff_from_chunks(&path, chunks).unwrap();
        let expected_diff = r#"@@ -3 +3,2 @@
 baz
+quux
"#;
        let expected = ApplyPatchFileUpdate {
            unified_diff: expected_diff.to_string(),
            content: "foo\nbar\nbaz\nquux\n".to_string(),
        };
        assert_eq!(expected, diff);
    }

    #[test]
    fn test_unified_diff_interleaved_changes() {
        // Original file with six lines.
        let dir = tempdir().unwrap();
        let path = dir.path().join("interleaved.txt");
        fs::write(&path, "a\nb\nc\nd\ne\nf\n").unwrap();

        // Patch replaces two separate lines and appends a new one at EOF using
        // three distinct chunks.
        let patch_body = format!(
            r#"*** Update File: {}
@@
 a
-b
+B
@@
 d
-e
+E
@@
 f
+g
*** End of File"#,
            path.display()
        );
        let patch = wrap_patch(&patch_body);

        // Extract chunks then build the unified diff.
        let parsed = parse_patch(&patch).unwrap();
        let chunks = match parsed.hunks.as_slice() {
            [Hunk::UpdateFile { chunks, .. }] => chunks,
            _ => panic!("Expected a single UpdateFile hunk"),
        };

        let diff = unified_diff_from_chunks(&path, chunks).unwrap();

        let expected_diff = r#"@@ -1,6 +1,7 @@
 a
-b
+B
 c
 d
-e
+E
 f
+g
"#;

        let expected = ApplyPatchFileUpdate {
            unified_diff: expected_diff.to_string(),
            content: "a\nB\nc\nd\nE\nf\ng\n".to_string(),
        };

        assert_eq!(expected, diff);

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(
            contents,
            r#"a
B
c
d
E
f
g
"#
        );
    }

    #[test]
    fn test_apply_patch_should_resolve_absolute_paths_in_cwd() {
        let session_dir = tempdir().unwrap();
        let relative_path = "source.txt";

        // Note that we need this file to exist for the patch to be "verified"
        // and parsed correctly.
        let session_file_path = session_dir.path().join(relative_path);
        fs::write(&session_file_path, "session directory content\n").unwrap();

        let argv = vec![
            "apply_patch".to_string(),
            r#"*** Begin Patch
*** Update File: source.txt
@@
-session directory content
+updated session directory content
*** End Patch"#
                .to_string(),
        ];

        let result = maybe_parse_apply_patch_verified(&argv, session_dir.path());

        // Verify the patch contents - as otherwise we may have pulled contents
        // from the wrong file (as we're using relative paths)
        assert_eq!(
            result,
            MaybeApplyPatchVerified::Body(ApplyPatchAction {
                changes: HashMap::from([(
                    session_dir.path().join(relative_path),
                    ApplyPatchFileChange::Update {
                        unified_diff: r#"@@ -1 +1 @@
-session directory content
+updated session directory content
"#
                        .to_string(),
                        move_path: None,
                        new_content: "updated session directory content\n".to_string(),
                    },
                )]),
                patch: argv[1].clone(),
                cwd: session_dir.path().to_path_buf(),
            })
        );
    }

    #[test]
    fn test_apply_patch_resolves_move_path_with_effective_cwd() {
        let session_dir = tempdir().unwrap();
        let worktree_rel = "alt";
        let worktree_dir = session_dir.path().join(worktree_rel);
        fs::create_dir_all(&worktree_dir).unwrap();

        let source_name = "old.txt";
        let dest_name = "renamed.txt";
        let source_path = worktree_dir.join(source_name);
        fs::write(&source_path, "before\n").unwrap();

        let patch = wrap_patch(&format!(
            r#"*** Update File: {source_name}
*** Move to: {dest_name}
@@
-before
+after"#
        ));

        let shell_script = format!("cd {worktree_rel} && apply_patch <<'PATCH'\n{patch}\nPATCH");
        let argv = vec!["bash".into(), "-lc".into(), shell_script];

        let result = maybe_parse_apply_patch_verified(&argv, session_dir.path());
        let action = match result {
            MaybeApplyPatchVerified::Body(action) => action,
            other => panic!("expected verified body, got {other:?}"),
        };

        assert_eq!(action.cwd, worktree_dir);

        let change = action
            .changes()
            .get(&worktree_dir.join(source_name))
            .expect("source file change present");

        match change {
            ApplyPatchFileChange::Update { move_path, .. } => {
                assert_eq!(
                    move_path.as_deref(),
                    Some(worktree_dir.join(dest_name).as_path())
                );
            }
            other => panic!("expected update change, got {other:?}"),
        }
    }

    #[test]
    fn test_apply_patch_fails_on_write_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("readonly.txt");
        fs::write(&path, "before\n").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).unwrap();

        let patch = wrap_patch(&format!(
            "*** Update File: {}\n@@\n-before\n+after\n*** End Patch",
            path.display()
        ));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = apply_patch(&patch, &mut stdout, &mut stderr);
        assert!(result.is_err());
    }
}
