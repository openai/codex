// src/git_guard.rs
//
// Git Diff Control - PatchGate for PRD-driven, minimal-churn autopilot.
// This module verifies a Proposed Diff Envelope against a Change Contract,
// performs a dry-run (git apply --check), optionally builds/tests, applies,
// commits, and reports detailed stats for TUI/RolloutRecorder.
//
// Requirements:
// - `git` available in PATH.
// - Repository is already initialized and repo_path points to its working tree.
// - You can call these blocking APIs from async code using spawn_blocking.
//
// Design principles:
// - Diff-first, contract-enforced changes with predictable budgets.
// - PRD-aware commit message (Conventional Commit + [TASK_ID]).
// - Minimal external deps: serde + color_eyre; std::process::Command for git.
//
// Author: Platform Architecture
// License: Apache-2.0

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use color_eyre::eyre::{bail, eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::metrics::{inc_ci_runs, Phase};

// Re-export ChangeContract from its own module to keep public path stable
pub use crate::change_contract::ChangeContract;

/// Minimal stats extracted from a unified diff.
/// These stats drive contract enforcement and TUI reporting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub touched_paths: Vec<String>,
    pub has_renames: bool,
    pub has_copies: bool,
    pub has_deletes: bool,
    pub has_binary: bool,
}

#[derive(Debug, Clone, Default)]
struct PerFileStats {
    path: String,
    added_lines: usize,
    removed_lines: usize,
    hunks: usize,
    is_new_file: bool,
    is_symlink: bool,
    exec_mode_change: bool,
    perms_change: bool,
    binary: bool,
    bytes_added: usize,
}

/// Result of an attempted patch application.
/// Use this structure to render badges in TUI and persist to rollout logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyReport {
    pub task_id: String,
    pub checked_ok: bool,
    pub applied: bool,
    pub committed: bool,
    pub commit_sha: Option<String>,
    pub stats: DiffStats,
    pub contract_violations: Vec<String>,
    pub notes: Vec<String>,
}

/// A terse envelope format for the Builder to emit.
/// Source of truth for changes: unified diff inside BEGIN/END markers.
#[derive(Debug, Clone, Serialize)]
pub struct DiffEnvelope {
    pub base_ref: String,
    pub task_id: String,
    pub rationale: String,
    pub diff: String, // unified diff content between ---BEGIN DIFF--- and ---END DIFF---
}

/// Policy for where to run dry-run/apply/commit.
#[derive(Debug, Clone)]
pub enum WorktreePolicy {
    /// Operate directly in the provided `repo_path`.
    InPlace,
    /// Create an ephemeral git worktree rooted at `base_ref`, using a dedicated
    /// branch `autopilot/{task_id}`, perform all operations there, and optionally
    /// remove it afterwards.
    EphemeralFromBaseRef { base_ref: String, task_id: String },
}

