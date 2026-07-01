//! Helpers for applying unified diffs using the system `git` binary.
//!
//! The entry point is [`apply_git_patch`], which writes a diff to a temporary
//! file, shells out to `git apply` with the right flags, and then parses the
//! command’s output into structured details. Callers can opt into dry-run
//! mode via [`ApplyGitRequest::preflight`] and inspect the resulting paths to
//! learn what would change before applying for real.

use once_cell::sync::Lazy;
use regex::Regex;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::FsmonitorOverride;
use crate::git_command::GitRunner;
use crate::safe_git::DISABLED_HOOKS_PATH;
#[cfg(test)]
use crate::safe_git::isolate_git_command_environment;

/// Parameters for invoking [`apply_git_patch`].
#[derive(Debug, Clone)]
pub struct ApplyGitRequest {
    pub cwd: PathBuf,
    pub diff: String,
    pub revert: bool,
    pub preflight: bool,
}

/// Result of running [`apply_git_patch`], including paths gleaned from stdout/stderr.
#[derive(Debug, Clone)]
pub struct ApplyGitResult {
    pub exit_code: i32,
    pub applied_paths: Vec<String>,
    pub skipped_paths: Vec<String>,
    pub conflicted_paths: Vec<String>,
    pub stdout: String,
    pub stderr: String,
    pub cmd_for_log: String,
}

/// Apply a unified diff to the target repository by shelling out to `git apply`.
///
/// When [`ApplyGitRequest::preflight`] is `true`, this behaves like `git apply --check` and
/// leaves the working tree untouched while still parsing the command output for diagnostics.
pub fn apply_git_patch(req: &ApplyGitRequest) -> io::Result<ApplyGitResult> {
    let git = GitRunner::for_cwd_io(&req.cwd)?;
    let mut cfg_parts = configured_git_config_parts();
    let git_root = resolve_git_root(&git, &req.cwd, &cfg_parts)?;

    // Write unified diff into a temporary file
    let (tmpdir, patch_path) = write_temp_patch(&req.diff)?;
    // Keep tmpdir alive until function end to ensure the file exists
    let _guard = tmpdir;

    if req.revert && !req.preflight {
        // Stage WT paths first to avoid index mismatch on revert.
        stage_paths(&git_root, &req.diff)?;
    }

    // Build git args
    let mut args: Vec<String> = vec!["apply".into(), "--3way".into()];
    if req.revert {
        args.push("-R".into());
    }

    cfg_parts.extend(safe_git_config_parts());

    args.push(patch_path.to_string_lossy().to_string());

    // Optional preflight: dry-run only; do not modify working tree
    if req.preflight {
        let mut check_args = vec!["apply".to_string(), "--check".to_string()];
        if req.revert {
            check_args.push("-R".to_string());
        }
        check_args.push(patch_path.to_string_lossy().to_string());
        let rendered = render_command_for_log(&git_root, &cfg_parts, &check_args);
        let (c_code, c_out, c_err) = run_git(&git, &git_root, &cfg_parts, &check_args)?;
        let (mut applied_paths, mut skipped_paths, mut conflicted_paths) =
            parse_git_apply_output(&c_out, &c_err);
        applied_paths.sort();
        applied_paths.dedup();
        skipped_paths.sort();
        skipped_paths.dedup();
        conflicted_paths.sort();
        conflicted_paths.dedup();
        return Ok(ApplyGitResult {
            exit_code: c_code,
            applied_paths,
            skipped_paths,
            conflicted_paths,
            stdout: c_out,
            stderr: c_err,
            cmd_for_log: rendered,
        });
    }

    let cmd_for_log = render_command_for_log(&git_root, &cfg_parts, &args);
    let (code, stdout, stderr) = run_git(&git, &git_root, &cfg_parts, &args)?;

    let (mut applied_paths, mut skipped_paths, mut conflicted_paths) =
        parse_git_apply_output(&stdout, &stderr);
    applied_paths.sort();
    applied_paths.dedup();
    skipped_paths.sort();
    skipped_paths.dedup();
    conflicted_paths.sort();
    conflicted_paths.dedup();

    Ok(ApplyGitResult {
        exit_code: code,
        applied_paths,
        skipped_paths,
        conflicted_paths,
        stdout,
        stderr,
        cmd_for_log,
    })
}

