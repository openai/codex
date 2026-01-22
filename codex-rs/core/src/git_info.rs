use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::util::resolve_path;
use codex_app_server_protocol::GitSha;
use codex_protocol::protocol::GitInfo;
use futures::future::join_all;
use serde::Deserialize;
use serde::Serialize;
use tokio::process::Command;
use tokio::time::Duration as TokioDuration;
use tokio::time::timeout;

/// Return `true` if the project folder specified by the `Config` is inside a
/// Git repository.
///
/// The check walks up the directory hierarchy looking for a `.git` file or
/// directory (note `.git` can be a file that contains a `gitdir` entry). This
/// approach does **not** require the `git` binary or the `git2` crate and is
/// therefore fairly lightweight.
///
/// Note that this does **not** detect *work‑trees* created with
/// `git worktree add` where the checkout lives outside the main repository
/// directory. If you need Codex to work from such a checkout simply pass the
/// `--allow-no-git-exec` CLI flag that disables the repo requirement.
pub fn get_git_repo_root(base_dir: &Path) -> Option<PathBuf> {
    let mut dir = base_dir.to_path_buf();

    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }

        // Pop one component (go up one directory).  `pop` returns false when
        // we have reached the filesystem root.
        if !dir.pop() {
            break;
        }
    }

    None
}

/// Timeout for git commands to prevent freezing on large repositories
const GIT_COMMAND_TIMEOUT: TokioDuration = TokioDuration::from_secs(5);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitDiffToRemote {
    pub sha: GitSha,
    pub diff: String,
}

/// Collect git repository information from the given working directory using command-line git.
/// Returns None if no git repository is found or if git operations fail.
/// Uses timeouts to prevent freezing on large repositories.
/// All git commands (except the initial repo check) run in parallel for better performance.
pub async fn collect_git_info(cwd: &Path) -> Option<GitInfo> {
    // Check if we're in a git repository first
    let is_git_repo = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd)
        .await?
        .status
        .success();

    if !is_git_repo {
        return None;
    }

    // Run all git info collection commands in parallel
    let (commit_result, branch_result, url_result) = tokio::join!(
        run_git_command_with_timeout(&["rev-parse", "HEAD"], cwd),
        run_git_command_with_timeout(&["rev-parse", "--abbrev-ref", "HEAD"], cwd),
        run_git_command_with_timeout(&["remote", "get-url", "origin"], cwd)
    );

    let mut git_info = GitInfo {
        commit_hash: None,
        branch: None,
        repository_url: None,
    };

    // Process commit hash
    if let Some(output) = commit_result
        && output.status.success()
        && let Ok(hash) = String::from_utf8(output.stdout)
    {
        git_info.commit_hash = Some(hash.trim().to_string());
    }

    // Process branch name
    if let Some(output) = branch_result
        && output.status.success()
        && let Ok(branch) = String::from_utf8(output.stdout)
    {
        let branch = branch.trim();
        if branch != "HEAD" {
            git_info.branch = Some(branch.to_string());
        }
    }

    // Process repository URL
    if let Some(output) = url_result
        && output.status.success()
        && let Ok(url) = String::from_utf8(output.stdout)
    {
        git_info.repository_url = Some(url.trim().to_string());
    }

    Some(git_info)
}

/// A minimal commit summary entry used for pickers (subject + timestamp + sha).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitLogEntry {
    pub sha: String,
    /// Unix timestamp (seconds since epoch) of the commit time (committer time).
    pub timestamp: i64,
    /// Single-line subject of the commit message.
    pub subject: String,
}

