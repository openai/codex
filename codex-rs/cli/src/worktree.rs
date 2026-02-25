use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_core::config::find_codex_home;
use codex_core::git_info::resolve_root_git_project_for_trust;
use std::ffi::OsString;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub(crate) const WORKTREE_AUTO_BRANCH_SENTINEL: &str = "__codex_worktree_auto_branch__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorktreeLaunchResult {
    pub(crate) branch: String,
    pub(crate) path: PathBuf,
    pub(crate) reused_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorktreeEntry {
    path: PathBuf,
    branch: Option<String>,
}

pub(crate) fn prepare_worktree_launch(
    cwd_override: Option<&Path>,
    requested_branch: Option<&str>,
) -> Result<WorktreeLaunchResult> {
    let cwd = match cwd_override {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir().context("failed to determine current directory")?,
    };

    let repo_root = resolve_root_git_project_for_trust(&cwd).ok_or_else(|| {
        anyhow::anyhow!(
            "`--worktree` requires running inside a git repository (or linked worktree)"
        )
    })?;
    let repo_root = std::fs::canonicalize(&repo_root).unwrap_or(repo_root);
    let codex_home =
        find_codex_home().context("failed to resolve codex home for worktree storage")?;

    let mut worktrees = list_worktrees(&repo_root)?;
    let mut branch_checked_out_in_primary = false;
    let branch = match requested_branch {
        Some(branch) => {
            let trimmed = branch.trim();
            if trimmed.is_empty() {
                bail!("--worktree branch name cannot be empty");
            }
            trimmed.to_string()
        }
        None => choose_auto_branch_name(&repo_root, &worktrees)?,
    };

    if let Some(existing) = find_worktree_for_branch(&worktrees, &branch) {
        if existing.path.exists() {
            let existing_path =
                std::fs::canonicalize(&existing.path).unwrap_or(existing.path.clone());
            if existing_path == repo_root {
                branch_checked_out_in_primary = true;
            } else {
                return Ok(WorktreeLaunchResult {
                    branch,
                    path: existing.path.clone(),
                    reused_existing: true,
                });
            }
        }

        run_git_checked(
            &repo_root,
            &[OsString::from("worktree"), OsString::from("prune")],
        )?;
        worktrees = list_worktrees(&repo_root)?;

        if let Some(existing) = find_worktree_for_branch(&worktrees, &branch)
            && existing.path.exists()
        {
            let existing_path =
                std::fs::canonicalize(&existing.path).unwrap_or(existing.path.clone());
            if existing_path == repo_root {
                branch_checked_out_in_primary = true;
            } else {
                return Ok(WorktreeLaunchResult {
                    branch,
                    path: existing.path.clone(),
                    reused_existing: true,
                });
            }
        }
    }

    let destination = worktree_destination(&repo_root, &codex_home, &branch);

    if let Some(existing) = worktrees.iter().find(|entry| entry.path == destination) {
        let other_branch = existing.branch.as_deref().unwrap_or("(detached HEAD)");
        bail!(
            "worktree path `{}` is already in use on branch `{other_branch}`",
            destination.display()
        );
    }

    if destination.exists() {
        bail!(
            "worktree path `{}` already exists; choose a different branch or remove the directory",
            destination.display()
        );
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create worktree parent directory `{}`",
                parent.display()
            )
        })?;
    }

    let branch_exists = branch_exists(&repo_root, &branch)?;
    if branch_exists {
        let mut args = vec![
            OsString::from("worktree"),
            OsString::from("add"),
            destination.as_os_str().to_os_string(),
            OsString::from(&branch),
        ];
        if branch_checked_out_in_primary {
            args.insert(2, OsString::from("--force"));
        }
        run_git_checked(&repo_root, &args)?;
    } else {
        run_git_checked(
            &repo_root,
            &[
                OsString::from("worktree"),
                OsString::from("add"),
                OsString::from("-b"),
                OsString::from(&branch),
                destination.as_os_str().to_os_string(),
            ],
        )?;
    }

    let final_path = std::fs::canonicalize(&destination).unwrap_or(destination);

    Ok(WorktreeLaunchResult {
        branch,
        path: final_path,
        reused_existing: false,
    })
}

fn choose_auto_branch_name(repo_root: &Path, worktrees: &[WorktreeEntry]) -> Result<String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time is before UNIX_EPOCH")?
        .as_secs();
    let pid = std::process::id();
    let base = format!("codex/{timestamp}-{pid}");

    let mut suffix = 0;
    loop {
        let candidate = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };

        let used_by_worktree = worktrees
            .iter()
            .any(|entry| entry.branch.as_deref() == Some(candidate.as_str()));
        if !used_by_worktree && !branch_exists(repo_root, &candidate)? {
            return Ok(candidate);
        }

        suffix += 1;
    }
}

fn worktree_destination(repo_root: &Path, codex_home: &Path, branch: &str) -> PathBuf {
    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| "repo".to_string(), sanitize_for_path);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    repo_root.to_string_lossy().hash(&mut hasher);
    let repo_fingerprint = format!("{:016x}", hasher.finish());
    let sanitized_branch = sanitize_for_path(branch);

    codex_home
        .join("worktrees")
        .join(format!("{repo_name}-{repo_fingerprint}"))
        .join(sanitized_branch)
}

