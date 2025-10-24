//! Utility to compute the current Git diff for the working directory.
//!
//! The implementation mirrors the behaviour of the TypeScript version in
//! `codex-cli`: it returns the diff for tracked changes as well as any
//! untracked files. When the current directory is not inside a Git
//! repository, the function returns `Ok((false, String::new()))`.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::task::JoinSet;

/// Return value of [`get_git_diff`].
///
/// * `bool` – Whether the current working directory is inside a Git repo.
/// * `String` – The concatenated diff (may be empty).
pub(crate) async fn get_git_diff() -> io::Result<(bool, String)> {
    // First check if we are inside a Git repository.
    if !inside_git_repo().await? {
        return Ok((false, String::new()));
    }

    let repo_root = git_toplevel().await?;

    let mut targets: Vec<(PathBuf, Option<String>)> = Vec::new();
    targets.push((repo_root.clone(), None));

    let mut submodules = collect_submodule_paths(&repo_root)?;
    submodules.sort();
    for submodule in submodules {
        let prefix = path_to_prefix(&submodule);
        let sub_abs = repo_root.join(&submodule);
        // Some checkouts list submodules in .gitmodules before they are
        // initialized; skip those directories until they become real repos so
        // /review continues to work in partially bootstrapped workspaces.
        if is_git_repo_dir(&sub_abs).await {
            targets.push((sub_abs, Some(prefix)));
        }
    }

    // Collect the workspace diff for the root and each submodule concurrently
    // while preserving deterministic ordering of the merged output.
    let mut join_set = JoinSet::new();
    // Preallocate slots so we can stitch results back together in a stable order.
    let mut ordered: Vec<Option<String>> = Vec::new();

    for (idx, (path, prefix)) in targets.into_iter().enumerate() {
        ordered.push(None);
        // Clone path so failures can mention the exact repository/submodule.
        let path_clone = path.clone();
        join_set.spawn(async move {
            let prefix_ref = prefix.as_deref();
            let result = collect_repo_diff(&path, prefix_ref).await;
            (idx, path_clone, result)
        });
    }

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok((idx, _path, Ok(diff))) => ordered[idx] = Some(diff),
            Ok((_, path, Err(err))) => {
                let message = format!("failed to collect git diff for {}: {err}", path.display());
                return Err(io::Error::other(message));
            }
            Err(err) => return Err(io::Error::other(err)),
        }
    }

    let mut combined = String::new();
    for diff in ordered.into_iter() {
        if let Some(text) = diff {
            combined.push_str(&text);
        }
    }

    Ok((true, combined))
}

/// Helper that executes `git` with the given `args` and returns `stdout` as a
/// UTF-8 string. Any non-zero exit status is considered an *error*.
async fn run_git_capture_stdout(args: &[&str]) -> io::Result<String> {
    run_git_capture_stdout_with_args(args.iter().map(|s| OsString::from(*s)).collect(), None).await
}

/// Determine if the current directory is inside a Git repository.
async fn inside_git_repo() -> io::Result<bool> {
    let status = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    match status {
        Ok(s) if s.success() => Ok(true),
        Ok(_) => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false), // git not installed
        Err(e) => Err(e),
    }
}