/// Return the last `limit` commits reachable from HEAD for the current branch.
/// Each entry contains the SHA, commit timestamp (seconds), and subject line.
/// Returns an empty vector if not in a git repo or on error/timeout.
pub async fn recent_commits(cwd: &Path, limit: usize) -> Vec<CommitLogEntry> {
    // Ensure we're in a git repo first to avoid noisy errors.
    let Some(out) = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd).await else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }

    let fmt = "%H%x1f%ct%x1f%s"; // <sha> <US> <commit_time> <US> <subject>
    let limit_arg = (limit > 0).then(|| limit.to_string());
    let mut args: Vec<String> = vec!["log".to_string()];
    if let Some(n) = &limit_arg {
        args.push("-n".to_string());
        args.push(n.clone());
    }
    args.push(format!("--pretty=format:{fmt}"));
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let Some(log_out) = run_git_command_with_timeout(&arg_refs, cwd).await else {
        return Vec::new();
    };
    if !log_out.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&log_out.stdout);
    let mut entries: Vec<CommitLogEntry> = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('\u{001f}');
        let sha = parts.next().unwrap_or("").trim();
        let ts_s = parts.next().unwrap_or("").trim();
        let subject = parts.next().unwrap_or("").trim();
        if sha.is_empty() || ts_s.is_empty() {
            continue;
        }
        let timestamp = ts_s.parse::<i64>().unwrap_or(0);
        entries.push(CommitLogEntry {
            sha: sha.to_string(),
            timestamp,
            subject: subject.to_string(),
        });
    }

    entries
}

/// Returns the closest git sha to HEAD that is on a remote as well as the diff to that sha.
pub async fn git_diff_to_remote(cwd: &Path) -> Option<GitDiffToRemote> {
    get_git_repo_root(cwd)?;

    let remotes = get_git_remotes(cwd).await?;
    let branches = branch_ancestry(cwd).await?;
    let base_sha = find_closest_sha(cwd, &branches, &remotes).await?;
    let diff = diff_against_sha(cwd, &base_sha).await?;

    Some(GitDiffToRemote {
        sha: base_sha,
        diff,
    })
}

/// Run a git command with a timeout to prevent blocking on large repositories
async fn run_git_command_with_timeout(args: &[&str], cwd: &Path) -> Option<std::process::Output> {
    let result = timeout(
        GIT_COMMAND_TIMEOUT,
        Command::new("git").args(args).current_dir(cwd).output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => Some(output),
        _ => None, // Timeout or error
    }
}

async fn get_git_remotes(cwd: &Path) -> Option<Vec<String>> {
    let output = run_git_command_with_timeout(&["remote"], cwd).await?;
    if !output.status.success() {
        return None;
    }
    let mut remotes: Vec<String> = String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .map(str::to_string)
        .collect();
    if let Some(pos) = remotes.iter().position(|r| r == "origin") {
        let origin = remotes.remove(pos);
        remotes.insert(0, origin);
    }
    Some(remotes)
}

