//! Helpers for applying unified diffs using the system `git` binary.
//!
//! The entry point is [`apply_git_patch`], which writes a diff to a temporary
//! file, shells out to `git apply` with the right flags, and then parses the
//! command’s output into structured details. Callers can opt into dry-run
//! mode via [`ApplyGitRequest::preflight`] and inspect the resulting paths to
//! learn what would change before applying for real.

use once_cell::sync::Lazy;
use regex::Regex;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::FsmonitorOverride;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::EXECUTABLE_FILTER_CONFIG_PATTERN;
use crate::safe_git::EXECUTABLE_PATCH_CONFIG_PATTERN;
use crate::safe_git::ensure_no_executable_git_config;

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
    let mut cfg_parts = configured_git_config_parts();
    let git_root = resolve_git_root(&req.cwd, &cfg_parts)?;
    ensure_no_executable_git_config(&git_root, EXECUTABLE_PATCH_CONFIG_PATTERN, &cfg_parts)?;

    // Write unified diff into a temporary file
    let (tmpdir, patch_path) = write_temp_patch(&req.diff)?;
    // Keep tmpdir alive until function end to ensure the file exists
    let _guard = tmpdir;
    let patch_paths = extract_effective_paths_from_patch(&patch_path, req.revert)?;
    ensure_paths_do_not_enter_submodules(&git_root, &patch_paths)?;

    if req.revert && !req.preflight {
        // Stage WT paths first to avoid index mismatch on revert.
        stage_effective_paths(&git_root, &patch_paths)?;
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
        let (c_code, c_out, c_err) = run_git(&git_root, &cfg_parts, &check_args)?;
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
    let (code, stdout, stderr) = run_git(&git_root, &cfg_parts, &args)?;

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

fn resolve_git_root(cwd: &Path, git_config_args: &[String]) -> io::Result<PathBuf> {
    let requested_cwd = std::fs::canonicalize(cwd)?;
    let out = std::process::Command::new("git")
        .args(git_config_args)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(&requested_cwd)
        .output()?;
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

fn run_git(cwd: &Path, git_cfg: &[String], args: &[String]) -> io::Result<(i32, String, String)> {
    let mut cmd = std::process::Command::new("git");
    for p in git_cfg {
        cmd.arg(p);
    }
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.current_dir(cwd).output()?;
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

fn extract_effective_paths_from_patch(patch_path: &Path, revert: bool) -> io::Result<Vec<String>> {
    let forward_paths = git_apply_numstat_paths(patch_path, revert)?;
    // `git apply --numstat` reports only the destination of a rename. Parse the
    // opposite orientation too so the submodule guard covers both endpoints.
    let reverse_paths = git_apply_numstat_paths(patch_path, !revert)?;
    if forward_paths.len() != reverse_paths.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "forward and reverse patch parsing returned different path counts",
        ));
    }
    let effective_paths: std::collections::BTreeSet<String> =
        forward_paths.into_iter().chain(reverse_paths).collect();
    if effective_paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "patch does not identify any paths",
        ));
    }
    effective_paths
        .into_iter()
        .map(validate_patch_path)
        .collect()
}

/// Best-effort extraction of the paths Git would apply.
///
/// Security-sensitive callers must use the fallible internal extractor so an
/// invalid or ambiguous patch is rejected instead of becoming an empty list.
pub fn extract_paths_from_patch(diff_text: &str) -> Vec<String> {
    let Ok((tmpdir, patch_path)) = write_temp_patch(diff_text) else {
        return Vec::new();
    };
    let paths =
        extract_effective_paths_from_patch(&patch_path, /*revert*/ false).unwrap_or_default();
    drop(tmpdir);
    paths
}

fn git_apply_numstat_paths(patch_path: &Path, revert: bool) -> io::Result<Vec<String>> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["apply", "--numstat", "-z"]);
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
    ] {
        cmd.env_remove(name);
    }
    if revert {
        cmd.arg("-R");
    }
    let out = cmd
        .arg("--")
        .arg(patch_path)
        .current_dir(patch_path.parent().unwrap_or_else(|| Path::new(".")))
        .output()?;
    if !out.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "failed to parse patch paths: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }

    parse_numstat_paths(&out.stdout)
}