/// Verify the diff against the contract, perform dry-run and (optionally) apply+commit.
///
/// Typical flow:
/// 1) Call with check_only=true for a quick gate.
/// 2) If OK, call with check_only=false to apply+commit.
/// 3) Render `ApplyReport` in TUI and persist it to rollout logs.
///
/// `build_and_test` allows callers to wire their own CI steps (e.g., `cargo test`,
/// `make test`). If `require_tests` is true and CI is provided, it runs pre-apply and
/// post-apply; otherwise only post-apply is executed.
pub fn verify_and_apply_patch<F>(
    repo_path: &Path,
    envelope: &DiffEnvelope,
    contract: &ChangeContract,
    commit_subject: &str,
    check_only: bool,
    worktree_policy: WorktreePolicy,
    build_and_test: Option<F>,
) -> Result<ApplyReport>
where
    F: Fn(&Path) -> Result<()> + Send + Sync,
{
    ensure_repo(repo_path)?;

    // Merge with optional repo config (.autopilot/config.toml)
    let contract = merge_contract_with_repo_config(repo_path, contract);

    // Compute stats and run sanity checks
    let stats = compute_diff_stats(&envelope.diff)?;
    let mut violations = Vec::new();
    sanity_check_diff(&envelope.diff, &mut violations);

    // Contract checks
    contract_check_paths(&envelope.diff, &contract, &stats, &mut violations)?;
    contract_check_budgets(&contract, &stats, &mut violations);
    let per_file = parse_per_file_stats(&envelope.diff)?;
    contract_check_governance(&contract, &per_file, &mut violations);
    scan_secrets_and_minified(&contract, &envelope.diff, &mut violations);

    if !violations.is_empty() {
        return Ok(ApplyReport {
            task_id: contract.task_id.clone(),
            checked_ok: false,
            applied: false,
            committed: false,
            commit_sha: None,
            stats,
            contract_violations: violations,
            notes: vec!["rejected: contract violations".to_string()],
        });
    }

    // Prepare worktree per policy.
    let mut wt = prepare_worktree(repo_path, &worktree_policy, &envelope.base_ref, &contract.task_id)?;

    // Ensure target worktree is clean.
    ensure_clean_worktree(&wt.root)?;

    // Acquire lock if we might mutate state (after cleanliness check to avoid
    // marking the worktree as dirty due to the lock artifact under .autopilot/).
    let _lock_guard = if check_only {
        None
    } else {
        Some(acquire_task_lock(repo_path, &contract.task_id)?)
    };

    // Dry-run check with strictness and fallback to 3-way when needed.
    let dry_run_ok = match git_apply_check_strict(&wt.root, &envelope.diff) {
        Ok(()) => true,
        Err(e1) => match git_apply_check_three_way(&wt.root, &envelope.diff) {
            Ok(()) => true,
            Err(e2) => {
                // Cleanup ephemeral context if created.
                wt.maybe_cleanup_on_error();
                return Err(eyre!(
                    "git apply --check failed (strict and 3-way): {} | {}",
                    e1, e2
                ));
            }
        },
    };
    debug_assert!(dry_run_ok);

    // CI hook pre-apply if require_tests.
    if let Some(ci) = build_and_test.as_ref() && contract.require_tests {
        inc_ci_runs(Phase::Pre);
        if let Err(e) = ci(&wt.root) {
            wt.maybe_cleanup_on_error();
            return Err(e.wrap_err("build/tests failed pre-apply"));
        }
    }

    if check_only {
        let report = ApplyReport {
            task_id: contract.task_id.clone(),
            checked_ok: true,
            applied: false,
            committed: false,
            commit_sha: None,
            stats,
            contract_violations: vec![],
            notes: vec!["dry-run ok".to_string()],
        };
        // Cleanup ephemeral worktree for check-only to avoid accumulation.
        wt.maybe_cleanup_on_success();
        return Ok(report);
    }

    // Apply patch on working tree with fallback.
    if let Err(e1) = git_apply_apply_strict(&wt.root, &envelope.diff)
        && let Err(e2) = git_apply_apply_three_way(&wt.root, &envelope.diff)
    {
        // Rollback if anything was staged/partially applied and cleanup.
        rollback_worktree(&wt);
        wt.maybe_cleanup_on_error();
        return Err(eyre!(
            "git apply failed (strict and 3-way): {} | {}",
            e1, e2
        ));
    }

    // CI hook post-apply (always recommended).
    if let Some(ci) = build_and_test.as_ref() {
        inc_ci_runs(Phase::Post);
        if let Err(e) = ci(&wt.root) {
            // Rollback on failure
            rollback_worktree(&wt);
            wt.maybe_cleanup_on_error();
            return Err(e.wrap_err("build/tests failed post-apply; rolled back"));
        }
    }

    // Stage & commit with Conventional Commit + PRD task reference.
    run_git(&wt.root, ["add", "-A"], None)?;
    let message = build_commit_message(commit_subject, &contract, &envelope.rationale);
    run_git(&wt.root, ["commit", "-m", &message], None).wrap_err("git commit failed")?;

    // Append commit trailers for reproducibility
    let trailers = compute_trailers(&wt.root, &contract, &envelope.diff);
    let _ = append_trailers_to_last_commit(&wt.root, &trailers);

    let sha = capture_head_sha(&wt.root).ok();
    // Persist artifacts (envelope/contract/report) under .autopilot/rollouts/TASK/TS
    let _ = persist_artifacts(
        &wt.repo_root,
        &contract,
        envelope,
        &ApplyReport {
            task_id: contract.task_id.clone(),
            checked_ok: true,
            applied: true,
            committed: true,
            commit_sha: sha.clone(),
            stats: stats.clone(),
            contract_violations: vec![],
            notes: vec!["applied and committed".to_string()],
        },
    );

    // Success cleanup (ephemeral only)
    wt.maybe_cleanup_on_success();

    Ok(ApplyReport {
        task_id: contract.task_id.clone(),
        checked_ok: true,
        applied: true,
        committed: true,
        commit_sha: sha,
        stats,
        contract_violations: vec![],
        notes: vec!["applied and committed".to_string()],
    })
}

/// Ensure repo_path points to a Git working tree.
fn ensure_repo(repo_path: &Path) -> Result<()> {
    if !repo_path.join(".git").exists() {
        bail!("Not a Git repository: {}", repo_path.display());
    }
    Ok(())
}

/// Ensure worktree is clean unless explicitly allowed via env override.
/// Use PATCHGATE_ALLOW_DIRTY=1 to bypass this safety net.
fn ensure_clean_worktree(repo_path: &Path) -> Result<()> {
    if std::env::var("PATCHGATE_ALLOW_DIRTY").ok().as_deref() == Some("1") {
        return Ok(());
    }
    let out = run_git(repo_path, ["status", "--porcelain"], None)?;
    if !out.trim().is_empty() {
        bail!(
            "Dirty worktree detected. Commit/stash changes or set PATCHGATE_ALLOW_DIRTY=1."
        );
    }
    Ok(())
}