/// Returns true when `path` behaves like a git worktree (rev-parse succeeds).
async fn is_git_repo_dir(path: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(path)
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

fn path_to_prefix(path: &Path) -> String {
    let mut s = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

async fn collect_repo_diff(repo_dir: &Path, prefix: Option<&str>) -> io::Result<String> {
    // Run tracked and untracked diffs concurrently so the CLI keeps latency low
    // while gathering the complete workspace snapshot.
    let (tracked_res, untracked_res) = tokio::join!(
        run_git_diff(repo_dir, prefix),
        run_git_untracked(repo_dir, prefix)
    );

    match (tracked_res, untracked_res) {
        (Ok(tracked), Ok(untracked)) => Ok(format!("{tracked}{untracked}")),
        (Err(err), _) => Err(err),
        (_, Err(err)) => Err(err),
    }
}

async fn run_git_diff(repo_dir: &Path, prefix: Option<&str>) -> io::Result<String> {
    let mut args = vec![OsString::from("diff"), OsString::from("--color")];
    if let Some(prefix) = prefix {
        push_prefix_args(prefix, &mut args);
    }
    run_git_capture_diff_with_args(args, Some(repo_dir)).await
}

async fn run_git_untracked(repo_dir: &Path, prefix: Option<&str>) -> io::Result<String> {
    let output = run_git_capture_stdout_with_args(
        vec![
            OsString::from("ls-files"),
            OsString::from("--others"),
            OsString::from("--exclude-standard"),
        ],
        Some(repo_dir),
    )
    .await?;

    if output.trim().is_empty() {
        return Ok(String::new());
    }

    let null_device: &Path = if cfg!(windows) {
        Path::new("NUL")
    } else {
        Path::new("/dev/null")
    };
    let null_path = null_device.to_str().unwrap_or("/dev/null").to_string();

    let mut join_set: JoinSet<io::Result<String>> = JoinSet::new();
    for file in output.split('\n').map(str::trim).filter(|s| !s.is_empty()) {
        let file = file.to_string();
        let repo_dir = repo_dir.to_path_buf();
        let null_path = null_path.clone();
        let prefix = prefix.map(|p| p.to_string());
        join_set.spawn(async move {
            let mut args = vec![
                OsString::from("diff"),
                OsString::from("--color"),
                OsString::from("--no-index"),
            ];
            if let Some(ref prefix) = prefix {
                push_prefix_args(prefix, &mut args);
            }
            args.push(OsString::from("--"));
            args.push(OsString::from(&null_path));
            args.push(OsString::from(&file));
            run_git_capture_diff_with_args(args, Some(&repo_dir)).await
        });
    }

    let mut out = String::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(diff)) => out.push_str(&diff),
            Ok(Err(err)) if err.kind() == io::ErrorKind::NotFound => {}
            Ok(Err(err)) => return Err(err),
            Err(_) => {}
        }
    }
    Ok(out)
}

fn push_prefix_args(prefix: &str, args: &mut Vec<OsString>) {
    let mut normalized = prefix.trim_matches('/').to_string();
    if !normalized.is_empty() {
        normalized.push('/');
    }
    let prefixed_a = format!("a/{normalized}");
    let prefixed_b = format!("b/{normalized}");
    args.push(OsString::from("--src-prefix"));
    args.push(OsString::from(prefixed_a));
    args.push(OsString::from("--dst-prefix"));
    args.push(OsString::from(prefixed_b));
}

async fn run_git_capture_stdout_with_args(
    args: Vec<OsString>,
    cwd: Option<&Path>,
) -> io::Result<String> {
    run_git_command(args, cwd, false).await
}

async fn run_git_capture_diff_with_args(
    args: Vec<OsString>,
    cwd: Option<&Path>,
) -> io::Result<String> {
    run_git_command(args, cwd, true).await
}

async fn run_git_command(
    args: Vec<OsString>,
    cwd: Option<&Path>,
    allow_exit_one: bool,
) -> io::Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let output = cmd.output().await?;

    let success = output.status.success() || (allow_exit_one && output.status.code() == Some(1));
    if success {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "git command failed (status {})",
            output.status
        )))
    }
}

async fn git_toplevel() -> io::Result<PathBuf> {
    let output = run_git_capture_stdout(&["rev-parse", "--show-toplevel"]).await?;
    let path = PathBuf::from(output.trim());
    Ok(path)
}

fn collect_submodule_paths(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut acc = Vec::new();
    collect_submodules_recursive(root, Path::new(""), &mut acc)?;
    Ok(acc)
}