/// Attempt to determine the repository's default branch name.
///
/// Preference order:
/// 1) The symbolic ref at `refs/remotes/<remote>/HEAD` for the first remote (origin prioritized)
/// 2) `git remote show <remote>` parsed for "HEAD branch: <name>"
/// 3) Local fallback to existing `main` or `master` if present
async fn get_default_branch(cwd: &Path) -> Option<String> {
    // Prefer the first remote (with origin prioritized)
    let remotes = get_git_remotes(cwd).await.unwrap_or_default();
    for remote in remotes {
        // Try symbolic-ref, which returns something like: refs/remotes/origin/main
        if let Some(symref_output) = run_git_command_with_timeout(
            &[
                "symbolic-ref",
                "--quiet",
                &format!("refs/remotes/{remote}/HEAD"),
            ],
            cwd,
        )
        .await
            && symref_output.status.success()
            && let Ok(sym) = String::from_utf8(symref_output.stdout)
        {
            let trimmed = sym.trim();
            if let Some((_, name)) = trimmed.rsplit_once('/') {
                return Some(name.to_string());
            }
        }

        // Fall back to parsing `git remote show <remote>` output
        if let Some(show_output) =
            run_git_command_with_timeout(&["remote", "show", &remote], cwd).await
            && show_output.status.success()
            && let Ok(text) = String::from_utf8(show_output.stdout)
        {
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("HEAD branch:") {
                    let name = rest.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }

    // No remote-derived default; try common local defaults if they exist
    get_default_branch_local(cwd).await
}

/// Determine the repository's default branch name, if available.
///
/// This inspects remote configuration first (including the symbolic `HEAD`
/// reference) and falls back to common local defaults such as `main` or
/// `master`. Returns `None` when the information cannot be determined, for
/// example when the current directory is not inside a Git repository.
pub async fn default_branch_name(cwd: &Path) -> Option<String> {
    get_default_branch(cwd).await
}

/// Attempt to determine the repository's default branch name from local branches.
async fn get_default_branch_local(cwd: &Path) -> Option<String> {
    for candidate in ["main", "master"] {
        if let Some(verify) = run_git_command_with_timeout(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{candidate}"),
            ],
            cwd,
        )
        .await
            && verify.status.success()
        {
            return Some(candidate.to_string());
        }
    }

    None
}

/// Build an ancestry of branches starting at the current branch and ending at the
/// repository's default branch (if determinable)..
async fn branch_ancestry(cwd: &Path) -> Option<Vec<String>> {
    // Discover current branch (ignore detached HEAD by treating it as None)
    let current_branch = run_git_command_with_timeout(&["rev-parse", "--abbrev-ref", "HEAD"], cwd)
        .await
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| s != "HEAD");

    // Discover default branch
    let default_branch = get_default_branch(cwd).await;

    let mut ancestry: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(cb) = current_branch.clone() {
        seen.insert(cb.clone());
        ancestry.push(cb);
    }
    if let Some(db) = default_branch
        && !seen.contains(&db)
    {
        seen.insert(db.clone());
        ancestry.push(db);
    }

    // Expand candidates: include any remote branches that already contain HEAD.
    // This addresses cases where we're on a new local-only branch forked from a
    // remote branch that isn't the repository default. We prioritize remotes in
    // the order returned by get_git_remotes (origin first).
    let remotes = get_git_remotes(cwd).await.unwrap_or_default();
    for remote in remotes {
        if let Some(output) = run_git_command_with_timeout(
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "--contains=HEAD",
                &format!("refs/remotes/{remote}"),
            ],
            cwd,
        )
        .await
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
        {
            for line in text.lines() {
                let short = line.trim();
                // Expect format like: "origin/feature"; extract the branch path after "remote/"
                if let Some(stripped) = short.strip_prefix(&format!("{remote}/"))
                    && !stripped.is_empty()
                    && !seen.contains(stripped)
                {
                    seen.insert(stripped.to_string());
                    ancestry.push(stripped.to_string());
                }
            }
        }
    }

    // Ensure we return Some vector, even if empty, to allow caller logic to proceed
    Some(ancestry)
}

// Helper for a single branch: return the remote SHA if present on any remote
// and the distance (commits ahead of HEAD) for that branch. The first item is
// None if the branch is not present on any remote. Returns None if distance
// could not be computed due to git errors/timeouts.
async fn branch_remote_and_distance(
    cwd: &Path,
    branch: &str,
    remotes: &[String],
) -> Option<(Option<GitSha>, usize)> {
    // Try to find the first remote ref that exists for this branch (origin prioritized by caller).
    let mut found_remote_sha: Option<GitSha> = None;
    let mut found_remote_ref: Option<String> = None;
    for remote in remotes {
        let remote_ref = format!("refs/remotes/{remote}/{branch}");
        let Some(verify_output) =
            run_git_command_with_timeout(&["rev-parse", "--verify", "--quiet", &remote_ref], cwd)
                .await
        else {
            // Mirror previous behavior: if the verify call times out/fails at the process level,
            // treat the entire branch as unusable.
            return None;
        };
        if !verify_output.status.success() {
            continue;
        }
        let Ok(sha) = String::from_utf8(verify_output.stdout) else {
            // Mirror previous behavior and skip the entire branch on parse failure.
            return None;
        };
        found_remote_sha = Some(GitSha::new(sha.trim()));
        found_remote_ref = Some(remote_ref);
        break;
    }

    // Compute distance as the number of commits HEAD is ahead of the branch.
    // Prefer local branch name if it exists; otherwise fall back to the remote ref (if any).
    let count_output = if let Some(local_count) =
        run_git_command_with_timeout(&["rev-list", "--count", &format!("{branch}..HEAD")], cwd)
            .await
    {
        if local_count.status.success() {
            local_count
        } else if let Some(remote_ref) = &found_remote_ref {
            match run_git_command_with_timeout(
                &["rev-list", "--count", &format!("{remote_ref}..HEAD")],
                cwd,
            )
            .await
            {
                Some(remote_count) => remote_count,
                None => return None,
            }
        } else {
            return None;
        }
    } else if let Some(remote_ref) = &found_remote_ref {
        match run_git_command_with_timeout(
            &["rev-list", "--count", &format!("{remote_ref}..HEAD")],
            cwd,
        )
        .await
        {
            Some(remote_count) => remote_count,
            None => return None,
        }
    } else {
        return None;
    };

    if !count_output.status.success() {
        return None;
    }
    let Ok(distance_str) = String::from_utf8(count_output.stdout) else {
        return None;
    };
    let Ok(distance) = distance_str.trim().parse::<usize>() else {
        return None;
    };

    Some((found_remote_sha, distance))
}