/// Run `git` with args in the given repo. `input` is piped to stdin if provided.
fn run_git<I, S>(repo_path: &Path, args: I, input: Option<&str>) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }

    let mut child = cmd.spawn().wrap_err("failed to spawn git")?;

    if let Some(body) = input {
        let mut stdin = child.stdin.take().ok_or_else(|| eyre!("no stdin"))?;
        stdin.write_all(body.as_bytes())?;
        drop(stdin);
    }

    let out = child.wait_with_output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("git error: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Base args for `git apply`. By default, unidiff-zero is DISABLED for strictness.
fn git_apply_base_args(check_only: bool) -> Vec<&'static str> {
    let mut args = vec!["apply"];
    // Default: strict (no --unidiff-zero). Allow opt-in via env.
    if std::env::var("PATCHGATE_GIT_UNIDIFF_ZERO").ok().as_deref() == Some("1") {
        args.push("--unidiff-zero");
    }
    if check_only {
        args.push("--check");
        args.push("--whitespace=nowarn");
    } else {
        args.push("--whitespace=fix");
    }
    args
}

fn git_apply_check_strict(repo_path: &Path, diff: &str) -> Result<()> {
    let args = git_apply_base_args(true);
    run_git(repo_path, args, Some(diff)).map(|_| ())
}

fn git_apply_check_three_way(repo_path: &Path, diff: &str) -> Result<()> {
    let mut args = git_apply_base_args(true);
    args.push("--3way");
    args.push("--index");
    run_git(repo_path, args, Some(diff)).map(|_| ())
}

fn git_apply_apply_strict(repo_path: &Path, diff: &str) -> Result<()> {
    let args = git_apply_base_args(false);
    run_git(repo_path, args, Some(diff)).map(|_| ())
}

fn git_apply_apply_three_way(repo_path: &Path, diff: &str) -> Result<()> {
    let mut args = git_apply_base_args(false);
    args.push("--3way");
    args.push("--index");
    run_git(repo_path, args, Some(diff)).map(|_| ())
}

/// Build a conventional commit message with PRD task reference and rationale.
fn build_commit_message(subject: &str, contract: &ChangeContract, rationale: &str) -> String {
    let mut line1 = format!(
        "{}(task): {} [{}]",
        contract.commit_prefix, subject, contract.task_id
    );
    if contract.require_signoff {
        line1.push_str("\n\nSigned-off-by: Autopilot <autopilot@example>");
    }
    if !rationale.trim().is_empty() {
        line1.push_str("\n\nRationale: ");
        line1.push_str(rationale.trim());
    }
    line1
}

/// Get current HEAD SHA (short).
fn capture_head_sha(repo_path: &Path) -> Result<String> {
    let out = run_git(repo_path, ["rev-parse", "--short", "HEAD"], None)?;
    Ok(out.trim().to_string())
}

/// Basic sanity checks on the diff body (format and obviously bad patterns).
fn sanity_check_diff(diff: &str, violations: &mut Vec<String>) {
    if !diff.contains("diff --git ") {
        violations.push("not a unified diff (missing 'diff --git' headers)".into());
    }
    // Very large diffs are suspicious (accidental refactor) – tune threshold as needed.
    if diff.len() > 2 * 1024 * 1024 {
        violations.push("diff too large (>2 MiB)".into());
    }
    if diff.contains("GIT binary patch") {
        violations.push("binary patch detected".into());
    }
}

