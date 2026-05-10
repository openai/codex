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
pub use manager::stale_managed_worktree_dirs;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRequest {
    pub codex_home: PathBuf,
    pub source_cwd: PathBuf,
    pub branch: String,
    pub base_ref: Option<String>,
    pub dirty_policy: DirtyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeResolution {
    pub reused: bool,
    pub info: WorktreeInfo,
    pub warnings: Vec<WorktreeWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub source: WorktreeSource,
    pub location: WorktreeLocation,
    pub repo_name: String,
    pub repo_root: PathBuf,
    pub common_git_dir: PathBuf,
    pub worktree_git_root: PathBuf,
    pub workspace_cwd: PathBuf,
    pub original_relative_cwd: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub owner_thread_id: Option<String>,
    pub metadata_path: PathBuf,
    pub dirty: DirtyState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeSource {
    Cli,
    App,
    Legacy,
    Git,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeLocation {
    Sibling,
    CodexHome,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeWarning {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeListQuery {
    pub codex_home: PathBuf,
    pub source_cwd: Option<PathBuf>,
    pub include_all_repos: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRemoveRequest {
    pub codex_home: PathBuf,
    pub source_cwd: Option<PathBuf>,
    pub name_or_path: String,
    pub force: bool,
    pub delete_branch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoveResult {
    pub removed_path: PathBuf,
    pub deleted_branch: Option<String>,
}
