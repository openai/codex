//! Compact status labels for sessions running inside managed worktrees.
//!
//! Labels are derived on demand from codex-worktree metadata so status surfaces can show the
//! worktree name, dirty state, and repository without depending on the full picker inventory. A
//! failure to resolve metadata is logged and treated as no label because the status line should not
//! block normal chat rendering.

use std::path::Path;

/// Minimal worktree identity shown in compact TUI surfaces.
///
/// This type intentionally omits paths and owner metadata. It is for display only; callers that need
/// to switch, bind, or remove a worktree must use WorktreeInfo from the worktree manager instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeLabel {
    pub(crate) name: String,
    pub(crate) branch: Option<String>,
    pub(crate) repo_name: String,
    pub(crate) dirty: bool,
}

impl WorktreeLabel {
    /// Formats the label as branch-or-name, dirty state, and repository name.
    pub(crate) fn summary(&self) -> String {
        let mut parts = vec![self.branch.clone().unwrap_or_else(|| self.name.clone())];
        parts.push(if self.dirty { "dirty" } else { "clean" }.to_string());
        parts.push(self.repo_name.clone());
        parts.join(" · ")
    }
}

/// Resolves a cwd to a compact managed-worktree label when metadata is available.
///
/// Errors are logged and suppressed so a stale metadata file cannot break status rendering. A caller
/// that needs to distinguish unmanaged from broken metadata should use codex_worktree directly.
pub(crate) fn label_for_cwd(codex_home: &Path, cwd: &Path) -> Option<WorktreeLabel> {
    let info = codex_worktree::resolve_worktree(codex_home, cwd)
        .inspect_err(|err| tracing::warn!(?err, "failed to resolve managed worktree label"))
        .ok()
        .flatten()?;
    Some(WorktreeLabel {
        name: info.name,
        branch: info.branch,
        repo_name: info.repo_name,
        dirty: info.dirty.is_dirty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn summary_includes_name_branch_and_repo() {
        let label = WorktreeLabel {
            name: String::from("parser-fix"),
            branch: Some(String::from("parser-fix")),
            repo_name: String::from("codex"),
            dirty: false,
        };

        assert_eq!(label.summary(), "parser-fix · clean · codex");
    }
}