/// Check path-level contract rules using parsed stats and diff headers.
fn contract_check_paths(
    _diff: &str,
    contract: &ChangeContract,
    stats: &DiffStats,
    violations: &mut Vec<String>,
) -> Result<()> {
    // Binary files flagged?
    if contract.forbid_binary && stats.has_binary {
        violations.push("binary changes are forbidden".into());
    }
    // Renames / copies / deletes?
    if stats.has_renames && !contract.allow_renames {
        violations.push("renames are not allowed".into());
    }
    if stats.has_copies && !contract.allow_copies {
        violations.push("copies are not allowed".into());
    }
    if stats.has_deletes && !contract.allow_deletes {
        violations.push("deletes are not allowed".into());
    }

    // Check allowed vs deny globs and path safety.
    for path in &stats.touched_paths {
        let p = normalize_sep(path);

        // Path traversal safety
        if p.split('/').any(|seg| seg == "..") {
            violations.push(format!("path traversal is forbidden: {p}"));
            continue;
        }
        // Always deny touching .git/**
        if p == ".git" || p.starts_with(".git/") || p.contains("/.git/") {
            violations.push(format!("path denied (git internals): {p}"));
            continue;
        }

        // Apply deny presets
        let mut is_denied = false;
        for pat in expand_deny_presets(&contract.deny_presets) {
            if wildcard_is_match(&pat, &p) {
                violations.push(format!("path denied by preset ({pat}): {p}"));
                is_denied = true;
                break;
            }
        }
        if is_denied {
            continue;
        }
        if contract
            .deny_paths
            .iter()
            .any(|pat| wildcard_is_match(pat, &p))
        {
            violations.push(format!("path denied by contract: {p}"));
            continue;
        }
        // Empty allowed_paths should default to allow all ("**")
        let allowed = if contract.allowed_paths.is_empty() {
            true
        } else {
            contract
                .allowed_paths
                .iter()
                .any(|pat| wildcard_is_match(pat, &p))
        };
        if !allowed {
            violations.push(format!("path not allowed by contract: {p}"));
        }
    }

    // If require_tests, we either (a) detect touched test files, or (b) rely on CI hook later.
    if contract.require_tests {
        let has_test_touch = stats
            .touched_paths
            .iter()
            .any(|p| looks_like_test_file(p));
        if !has_test_touch {
            // Soft note only; the CI closure will be required to succeed.
        }
    }

    Ok(())
}

/// Check numerical budgets: files/added/removed.
fn contract_check_budgets(contract: &ChangeContract, stats: &DiffStats, violations: &mut Vec<String>) {
    if let Some(max) = contract.max_files_changed && stats.files_changed > max {
        violations.push(format!(
            "files_changed {} exceeds max {}",
            stats.files_changed, max
        ));
    }
    if let Some(max) = contract.max_lines_added && stats.lines_added > max {
        violations.push(format!(
            "lines_added {} exceeds max {}",
            stats.lines_added, max
        ));
    }
    if let Some(max) = contract.max_lines_removed && stats.lines_removed > max {
        violations.push(format!(
            "lines_removed {} exceeds max {}",
            stats.lines_removed, max
        ));
    }
}

/// Enforce P1 governance: per-file budgets, extensions, symlinks/exec/perm changes, new files.
fn contract_check_governance(contract: &ChangeContract, per_file: &[PerFileStats], violations: &mut Vec<String>) {
    // Count new files
    let new_files = per_file.iter().filter(|f| f.is_new_file).count();
    if let Some(max) = contract.max_new_files && new_files > max {
        violations.push(format!("new_files {new_files} exceeds max {max}"));
    }

    for f in per_file {
        // Allowed extensions
        if !contract.allowed_extensions.is_empty() {
            let ext_ok = std::path::Path::new(&f.path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| contract.allowed_extensions.iter().any(|ae| ae.eq_ignore_ascii_case(e)))
                .unwrap_or(false);
            if !ext_ok {
                violations.push(format!("disallowed extension for file: {}", f.path));
            }
        }

        // Per-file budgets
        if let Some(max) = contract.max_lines_added_per_file && f.added_lines > max {
            violations.push(format!(
                "file {}: lines_added {} exceeds max {}",
                f.path, f.added_lines, max
            ));
        }
        if let Some(max) = contract.max_hunks_per_file && f.hunks > max {
            violations.push(format!(
                "file {}: hunks {} exceeds max {}",
                f.path, f.hunks, max
            ));
        }
        if let Some(max) = contract.max_bytes_per_file && f.bytes_added > max {
            violations.push(format!(
                "file {}: bytes_added {} exceeds max {}",
                f.path, f.bytes_added, max
            ));
        }

        // Metadata constraints
        if contract.forbid_symlinks && f.is_symlink {
            violations.push(format!("file {}: symlink changes are forbidden", f.path));
        }
        if contract.forbid_exec_mode_changes && f.exec_mode_change {
            violations.push(format!(
                "file {}: exec mode changes (100755) are forbidden",
                f.path
            ));
        }
        if contract.forbid_permissions_changes && f.perms_change {
            violations.push(format!("file {}: permission mode changes are forbidden", f.path));
        }
    }
}

