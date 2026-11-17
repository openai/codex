use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::WeakShared;

use crate::GitSha;
use crate::get_git_repo_root;
use crate::info::detect_local_fsmonitor_override;
use crate::info::run_git_command_with_timeout_from;

/// The HEAD and worktree changes observed by the same Git status command.
#[derive(Clone, Debug, PartialEq)]
pub struct GitWorktreeStatus {
    pub head_commit_hash: Option<GitSha>,
    pub has_changes: bool,
}

type GitStatusFuture = BoxFuture<'static, Option<GitWorktreeStatus>>;
type GitStatusRuns = Mutex<HashMap<PathBuf, WeakShared<GitStatusFuture>>>;

const MAX_PROBE_DIRECTORIES: usize = 4;

fn git_status_runs() -> &'static GitStatusRuns {
    static RUNS: OnceLock<GitStatusRuns> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn change_probe_runs() -> &'static GitStatusRuns {
    static RUNS: OnceLock<GitStatusRuns> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Reports repository changes without inspecting uncommitted submodule contents.
pub async fn get_has_changes_in_repo(cwd: &Path, repo_root: &Path) -> Option<bool> {
    get_worktree_status_in_repo(cwd, repo_root)
        .await
        .map(|status| status.has_changes)
}

/// Reads HEAD and worktree changes together, skipping uncommitted submodule contents.
pub async fn get_worktree_status_in_repo(
    cwd: &Path,
    repo_root: &Path,
) -> Option<GitWorktreeStatus> {
    let git = PathBuf::from("git");
    let cwd = cwd.to_path_buf();
    let key = canonical_worktree_root(repo_root).await;
    share_git_status_run(key, move || async move {
        let fsmonitor = detect_local_fsmonitor_override(&git, &cwd).await;
        // Submodule worktree scans can execute filters selected by task-writable
        // Git configuration. Skip those scans while retaining gitlink changes.
        let output = run_git_command_with_timeout_from(
            &git,
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "--no-ahead-behind",
                "--ignore-submodules=dirty",
            ],
            &cwd,
            fsmonitor,
        )
        .await?;
        if !output.status.success() {
            return None;
        }
        parse_worktree_status(&output.stdout)
    })
    .await
}

/// Returns a positive change observation from a few recently modified root
/// directories. A clean, failed, or incomplete probe cannot establish that the
/// complete worktree is clean and returns `None`.
pub async fn probe_worktree_changes(cwd: &Path) -> Option<(PathBuf, GitWorktreeStatus)> {
    let root = get_git_repo_root(cwd)?;
    let probe_root = root.clone();
    let cwd = cwd.to_path_buf();
    // Relative Git environment paths can resolve differently in sibling cwds.
    let status = share_git_run(change_probe_runs(), cwd.clone(), move || async move {
        run_change_probe(&cwd, &probe_root).await
    })
    .await?;
    Some((root, status))
}

async fn run_change_probe(cwd: &Path, root: &Path) -> Option<GitWorktreeStatus> {
    let mut entries = tokio::fs::read_dir(root).await.ok()?;
    let mut recent = BinaryHeap::new();
    while let Some(entry) = entries.next_entry().await.ok()? {
        if entry.file_name() == ".git" || !entry.file_type().await.is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        if let Ok(metadata) = entry.metadata().await
            && let Ok(modified) = metadata.modified()
        {
            recent.push(Reverse((modified, entry.file_name())));
            if recent.len() > MAX_PROBE_DIRECTORIES {
                recent.pop();
            }
        }
    }
    if recent.is_empty() {
        return None;
    }

    let mut args: Vec<OsString> = [
        "status",
        "--porcelain=v2",
        "--branch",
        "--no-ahead-behind",
        "--ignore-submodules=dirty",
        "--",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.extend(recent.into_iter().map(|Reverse((_, name))| {
        let mut pathspec = OsString::from(":(top,literal)");
        pathspec.push(name);
        pathspec
    }));

    let git = Path::new("git");
    let fsmonitor = detect_local_fsmonitor_override(git, cwd).await;
    let output = run_git_command_with_timeout_from(git, &args, cwd, fsmonitor).await?;
    if !output.status.success() {
        return None;
    }
    parse_worktree_status(&output.stdout).filter(|status| status.has_changes)
}

fn parse_worktree_status(output: &[u8]) -> Option<GitWorktreeStatus> {
    let mut oid = None;
    let mut has_changes = false;
    // Without -z, Git quotes newlines in paths, including both paths in a rename.
    for line in output.strip_suffix(b"\n")?.split(|&byte| byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if let Some(value) = line.strip_prefix(b"# branch.oid ") {
            if oid.replace(value).is_some() {
                return None;
            }
        } else {
            // Porcelain v2: # header; 1 ordinary, 2 rename/copy, u unmerged, ? untracked.
            match line {
                [b'#', b' ', ..] => {}
                [b'1' | b'2' | b'u' | b'?', b' ', ..] => has_changes = true,
                _ => return None,
            }
        }
    }
    let head_commit_hash = match oid? {
        b"(initial)" => None,
        hash if matches!(hash.len(), 40 | 64) && hash.iter().all(u8::is_ascii_hexdigit) => {
            Some(GitSha::new(str::from_utf8(hash).ok()?))
        }
        _ => return None,
    };
    Some(GitWorktreeStatus {
        head_commit_hash,
        has_changes,
    })
}

async fn canonical_worktree_root(repo_root: &Path) -> PathBuf {
    tokio::fs::canonicalize(repo_root)
        .await
        .unwrap_or_else(|_| repo_root.to_path_buf())
}

async fn share_git_status_run<F, Fut>(key: PathBuf, run: F) -> Option<GitWorktreeStatus>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Option<GitWorktreeStatus>> + Send + 'static,
{
    share_git_run(git_status_runs(), key, run).await
}

async fn share_git_run<F, Fut>(
    runs: &'static GitStatusRuns,
    key: PathBuf,
    run: F,
) -> Option<GitWorktreeStatus>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Option<GitWorktreeStatus>> + Send + 'static,
{
    let result = {
        let mut runs = runs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(result) = runs
            .get(&key)
            .and_then(WeakShared::upgrade)
            .filter(|result| result.peek().is_none())
        {
            result
        } else {
            runs.retain(|_, result| {
                result
                    .upgrade()
                    .is_some_and(|result| result.peek().is_none())
            });
            let result = run().boxed().shared();
            if let Some(weak_result) = result.downgrade() {
                runs.insert(key, weak_result);
            }
            result
        }
    };
    result.await
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