fn resolve_git_root(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
) -> io::Result<PathBuf> {
    let requested_cwd = std::fs::canonicalize(cwd)?;
    let mut command = git.command();
    command
        .args(git_config_args)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(&requested_cwd);
    let out = git.output(command)?;
    let code = out.status.code().unwrap_or(-1);
    if code != 0 {
        return Err(io::Error::other(format!(
            "not a git repository (exit {}): {}",
            code,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let reported_root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let root = std::fs::canonicalize(&reported_root)?;
    let expected_root = crate::get_git_repo_root(&requested_cwd)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "refusing to apply a patch because Git resolved worktree {} without a .git marker above requested cwd {}",
                    root.display(),
                    requested_cwd.display()
                ),
            )
        })
        .and_then(std::fs::canonicalize)?;
    if root != expected_root {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "refusing to apply a patch because Git resolved worktree {} instead of expected worktree {} for requested cwd {}",
                root.display(),
                expected_root.display(),
                requested_cwd.display()
            ),
        ));
    }
    Ok(root)
}

fn configured_git_config_parts() -> Vec<String> {
    let mut cfg_parts = Vec::new();
    if let Ok(cfg) = std::env::var("CODEX_APPLY_GIT_CFG") {
        for pair in cfg.split(',') {
            let pair = pair.trim();
            if pair.is_empty() || !pair.contains('=') {
                continue;
            }
            cfg_parts.push("-c".to_string());
            cfg_parts.push(pair.to_string());
        }
    }
    cfg_parts
}

fn write_temp_patch(diff: &str) -> io::Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("patch.diff");
    std::fs::write(&path, diff)?;
    Ok((dir, path))
}

fn run_git(
    git: &GitRunner,
    cwd: &Path,
    git_cfg: &[String],
    args: &[String],
) -> io::Result<(i32, String, String)> {
    let mut cmd = git.command();
    for p in git_cfg {
        cmd.arg(p);
    }
    for a in args {
        cmd.arg(a);
    }
    cmd.current_dir(cwd);
    let out = git.output(cmd)?;
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((code, stdout, stderr))
}

fn safe_git_config_parts() -> Vec<String> {
    vec![
        "-c".to_string(),
        format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
        "-c".to_string(),
        FsmonitorOverride::Disabled.git_config_arg().to_string(),
    ]
}

fn quote_shell(s: &str) -> String {
    let simple = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_.:/@%+".contains(c));
    if simple {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn render_command_for_log(cwd: &Path, git_cfg: &[String], args: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push("git".to_string());
    for a in git_cfg {
        parts.push(quote_shell(a));
    }
    for a in args {
        parts.push(quote_shell(a));
    }
    format!(
        "(cd {} && {})",
        quote_shell(&cwd.display().to_string()),
        parts.join(" ")
    )
}

/// Collect every path referenced by the diff headers inside `diff --git` sections.
pub fn extract_paths_from_patch(diff_text: &str) -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    for raw_line in diff_text.lines() {
        let line = raw_line.trim();
        let Some(rest) = line.strip_prefix("diff --git ") else {
            continue;
        };
        let Some((a, b)) = parse_diff_git_paths(rest) else {
            continue;
        };
        if let Some(a) = normalize_diff_path(&a, "a/") {
            set.insert(a);
        }
        if let Some(b) = normalize_diff_path(&b, "b/") {
            set.insert(b);
        }
    }
    set.into_iter().collect()
}

fn parse_diff_git_paths(line: &str) -> Option<(String, String)> {
    let mut chars = line.chars().peekable();
    let first = read_diff_git_token(&mut chars)?;
    let second = read_diff_git_token(&mut chars)?;
    Some((first, second))
}

fn read_diff_git_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }
    let quote = match chars.peek().copied() {
        Some('"') | Some('\'') => chars.next(),
        _ => None,
    };
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                break;
            }
            if c == '\\' {
                out.push('\\');
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
        } else if c.is_whitespace() {
            break;
        }
        out.push(c);
    }
    if out.is_empty() && quote.is_none() {
        None
    } else {
        Some(match quote {
            Some(_) => unescape_c_string(&out),
            None => out,
        })
    }
}

