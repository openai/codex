//! Tests Git output handling and isolation from configured hooks and filters.

use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::GitOperation;
use super::configured_filters;
use super::git_output;
use super::git_stdout;

fn setup_git(cwd: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git").current_dir(cwd).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git setup failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn repository() -> Result<(TempDir, std::path::PathBuf)> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("repository");
    fs::create_dir(&root)?;
    setup_git(&root, &["init", "-b", "main"])?;
    setup_git(&root, &["config", "user.name", "Codex Test"])?;
    setup_git(&root, &["config", "user.email", "codex@example.invalid"])?;
    fs::write(root.join("tracked.txt"), "tracked\n")?;
    setup_git(&root, &["add", "tracked.txt"])?;
    setup_git(&root, &["commit", "--no-gpg-sign", "-m", "initial"])?;
    Ok((directory, root))
}

#[test]
fn git_stdout_returns_trimmed_output() -> Result<()> {
    let (_directory, root) = repository()?;
    assert_eq!(git_stdout(&root, ["branch", "--show-current"])?, "main");
    Ok(())
}

#[test]
fn git_output_reports_nonzero_exit_status() -> Result<()> {
    let (_directory, root) = repository()?;
    let error = git_output(
        &root,
        GitOperation::WorkingTree,
        ["rev-parse", "--verify", "missing-ref"],
    )
    .expect_err("unknown revision should fail");
    assert!(error.to_string().contains("git command failed"));
    Ok(())
}

#[test]
fn discovers_filter_drivers_with_dotted_names() -> Result<()> {
    let (_directory, root) = repository()?;
    setup_git(
        &root,
        &["config", "filter.some.nested.driver.smudge", "cat"],
    )?;
    setup_git(&root, &["config", "filter.simple.required", "true"])?;
    setup_git(&root, &["config", "filter.x=y.process", "cat"])?;

    let filters = configured_filters(&root)?;
    assert!(filters.contains("some.nested.driver"));
    assert!(filters.contains("simple"));
    assert!(filters.contains("x=y"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn checkout_does_not_execute_configured_hooks_fsmonitor_or_filters() -> Result<()> {
    let (directory, root) = repository()?;
    fs::write(root.join(".gitattributes"), "*.txt filter=x=y -text\n")?;
    setup_git(&root, &["add", ".gitattributes"])?;
    setup_git(&root, &["commit", "--no-gpg-sign", "-m", "attributes"])?;

    let hooks = directory.path().join("hooks");
    fs::create_dir(&hooks)?;
    let hook_marker = directory.path().join("hook-executed");
    let hook = hooks.join("post-checkout");
    write_marker_script(&hook, &hook_marker)?;

    let fsmonitor_marker = directory.path().join("fsmonitor-executed");
    let fsmonitor = directory.path().join("fsmonitor");
    write_marker_script(&fsmonitor, &fsmonitor_marker)?;

    let filter_marker = directory.path().join("filter-executed");
    let filter = directory.path().join("filter");
    write_marker_script(&filter, &filter_marker)?;

    setup_git(
        &root,
        &[
            "config",
            "core.hooksPath",
            hooks.to_str().context("hooks path")?,
        ],
    )?;
    setup_git(
        &root,
        &[
            "config",
            "core.fsmonitor",
            fsmonitor.to_str().context("fsmonitor path")?,
        ],
    )?;
    for attribute in ["clean", "smudge", "process"] {
        setup_git(
            &root,
            &[
                "config",
                &format!("filter.x=y.{attribute}"),
                filter.to_str().context("filter path")?,
            ],
        )?;
    }
    setup_git(&root, &["config", "filter.x=y.required", "true"])?;

    let checkout = directory.path().join("checkout");
    git_output(
        &root,
        GitOperation::WorkingTree,
        [
            "worktree",
            "add",
            "--detach",
            checkout.to_str().context("checkout path")?,
            "HEAD",
        ],
    )?;
    git_output(
        &checkout,
        GitOperation::WorkingTree,
        ["status", "--porcelain"],
    )?;

    assert!(!hook_marker.exists(), "repository hook was executed");
    assert!(!fsmonitor_marker.exists(), "fsmonitor helper was executed");
    assert!(!filter_marker.exists(), "attribute filter was executed");
    assert_eq!(
        fs::read_to_string(checkout.join("tracked.txt"))?,
        "tracked\n"
    );
    Ok(())
}

#[cfg(unix)]
fn write_marker_script(path: &Path, marker: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let marker = marker.to_str().context("marker path is not valid UTF-8")?;
    fs::write(
        path,
        format!("#!/bin/sh\nprintf executed > '{marker}'\ncat\n"),
    )?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn default_base_ignores_unrelated_non_utf8_refs() -> Result<()> {
    let (_directory, root) = repository()?;
    let head = git_stdout(&root, ["rev-parse", "HEAD"])?;
    // Packed refs work even on filesystems that cannot store non-UTF-8 filenames.
    fs::write(
        root.join(".git/packed-refs"),
        [head.as_bytes(), b" refs/heads/unrelated-\xff\n"].concat(),
    )?;
    assert_eq!(super::default_worktree_base(&root)?, "refs/heads/main");
    setup_git(
        &root,
        &["update-ref", "refs/remotes/origin/release/stable", "HEAD"],
    )?;
    setup_git(
        &root,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/release/stable",
        ],
    )?;
    assert_eq!(
        super::default_worktree_base(&root)?,
        "refs/remotes/origin/release/stable"
    );
    Ok(())
}
