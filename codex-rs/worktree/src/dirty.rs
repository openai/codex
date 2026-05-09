use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::git;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyPolicy {
    Fail,
    Ignore,
    CopyTracked,
    CopyAll,
    MoveTracked,
    MoveAll,
}

#[derive(Debug)]
struct TransferPlan {
    staged_diff: Vec<u8>,
    unstaged_diff: Vec<u8>,
    tracked_paths: Vec<PathBuf>,
    untracked_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct PreparedDirtyTransfer {
    move_plan: Option<MovePlan>,
}

#[derive(Debug)]
struct MovePlan {
    transfer: TransferPlan,
    move_untracked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirtyState {
    pub has_staged_changes: bool,
    pub has_unstaged_changes: bool,
    pub has_untracked_files: bool,
}

impl DirtyState {
    pub fn is_dirty(&self) -> bool {
        self.has_staged_changes || self.has_unstaged_changes || self.has_untracked_files
    }
}

pub fn dirty_state(root: &Path) -> Result<DirtyState> {
    let staged = git::bytes(root, &["diff", "--cached", "--name-only", "-z"])?;
    let unstaged = git::bytes(root, &["diff", "--name-only", "-z"])?;
    let untracked = git::bytes(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    Ok(DirtyState {
        has_staged_changes: !staged.is_empty(),
        has_unstaged_changes: !unstaged.is_empty(),
        has_untracked_files: !untracked.is_empty(),
    })
}

pub fn validate_dirty_policy_before_create(
    source_root: &Path,
    policy: DirtyPolicy,
) -> Result<Vec<String>> {
    let state = dirty_state(source_root)?;
    if !state.is_dirty() {
        return Ok(Vec::new());
    }

    match policy {
        DirtyPolicy::Fail => bail_for_dirty_source(),
        DirtyPolicy::Ignore => Ok(vec![
            "source checkout has uncommitted changes; the new worktree was created without them"
                .to_string(),
        ]),
        DirtyPolicy::CopyTracked | DirtyPolicy::MoveTracked => {
            if state.has_untracked_files {
                Ok(vec![
                    "untracked files were left in the source checkout; use --worktree-dirty copy-all or move-all to carry them"
                        .to_string(),
                ])
            } else {
                Ok(Vec::new())
            }
        }
        DirtyPolicy::CopyAll | DirtyPolicy::MoveAll => Ok(Vec::new()),
    }
}

pub(crate) fn prepare_dirty_policy_after_create(
    source_root: &Path,
    worktree_root: &Path,
    policy: DirtyPolicy,
) -> Result<PreparedDirtyTransfer> {
    let state = dirty_state(source_root)?;
    if !state.is_dirty() {
        return Ok(PreparedDirtyTransfer { move_plan: None });
    }

    match policy {
        DirtyPolicy::Fail | DirtyPolicy::Ignore => Ok(PreparedDirtyTransfer { move_plan: None }),
        DirtyPolicy::CopyTracked => {
            let plan = TransferPlan::capture(source_root)?;
            plan.apply_tracked_diff(worktree_root)?;
            Ok(PreparedDirtyTransfer { move_plan: None })
        }
        DirtyPolicy::CopyAll => {
            let plan = TransferPlan::capture(source_root)?;
            plan.apply_tracked_diff(worktree_root)?;
            copy_untracked_files_at_paths(source_root, worktree_root, &plan.untracked_paths)?;
            Ok(PreparedDirtyTransfer { move_plan: None })
        }
        DirtyPolicy::MoveTracked => {
            let plan = TransferPlan::capture(source_root)?;
            plan.apply_tracked_diff(worktree_root)?;
            Ok(PreparedDirtyTransfer {
                move_plan: Some(MovePlan {
                    transfer: plan,
                    move_untracked: false,
                }),
            })
        }
        DirtyPolicy::MoveAll => {
            let plan = TransferPlan::capture(source_root)?;
            plan.apply_tracked_diff(worktree_root)?;
            copy_untracked_files_at_paths(source_root, worktree_root, &plan.untracked_paths)?;
            Ok(PreparedDirtyTransfer {
                move_plan: Some(MovePlan {
                    transfer: plan,
                    move_untracked: true,
                }),
            })
        }
    }
}

pub(crate) fn finalize_dirty_policy_after_create(
    source_root: &Path,
    prepared: PreparedDirtyTransfer,
) -> Result<()> {
    let Some(move_plan) = prepared.move_plan else {
        return Ok(());
    };
    move_plan
        .transfer
        .clean_source_after_move(source_root, move_plan.move_untracked)
        .with_context(|| {
            "worktree already contains transferred changes, but failed to clean the source checkout after move"
        })
}

fn bail_for_dirty_source<T>() -> Result<T> {
    anyhow::bail!(
        "source checkout has uncommitted changes; use --worktree-dirty ignore, copy-tracked, copy-all, move-tracked, or move-all"
    );
}

impl TransferPlan {
    fn capture(source_root: &Path) -> Result<Self> {
        Ok(Self {
            staged_diff: git::bytes(source_root, &["diff", "--cached", "--binary"])?,
            unstaged_diff: git::bytes(source_root, &["diff", "--binary"])?,
            tracked_paths: tracked_paths(source_root)?,
            untracked_paths: untracked_paths(source_root)?,
        })
    }

    fn apply_tracked_diff(&self, worktree_root: &Path) -> Result<()> {
        if !self.staged_diff.is_empty() {
            git::status_with_stdin(
                worktree_root,
                &["apply", "--index", "--binary", "-"],
                &self.staged_diff,
            )
            .context("failed to apply staged changes to worktree")?;
        }
        if !self.unstaged_diff.is_empty() {
            git::status_with_stdin(
                worktree_root,
                &["apply", "--binary", "-"],
                &self.unstaged_diff,
            )
            .context("failed to apply unstaged changes to worktree")?;
        }
        Ok(())
    }

    fn clean_source_after_move(&self, source_root: &Path, move_untracked: bool) -> Result<()> {
        if has_head(source_root) {
            git::status(source_root, &["reset", "--hard", "HEAD"])
                .context("failed to clean tracked changes from source checkout after move")?;
        } else {
            git::status(source_root, &["read-tree", "--empty"])
                .context("failed to clear unborn source index after move")?;
            for relative_path in &self.tracked_paths {
                remove_file_if_present(source_root, relative_path, "tracked")?;
            }
        }
        if move_untracked {
            for relative_path in &self.untracked_paths {
                remove_file_if_present(source_root, relative_path, "untracked")?;
            }
        }
        Ok(())
    }
}

fn tracked_paths(source_root: &Path) -> Result<Vec<PathBuf>> {
    let staged = git::bytes(source_root, &["diff", "--cached", "--name-only", "-z"])?;
    let unstaged = git::bytes(source_root, &["diff", "--name-only", "-z"])?;
    let mut paths = paths_from_nul_separated(&staged)?;
    paths.extend(paths_from_nul_separated(&unstaged)?);
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn paths_from_nul_separated(output: &[u8]) -> Result<Vec<PathBuf>> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|raw_path| {
            let relative_path = PathBuf::from(String::from_utf8_lossy(raw_path).into_owned());
            ensure_safe_relative_path(&relative_path)?;
            Ok(relative_path)
        })
        .collect()
}

fn has_head(source_root: &Path) -> bool {
    git::status(source_root, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

fn remove_file_if_present(source_root: &Path, relative_path: &Path, kind: &str) -> Result<()> {
    match fs::remove_file(source_root.join(relative_path)) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| {
            format!(
                "failed to remove moved {kind} path {} from source checkout",
                relative_path.display()
            )
        }),
    }
}

fn untracked_paths(source_root: &Path) -> Result<Vec<PathBuf>> {
    let output = git::bytes(
        source_root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    paths_from_nul_separated(&output)
}

fn copy_untracked_files_at_paths(
    source_root: &Path,
    worktree_root: &Path,
    paths: &[PathBuf],
) -> Result<()> {
    for relative_path in paths {
        ensure_safe_relative_path(relative_path)?;
        let source = source_root.join(relative_path);
        let destination = worktree_root.join(relative_path);
        let metadata = fs::symlink_metadata(&source)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&source)?;
            create_symlink(&target, &destination)?;
        } else if metadata.is_file() {
            fs::copy(&source, &destination)?;
        }
    }
    Ok(())
}

fn ensure_safe_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        anyhow::bail!(
            "refusing to copy absolute untracked path {}",
            path.display()
        );
    }
    if path.components().any(|component| {
        matches!(component, Component::ParentDir)
            || matches!(component, Component::Normal(value) if value == ".git")
    }) {
        anyhow::bail!("refusing to copy unsafe untracked path {}", path.display());
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, destination).map_err(Into::into)
}

#[cfg(windows)]
fn create_symlink(target: &Path, destination: &Path) -> Result<()> {
    std::os::windows::fs::symlink_file(target, destination).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_state_reports_clean_by_default() {
        assert!(!DirtyState::default().is_dirty());
    }
}
