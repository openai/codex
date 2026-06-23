use std::collections::HashMap;
use std::sync::LazyLock;

use codex_exec_server::ExecutorFileSystem;
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
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
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

fn classify_shell_name(shell: &str, convention: PathConvention) -> Option<String> {
    let basename = convention.path_segments(shell).next_back()?;
    let stem = basename
        .rsplit_once('.')
        .and_then(|(stem, _extension)| (!stem.is_empty()).then_some(stem))
        .unwrap_or(basename);
    Some(stem.to_ascii_lowercase())
}

fn classify_shell(shell: &str, flag: &str, convention: PathConvention) -> Option<ApplyPatchShell> {
    classify_shell_name(shell, convention).and_then(|name| match name.as_str() {
        "bash" | "zsh" | "sh" if matches!(flag, "-lc" | "-c") => Some(ApplyPatchShell::Unix),
        "pwsh" | "powershell" if flag.eq_ignore_ascii_case("-command") => {
            Some(ApplyPatchShell::PowerShell)
        }
        "cmd" if flag.eq_ignore_ascii_case("/c") => Some(ApplyPatchShell::Cmd),
        _ => None,
    })
}

fn can_skip_flag(shell: &str, flag: &str, convention: PathConvention) -> bool {
    classify_shell_name(shell, convention).is_some_and(|name| {
        matches!(name.as_str(), "pwsh" | "powershell") && flag.eq_ignore_ascii_case("-noprofile")
    })
}