fn sanitize_for_path(value: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_was_dash = false;

    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            ch
        } else {
            '-'
        };

        if normalized == '-' {
            if !previous_was_dash {
                sanitized.push(normalized);
            }
            previous_was_dash = true;
        } else {
            sanitized.push(normalized);
            previous_was_dash = false;
        }
    }

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "branch".to_string()
    } else {
        trimmed.to_string()
    }
}

fn find_worktree_for_branch<'a>(
    worktrees: &'a [WorktreeEntry],
    branch: &str,
) -> Option<&'a WorktreeEntry> {
    worktrees
        .iter()
        .find(|entry| entry.branch.as_deref() == Some(branch))
}

fn branch_exists(repo_root: &Path, branch: &str) -> Result<bool> {
    let args = vec![
        OsString::from("show-ref"),
        OsString::from("--verify"),
        OsString::from("--quiet"),
        OsString::from(format!("refs/heads/{branch}")),
    ];
    let output = run_git(repo_root, &args)?;

    if output.status.success() {
        return Ok(true);
    }

    if output.status.code() == Some(1) {
        return Ok(false);
    }

    let command = render_git_command(&args);
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    bail!("`git {command}` failed: {stderr}");
}

fn list_worktrees(repo_root: &Path) -> Result<Vec<WorktreeEntry>> {
    let output = run_git_checked(
        repo_root,
        &[
            OsString::from("worktree"),
            OsString::from("list"),
            OsString::from("--porcelain"),
        ],
    )?;
    let stdout = String::from_utf8(output.stdout)
        .context("`git worktree list --porcelain` output was not UTF-8")?;

    Ok(parse_worktree_list_porcelain(&stdout, repo_root))
}

fn parse_worktree_list_porcelain(stdout: &str, repo_root: &Path) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    let mut path: Option<PathBuf> = None;
    let mut branch: Option<String> = None;

    for line in stdout.lines() {
        if line.is_empty() {
            if let Some(path) = path.take() {
                entries.push(WorktreeEntry {
                    path,
                    branch: branch.take(),
                });
            }
            continue;
        }

        if let Some(raw_path) = line.strip_prefix("worktree ") {
            let parsed = PathBuf::from(raw_path);
            path = Some(if parsed.is_absolute() {
                parsed
            } else {
                repo_root.join(parsed)
            });
            continue;
        }

        if let Some(raw_branch) = line.strip_prefix("branch ") {
            let normalized = raw_branch
                .strip_prefix("refs/heads/")
                .unwrap_or(raw_branch)
                .to_string();
            branch = Some(normalized);
        }
    }

    if let Some(path) = path {
        entries.push(WorktreeEntry { path, branch });
    }

    entries
}

fn run_git_checked(repo_root: &Path, args: &[OsString]) -> Result<std::process::Output> {
    let output = run_git(repo_root, args)?;
    if output.status.success() {
        return Ok(output);
    }

    let command = render_git_command(args);
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    bail!("`git {command}` failed: {stderr}");
}

fn run_git(repo_root: &Path, args: &[OsString]) -> Result<std::process::Output> {
    let command = render_git_command(args);
    Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .with_context(|| {
            format!(
                "failed to execute `git {command}` in `{}`",
                repo_root.display()
            )
        })
}

fn render_git_command(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn sanitize_for_path_replaces_non_safe_chars_and_collapses_runs() {
        assert_eq!(
            sanitize_for_path("feature/new*branch"),
            "feature-new-branch"
        );
        assert_eq!(sanitize_for_path("***"), "branch");
        assert_eq!(sanitize_for_path("a---b"), "a-b");
    }

    #[test]
    fn parse_worktree_list_porcelain_reads_paths_and_branches() {
        let repo_root = Path::new("/repo");
        let stdout = "worktree /repo\nHEAD abc\nbranch refs/heads/main\n\nworktree /repo/.codex/worktrees/repo-feature\nHEAD def\nbranch refs/heads/feature\n";

        let parsed = parse_worktree_list_porcelain(stdout, repo_root);
        assert_eq!(
            parsed,
            vec![
                WorktreeEntry {
                    path: PathBuf::from("/repo"),
                    branch: Some("main".to_string())
                },
                WorktreeEntry {
                    path: PathBuf::from("/repo/.codex/worktrees/repo-feature"),
                    branch: Some("feature".to_string())
                }
            ]
        );
    }

    #[test]
    fn worktree_destination_uses_codex_home_namespace() {
        let repo_root = Path::new("/projects/acme/repo");
        let codex_home = Path::new("/home/me/.codex");
        let destination = worktree_destination(repo_root, codex_home, "feature/new-api");
        let rendered = destination.to_string_lossy();

        assert!(rendered.starts_with("/home/me/.codex/worktrees/repo-"));
        assert!(rendered.ends_with("/feature-new-api"));
    }
}