fn normalize_diff_path(raw: &str, prefix: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "/dev/null" || trimmed == format!("{prefix}dev/null") {
        return None;
    }
    let trimmed = trimmed.strip_prefix(prefix).unwrap_or(trimmed);
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn unescape_c_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        match next {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000C}'),
            'a' => out.push('\u{0007}'),
            'v' => out.push('\u{000B}'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '0'..='7' => {
                let mut value = next.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match chars.peek() {
                        Some('0'..='7') => {
                            if let Some(digit) = chars.next() {
                                value = value * 8 + digit.to_digit(8).unwrap_or(0);
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                if let Some(ch) = std::char::from_u32(value) {
                    out.push(ch);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Stage only the files that actually exist on disk for the given diff.
pub fn stage_paths(git_root: &Path, diff: &str) -> io::Result<()> {
    let git = GitRunner::for_cwd_io(git_root)?;
    let paths = extract_paths_from_patch(diff);
    let mut existing: Vec<String> = Vec::new();
    for p in paths {
        let joined = git_root.join(&p);
        if std::fs::symlink_metadata(&joined).is_ok() {
            existing.push(p);
        }
    }
    if existing.is_empty() {
        return Ok(());
    }
    let mut cmd = git.command();
    cmd.args(safe_git_config_parts());
    cmd.arg("add");
    cmd.arg("--");
    for p in &existing {
        cmd.arg(OsStr::new(p));
    }
    cmd.current_dir(git_root);
    let out = git.output(cmd)?;
    let _code = out.status.code().unwrap_or(-1);
    // We do not hard fail staging; best-effort is OK. Return Ok even on non-zero.
    Ok(())
}

// ============ Parser ported from VS Code (TS) ============

/// Parse `git apply` output into applied/skipped/conflicted path groupings.
pub fn parse_git_apply_output(
    stdout: &str,
    stderr: &str,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let combined = [stdout, stderr]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    let mut applied = std::collections::BTreeSet::new();
    let mut skipped = std::collections::BTreeSet::new();
    let mut conflicted = std::collections::BTreeSet::new();
    let mut last_seen_path: Option<String> = None;

    fn add(set: &mut std::collections::BTreeSet<String>, raw: &str) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let first = trimmed.chars().next().unwrap_or('\0');
        let last = trimmed.chars().last().unwrap_or('\0');
        let unquoted = if (first == '"' || first == '\'') && last == first && trimmed.len() >= 2 {
            unescape_c_string(&trimmed[1..trimmed.len() - 1])
        } else {
            trimmed.to_string()
        };
        if !unquoted.is_empty() {
            set.insert(unquoted);
        }
    }

    static APPLIED_CLEAN: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Applied patch(?: to)?\\s+(?P<path>.+?)\\s+cleanly\\.?$"));
    static APPLIED_CONFLICTS: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Applied patch(?: to)?\\s+(?P<path>.+?)\\s+with conflicts\\.?$"));
    static APPLYING_WITH_REJECTS: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^Applying patch\\s+(?P<path>.+?)\\s+with\\s+\\d+\\s+rejects?\\.{0,3}$")
    });
    static CHECKING_PATCH: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Checking patch\\s+(?P<path>.+?)\\.\\.\\.$"));
    static UNMERGED_LINE: Lazy<Regex> = Lazy::new(|| regex_ci("^U\\s+(?P<path>.+)$"));
    static PATCH_FAILED: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+patch failed:\\s+(?P<path>.+?)(?::\\d+)?(?:\\s|$)"));
    static DOES_NOT_APPLY: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+(?P<path>.+?):\\s+patch does not apply$"));
    static THREE_WAY_START: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^(?:Performing three-way merge|Falling back to three-way merge)\\.\\.\\.$")
    });
    static THREE_WAY_FAILED: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Failed to perform three-way merge\\.\\.\\.$"));
    static FALLBACK_DIRECT: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Falling back to direct application\\.\\.\\.$"));
    static LACKS_BLOB: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^(?:error: )?repository lacks the necessary blob to (?:perform|fall back on) 3-?way merge\\.?$",
        )
    });
    static INDEX_MISMATCH: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+(?P<path>.+?):\\s+does not match index\\b"));
    static NOT_IN_INDEX: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+(?P<path>.+?):\\s+does not exist in index\\b"));
    static ALREADY_EXISTS_WT: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^error:\\s+(?P<path>.+?)\\s+already exists in (?:the )?working directory\\b")
    });
    static FILE_EXISTS: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+patch failed:\\s+(?P<path>.+?)\\s+File exists"));
    static RENAMED_DELETED: Lazy<Regex> =
        Lazy::new(|| regex_ci("^error:\\s+path\\s+(?P<path>.+?)\\s+has been renamed\\/deleted"));
    static CANNOT_APPLY_BINARY: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^error:\\s+cannot apply binary patch to\\s+['\\\"]?(?P<path>.+?)['\\\"]?\\s+without full index line$",
        )
    });
    static BINARY_DOES_NOT_APPLY: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^error:\\s+binary patch does not apply to\\s+['\\\"]?(?P<path>.+?)['\\\"]?$")
    });
    static BINARY_INCORRECT_RESULT: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^error:\\s+binary patch to\\s+['\\\"]?(?P<path>.+?)['\\\"]?\\s+creates incorrect result\\b",
        )
    });
    static CANNOT_READ_CURRENT: Lazy<Regex> = Lazy::new(|| {
        regex_ci("^error:\\s+cannot read the current contents of\\s+['\\\"]?(?P<path>.+?)['\\\"]?$")
    });
    static SKIPPED_PATCH: Lazy<Regex> =
        Lazy::new(|| regex_ci("^Skipped patch\\s+['\\\"]?(?P<path>.+?)['\\\"]\\.$"));
    static CANNOT_MERGE_BINARY_WARN: Lazy<Regex> = Lazy::new(|| {
        regex_ci(
            "^warning:\\s*Cannot merge binary files:\\s+(?P<path>.+?)\\s+\\(ours\\s+vs\\.\\s+theirs\\)",
        )
    });

    for raw_line in combined.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // === "Checking patch <path>..." tracking ===
        if let Some(c) = CHECKING_PATCH.captures(line) {
            if let Some(m) = c.name("path") {
                last_seen_path = Some(m.as_str().to_string());
            }
            continue;
        }

        // === Status lines ===
        if let Some(c) = APPLIED_CLEAN.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut applied, m.as_str());
                let p = applied.iter().next_back().cloned();
                if let Some(p) = p {
                    conflicted.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }
        if let Some(c) = APPLIED_CONFLICTS.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }
        if let Some(c) = APPLYING_WITH_REJECTS.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }

        // === “U <path>” after conflicts ===
        if let Some(c) = UNMERGED_LINE.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }

        // === Early hints ===
        if PATCH_FAILED.is_match(line) || DOES_NOT_APPLY.is_match(line) {
            if let Some(c) = PATCH_FAILED
                .captures(line)
                .or_else(|| DOES_NOT_APPLY.captures(line))
                && let Some(m) = c.name("path")
            {
                add(&mut skipped, m.as_str());
                last_seen_path = Some(m.as_str().to_string());
            }
            continue;
        }

        // === Ignore narration ===
        if THREE_WAY_START.is_match(line) || FALLBACK_DIRECT.is_match(line) {
            continue;
        }

        // === 3-way failed entirely; attribute to last_seen_path ===
        if THREE_WAY_FAILED.is_match(line) || LACKS_BLOB.is_match(line) {
            if let Some(p) = last_seen_path.clone() {
                add(&mut skipped, &p);
                applied.remove(&p);
                conflicted.remove(&p);
            }
            continue;
        }

        // === Skips / I/O problems ===
        if let Some(c) = INDEX_MISMATCH
            .captures(line)
            .or_else(|| NOT_IN_INDEX.captures(line))
            .or_else(|| ALREADY_EXISTS_WT.captures(line))
            .or_else(|| FILE_EXISTS.captures(line))
            .or_else(|| RENAMED_DELETED.captures(line))
            .or_else(|| CANNOT_APPLY_BINARY.captures(line))
            .or_else(|| BINARY_DOES_NOT_APPLY.captures(line))
            .or_else(|| BINARY_INCORRECT_RESULT.captures(line))
            .or_else(|| CANNOT_READ_CURRENT.captures(line))
            .or_else(|| SKIPPED_PATCH.captures(line))
        {
            if let Some(m) = c.name("path") {
                add(&mut skipped, m.as_str());
                let p_now = skipped.iter().next_back().cloned();
                if let Some(p) = p_now {
                    applied.remove(&p);
                    conflicted.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }

        // === Warnings that imply conflicts ===
        if let Some(c) = CANNOT_MERGE_BINARY_WARN.captures(line) {
            if let Some(m) = c.name("path") {
                add(&mut conflicted, m.as_str());
                let p = conflicted.iter().next_back().cloned();
                if let Some(p) = p {
                    applied.remove(&p);
                    skipped.remove(&p);
                    last_seen_path = Some(p);
                }
            }
            continue;
        }
    }

    // Final precedence: conflicts > applied > skipped
    for p in conflicted.iter() {
        applied.remove(p);
        skipped.remove(p);
    }
    for p in applied.iter() {
        skipped.remove(p);
    }

    (
        applied.into_iter().collect(),
        skipped.into_iter().collect(),
        conflicted.into_iter().collect(),
    )
}