/// Compute stats and touched paths from a unified diff.
/// We parse headers (`diff --git`, `--- a/`, `+++ b/`) and count +/- hunks.
/// Lines starting with "+++" / "---" are not counted as changes.
pub fn compute_diff_stats(diff: &str) -> Result<DiffStats> {
    let mut files = 0usize;
    let mut plus = 0usize;
    let mut minus = 0usize;
    let mut paths: Vec<String> = Vec::new();
    let mut has_renames = false;
    let mut has_deletes = false;
    let mut has_copies = false;
    let mut has_binary = false;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            files += 1;
        } else if line.starts_with("rename from ")
            || line.starts_with("rename to ")
            || line.starts_with("similarity index ")
        {
            has_renames = true;
        } else if line.starts_with("deleted file mode ") {
            has_deletes = true;
        } else if line.starts_with("copy from ") || line.starts_with("copy to ") {
            has_copies = true;
        } else if line.starts_with("GIT binary patch") || line.starts_with("Binary files ") {
            has_binary = true;
        } else if let Some(p) = line.strip_prefix("+++ ") {
            // Example: "+++ b/path/to/file" or "+++ /dev/null"
            if !p.ends_with("/dev/null") {
                let p2 = p
                    .trim_start_matches("a/")
                    .trim_start_matches("b/")
                    .trim()
                    .to_string();
                if !paths.contains(&p2) {
                    paths.push(p2);
                }
            } else {
                has_deletes = true; // b/dev/null often indicates delete
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            plus += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            minus += 1;
        }
    }

    Ok(DiffStats {
        files_changed: files,
        lines_added: plus,
        lines_removed: minus,
        touched_paths: paths,
        has_renames,
        has_copies,
        has_deletes,
        has_binary,
    })
}

/// Parse per-file stats for governance checks.
fn parse_per_file_stats(diff: &str) -> Result<Vec<PerFileStats>> {
    let mut current: Option<PerFileStats> = None;
    let mut files: Vec<PerFileStats> = Vec::new();

    for line in diff.lines() {
        if let Some(pathline) = line.strip_prefix("diff --git ") {
            if let Some(f) = current.take() {
                files.push(f);
            }
            // parse path from "a/path b/path"
            let mut parts = pathline.split_whitespace();
            let a = parts.next();
            let b = parts.next();
            let path = b
                .and_then(|p| p.strip_prefix("b/"))
                .or_else(|| a.and_then(|p| p.strip_prefix("a/")))
                .unwrap_or("")
                .to_string();
            current = Some(PerFileStats { path, ..Default::default() });
        } else if let Some(f) = current.as_mut() {
            if line.starts_with("new file mode ") {
                f.is_new_file = true;
                if line.contains("120000") {
                    f.is_symlink = true;
                } else if line.contains("100755") {
                    f.exec_mode_change = true;
                }
            } else if line.starts_with("deleted file mode ") {
                // nothing specific here for P1 limits, global delete handled elsewhere
            } else if line.starts_with("old mode ") || line.starts_with("new mode ") {
                // Track permission/exec bit changes; determine exec specifically
                if line.starts_with("old mode ") {
                    // look ahead to next new mode if possible
                }
                f.perms_change = true;
                if line.contains("100755") {
                    f.exec_mode_change = true;
                }
            } else if line.starts_with("Binary files ") {
                f.binary = true;
            } else if line.starts_with("@@ ") {
                f.hunks += 1;
            } else if line.starts_with("+++") || line.starts_with("---") {
                // ignore header markers
            } else if line.starts_with('+') {
                // Count content +bytes/lines (skip header lines above)
                f.added_lines += 1;
                // subtract the leading '+'
                f.bytes_added += line.len().saturating_sub(1);
            } else if line.starts_with('-') {
                f.removed_lines += 1;
            }
        }
    }
    if let Some(f) = current.take() {
        files.push(f);
    }
    Ok(files)
}

fn expand_deny_presets(presets: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for p in presets {
        match p.as_str() {
            "node_modules" => {
                out.push("node_modules/**".into());
                out.push("**/node_modules/**".into());
            }
            "dist" => {
                out.push("dist/**".into());
                out.push("**/dist/**".into());
            }
            "vendor" => {
                out.push("vendor/**".into());
                out.push("**/vendor/**".into());
            }
            other => {
                // Treat as a directory name preset
                out.push(format!("{other}/**"));
                out.push(format!("**/{other}/**"));
            }
        }
    }
    out
}