// Finds the closest sha that exist on any of branches and also exists on any of the remotes.
async fn find_closest_sha(cwd: &Path, branches: &[String], remotes: &[String]) -> Option<GitSha> {
    // A sha and how many commits away from HEAD it is.
    let mut closest_sha: Option<(GitSha, usize)> = None;
    for branch in branches {
        let Some((maybe_remote_sha, distance)) =
            branch_remote_and_distance(cwd, branch, remotes).await
        else {
            continue;
        };
        let Some(remote_sha) = maybe_remote_sha else {
            // Preserve existing behavior: skip branches that are not present on a remote.
            continue;
        };
        match &closest_sha {
            None => closest_sha = Some((remote_sha, distance)),
            Some((_, best_distance)) if distance < *best_distance => {
                closest_sha = Some((remote_sha, distance));
            }
            _ => {}
        }
    }
    closest_sha.map(|(sha, _)| sha)
}

async fn diff_against_sha(cwd: &Path, sha: &GitSha) -> Option<String> {
    let output =
        run_git_command_with_timeout(&["diff", "--no-textconv", "--no-ext-diff", &sha.0], cwd)
            .await?;
    // 0 is success and no diff.
    // 1 is success but there is a diff.
    let exit_ok = output.status.code().is_some_and(|c| c == 0 || c == 1);
    if !exit_ok {
        return None;
    }
    let mut diff = String::from_utf8(output.stdout).ok()?;

    if let Some(untracked_output) =
        run_git_command_with_timeout(&["ls-files", "--others", "--exclude-standard"], cwd).await
        && untracked_output.status.success()
    {
        let untracked: Vec<String> = String::from_utf8(untracked_output.stdout)
            .ok()?
            .lines()
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .collect();

        if !untracked.is_empty() {
            // Use platform-appropriate null device and guard paths with `--`.
            let null_device: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
            let futures_iter = untracked.into_iter().map(|file| async move {
                let file_owned = file;
                let args_vec: Vec<&str> = vec![
                    "diff",
                    "--no-textconv",
                    "--no-ext-diff",
                    "--binary",
                    "--no-index",
                    // -- ensures that filenames that start with - are not treated as options.
                    "--",
                    null_device,
                    &file_owned,
                ];
                run_git_command_with_timeout(&args_vec, cwd).await
            });
            let results = join_all(futures_iter).await;
            for extra in results.into_iter().flatten() {
                if extra.status.code().is_some_and(|c| c == 0 || c == 1)
                    && let Ok(s) = String::from_utf8(extra.stdout)
                {
                    diff.push_str(&s);
                }
            }
        }
    }

    Some(diff)
}

/// Resolve the path that should be used for trust checks. Similar to
/// `[get_git_repo_root]`, but resolves to the root of the main
/// repository. Handles worktrees.
pub fn resolve_root_git_project_for_trust(cwd: &Path) -> Option<PathBuf> {
    let base = if cwd.is_dir() { cwd } else { cwd.parent()? };

    // TODO: we should make this async, but it's primarily used deep in
    // callstacks of sync code, and should almost always be fast
    let git_dir_out = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(base)
        .output()
        .ok()?;
    if !git_dir_out.status.success() {
        return None;
    }
    let git_dir_s = String::from_utf8(git_dir_out.stdout)
        .ok()?
        .trim()
        .to_string();

    let git_dir_path_raw = resolve_path(base, &PathBuf::from(&git_dir_s));

    // Normalize to handle macOS /var vs /private/var and resolve ".." segments.
    let git_dir_path = std::fs::canonicalize(&git_dir_path_raw).unwrap_or(git_dir_path_raw);
    git_dir_path.parent().map(Path::to_path_buf)
}