fn regex_ci(pat: &str) -> Regex {
    Regex::new(&format!("(?i){pat}")).unwrap_or_else(|e| panic!("invalid regex: {e}"))
}

#[cfg(all(test, unix))]
#[path = "apply_transport_tests.rs"]
mod transport_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
        let mut command = std::process::Command::new(args[0]);
        isolate_git_command_environment(&mut command);
        let out = command
            .args(&args[1..])
            .current_dir(cwd)
            .output()
            .expect("spawn ok");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn run_isolated_test(test_name: &str, env: &[(&str, &OsStr)]) {
        let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
        isolate_git_command_environment(&mut command);
        command
            .arg(test_name)
            .arg("--exact")
            .arg("--nocapture")
            .env("CODEX_GIT_UTILS_APPLY_ENV_CHILD", "1")
            .env("RUST_TEST_THREADS", "1");
        for (name, value) in env {
            command.env(name, value);
        }
        let output = command.output().expect("run isolated test process");
        assert!(
            output.status.success(),
            "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        // git init and minimal identity
        let _ = run(root, &["git", "init"]);
        let _ = run(root, &["git", "config", "user.email", "codex@example.com"]);
        let _ = run(root, &["git", "config", "user.name", "Codex"]);
        dir
    }

    fn read_file_normalized(path: &Path) -> String {
        std::fs::read_to_string(path)
            .expect("read file")
            .replace("\r\n", "\n")
    }

    #[cfg(unix)]
    fn trusted_git_directory() -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
            .find(|directory| directory.is_absolute() && directory.join("git").is_file())
            .expect("trusted Git directory")
    }

    #[test]
    fn extract_paths_handles_quoted_headers() {
        let diff = "diff --git \"a/hello world.txt\" \"b/hello world.txt\"\nnew file mode 100644\n--- /dev/null\n+++ b/hello world.txt\n@@ -0,0 +1 @@\n+hi\n";
        let paths = extract_paths_from_patch(diff);
        assert_eq!(paths, vec!["hello world.txt".to_string()]);
    }

    #[test]
    fn extract_paths_ignores_dev_null_header() {
        let diff = "diff --git a/dev/null b/ok.txt\nnew file mode 100644\n--- /dev/null\n+++ b/ok.txt\n@@ -0,0 +1 @@\n+hi\n";
        let paths = extract_paths_from_patch(diff);
        assert_eq!(paths, vec!["ok.txt".to_string()]);
    }

    #[test]
    fn extract_paths_unescapes_c_style_in_quoted_headers() {
        let diff = "diff --git \"a/hello\\tworld.txt\" \"b/hello\\tworld.txt\"\nnew file mode 100644\n--- /dev/null\n+++ b/hello\tworld.txt\n@@ -0,0 +1 @@\n+hi\n";
        let paths = extract_paths_from_patch(diff);
        assert_eq!(paths, vec!["hello\tworld.txt".to_string()]);
    }

    #[test]
    fn parse_output_unescapes_quoted_paths() {
        let stderr = "error: patch failed: \"hello\\tworld.txt\":1\n";
        let (applied, skipped, conflicted) = parse_git_apply_output("", stderr);
        assert_eq!(applied, Vec::<String>::new());
        assert_eq!(conflicted, Vec::<String>::new());
        assert_eq!(skipped, vec!["hello\tworld.txt".to_string()]);
    }

    #[test]
    fn apply_add_success() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        let nested_cwd = root.join("nested");
        std::fs::create_dir(&nested_cwd).expect("nested cwd");

        let diff = "diff --git a/hello.txt b/hello.txt\nnew file mode 100644\n--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1,2 @@\n+hello\n+world\n";
        let req = ApplyGitRequest {
            cwd: nested_cwd,
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let r = apply_git_patch(&req).expect("run apply");
        assert_eq!(r.exit_code, 0, "exit code 0");
        // File exists now
        assert!(root.join("hello.txt").exists());
    }

    #[test]
    fn apply_uses_cwd_repo_despite_inherited_repository_selectors() {
        let _g = env_lock().lock().unwrap();
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_ENV_CHILD").is_none() {
            let alternate = init_repo();
            let alternate_root = alternate.path();
            std::fs::write(alternate_root.join("sentinel.txt"), "alternate\n")
                .expect("write alternate sentinel");
            let (add_code, _, add_err) = run(alternate_root, &["git", "add", "sentinel.txt"]);
            assert_eq!(add_code, 0, "add alternate sentinel: {add_err}");
            let (commit_code, _, commit_err) =
                run(alternate_root, &["git", "commit", "-m", "alternate"]);
            assert_eq!(commit_code, 0, "commit alternate sentinel: {commit_err}");

            let alternate_git_dir = alternate_root.join(".git");
            let alternate_index = alternate_git_dir.join("index");
            run_isolated_test(
                "apply::tests::apply_uses_cwd_repo_despite_inherited_repository_selectors",
                &[
                    ("GIT_DIR", alternate_git_dir.as_os_str()),
                    ("GIT_WORK_TREE", alternate_root.as_os_str()),
                    ("GIT_COMMON_DIR", alternate_git_dir.as_os_str()),
                    ("GIT_INDEX_FILE", alternate_index.as_os_str()),
                    ("GIT_PREFIX", OsStr::new("elsewhere/")),
                ],
            );
            assert_eq!(
                read_file_normalized(&alternate_root.join("sentinel.txt")),
                "alternate\n"
            );
            return;
        }

        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("file.txt"), "old\n").expect("write target file");
        let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
        assert_eq!(add_code, 0, "add target file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "target"]);
        assert_eq!(commit_code, 0, "commit target file: {commit_err}");

        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
            revert: false,
            preflight: false,
        })
        .expect("apply in cwd-selected repository");
        assert_eq!(result.exit_code, 0);
        assert_eq!(read_file_normalized(&root.join("file.txt")), "new\n");
    }

    #[cfg(unix)]
    #[test]
    fn apply_uses_logical_process_cwd_to_reject_enclosing_git() {
        use std::os::unix::fs::PermissionsExt;

        let _g = env_lock().lock().unwrap();
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_CHILD").is_none() {
            let fixture = tempfile::tempdir().expect("fixture");
            let outer = fixture.path().join("outer");
            let physical_nested = fixture.path().join("physical-nested");
            let lexical_nested = outer.join("nested");
            let outer_bin = outer.join("bin");
            let outer_git = outer_bin.join("git");
            let marker = outer_bin.join("git.ran");
            std::fs::create_dir_all(&outer_bin).expect("outer Git directory");
            std::fs::create_dir_all(&physical_nested).expect("physical nested repository");
            let (outer_init, _, outer_err) = run(&outer, &["git", "init", "-q"]);
            assert_eq!(outer_init, 0, "init outer repository: {outer_err}");
            let (nested_init, _, nested_err) = run(&physical_nested, &["git", "init", "-q"]);
            assert_eq!(nested_init, 0, "init nested repository: {nested_err}");
            std::os::unix::fs::symlink(&physical_nested, &lexical_nested)
                .expect("symlink nested repository");
            std::fs::write(&outer_git, "#!/bin/sh\nprintf ran >\"$0.ran\"\nexit 1\n")
                .expect("outer Git shim");
            let mut permissions = std::fs::metadata(&outer_git)
                .expect("outer Git metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&outer_git, permissions).expect("executable outer Git");
            let path = std::env::join_paths([outer_bin, trusted_git_directory()]).expect("PATH");

            let mut command =
                std::process::Command::new(std::env::current_exe().expect("test binary"));
            isolate_git_command_environment(&mut command);
            let output = command
                .arg("apply::tests::apply_uses_logical_process_cwd_to_reject_enclosing_git")
                .arg("--exact")
                .arg("--nocapture")
                .current_dir(&lexical_nested)
                .env("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_CHILD", "1")
                .env("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_MARKER", &marker)
                .env("PWD", &lexical_nested)
                .env("PATH", path)
                .env("RUST_TEST_THREADS", "1")
                .output()
                .expect("run isolated logical-cwd test");
            assert!(
                output.status.success(),
                "isolated logical-cwd test failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!marker.exists(), "enclosing Git shim must not run");
            return;
        }

        let cwd = std::env::current_dir().expect("physical process cwd");
        let marker = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_MARKER").expect("marker path"),
        );
        let result = apply_git_patch(&ApplyGitRequest {
            cwd,
            diff: "diff --git a/hello.txt b/hello.txt\nnew file mode 100644\n--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1 @@\n+hello\n".to_string(),
            revert: false,
            preflight: true,
        })
        .expect("preflight through trusted Git");
        assert_eq!(result.exit_code, 0, "preflight should succeed");
        assert!(!marker.exists(), "enclosing Git shim must not run");
    }

    #[test]
    fn apply_modify_conflict() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // seed file and commit
        std::fs::write(root.join("file.txt"), "line1\nline2\nline3\n").unwrap();
        let _ = run(root, &["git", "add", "file.txt"]);
        let _ = run(root, &["git", "commit", "-m", "seed"]);
        // local edit (unstaged)
        std::fs::write(root.join("file.txt"), "line1\nlocal2\nline3\n").unwrap();
        // patch wants to change the same line differently
        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-line2\n+remote2\n line3\n";
        let req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let r = apply_git_patch(&req).expect("run apply");
        assert_ne!(r.exit_code, 0, "non-zero exit on conflict");
    }

    #[test]
    fn apply_modify_skipped_missing_index() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // Try to modify a file that is not in the index
        let diff = "diff --git a/ghost.txt b/ghost.txt\n--- a/ghost.txt\n+++ b/ghost.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let r = apply_git_patch(&req).expect("run apply");
        assert_ne!(r.exit_code, 0, "non-zero exit on missing index");
    }

    #[test]
    fn apply_then_revert_success() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // Seed file and commit original content
        std::fs::write(root.join("file.txt"), "orig\n").unwrap();
        let _ = run(root, &["git", "add", "file.txt"]);
        let _ = run(root, &["git", "commit", "-m", "seed"]);

        // Forward patch: orig -> ORIG
        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+ORIG\n";
        let apply_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let res_apply = apply_git_patch(&apply_req).expect("apply ok");
        assert_eq!(res_apply.exit_code, 0, "forward apply succeeded");
        let after_apply = read_file_normalized(&root.join("file.txt"));
        assert_eq!(after_apply, "ORIG\n");

        // Revert patch: ORIG -> orig (stage paths first; engine handles it)
        let revert_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: true,
            preflight: false,
        };
        let res_revert = apply_git_patch(&revert_req).expect("revert ok");
        assert_eq!(res_revert.exit_code, 0, "revert apply succeeded");
        let after_revert = read_file_normalized(&root.join("file.txt"));
        assert_eq!(after_revert, "orig\n");
    }

    #[test]
    fn revert_preflight_does_not_stage_index() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // Seed repo and apply forward patch so the working tree reflects the change.
        std::fs::write(root.join("file.txt"), "orig\n").unwrap();
        let _ = run(root, &["git", "add", "file.txt"]);
        let _ = run(root, &["git", "commit", "-m", "seed"]);

        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+ORIG\n";
        let apply_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let res_apply = apply_git_patch(&apply_req).expect("apply ok");
        assert_eq!(res_apply.exit_code, 0, "forward apply succeeded");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-am", "apply change"]);
        assert_eq!(commit_code, 0, "commit applied change: {commit_err}");

        let (_code_before, staged_before, _stderr_before) =
            run(root, &["git", "diff", "--cached", "--name-only"]);

        let preflight_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: true,
            preflight: true,
        };
        let res_preflight = apply_git_patch(&preflight_req).expect("preflight ok");
        assert_eq!(res_preflight.exit_code, 0, "revert preflight succeeded");
        let (_code_after, staged_after, _stderr_after) =
            run(root, &["git", "diff", "--cached", "--name-only"]);
        assert_eq!(
            staged_after.trim(),
            staged_before.trim(),
            "preflight should not stage new paths",
        );

        let after_preflight = read_file_normalized(&root.join("file.txt"));
        assert_eq!(after_preflight, "ORIG\n");
    }

    #[test]
    fn preflight_blocks_partial_changes() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // Build a multi-file diff: one valid add (ok.txt) and one invalid modify (ghost.txt)
        let diff = "diff --git a/ok.txt b/ok.txt\nnew file mode 100644\n--- /dev/null\n+++ b/ok.txt\n@@ -0,0 +1,2 @@\n+alpha\n+beta\n\n\
diff --git a/ghost.txt b/ghost.txt\n--- a/ghost.txt\n+++ b/ghost.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";

        // 1) With preflight enabled, nothing should be changed (even though ok.txt could be added)
        let req1 = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: true,
        };
        let r1 = apply_git_patch(&req1).expect("preflight apply");
        assert_ne!(r1.exit_code, 0, "preflight reports failure");
        assert!(
            !root.join("ok.txt").exists(),
            "preflight must prevent adding ok.txt"
        );
        assert!(
            r1.cmd_for_log.contains("--check"),
            "preflight path recorded --check"
        );

        // 2) Without preflight, we should see no --check in the executed command
        let req2 = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let r2 = apply_git_patch(&req2).expect("direct apply");
        assert_ne!(r2.exit_code, 0, "apply is expected to fail overall");
        assert!(
            !r2.cmd_for_log.contains("--check"),
            "non-preflight path should not use --check"
        );
    }

    #[test]
    fn resolve_git_root_rejects_core_worktree_redirection() {
        let temp = tempfile::tempdir().expect("tempdir");
        let attacker = temp.path().join("attacker");
        let victim = temp.path().join("victim");
        std::fs::create_dir_all(&attacker).expect("attacker");
        std::fs::create_dir_all(&victim).expect("victim");
        let (init_code, _, init_err) = run(&attacker, &["git", "init"]);
        assert_eq!(init_code, 0, "init attacker repo: {init_err}");

        for redirected_worktree in [&victim, temp.path()] {
            let redirected_worktree = redirected_worktree.to_string_lossy();
            let (config_code, _, config_err) = run(
                &attacker,
                &["git", "config", "core.worktree", &redirected_worktree],
            );
            assert_eq!(config_code, 0, "configure core.worktree: {config_err}");

            let git = GitRunner::for_cwd_io(&attacker).expect("trusted Git");
            let error =
                resolve_git_root(&git, &attacker, &[]).expect_err("reject redirected worktree");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("instead of expected worktree"));
        }
    }
}
