//! Managed Git worktree operations shared by Codex CLI, TUI, and app-server entry points.
//!
//! This crate owns the filesystem and Git contracts for Codex-managed worktrees: deriving stable
//! locations, creating or reusing a worktree, transferring pending changes when requested, writing
//! ownership metadata, listing known worktrees, and removing only worktrees that can be proven to be
//! Codex-managed. UI and transport layers should treat this crate as the source of truth for those
//! semantics rather than reimplementing Git path or metadata checks.
//!
//! The crate deliberately does not start Codex sessions, update TUI state, or decide whether a
//! conversation should be forked. Callers provide the source checkout, target branch, base ref, and
//! dirty-change policy; the returned metadata describes the resulting worktree so higher layers can
//! attach their own session lifecycle.

mod dirty;
mod git;
mod manager;
mod metadata;
mod paths;

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

pub use dirty::DirtyPolicy;
pub use dirty::DirtyState;
pub use dirty::dirty_state;
pub use manager::ensure_worktree;
pub use manager::list_worktrees;
pub use manager::remove_worktree;
pub use manager::resolve_worktree;
pub use metadata::WorktreeMetadata;
pub use metadata::WorktreeThreadMetadata;
pub use metadata::bind_thread;
pub use metadata::read_worktree_metadata;
pub use metadata::write_worktree_metadata;
pub use paths::codex_worktrees_root;
pub use paths::is_managed_worktree_path;
pub use paths::repo_fingerprint;
pub use paths::sibling_worktree_git_root;
pub use paths::slugify_name;

/// Request to create or reuse a managed worktree for a branch.
///
/// The source cwd may point anywhere inside the source repository; creation preserves that relative
/// cwd in the returned workspace cwd. The base ref is only used when creating a new branch, while an
/// existing branch is checked out directly. Passing the wrong Codex home can make listing and
/// metadata discovery disagree with the caller's later session binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRequest {
    /// Codex home directory used for metadata discovery and app-style worktree roots.
    pub codex_home: PathBuf,
    /// Current directory inside the source repository that the worktree should mirror.
    pub source_cwd: PathBuf,
    /// Branch name to create, reuse, or resolve.
    pub branch: String,
    /// Optional starting ref for a new branch; omitted means Git's current HEAD.
    pub base_ref: Option<String>,
    /// Policy for pending changes in the source checkout.
    pub dirty_policy: DirtyPolicy,
}

/// Result of creating or reusing a managed worktree.
///
/// Reused is true when the requested Codex-managed worktree already existed and matched the
/// requested branch. Warnings are non-fatal user-facing messages, most commonly dirty-change policy
/// outcomes that should be surfaced before the caller starts or forks a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeResolution {
    /// Whether an existing managed worktree was returned instead of creating a new one.
    pub reused: bool,
    /// Full inventory record for the selected worktree.
    pub info: WorktreeInfo,
    /// Non-fatal warnings that callers should display to the user.
    pub warnings: Vec<WorktreeWarning>,
}

/// Inventory record for a Git worktree visible to Codex.
///
/// The path fields intentionally distinguish the source repository root, the Git worktree root, and
/// the workspace cwd that should be used when launching Codex. Using the Git worktree root as the
/// session cwd would drop the user's original subdirectory context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// Stable repository fingerprint used to group worktrees from the same repository.
    pub id: String,
    /// Human-readable worktree name.
    pub name: String,
    /// Filesystem/search-friendly slug derived from the name or branch.
    pub slug: String,
    /// System that created or exposed this worktree.
    pub source: WorktreeSource,
    /// Broad filesystem placement category.
    pub location: WorktreeLocation,
    /// Display name of the source repository.
    pub repo_name: String,
    /// Source repository root that owns the Git common directory.
    pub repo_root: PathBuf,
    /// Git common directory used to match worktrees across paths.
    pub common_git_dir: PathBuf,
    /// Root directory of the Git worktree itself.
    pub worktree_git_root: PathBuf,
    /// Directory Codex should use as cwd when entering this worktree.
    pub workspace_cwd: PathBuf,
    /// Relative cwd from the source repository root that produced workspace_cwd.
    pub original_relative_cwd: PathBuf,
    /// Checked-out branch, when the worktree has one.
    pub branch: Option<String>,
    /// Current HEAD revision, when the worktree has one.
    pub head: Option<String>,
    /// Codex thread currently associated with this worktree, if known.
    pub owner_thread_id: Option<String>,
    /// Metadata file path used for diagnostics and display.
    pub metadata_path: PathBuf,
    /// Current dirty state of the worktree.
    pub dirty: DirtyState,
}

/// Origin of a worktree entry in the combined Codex inventory.
///
/// Callers should not use this as an ownership proof for destructive operations. Removal relies on
/// metadata checks in remove_worktree, because Git and legacy entries can still appear in lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeSource {
    /// Created by the Codex CLI/TUI managed-worktree flow.
    Cli,
    /// Created by the Codex app worktree flow.
    App,
    /// Older Codex metadata layout that still needs to be listed.
    Legacy,
    /// Plain Git worktree discovered from git worktree list.
    Git,
}

/// Filesystem placement category for a worktree.
///
/// This is a display and filtering hint, not an absolute path contract. Code that needs an actual
/// path must use worktree_git_root or workspace_cwd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeLocation {
    /// Sibling directory next to the source repository.
    Sibling,
    /// Worktree stored under CODEX_HOME/worktrees.
    CodexHome,
    /// Worktree outside Codex's managed directory conventions.
    External,
}

/// Non-fatal message produced while satisfying a worktree request.
///
/// Warnings are already phrased for users. Treating them as errors would make intentional policies
/// like leaving dirty changes behind look like failed worktree creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeWarning {
    /// User-facing warning text.
    pub message: String,
}

/// Query for listing worktrees visible to Codex.
///
/// When include_all_repos is false, source_cwd is required and results are filtered to the matching
/// repository. Passing include_all_repos true is intended for inventory tools, not for picker flows
/// that should stay scoped to the current checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeListQuery {
    /// Codex home directory used to discover app-style worktrees.
    pub codex_home: PathBuf,
    /// Current directory used to identify the repository filter.
    pub source_cwd: Option<PathBuf>,
    /// Whether to include worktrees from all repositories under Codex-managed roots.
    pub include_all_repos: bool,
}

/// Request to remove a Codex-managed worktree.
///
/// The target may be a branch/name/slug or an absolute path. Destructive operations still verify
/// Codex metadata before removing the worktree, so a matching display name alone is not enough to
/// delete arbitrary Git worktrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRemoveRequest {
    /// Codex home directory used for name lookup.
    pub codex_home: PathBuf,
    /// Optional current directory used to scope name lookup to one repository.
    pub source_cwd: Option<PathBuf>,
    /// Worktree branch, name, slug, or absolute path to remove.
    pub name_or_path: String,
    /// Whether to force removal of a dirty worktree.
    pub force: bool,
    /// Whether to delete the associated branch after removing the worktree.
    pub delete_branch: bool,
}

/// Result of removing a Codex-managed worktree.
///
/// The deleted branch is set only when branch deletion was requested and Git reported a branch for
/// the removed worktree. Callers should display the removed path even when no branch was deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoveResult {
    /// Filesystem path removed by Git.
    pub removed_path: PathBuf,
    /// Branch deleted after removal, if any.
    pub deleted_branch: Option<String>,
}