/// Returns a list of local git branches.
/// Includes the default branch at the beginning of the list, if it exists.
pub async fn local_git_branches(cwd: &Path) -> Vec<String> {
    let mut branches: Vec<String> = if let Some(out) =
        run_git_command_with_timeout(&["branch", "--format=%(refname:short)"], cwd).await
        && out.status.success()
    {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    branches.sort_unstable();

    if let Some(base) = get_default_branch_local(cwd).await
        && let Some(pos) = branches.iter().position(|name| name == &base)
    {
        let base_branch = branches.remove(pos);
        branches.insert(0, base_branch);
    }

    branches
}

/// Returns the current checked out branch name.
pub async fn current_branch_name(cwd: &Path) -> Option<String> {
    let out = run_git_command_with_timeout(&["branch", "--show-current"], cwd).await?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    use core_test_support::skip_if_sandbox;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper function to create a test git repository
    async fn create_test_git_repo(temp_dir: &TempDir) -> PathBuf {
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).expect("Failed to create repo dir");
        let envs = vec![
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
        ];

        // Initialize git repo
        Command::new("git")
            .envs(envs.clone())
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to init git repo");

        // Configure git user (required for commits)
        Command::new("git")
            .envs(envs.clone())
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user name");

        Command::new("git")
            .envs(envs.clone())
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user email");

        // Create a test file and commit it
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").expect("Failed to write test file");

        Command::new("git")
            .envs(envs.clone())
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add files");

        Command::new("git")
            .envs(envs.clone())
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to commit");

        repo_path
    }

    #[tokio::test]
    async fn test_recent_commits_non_git_directory_returns_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let entries = recent_commits(temp_dir.path(), 10).await;
        assert!(entries.is_empty(), "expected no commits outside a git repo");
    }

    #[tokio::test]
    async fn test_recent_commits_orders_and_limits() {
        skip_if_sandbox!();
        use tokio::time::Duration;
        use tokio::time::sleep;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Make three distinct commits with small delays to ensure ordering by timestamp.
        fs::write(repo_path.join("file.txt"), "one").unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "first change"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git commit 1");

        sleep(Duration::from_millis(1100)).await;

        fs::write(repo_path.join("file.txt"), "two").unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git add 2");
        Command::new("git")
            .args(["commit", "-m", "second change"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git commit 2");

        sleep(Duration::from_millis(1100)).await;

        fs::write(repo_path.join("file.txt"), "three").unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git add 3");
        Command::new("git")
            .args(["commit", "-m", "third change"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("git commit 3");

        // Request the latest 3 commits; should be our three changes in reverse time order.
        let entries = recent_commits(&repo_path, 3).await;
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].subject, "third change");
        assert_eq!(entries[1].subject, "second change");
        assert_eq!(entries[2].subject, "first change");
        // Basic sanity on SHA formatting
        for e in entries {
            assert!(e.sha.len() >= 7 && e.sha.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    async fn create_test_git_repo_with_remote(temp_dir: &TempDir) -> (PathBuf, String) {
        let repo_path = create_test_git_repo(temp_dir).await;
        let remote_path = temp_dir.path().join("remote.git");

        Command::new("git")
            .args(["init", "--bare", remote_path.to_str().unwrap()])
            .output()
            .await
            .expect("Failed to init bare remote");

        Command::new("git")
            .args(["remote", "add", "origin", remote_path.to_str().unwrap()])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add remote");

        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to get branch");
        let branch = String::from_utf8(output.stdout).unwrap().trim().to_string();

        Command::new("git")
            .args(["push", "-u", "origin", &branch])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to push initial commit");

        (repo_path, branch)
    }

    #[tokio::test]
    async fn test_collect_git_info_non_git_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let result = collect_git_info(temp_dir.path()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_collect_git_info_git_repository() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        // Should have commit hash
        assert!(git_info.commit_hash.is_some());
        let commit_hash = git_info.commit_hash.unwrap();
        assert_eq!(commit_hash.len(), 40); // SHA-1 hash should be 40 characters
        assert!(commit_hash.chars().all(|c| c.is_ascii_hexdigit()));

        // Should have branch (likely "main" or "master")
        assert!(git_info.branch.is_some());
        let branch = git_info.branch.unwrap();
        assert!(branch == "main" || branch == "master");

        // Repository URL might be None for local repos without remote
        // This is acceptable behavior
    }

    #[tokio::test]
    async fn test_collect_git_info_with_remote() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Add a remote origin
        Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add remote");

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        let remote_url_output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to read remote url");
        // Some dev environments rewrite remotes (e.g., force SSH), so compare against
        // whatever URL Git reports instead of a fixed placeholder.
        let expected_remote = String::from_utf8(remote_url_output.stdout)
            .unwrap()
            .trim()
            .to_string();

        // Should have repository URL
        assert_eq!(git_info.repository_url, Some(expected_remote));
    }

    #[tokio::test]
    async fn test_collect_git_info_detached_head() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Get the current commit hash
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to get HEAD");
        let commit_hash = String::from_utf8(output.stdout).unwrap().trim().to_string();

        // Checkout the commit directly (detached HEAD)
        Command::new("git")
            .args(["checkout", &commit_hash])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to checkout commit");

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        // Should have commit hash
        assert!(git_info.commit_hash.is_some());
        // Branch should be None for detached HEAD (since rev-parse --abbrev-ref HEAD returns "HEAD")
        assert!(git_info.branch.is_none());
    }

    #[tokio::test]
    async fn test_collect_git_info_with_branch() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Create and checkout a new branch
        Command::new("git")
            .args(["checkout", "-b", "feature-branch"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to create branch");

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info from repo");

        // Should have the new branch name
        assert_eq!(git_info.branch, Some("feature-branch".to_string()));
    }

    #[tokio::test]
    async fn test_get_git_working_tree_state_clean_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let (repo_path, branch) = create_test_git_repo_with_remote(&temp_dir).await;

        let remote_sha = Command::new("git")
            .args(["rev-parse", &format!("origin/{branch}")])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to rev-parse remote");
        let remote_sha = String::from_utf8(remote_sha.stdout)
            .unwrap()
            .trim()
            .to_string();

        let state = git_diff_to_remote(&repo_path)
            .await
            .expect("Should collect working tree state");
        assert_eq!(state.sha, GitSha::new(&remote_sha));
        assert!(state.diff.is_empty());
    }

    #[tokio::test]
    async fn test_get_git_working_tree_state_with_changes() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let (repo_path, branch) = create_test_git_repo_with_remote(&temp_dir).await;

        let tracked = repo_path.join("test.txt");
        fs::write(&tracked, "modified").unwrap();
        fs::write(repo_path.join("untracked.txt"), "new").unwrap();

        let remote_sha = Command::new("git")
            .args(["rev-parse", &format!("origin/{branch}")])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to rev-parse remote");
        let remote_sha = String::from_utf8(remote_sha.stdout)
            .unwrap()
            .trim()
            .to_string();

        let state = git_diff_to_remote(&repo_path)
            .await
            .expect("Should collect working tree state");
        assert_eq!(state.sha, GitSha::new(&remote_sha));
        assert!(state.diff.contains("test.txt"));
        assert!(state.diff.contains("untracked.txt"));
    }

    #[tokio::test]
    async fn test_get_git_working_tree_state_branch_fallback() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let (repo_path, _branch) = create_test_git_repo_with_remote(&temp_dir).await;

        Command::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to create feature branch");
        Command::new("git")
            .args(["push", "-u", "origin", "feature"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to push feature branch");

        Command::new("git")
            .args(["checkout", "-b", "local-branch"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to create local branch");

        let remote_sha = Command::new("git")
            .args(["rev-parse", "origin/feature"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to rev-parse remote");
        let remote_sha = String::from_utf8(remote_sha.stdout)
            .unwrap()
            .trim()
            .to_string();

        let state = git_diff_to_remote(&repo_path)
            .await
            .expect("Should collect working tree state");
        assert_eq!(state.sha, GitSha::new(&remote_sha));
    }

    #[test]
    fn resolve_root_git_project_for_trust_returns_none_outside_repo() {
        let tmp = TempDir::new().expect("tempdir");
        assert!(resolve_root_git_project_for_trust(tmp.path()).is_none());
    }

    #[tokio::test]
    async fn resolve_root_git_project_for_trust_regular_repo_returns_repo_root() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;
        let expected = std::fs::canonicalize(&repo_path).unwrap();

        assert_eq!(
            resolve_root_git_project_for_trust(&repo_path),
            Some(expected.clone())
        );
        let nested = repo_path.join("sub/dir");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(resolve_root_git_project_for_trust(&nested), Some(expected));
    }

    #[tokio::test]
    async fn resolve_root_git_project_for_trust_detects_worktree_and_returns_main_root() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Create a linked worktree
        let wt_root = temp_dir.path().join("wt");
        let _ = std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                wt_root.to_str().unwrap(),
                "-b",
                "feature/x",
            ])
            .current_dir(&repo_path)
            .output()
            .expect("git worktree add");

        let expected = std::fs::canonicalize(&repo_path).ok();
        let got = resolve_root_git_project_for_trust(&wt_root)
            .and_then(|p| std::fs::canonicalize(p).ok());
        assert_eq!(got, expected);
        let nested = wt_root.join("nested/sub");
        std::fs::create_dir_all(&nested).unwrap();
        let got_nested =
            resolve_root_git_project_for_trust(&nested).and_then(|p| std::fs::canonicalize(p).ok());
        assert_eq!(got_nested, expected);
    }

    #[test]
    fn resolve_root_git_project_for_trust_non_worktrees_gitdir_returns_none() {
        let tmp = TempDir::new().expect("tempdir");
        let proj = tmp.path().join("proj");
        std::fs::create_dir_all(proj.join("nested")).unwrap();

        // `.git` is a file but does not point to a worktrees path
        std::fs::write(
            proj.join(".git"),
            format!(
                "gitdir: {}\n",
                tmp.path().join("some/other/location").display()
            ),
        )
        .unwrap();

        assert!(resolve_root_git_project_for_trust(&proj).is_none());
        assert!(resolve_root_git_project_for_trust(&proj.join("nested")).is_none());
    }

    #[tokio::test]
    async fn test_get_git_working_tree_state_unpushed_commit() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let (repo_path, branch) = create_test_git_repo_with_remote(&temp_dir).await;

        let remote_sha = Command::new("git")
            .args(["rev-parse", &format!("origin/{branch}")])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to rev-parse remote");
        let remote_sha = String::from_utf8(remote_sha.stdout)
            .unwrap()
            .trim()
            .to_string();

        fs::write(repo_path.join("test.txt"), "updated").unwrap();
        Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add file");
        Command::new("git")
            .args(["commit", "-m", "local change"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to commit");

        let state = git_diff_to_remote(&repo_path)
            .await
            .expect("Should collect working tree state");
        assert_eq!(state.sha, GitSha::new(&remote_sha));
        assert!(state.diff.contains("updated"));
    }

    #[test]
    fn test_git_info_serialization() {
        let git_info = GitInfo {
            commit_hash: Some("abc123def456".to_string()),
            branch: Some("main".to_string()),
            repository_url: Some("https://github.com/example/repo.git".to_string()),
        };

        let json = serde_json::to_string(&git_info).expect("Should serialize GitInfo");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse JSON");

        assert_eq!(parsed["commit_hash"], "abc123def456");
        assert_eq!(parsed["branch"], "main");
        assert_eq!(
            parsed["repository_url"],
            "https://github.com/example/repo.git"
        );
    }

    #[test]
    fn test_git_info_serialization_with_nones() {
        let git_info = GitInfo {
            commit_hash: None,
            branch: None,
            repository_url: None,
        };

        let json = serde_json::to_string(&git_info).expect("Should serialize GitInfo");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse JSON");

        // Fields with None values should be omitted due to skip_serializing_if
        assert!(!parsed.as_object().unwrap().contains_key("commit_hash"));
        assert!(!parsed.as_object().unwrap().contains_key("branch"));
        assert!(!parsed.as_object().unwrap().contains_key("repository_url"));
    }
}
