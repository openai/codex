//! Utility to compute the current Git diff for the working directory.
//!
//! The implementation mirrors the behaviour of the TypeScript version in
//! `codex-cli`: it returns the diff for tracked changes as well as any
//! untracked files. When the current directory is not inside a Git
//! repository, the function returns `Ok((false, String::new()))`.

use std::io;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

/// Return value of [`get_git_diff`].
///
/// * `bool` – Whether the current working directory is inside a Git repo.
/// * `String` – The concatenated diff (may be empty).
pub(crate) fn get_git_diff() -> io::Result<(bool, String)> {
    // First check if we are inside a Git repository.
    if !inside_git_repo()? {
        return Ok((false, String::new()));
    }

    // 1. Diff for tracked files.
    let tracked_diff = run_git_capture_diff(&["diff", "--color"])?;

    // 2. Determine untracked files.
    let untracked_output = run_git_capture_stdout(&["ls-files", "--others", "--exclude-standard"])?;

    let mut untracked_diff = String::new();
    let null_device: &Path = if cfg!(windows) {
        Path::new("NUL")
    } else {
        Path::new("/dev/null")
    };

    for file in untracked_output
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // Use `git diff --no-index` to generate a diff against the null device.
        let args = [
            "diff",
            "--color",
            "--no-index",
            "--",
            null_device.to_str().unwrap_or("/dev/null"),
            file,
        ];

        match run_git_capture_diff(&args) {
            Ok(diff) => untracked_diff.push_str(&diff),
            // If the file disappeared between ls-files and diff we ignore the error.
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }

    Ok((true, format!("{tracked_diff}{untracked_diff}")))
}

/// Helper that executes `git` with the given `args` and returns `stdout` as a
/// UTF-8 string. Any non-zero exit status is considered an *error*.
fn run_git_capture_stdout(args: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

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
fn run_git_capture_diff(args: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

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
fn inside_git_repo() -> io::Result<bool> {
    let status = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => Ok(true),
        Ok(_) => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false), // git not installed
        Err(e) => Err(e),
    }
}