fn parse_numstat_paths(output: &[u8]) -> io::Result<Vec<String>> {
    if !output.is_empty() && !output.ends_with(&[0]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "git apply returned an unterminated numstat path record",
        ));
    }
    let mut paths = Vec::new();
    let mut records = output.split(|byte| *byte == 0).peekable();
    while let Some(record) = records.next() {
        if record.is_empty() && records.peek().is_none() {
            break;
        }
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let _added = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "git apply returned an ambiguous numstat path record",
            )
        })?;
        let _deleted = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "git apply returned an ambiguous numstat path record",
            )
        })?;
        let path = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "git apply returned an ambiguous numstat path record",
            )
        })?;
        if path.is_empty() {
            let old = records
                .next()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "git apply returned an incomplete rename path record",
                    )
                })?;
            let new = records
                .next()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "git apply returned an incomplete rename path record",
                    )
                })?;
            insert_numstat_path(&mut paths, old)?;
            insert_numstat_path(&mut paths, new)?;
        } else {
            insert_numstat_path(&mut paths, path)?;
        }
    }
    Ok(paths)
}

fn insert_numstat_path(paths: &mut Vec<String>, path: &[u8]) -> io::Result<()> {
    let path = std::str::from_utf8(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "git apply returned a non-UTF-8 patch path",
        )
    })?;
    paths.push(path.to_string());
    Ok(())
}

fn validate_patch_path(path: String) -> io::Result<String> {
    if path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.as_bytes().get(1) == Some(&b':')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "patch path is not a normalized repository-relative path",
        ));
    }
    Ok(path)
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
    let (tmpdir, patch_path) = write_temp_patch(diff)?;
    let paths = extract_effective_paths_from_patch(&patch_path, /*revert*/ true)?;
    let _guard = tmpdir;
    stage_effective_paths(git_root, &paths)
}

fn stage_effective_paths(git_root: &Path, paths: &[String]) -> io::Result<()> {
    ensure_no_executable_git_config(git_root, EXECUTABLE_FILTER_CONFIG_PATTERN, &[])?;
    let mut existing: Vec<String> = Vec::new();
    for p in paths {
        let joined = git_root.join(p);
        if std::fs::symlink_metadata(&joined).is_ok() {
            existing.push(p.clone());
        }
    }
    if existing.is_empty() {
        return Ok(());
    }
    ensure_paths_do_not_enter_submodules(git_root, &existing)?;
    let mut args = vec![
        "--literal-pathspecs".to_string(),
        "add".to_string(),
        "--".to_string(),
    ];
    args.extend(existing);
    let config_parts = safe_git_config_parts();
    let (_code, _, _) = run_git(git_root, &config_parts, &args)?;
    // We do not hard fail staging; best-effort is OK. Return Ok even on non-zero.
    Ok(())
}