fn collect_submodules_recursive(
    absolute_dir: &Path,
    relative_dir: &Path,
    acc: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let gitmodules_path = absolute_dir.join(".gitmodules");
    if !gitmodules_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(gitmodules_path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(path_value) = trimmed.strip_prefix("path =") {
            let rel_path = Path::new(path_value.trim());
            let combined_rel = if relative_dir.as_os_str().is_empty() {
                rel_path.to_path_buf()
            } else {
                relative_dir.join(rel_path)
            };
            let abs_path = absolute_dir.join(rel_path);
            if abs_path.exists() {
                acc.push(combined_rel.clone());
                collect_submodules_recursive(&abs_path, &combined_rel, acc)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    struct DirGuard {
        original: PathBuf,
    }

    impl DirGuard {
        fn new(target: &Path) -> io::Result<Self> {
            let original = std::env::current_dir()?;
            std::env::set_current_dir(target)?;
            Ok(Self { original })
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Verifies that `get_git_diff` surfaces staged/unstaged edits and untracked files
    /// within git submodules so `/review uncommitted changes` evaluates the real diffs
    /// instead of stale submodule pointers.
    #[tokio::test]
    async fn diff_includes_submodule_worktree_changes() -> io::Result<()> {
        let temp = TempDir::new()?;
        let parent = temp.path().join("workspace");
        let sub_src = temp.path().join("submodule-src");
        fs::create_dir(&parent)?;
        fs::create_dir(&sub_src)?;

        git(&sub_src, &["init"])?;
        git(&sub_src, &["config", "user.email", "codex@example.com"])?;
        git(&sub_src, &["config", "user.name", "Codex Tests"])?;
        fs::create_dir(sub_src.join("lib"))?;
        fs::write(sub_src.join("lib").join("main.txt"), "initial\n")?;
        git(&sub_src, &["add", "."])?;
        git(&sub_src, &["commit", "-m", "initial"])?;

        git(&parent, &["init"])?;
        git(&parent, &["config", "user.email", "codex@example.com"])?;
        git(&parent, &["config", "user.name", "Codex Tests"])?;
        let status = StdCommand::new("git")
            .arg("-c")
            .arg("protocol.file.allow=always")
            .arg("submodule")
            .arg("add")
            .arg(sub_src.to_str().unwrap())
            .arg("modules/example")
            .current_dir(&parent)
            .status()?;
        if !status.success() {
            return Err(io::Error::other("failed to add submodule"));
        }
        git(&parent, &["add", ".gitmodules", "modules/example"])?;
        git(&parent, &["commit", "-m", "Add submodule"])?;

        let sub_clone = parent.join("modules/example");
        fs::write(sub_clone.join("lib").join("main.txt"), "modified\n")?;
        fs::write(sub_clone.join("lib").join("new.txt"), "new file\n")?;

        let _guard = DirGuard::new(&parent)?;
        let (is_repo, diff) = get_git_diff().await?;
        assert!(is_repo);
        assert!(
            diff.contains("modules/example/lib/main.txt"),
            "diff should mention modified submodule file: {diff}"
        );
        assert!(
            diff.contains("modules/example/lib/new.txt"),
            "diff should include untracked submodule file: {diff}"
        );
        Ok(())
    }

    fn git(dir: &Path, args: &[&str]) -> io::Result<()> {
        let status = StdCommand::new("git")
            .args(args)
            .current_dir(dir)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "git {:?} failed with status {}",
                args, status
            )))
        }
    }

    /// Ensures get_git_diff tolerates submodules listed in .gitmodules but not initialized.
    #[tokio::test]
    async fn diff_ignores_uninitialized_submodule() -> io::Result<()> {
        let temp = TempDir::new()?;
        let parent = temp.path().join("workspace");
        std::fs::create_dir(&parent)?;

        git(&parent, &["init"])?;
        git(&parent, &["config", "user.email", "codex@example.com"])?;
        git(&parent, &["config", "user.name", "Codex Tests"])?;

        // Write a .gitmodules entry pointing to an empty directory that is not a git repo.
        let modules_dir = parent.join("modules/example");
        std::fs::create_dir_all(&modules_dir)?;
        std::fs::write(
            parent.join(".gitmodules"),
            "[submodule \"example\"]\n\tpath = modules/example\n\turl = https://example.invalid/repo.git\n",
        )?;

        // Stage file so main repo is clean.
        git(&parent, &["add", ".gitmodules"])?;
        git(&parent, &["commit", "-m", "Add placeholder submodule"])?;

        let _guard = DirGuard::new(&parent)?;
        let (is_repo, diff) = get_git_diff().await?;
        assert!(is_repo);
        assert!(
            !diff.contains("modules/example"),
            "uninitialized submodule should be skipped from diffs: {diff}"
        );
        Ok(())
    }
}