fn parse_shell_script<'a>(argv: &'a [String], cwd: &PathUri) -> Option<(ApplyPatchShell, &'a str)> {
    let convention = cwd.infer_path_convention()?;
    match argv {
        [shell, flag, script] => classify_shell(shell, flag, convention).map(|shell_type| {
            let script = script.as_str();
            (shell_type, script)
        }),
        [shell, skip_flag, flag, script] => {
            if !can_skip_flag(shell, skip_flag, convention) {
                return None;
            }
            classify_shell(shell, flag, convention).map(|shell_type| {
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
/// `cwd` supplies the path convention used to interpret the shell executable in `argv`.
pub fn maybe_parse_apply_patch(argv: &[String], cwd: &PathUri) -> MaybeApplyPatch {
    match argv {
        // Direct invocation: apply_patch <patch>
        [cmd, body] if APPLY_PATCH_COMMANDS.contains(&cmd.as_str()) => match parse_patch(body) {
            Ok(source) => MaybeApplyPatch::Body(source),
            Err(e) => MaybeApplyPatch::PatchParseError(e),
        },
        // Shell heredoc form: (optional `cd <path> &&`) apply_patch <<'EOF' ...
        _ => match parse_shell_script(argv, cwd) {
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

/// `cwd` must identify an absolute environment-native path so relative patch paths can be
/// resolved without projecting them onto the app-server or exec-server host.
pub async fn maybe_parse_apply_patch_verified(
    argv: &[String],
    cwd: &PathUri,
    fs: &dyn ExecutorFileSystem,
    sandbox: Option<&codex_exec_server::FileSystemSandboxContext>,
) -> MaybeApplyPatchVerified {
    // Detect a raw patch body passed directly as the command or as the body of a shell
    // script. In these cases, report an explicit error rather than applying the patch.
    if let [body] = argv
        && parse_patch(body).is_ok()
    {
        return MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation);
    }
    if let Some((_, script)) = parse_shell_script(argv, cwd)
        && parse_patch(script).is_ok()
    {
        return MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation);
    }

    match maybe_parse_apply_patch(argv, cwd) {
        MaybeApplyPatch::Body(args) => verify_apply_patch_args(args, cwd, fs, sandbox).await,
        MaybeApplyPatch::ShellParseError(e) => MaybeApplyPatchVerified::ShellParseError(e),
        MaybeApplyPatch::PatchParseError(e) => MaybeApplyPatchVerified::CorrectnessError(e.into()),
        MaybeApplyPatch::NotApplyPatch => MaybeApplyPatchVerified::NotApplyPatch,
    }
}

pub async fn verify_apply_patch_args(
    args: ApplyPatchArgs,
    cwd: &PathUri,
    fs: &dyn ExecutorFileSystem,
    sandbox: Option<&codex_exec_server::FileSystemSandboxContext>,
) -> MaybeApplyPatchVerified {
    match try_verify_apply_patch_args(args, cwd, fs, sandbox).await {
        Ok(action) => MaybeApplyPatchVerified::Body(action),
        Err(err) => MaybeApplyPatchVerified::CorrectnessError(err),
    }
}

async fn try_verify_apply_patch_args(
    args: ApplyPatchArgs,
    cwd: &PathUri,
    fs: &dyn ExecutorFileSystem,
    sandbox: Option<&codex_exec_server::FileSystemSandboxContext>,
) -> Result<ApplyPatchAction, ApplyPatchError> {
    let ApplyPatchArgs {
        patch,
        hunks,
        workdir,
        ..
    } = args;
    let effective_cwd = workdir
        .as_ref()
        .map(|dir| cwd.join(dir))
        .transpose()?
        .unwrap_or_else(|| cwd.clone());
    let mut changes = HashMap::new();
    for hunk in hunks {
        let path = hunk.resolve_path(&effective_cwd)?;
        match hunk {
            Hunk::AddFile { contents, .. } => {
                changes.insert(path, ApplyPatchFileChange::Add { content: contents });
            }
            Hunk::DeleteFile { .. } => {
                let content = fs.read_file_text(&path, sandbox).await.map_err(|source| {
                    ApplyPatchError::IoError(IoError {
                        context: format!("Failed to read {}", path.inferred_native_path_string()),
                        source,
                    })
                })?;
                changes.insert(path, ApplyPatchFileChange::Delete { content });
            }
            Hunk::UpdateFile {
                move_path, chunks, ..
            } => {
                let ApplyPatchFileUpdate {
                    unified_diff,
                    content: contents,
                    ..
                } = unified_diff_from_chunks(&path, &chunks, fs, sandbox).await?;
                changes.insert(
                    path,
                    ApplyPatchFileChange::Update {
                        unified_diff,
                        move_path: move_path
                            .map(|path| effective_cwd.join(&path.to_string_lossy()))
                            .transpose()?,
                        new_content: contents,
                    },
                );
            }
        }
    }
    Ok(ApplyPatchAction {
        changes,
        patch,
        cwd: effective_cwd,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_diff_from_chunks;
    use assert_matches::assert_matches;
    use codex_exec_server::LOCAL_FS;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::PathBuf;
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

    #[track_caller]
    fn assert_match_args(args: Vec<String>, expected_workdir: Option<&str>) {
        assert_match_args_with_cwd(
            args,
            &PathUri::parse("file:///workspace").expect("valid POSIX test cwd"),
            expected_workdir,
        );
    }

    #[track_caller]
    fn assert_match_args_with_cwd(
        args: Vec<String>,
        cwd: &PathUri,
        expected_workdir: Option<&str>,
    ) {
        match maybe_parse_apply_patch(&args, cwd) {
            MaybeApplyPatch::Body(ApplyPatchArgs { hunks, workdir, .. }) => {
                assert_eq!(workdir.as_deref(), expected_workdir);
                assert_eq!(hunks, expected_single_add());
            }
            result => panic!("expected MaybeApplyPatch::Body got {result:?}"),
        }
    }

    #[track_caller]
    fn assert_match(script: &str, expected_workdir: Option<&str>) {
        let args = args_bash(script);
        assert_match_args(args, expected_workdir);
    }

    fn assert_not_match(script: &str) {
        let args = args_bash(script);
        assert_matches!(
            maybe_parse_apply_patch(
                &args,
                &PathUri::parse("file:///workspace").expect("valid POSIX test cwd"),
            ),
            MaybeApplyPatch::NotApplyPatch
        );
    }

    #[tokio::test]
    async fn test_implicit_patch_single_arg_is_error() {
        let patch = "*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch".to_string();
        let args = vec![patch];
        let dir = tempdir().unwrap();
        assert_matches!(
            maybe_parse_apply_patch_verified(
                &args,
                &PathUri::from_host_native_path(dir.path()).expect("absolute test path"),
                LOCAL_FS.as_ref(),
                /*sandbox*/ None,
            )
            .await,
            MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation)
        );
    }

    #[tokio::test]
    async fn test_implicit_patch_bash_script_is_error() {
        let script = "*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch";
        let args = args_bash(script);
        let dir = tempdir().unwrap();
        assert_matches!(
            maybe_parse_apply_patch_verified(
                &args,
                &PathUri::from_host_native_path(dir.path()).expect("absolute test path"),
                LOCAL_FS.as_ref(),
                /*sandbox*/ None,
            )
            .await,
            MaybeApplyPatchVerified::CorrectnessError(ApplyPatchError::ImplicitInvocation)
        );
    }

    #[tokio::test]
    async fn test_literal() {
        let args = strs_to_strings(&[
            "apply_patch",
            r#"*** Begin Patch
*** Add File: foo
+hi
*** End Patch
"#,
        ]);

        match maybe_parse_apply_patch(
            &args,
            &PathUri::parse("file:///workspace").expect("valid POSIX test cwd"),
        ) {
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

    #[tokio::test]
    async fn test_literal_applypatch() {
        let args = strs_to_strings(&[
            "applypatch",
            r#"*** Begin Patch
*** Add File: foo
+hi
*** End Patch
"#,
        ]);

        match maybe_parse_apply_patch(
            &args,
            &PathUri::parse("file:///workspace").expect("valid POSIX test cwd"),
        ) {
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

    #[tokio::test]
    async fn test_heredoc() {
        assert_match(&heredoc_script(""), /*expected_workdir*/ None);
    }

    #[tokio::test]
    async fn test_heredoc_non_login_shell() {
        let script = heredoc_script("");
        let args = strs_to_strings(&["bash", "-c", &script]);
        assert_match_args(args, /*expected_workdir*/ None);
    }

    #[tokio::test]
    async fn test_heredoc_applypatch() {
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

        match maybe_parse_apply_patch(
            &args,
            &PathUri::parse("file:///workspace").expect("valid POSIX test cwd"),
        ) {
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

    #[tokio::test]
    async fn test_powershell_heredoc() {
        let script = heredoc_script("");
        assert_match_args(args_powershell(&script), /*expected_workdir*/ None);
    }
    #[tokio::test]
    async fn test_powershell_heredoc_no_profile() {
        let script = heredoc_script("");
        assert_match_args(
            args_powershell_no_profile(&script),
            /*expected_workdir*/ None,
        );
    }
    #[tokio::test]
    async fn test_pwsh_heredoc() {
        let script = heredoc_script("");
        assert_match_args(args_pwsh(&script), /*expected_workdir*/ None);
    }

    #[tokio::test]
    async fn test_apply_patch_interception_uses_cwd_convention_for_windows_pwsh_path() {
        let script = heredoc_script("");
        assert_match_args_with_cwd(
            strs_to_strings(&[
                r"C:\Program Files\PowerShell\7\pwsh.exe",
                "-NoProfile",
                "-Command",
                &script,
            ]),
            &PathUri::parse("file:///C:/windows").expect("valid Windows test cwd"),
            /*expected_workdir*/ None,
        );
    }

    #[tokio::test]
    async fn test_cmd_heredoc_with_cd() {
        let script = heredoc_script("cd foo && ");
        assert_match_args(args_cmd(&script), Some("foo"));
    }

    #[tokio::test]
    async fn test_heredoc_with_leading_cd() {
        assert_match(&heredoc_script("cd foo && "), Some("foo"));
    }

    #[tokio::test]
    async fn test_cd_with_semicolon_is_ignored() {
        assert_not_match(&heredoc_script("cd foo; "));
    }

    #[tokio::test]
    async fn test_cd_or_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("cd bar || "));
    }

    #[tokio::test]
    async fn test_cd_pipe_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("cd bar | "));
    }

    #[tokio::test]
    async fn test_cd_single_quoted_path_with_spaces() {
        assert_match(&heredoc_script("cd 'foo bar' && "), Some("foo bar"));
    }

    #[tokio::test]
    async fn test_cd_double_quoted_path_with_spaces() {
        assert_match(&heredoc_script("cd \"foo bar\" && "), Some("foo bar"));
    }

    #[tokio::test]
    async fn test_echo_and_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("echo foo && "));
    }

    #[tokio::test]
    async fn test_apply_patch_with_arg_is_ignored() {
        let script = "apply_patch foo <<'PATCH'\n*** Begin Patch\n*** Add File: foo\n+hi\n*** End Patch\nPATCH";
        assert_not_match(script);
    }

    #[tokio::test]
    async fn test_double_cd_then_apply_patch_is_ignored() {
        assert_not_match(&heredoc_script("cd foo && cd bar && "));
    }

    #[tokio::test]
    async fn test_cd_two_args_is_ignored() {
        assert_not_match(&heredoc_script("cd foo bar && "));
    }

    #[tokio::test]
    async fn test_cd_then_apply_patch_then_extra_is_ignored() {
        let script = heredoc_script_ps("cd bar && ", " && echo done");
        assert_not_match(&script);
    }

    #[tokio::test]
    async fn test_echo_then_cd_and_apply_patch_is_ignored() {
        // Ensure preceding commands before the `cd && apply_patch <<...` sequence do not match.
        assert_not_match(&heredoc_script("echo foo; cd bar && "));
    }

    #[tokio::test]
    async fn test_unified_diff_last_line_replacement() {
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

        let path_uri = PathUri::from_host_native_path(&path).expect("absolute test path");
        let diff =
            unified_diff_from_chunks(&path_uri, chunks, LOCAL_FS.as_ref(), /*sandbox*/ None)
                .await
                .unwrap();
        let expected_diff = r#"@@ -2,2 +2,2 @@
 bar
-baz
+BAZ
"#;
        let expected = ApplyPatchFileUpdate {
            unified_diff: expected_diff.to_string(),
            original_content: "foo\nbar\nbaz\n".to_string(),
            content: "foo\nbar\nBAZ\n".to_string(),
        };
        assert_eq!(expected, diff);
    }

    #[tokio::test]
    async fn test_unified_diff_insert_at_eof() {
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

        let path_uri = PathUri::from_host_native_path(&path).expect("absolute test path");
        let diff =
            unified_diff_from_chunks(&path_uri, chunks, LOCAL_FS.as_ref(), /*sandbox*/ None)
                .await
                .unwrap();
        let expected_diff = r#"@@ -3 +3,2 @@
 baz
+quux
"#;
        let expected = ApplyPatchFileUpdate {
            unified_diff: expected_diff.to_string(),
            original_content: "foo\nbar\nbaz\n".to_string(),
            content: "foo\nbar\nbaz\nquux\n".to_string(),
        };
        assert_eq!(expected, diff);
    }

    #[tokio::test]
    async fn test_apply_patch_should_resolve_absolute_paths_in_cwd() {
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

        let result = maybe_parse_apply_patch_verified(
            &argv,
            &PathUri::from_host_native_path(session_dir.path()).expect("absolute test path"),
            LOCAL_FS.as_ref(),
            /*sandbox*/ None,
        )
        .await;

        // Verify the patch contents - as otherwise we may have pulled contents
        // from the wrong file (as we're using relative paths)
        assert_eq!(
            result,
            MaybeApplyPatchVerified::Body(ApplyPatchAction {
                changes: HashMap::from([(
                    PathUri::from_host_native_path(session_dir.path().join(relative_path))
                        .expect("absolute test path"),
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
                cwd: PathUri::from_host_native_path(session_dir.path())
                    .expect("absolute test path"),
            })
        );
    }

    #[tokio::test]
    async fn test_apply_patch_resolves_move_path_with_effective_cwd() {
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

        let result = maybe_parse_apply_patch_verified(
            &argv,
            &PathUri::from_host_native_path(session_dir.path()).expect("absolute test path"),
            LOCAL_FS.as_ref(),
            /*sandbox*/ None,
        )
        .await;
        let action = match result {
            MaybeApplyPatchVerified::Body(action) => action,
            other => panic!("expected verified body, got {other:?}"),
        };

        assert_eq!(
            action.cwd.to_abs_path().unwrap().as_path(),
            worktree_dir.as_path()
        );

        let source_path = PathUri::from_host_native_path(worktree_dir.join(source_name))
            .expect("absolute test path");
        let change = action
            .changes()
            .get(&source_path)
            .expect("source file change present");

        match change {
            ApplyPatchFileChange::Update { move_path, .. } => {
                let expected_move_path =
                    PathUri::from_host_native_path(worktree_dir.join(dest_name))
                        .expect("absolute test path");
                assert_eq!(move_path.as_ref(), Some(&expected_move_path));
            }
            other => panic!("expected update change, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_unreadable_destinations_still_verify() {
        let session_dir = tempdir().unwrap();
        fs::write(session_dir.path().join("binary.dat"), [0xff, 0xfe, 0xfd]).unwrap();
        let cwd = PathUri::from_host_native_path(session_dir.path()).expect("absolute test path");
        let add_argv = vec![
            "apply_patch".to_string(),
            "*** Begin Patch\n*** Add File: binary.dat\n+text\n*** End Patch".to_string(),
        ];
        fs::write(session_dir.path().join("source.txt"), "before\n").unwrap();
        let move_argv = vec![
            "apply_patch".to_string(),
            "*** Begin Patch\n*** Update File: source.txt\n*** Move to: binary.dat\n@@\n-before\n+after\n*** End Patch".to_string(),
        ];

        for argv in [add_argv, move_argv] {
            let result = maybe_parse_apply_patch_verified(
                &argv,
                &cwd,
                LOCAL_FS.as_ref(),
                /*sandbox*/ None,
            )
            .await;

            assert!(matches!(result, MaybeApplyPatchVerified::Body(_)));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_delete_symlink_still_verifies() {
        use std::os::unix::fs::symlink;

        let session_dir = tempdir().unwrap();
        fs::write(session_dir.path().join("target.txt"), "target\n").unwrap();
        symlink(
            session_dir.path().join("target.txt"),
            session_dir.path().join("link.txt"),
        )
        .unwrap();
        let argv = vec![
            "apply_patch".to_string(),
            "*** Begin Patch\n*** Delete File: link.txt\n*** End Patch".to_string(),
        ];

        let result = maybe_parse_apply_patch_verified(
            &argv,
            &PathUri::from_host_native_path(session_dir.path()).expect("absolute test path"),
            LOCAL_FS.as_ref(),
            /*sandbox*/ None,
        )
        .await;

        assert!(matches!(result, MaybeApplyPatchVerified::Body(_)));
    }
}