fn scan_secrets_and_minified(contract: &ChangeContract, diff: &str, violations: &mut Vec<String>) {
    use regex_lite::Regex;
    if !contract.forbid_secrets && !contract.forbid_minified {
        return;
    }
    let re = [
        Regex::new(r"AKIA[0-9A-Z]{16}").ok(),
        Regex::new(r#"(?i)aws[_-]?secret[_-]?access[_-]?key\s*[:=]\s*['"]?[A-Za-z0-9/+=]{40}['"]?$"#).ok(),
        Regex::new(r"-----BEGIN (?:RSA |EC |)?PRIVATE KEY-----").ok(),
        Regex::new(r"ghp_[A-Za-z0-9]{36}").ok(),
        Regex::new(r#"AIza[0-9A-Za-z\-_]{35}"#).ok(),
    ];

    let mut current_path: Option<String> = None;
    let mut long_line_hits: std::collections::HashMap<String, usize> = Default::default();

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // set current path to b/path
            let mut parts = rest.split_whitespace();
            let a = parts.next(); let b = parts.next();
            let path = b
                .and_then(|p| p.strip_prefix("b/"))
                .or_else(|| a.and_then(|p| p.strip_prefix("a/")))
                .unwrap_or("")
                .to_string();
            current_path = Some(path);
            continue;
        }
        if !line.starts_with('+') || line.starts_with("+++") {
            continue;
        }
        let path = match &current_path { Some(p) => p.clone(), None => continue };
        let body = &line[1..];
        if contract.forbid_secrets
            && re.iter().any(|r| r.as_ref().is_some_and(|rx| rx.is_match(body)))
        {
            violations.push(format!("file {path}: suspected secret in diff hunk"));
        }
        if contract.forbid_minified {
            let len = body.len();
            if len > 1000 {
                *long_line_hits.entry(path.clone()).or_default() += 1;
            } else if len > 200 {
                let spaces = body.chars().filter(|c| c.is_whitespace()).count();
                if spaces * 10 < len { // < 10% whitespace
                    *long_line_hits.entry(path.clone()).or_default() += 1;
                }
            }
        }
    }

    for (path, hits) in long_line_hits {
        if hits > 0 {
            violations.push(format!("file {path}: minified-like content ({hits} long lines)"));
        }
    }
}

/// Heuristic test-file detection (extend as needed).
fn looks_like_test_file(p: &str) -> bool {
    let p = p.to_ascii_lowercase();
    p.contains("test")
        || p.ends_with("_test.go")
        || p.ends_with(".spec.ts")
        || p.ends_with(".test.ts")
        || p.ends_with(".test.js")
}

/// Normalize path separators for glob matching.
fn normalize_sep<S: AsRef<str>>(s: S) -> String {
    s.as_ref().replace('\\', "/")
}

/// Glob matcher supporting `*`, `?`, and `**` with path semantics:
/// - `?` matches exactly one non-`/` character
/// - `*` matches zero or more non-`/` characters (single path segment)
/// - `**` matches zero or more characters including `/` (multi-segment)
fn wildcard_is_match(pattern: &str, text: &str) -> bool {
    let p = normalize_sep(pattern);
    let t = normalize_sep(text);
    glob_match(p.as_bytes(), t.as_bytes())
}

fn glob_match(pat: &[u8], txt: &[u8]) -> bool {
    // Recursive matcher with memoization to keep it predictable.
    fn rec(pat: &[u8], txt: &[u8], i: usize, j: usize, memo: &mut Vec<Vec<i8>>) -> bool {
        if i == pat.len() {
            return j == txt.len();
        }
        if memo[i][j] != 0 {
            return memo[i][j] > 0;
        }

        let result = if pat[i] == b'*' {
            // Check if this is a `**`
            if i + 1 < pat.len() && pat[i + 1] == b'*' {
                // Collapse multiple *'s
                let mut k = i;
                while k + 1 < pat.len() && pat[k] == b'*' && pat[k + 1] == b'*' {
                    k += 1;
                }
                let ni = k + 1; // position after the collapsed **
                // `**` matches any sequence including '/'
                let mut jj = j;
                while jj <= txt.len() {
                    if rec(pat, txt, ni, jj, memo) {
                        return set_true(memo, i, j);
                    }
                    jj += 1;
                }
                false
            } else {
                // Single '*' matches any sequence without '/'
                let mut jj = j;
                if rec(pat, txt, i + 1, j, memo) {
                    return set_true(memo, i, j);
                }
                while jj < txt.len() && txt[jj] != b'/' {
                    jj += 1;
                    if rec(pat, txt, i + 1, jj, memo) {
                        return set_true(memo, i, j);
                    }
                }
                false
            }
        } else if pat[i] == b'?' {
            j < txt.len() && txt[j] != b'/' && rec(pat, txt, i + 1, j + 1, memo)
        } else {
            j < txt.len() && pat[i] == txt[j] && rec(pat, txt, i + 1, j + 1, memo)
        };

        memo[i][j] = if result { 1 } else { -1 };
        result
    }

    fn set_true(memo: &mut [Vec<i8>], i: usize, j: usize) -> bool {
        memo[i][j] = 1;
        true
    }

    // Memo table sized [pat.len()+1][txt.len()+1]; store tri-state {-1,0,1}
    let mut memo = vec![vec![0i8; txt.len() + 1]; pat.len() + 1];
    rec(pat, txt, 0, 0, &mut memo)
}

