//! Utility to compute the current Git diff for the working directory.
//!
//! The implementation mirrors the behaviour of the TypeScript version in
//! `codex-cli`: it returns the diff for tracked changes as well as any
//! untracked files. When the current directory is not inside a Git
//! repository, the function returns `Ok((false, String::new()))`.

use std::io;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Return value of [`get_git_diff`].
///
/// * `bool` – Whether the current working directory is inside a Git repo.
/// * `String` – The concatenated diff (may be empty).
pub(crate) async fn get_git_diff() -> io::Result<(bool, String)> {
    // First check if we are inside a Git repository.
    if !inside_git_repo().await? {
        return Ok((false, String::new()));
    }

    // Run tracked diff and untracked file listing in parallel.
    let (tracked_diff_res, untracked_output_res) = tokio::join!(
        run_git_capture_diff(&["diff", "--color"]),
        run_git_capture_stdout(&["ls-files", "--others", "--exclude-standard"]),
    );
    let tracked_diff = tracked_diff_res?;
    let untracked_output = untracked_output_res?;

    let mut untracked_diff = String::new();
    let null_device: &Path = if cfg!(windows) {
        Path::new("NUL")
    } else {
        Path::new("/dev/null")
    };

    let null_path = null_device.to_str().unwrap_or("/dev/null").to_string();
    let mut join_set: tokio::task::JoinSet<io::Result<String>> = tokio::task::JoinSet::new();
    for file in untracked_output
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let null_path = null_path.clone();
        let file = file.to_string();
        join_set.spawn(async move {
            let args = ["diff", "--color", "--no-index", "--", &null_path, &file];
            run_git_capture_diff(&args).await
        });
    }
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(diff)) => untracked_diff.push_str(&diff),
            Ok(Err(err)) if err.kind() == io::ErrorKind::NotFound => {}
            Ok(Err(err)) => return Err(err),
            Err(_) => {}
        }
    }

    Ok((true, format!("{tracked_diff}{untracked_diff}")))
}

/// Helper that executes `git` with the given `args` and returns `stdout` as a
/// UTF-8 string. Any non-zero exit status is considered an *error*.
async fn run_git_capture_stdout(args: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "git {:?} failed with status {}",
            args, output.status
        )))
    }
}

/// Like [`run_git_capture_stdout`] but treats exit status 1 as success and
/// returns stdout. Git returns 1 for diffs when differences are present.
async fn run_git_capture_diff(args: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;

    if output.status.success() || output.status.code() == Some(1) {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "git {:?} failed with status {}",
            args, output.status
        )))
    }
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