fn ensure_paths_do_not_enter_submodules(git_root: &Path, paths: &[String]) -> io::Result<()> {
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to inspect an empty patch path set",
        ));
    }
    let mut candidates = std::collections::BTreeSet::new();
    for path in paths {
        let mut components = path.split('/').filter(|component| !component.is_empty());
        let Some(first) = components.next() else {
            continue;
        };
        let mut candidate = first.to_string();
        candidates.insert(candidate.clone());
        for component in components {
            candidate.push('/');
            candidate.push_str(component);
            candidates.insert(candidate.clone());
        }
    }

    let canonical_root = std::fs::canonicalize(git_root)?;
    let mut canonical_candidates = Vec::new();
    for candidate in &candidates {
        match std::fs::canonicalize(git_root.join(candidate)) {
            Ok(resolved) => {
                let relative = resolved.strip_prefix(&canonical_root).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "patch path alias resolves outside the Git worktree",
                    )
                })?;
                let relative = relative.to_str().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "patch path alias is not valid UTF-8",
                    )
                })?;
                if relative.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "patch path alias resolves to the Git worktree root",
                    ));
                }
                canonical_candidates.push(relative.replace(std::path::MAIN_SEPARATOR, "/"));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    candidates.extend(canonical_candidates);

    let mut args = vec![
        "ls-files".to_string(),
        "--stage".to_string(),
        "-z".to_string(),
        "--".to_string(),
    ];
    args.extend(
        candidates
            .into_iter()
            .map(|candidate| format!(":(icase,literal){candidate}")),
    );
    let config_parts = safe_git_config_parts();
    let (code, stdout, stderr) = run_git(git_root, &config_parts, &args)?;
    if code != 0 {
        return Err(io::Error::other(format!(
            "failed to inspect patch paths for submodules (exit {code}): {}",
            stderr.trim()
        )));
    }
    if stdout
        .split('\0')
        .filter(|record| !record.is_empty())
        .any(|record| record.starts_with("160000 "))
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to stage a patch path that is a submodule or enters a submodule",
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
        let out = std::process::Command::new(args[0])
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

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        // git init and minimal identity
        let _ = run(root, &["git", "init"]);
        let _ = run(root, &["git", "config", "user.email", "codex@example.com"]);
        let _ = run(root, &["git", "config", "user.name", "Codex"]);
        dir
    }

    fn effective_paths(diff: &str, revert: bool) -> io::Result<Vec<String>> {
        let (tmpdir, patch_path) = write_temp_patch(diff)?;
        let paths = extract_effective_paths_from_patch(&patch_path, revert)?;
        drop(tmpdir);
        Ok(paths)
    }

    fn read_file_normalized(path: &Path) -> String {
        std::fs::read_to_string(path)
            .expect("read file")
            .replace("\r\n", "\n")
    }

    fn commit_filter_attributes(root: &Path, tracked_path: &str) {
        std::fs::write(
            root.join(".gitattributes"),
            format!("{tracked_path} filter=x=y\n"),
        )
        .expect("write attributes");
        let (add_code, _, add_err) = run(root, &["git", "add", ".gitattributes"]);
        assert_eq!(add_code, 0, "add attributes: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "attributes"]);
        assert_eq!(commit_code, 0, "commit attributes: {commit_err}");
    }

    fn configure_clean_filter(root: &Path, tracked_path: &str) {
        commit_filter_attributes(root, tracked_path);
        let (config_code, _, config_err) = run(
            root,
            &[
                "git",
                "config",
                "filter.x=y.clean",
                "git config codex.filterran true && git hash-object --stdin",
            ],
        );
        assert_eq!(config_code, 0, "configure filter: {config_err}");
    }

    fn configure_worktree_clean_filter(root: &Path, tracked_path: &str) {
        commit_filter_attributes(root, tracked_path);
        let (extension_code, _, extension_err) = run(
            root,
            &["git", "config", "extensions.worktreeConfig", "true"],
        );
        assert_eq!(extension_code, 0, "enable worktree config: {extension_err}");
        let (config_code, _, config_err) = run(
            root,
            &[
                "git",
                "config",
                "--worktree",
                "filter.x=y.clean",
                "git config codex.filterran true && git hash-object --stdin",
            ],
        );
        assert_eq!(config_code, 0, "configure worktree filter: {config_err}");
    }

    fn configured_filter_ran(root: &Path) -> bool {
        let (code, _, _) = run(root, &["git", "config", "--get", "codex.filterran"]);
        code == 0
    }

    fn configure_merge_driver(root: &Path, tracked_path: &str) {
        std::fs::write(
            root.join(".gitattributes"),
            format!("{tracked_path} merge=codex-test\n"),
        )
        .expect("write attributes");
        let (add_code, _, add_err) = run(root, &["git", "add", ".gitattributes"]);
        assert_eq!(add_code, 0, "add attributes: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "attributes"]);
        assert_eq!(commit_code, 0, "commit attributes: {commit_err}");
        let (config_code, _, config_err) = run(
            root,
            &[
                "git",
                "config",
                "merge.codex-test.driver",
                "git config codex.mergeran true && false",
            ],
        );
        assert_eq!(config_code, 0, "configure merge driver: {config_err}");
    }

    fn init_submodule_with_clean_filter(parent: &Path) {
        let source = tempfile::tempdir().expect("submodule source");
        let source_root = source.path();
        let _ = run(source_root, &["git", "init"]);
        let _ = run(
            source_root,
            &["git", "config", "user.email", "codex@example.com"],
        );
        let _ = run(source_root, &["git", "config", "user.name", "Codex"]);
        std::fs::write(source_root.join("file.txt"), "original\n").expect("write submodule file");
        std::fs::write(
            source_root.join(".gitattributes"),
            "file.txt filter=codex-test\n",
        )
        .expect("write submodule attributes");
        let _ = run(source_root, &["git", "add", "."]);
        let _ = run(source_root, &["git", "commit", "-m", "seed"]);

        let source_path = source_root.to_string_lossy().into_owned();
        let (add_code, _, add_err) = run(
            parent,
            &[
                "git",
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                &source_path,
                "nested",
            ],
        );
        assert_eq!(add_code, 0, "add submodule: {add_err}");
        let _ = run(parent, &["git", "commit", "-m", "add submodule"]);

        let nested = parent.join("nested");
        let (config_code, _, config_err) = run(
            &nested,
            &[
                "git",
                "config",
                "filter.codex-test.clean",
                "git config codex.filterran true && git hash-object --stdin",
            ],
        );
        assert_eq!(config_code, 0, "configure submodule filter: {config_err}");
        std::fs::write(nested.join("file.txt"), "modified\n").expect("dirty submodule file");
    }

    #[test]
    fn effective_paths_cover_supported_patch_headers() {
        let cases = [
            (
                "quoted new file",
                "diff --git \"a/hello world.txt\" \"b/hello world.txt\"\nnew file mode 100644\n--- /dev/null\n+++ b/hello world.txt\n@@ -0,0 +1 @@\n+hi\n",
                vec!["hello world.txt"],
            ),
            (
                "unquoted spaced path",
                "diff --git a/space name.txt b/space name.txt\n--- a/space name.txt\n+++ b/space name.txt\n@@ -1 +1 @@\n-old\n+new\n",
                vec!["space name.txt"],
            ),
            (
                "headerless p0 inference",
                "--- headerless-p0.txt\n+++ headerless-p0.txt\n@@ -1 +1 @@\n-old\n+new\n",
                vec!["headerless-p0.txt"],
            ),
            (
                "headerless unified diff",
                "--- old/headerless.txt\n+++ new/headerless.txt\n@@ -1 +1 @@\n-old\n+new\n",
                vec!["headerless.txt"],
            ),
            (
                "arbitrary prefixes",
                "diff --git left/file.txt right/file.txt\n--- before/file.txt\n+++ after/file.txt\n@@ -1 +1 @@\n-old\n+new\n",
                vec!["file.txt"],
            ),
            (
                "deleted file",
                "diff --git a/gone.txt b/gone.txt\ndeleted file mode 100644\n--- a/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-old\n",
                vec!["gone.txt"],
            ),
            (
                "literal dev/null path",
                "diff --git a/dev/null b/dev/null\n--- a/dev/null\n+++ b/dev/null\n@@ -1 +1 @@\n-old\n+new\n",
                vec!["dev/null"],
            ),
            (
                "rename",
                "diff --git a/rename-old.txt b/rename-new.txt\nsimilarity index 100%\nrename from rename-old.txt\nrename to rename-new.txt\n",
                vec!["rename-new.txt", "rename-old.txt"],
            ),
            (
                "copy",
                "diff --git a/copy-old.txt b/copy-new.txt\nsimilarity index 100%\ncopy from copy-old.txt\ncopy to copy-new.txt\n",
                vec!["copy-new.txt", "copy-old.txt"],
            ),
        ];

        for (name, diff, expected) in cases {
            for revert in [false, true] {
                assert_eq!(
                    effective_paths(diff, revert).unwrap_or_else(|error| panic!("{name}: {error}")),
                    expected,
                    "{name}, revert={revert}"
                );
            }
            assert_eq!(extract_paths_from_patch(diff), expected, "{name}");
        }

        let nul_rename_paths = parse_numstat_paths(b"0\t0\t\0old name.txt\0new name.txt\0")
            .expect("parse NUL-delimited rename paths");
        assert_eq!(
            nul_rename_paths,
            vec!["old name.txt".to_string(), "new name.txt".to_string()]
        );
    }

    #[test]
    fn effective_paths_follow_git_for_mismatched_headers() {
        let mismatch = "diff --git a/safe.txt b/safe.txt\n--- a/nested/file.txt\n+++ b/nested/file.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let expected = vec!["nested/file.txt".to_string()];
        assert_eq!(
            effective_paths(mismatch, /*revert*/ false).unwrap(),
            expected
        );
        assert_eq!(
            effective_paths(mismatch, /*revert*/ true).unwrap(),
            expected
        );
        assert_eq!(extract_paths_from_patch(mismatch), expected);
    }

    #[test]
    fn effective_paths_reject_platform_ambiguous_paths() {
        let error = effective_paths("", /*revert*/ false).expect_err("reject empty patch paths");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let error = validate_patch_path("..\\nested\\file.txt".to_string())
            .expect_err("reject Windows path separators");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let error =
            validate_patch_path("C:/outside.txt".to_string()).expect_err("reject drive prefix");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
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
    fn apply_rejects_configured_clean_filter_without_running_it() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
        let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
        assert_eq!(add_code, 0, "add file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "seed"]);
        assert_eq!(commit_code, 0, "commit file: {commit_err}");
        configure_clean_filter(root, "file.txt");

        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+next\n";
        let preflight_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: true,
        };
        let error = apply_git_patch(&preflight_req).expect_err("reject configured filter");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(!configured_filter_ran(root));
        assert_eq!(read_file_normalized(&root.join("file.txt")), "orig\n");

        let stage_error = stage_paths(root, diff).expect_err("reject configured filter");
        assert_eq!(stage_error.kind(), io::ErrorKind::Unsupported);
        assert!(!configured_filter_ran(root));
    }

    #[test]
    fn apply_rejects_configured_merge_driver_without_running_it() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
        let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
        assert_eq!(add_code, 0, "add file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "seed"]);
        assert_eq!(commit_code, 0, "commit file: {commit_err}");
        configure_merge_driver(root, "file.txt");

        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+next\n";
        let request = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let error = apply_git_patch(&request).expect_err("reject configured merge driver");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
        assert_ne!(marker_code, 0, "merge driver must not run");
        assert_eq!(read_file_normalized(&root.join("file.txt")), "orig\n");
    }

    #[test]
    fn apply_rejects_worktree_scoped_clean_filter_without_running_it() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
        let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
        assert_eq!(add_code, 0, "add file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "seed"]);
        assert_eq!(commit_code, 0, "commit file: {commit_err}");
        configure_worktree_clean_filter(root, "file.txt");

        let request = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+next\n".to_string(),
            revert: false,
            preflight: true,
        };
        let error = apply_git_patch(&request).expect_err("reject worktree filter");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(!configured_filter_ran(root));
        assert_eq!(read_file_normalized(&root.join("file.txt")), "orig\n");
    }

    #[test]
    fn apply_probe_rejects_command_scoped_clean_filter() {
        let repo = init_repo();
        let config_args = vec![
            "-c".to_string(),
            "filter.codex-test.clean=git hash-object --stdin".to_string(),
        ];

        let error = ensure_no_executable_git_config(
            repo.path(),
            EXECUTABLE_PATCH_CONFIG_PATTERN,
            &config_args,
        )
        .expect_err("reject command-scoped filter");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
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

            let error = resolve_git_root(&attacker, &[]).expect_err("reject redirected worktree");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("instead of expected worktree"));
        }
    }

    #[test]
    fn stage_paths_rejects_gitlink_before_entering_submodule() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        init_submodule_with_clean_filter(root);

        let diff = "diff --git a/nested b/nested\nindex 1111111..2222222 160000\n--- a/nested\n+++ b/nested\n@@ -1 +1 @@\n-Subproject commit 1111111111111111111111111111111111111111\n+Subproject commit 2222222222222222222222222222222222222222\n";
        let error = stage_paths(root, diff).expect_err("reject gitlink staging");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(!configured_filter_ran(&root.join("nested")));
    }

    #[cfg(unix)]
    #[test]
    fn gitlink_probe_resolves_filesystem_aliases_and_rejects_escapes() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        init_submodule_with_clean_filter(root);

        std::os::unix::fs::symlink("nested", root.join("alias")).expect("create gitlink alias");
        let error = ensure_paths_do_not_enter_submodules(root, &["alias/file.txt".to_string()])
            .expect_err("reject alias to gitlink");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);

        let outside = tempfile::tempdir().expect("outside directory");
        std::os::unix::fs::symlink(outside.path(), root.join("outside"))
            .expect("create outside alias");
        let error = ensure_paths_do_not_enter_submodules(root, &["outside/file.txt".to_string()])
            .expect_err("reject alias outside worktree");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn patch_variants_reject_paths_inside_submodules() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        init_submodule_with_clean_filter(root);
        let cases = [
            (
                "git-format",
                "diff --git a/nested/file.txt b/nested/file.txt\n--- a/nested/file.txt\n+++ b/nested/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
                io::ErrorKind::Unsupported,
            ),
            (
                "headerless",
                "--- old/nested/file.txt\n+++ new/nested/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
                io::ErrorKind::Unsupported,
            ),
            (
                "case-folded gitlink ancestor",
                "--- old/NESTED/file.txt\n+++ new/NESTED/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
                io::ErrorKind::Unsupported,
            ),
            (
                "mismatched headers",
                "diff --git a/safe.txt b/safe.txt\n--- old/nested/file.txt\n+++ new/nested/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
                io::ErrorKind::Unsupported,
            ),
            (
                "mismatched rename metadata",
                "diff --git a/safe-old.txt b/safe-new.txt\nsimilarity index 100%\nrename from nested/file.txt\nrename to nested/renamed.txt\n",
                io::ErrorKind::Unsupported,
            ),
        ];

        for (name, diff, expected_kind) in cases {
            for (revert, preflight) in [(false, true), (false, false), (true, true), (true, false)]
            {
                let request = ApplyGitRequest {
                    cwd: root.to_path_buf(),
                    diff: diff.to_string(),
                    revert,
                    preflight,
                };
                let error = apply_git_patch(&request).unwrap_err();
                assert_eq!(error.kind(), expected_kind, "{name}");
                assert!(!configured_filter_ran(&root.join("nested")));
            }
        }
    }

    #[test]
    fn headerless_patch_ignores_unrelated_submodule_across_variants() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        init_submodule_with_clean_filter(root);
        ensure_paths_do_not_enter_submodules(root, &[":(glob)nested/**".to_string()])
            .expect("treat patch paths as literal pathspecs");
        std::fs::write(root.join("root.txt"), "old\n").expect("write root file");
        let (add_code, _, add_err) = run(root, &["git", "add", "root.txt"]);
        assert_eq!(add_code, 0, "add root file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "root file"]);
        assert_eq!(commit_code, 0, "commit root file: {commit_err}");
        let diff = "--- old/root.txt\n+++ new/root.txt\n@@ -1 +1 @@\n-old\n+new\n";

        for (revert, preflight, expected) in [
            (false, true, "old\n"),
            (false, false, "new\n"),
            (true, true, "new\n"),
            (true, false, "old\n"),
        ] {
            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff: diff.to_string(),
                revert,
                preflight,
            })
            .unwrap_or_else(|error| panic!("revert={revert}, preflight={preflight}: {error}"));
            assert_eq!(
                result.exit_code, 0,
                "revert={revert}, preflight={preflight}"
            );
            assert_eq!(read_file_normalized(&root.join("root.txt")), expected);
            assert!(!configured_filter_ran(&root.join("nested")));
        }
    }
}