/// Parse a strict DiffEnvelope from the Builder raw output.
/// Expected markers and fields; returns error if diff body is missing.
pub fn parse_diff_envelope(raw: &str) -> Result<DiffEnvelope> {
    let base_ref = extract_between(raw, "base_ref:", "\n")
        .unwrap_or_else(|| "main".into())
        .trim()
        .to_string();
    let task_id = extract_between(raw, "task_id:", "\n")
        .unwrap_or_default()
        .trim()
        .to_string();
    let rationale = extract_between(raw, "rationale:", "\n")
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_string();
    let diff_body = extract_between(raw, "---BEGIN DIFF---", "---END DIFF---")
        .ok_or_else(|| eyre!("diff body not found in envelope"))?;
    Ok(DiffEnvelope {
        base_ref,
        task_id,
        rationale,
        diff: diff_body.trim().to_string(),
    })
}

fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(s[i..j].to_string())
}

/// Optional helper: compute repo diff between base_ref and HEAD (for reporting).
pub fn compute_repo_diff(repo_path: &Path, base_ref: &str) -> Result<String> {
    // For staged-only use --staged; for all unstaged use `git diff` alone.
    let out = run_git(
        repo_path,
        ["diff", "--unified=3", &format!("{base_ref}...HEAD")],
        None,
    )?;
    Ok(out)
}

// ===== Worktree and locking helpers =============================================================

// ===== Reproducibility (trailers + artifacts) ===================================================

#[derive(Debug, Clone)]
struct CommitTrailers {
    prd_ref: String,
    contract_hash: String,
    diff_hash: String,
    task_id: String,
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    hex::encode(out)
}

fn compute_trailers(repo_path: &Path, contract: &ChangeContract, diff: &str) -> CommitTrailers {
    let prd_ref = std::fs::read(repo_path.join("PRD.md")).map(|b| sha256_hex(&b)).unwrap_or_else(|_| "NA".into());
    let contract_json = serde_json::to_vec(contract).unwrap_or_default();
    let contract_hash = sha256_hex(&contract_json);
    let diff_hash = sha256_hex(diff.as_bytes());
    CommitTrailers { prd_ref, contract_hash, diff_hash, task_id: contract.task_id.clone() }
}

fn append_trailers_to_last_commit(repo_path: &Path, t: &CommitTrailers) -> Result<()> {
    // Append trailers to the last commit message using --amend
    let trailer_text = format!(
        "PRD-Ref: {}\nContract-Hash: {}\nDiff-Hash: {}\nTask-Id: {}\n",
        t.prd_ref, t.contract_hash, t.diff_hash, t.task_id
    );
    let last_msg = run_git(repo_path, ["log", "-1", "--pretty=%B"], None)?;
    let new_msg = format!("{}\n{}", last_msg.trim_end(), trailer_text);
    run_git(repo_path, ["commit", "--amend", "-m", &new_msg], None)?;
    Ok(())
}

fn persist_artifacts(repo_path: &Path, contract: &ChangeContract, envelope: &DiffEnvelope, report: &ApplyReport) -> Result<PathBuf> {
    use chrono::Utc;
    let ts = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let dir = repo_path.join(".autopilot").join("rollouts").join(&contract.task_id).join(ts);
    std::fs::create_dir_all(&dir)?;
    let env_path = dir.join("envelope.json");
    let contract_path = dir.join("contract.json");
    let report_path = dir.join("report.json");
    std::fs::write(&env_path, serde_json::to_vec_pretty(envelope)?)?;
    std::fs::write(&contract_path, serde_json::to_vec_pretty(contract)?)?;
    std::fs::write(&report_path, serde_json::to_vec_pretty(report)?)?;
    Ok(dir)
}

struct LockGuard {
    path: PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn acquire_task_lock(repo_path: &Path, task_id: &str) -> Result<LockGuard> {
    let lock_dir = repo_path.join(".autopilot/locks");
    std::fs::create_dir_all(&lock_dir)?;
    let repo_hash = sha256_hex(
        repo_path
            .canonicalize()
            .unwrap_or_else(|_| repo_path.to_path_buf())
            .to_string_lossy()
            .as_bytes(),
    );
    let lock_path = lock_dir.join(format!("{}.{}.lock", &repo_hash[..8], task_id));
    let f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path);
    match f {
        Ok(_) => Ok(LockGuard { path: lock_path }),
        Err(e) => bail!("another task is running or lock exists: {}", e),
    }
}

struct WorktreeCtx {
    root: PathBuf,
    ephemeral: bool,
    repo_root: PathBuf,
}

impl WorktreeCtx {
    fn maybe_cleanup_on_success(&mut self) {
        if self.ephemeral {
            // Best-effort removal; ignore errors.
            let _ = run_git(&self.repo_root, ["worktree", "remove", "--force", self.root.to_string_lossy().as_ref()], None);
        }
    }
    fn maybe_cleanup_on_error(&mut self) {
        if self.ephemeral {
            let _ = run_git(&self.repo_root, ["worktree", "remove", "--force", self.root.to_string_lossy().as_ref()], None);
        }
    }
}

fn prepare_worktree(
    repo_path: &Path,
    policy: &WorktreePolicy,
    base_ref: &str,
    task_id: &str,
) -> Result<WorktreeCtx> {
    match policy {
        WorktreePolicy::InPlace => Ok(WorktreeCtx {
            root: repo_path.to_path_buf(),
            ephemeral: false,
            repo_root: repo_path.to_path_buf(),
        }),
        WorktreePolicy::EphemeralFromBaseRef { .. } => {
            // Ensure refs are up-to-date
            let _ = run_git(repo_path, ["fetch", "--all", "--prune"], None);

            let ww = repo_path.join(".worktrees").join("autopilot").join(task_id);
            std::fs::create_dir_all(ww.parent().unwrap_or(repo_path))?;

            // Create a new branch to avoid branch checkout conflicts.
            let new_branch = format!("autopilot/{task_id}");
            // `git worktree add <path> -b <new_branch> <base_ref>`
            run_git(
                repo_path,
                [
                    "worktree",
                    "add",
                    ww.to_string_lossy().as_ref(),
                    "-b",
                    &new_branch,
                    base_ref,
                ],
                None,
            )
            .wrap_err("git worktree add failed")?;

            // Verify lineage: base_ref is ancestor of HEAD
            let _ = run_git(&ww, ["merge-base", "--is-ancestor", base_ref, "HEAD"], None)
                .wrap_err("merge-base ancestry check failed")?;

            Ok(WorktreeCtx {
                root: ww,
                ephemeral: true,
                repo_root: repo_path.to_path_buf(),
            })
        }
    }
}

fn rollback_worktree(wt: &WorktreeCtx) {
    if wt.ephemeral {
        // Just force-remove the ephemeral worktree
        let _ = run_git(&wt.repo_root, ["worktree", "remove", "--force", wt.root.to_string_lossy().as_ref()], None);
    } else {
        // Reset and clean in-place
        let _ = run_git(&wt.root, ["reset", "--hard", "--quiet"], None);
        let _ = run_git(&wt.root, ["clean", "-fdx", "--quiet"], None);
    }
}

#[derive(Debug, Default, Deserialize)]
struct AutopilotConfig {
    #[serde(default)]
    deny_presets: Vec<String>,
    #[serde(default)]
    forbid_secrets: Option<bool>,
    #[serde(default)]
    forbid_minified: Option<bool>,
}

fn merge_contract_with_repo_config(repo_path: &Path, base: &ChangeContract) -> ChangeContract {
    let cfg_path = repo_path.join(".autopilot").join("config.toml");
    if !cfg_path.exists() {
        return base.clone();
    }
    let mut merged = base.clone();
    if let Ok(body) = std::fs::read_to_string(cfg_path)
        && let Ok(cfg) = toml::from_str::<AutopilotConfig>(&body)
    {
            if !cfg.deny_presets.is_empty() {
                // merge presets; keep unique
                let mut set: std::collections::BTreeSet<String> = merged.deny_presets.iter().cloned().collect();
                for p in cfg.deny_presets { set.insert(p); }
                merged.deny_presets = set.into_iter().collect();
            }
            if let Some(b) = cfg.forbid_secrets { merged.forbid_secrets = b; }
            if let Some(b) = cfg.forbid_minified { merged.forbid_minified = b; }
        }
    merged
}

// ===== Tests ====================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_semantics_single_vs_double_star() {
        // '*' does not cross '/'
        assert!(wildcard_is_match("pkg/*.rs", "pkg/foo.rs"));
        assert!(!wildcard_is_match("pkg/*.rs", "pkg/nested/foo.rs"));
        // '**' crosses '/'
        assert!(wildcard_is_match("pkg/**/*.rs", "pkg/nested/foo.rs"));
        assert!(wildcard_is_match("**", "any/path/here"));
        // '?' matches exactly one non-slash
        assert!(wildcard_is_match("internal/??.rs", "internal/ab.rs"));
        assert!(!wildcard_is_match("internal/??.rs", "internal/abc.rs"));
    }

    #[test]
    fn parse_envelope_errors_without_diff() {
        let raw = "base_ref: main\ntask_id: T-1\nrationale: \"x\"";
        assert!(parse_diff_envelope(raw).is_err());
    }

    #[test]
    fn compute_stats_counts_lines_and_files() {
        let diff = r#"
diff --git a/a.txt b/a.txt
index e69de29..4b825dc 100644
--- a/a.txt
+++ b/a.txt
@@ -0,0 +1,2 @@
+hello
+world
"#;
        let st = match compute_diff_stats(diff) {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        };
        assert_eq!(st.files_changed, 1);
        assert_eq!(st.lines_added, 2);
        assert_eq!(st.lines_removed, 0);
        assert_eq!(st.touched_paths.len(), 1);
        assert_eq!(st.touched_paths[0], "a.txt");
    }
}
