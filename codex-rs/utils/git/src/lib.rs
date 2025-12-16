use std::fmt;
use std::path::PathBuf;

mod apply;
mod branch;
mod errors;
mod ghost_commits;
mod operations;
mod platform;

pub use apply::ApplyGitRequest;
pub use apply::ApplyGitResult;
pub use apply::apply_git_patch;
pub use apply::extract_paths_from_patch;
pub use apply::parse_git_apply_output;
pub use apply::stage_paths;
pub use branch::merge_base_with_head;
pub use errors::GitToolingError;
pub use ghost_commits::CreateGhostCommitOptions;
pub use ghost_commits::GhostSnapshotConfig;
pub use ghost_commits::GhostSnapshotReport;
pub use ghost_commits::IgnoredUntrackedFile;
pub use ghost_commits::LargeUntrackedDir;
pub use ghost_commits::RestoreGhostCommitOptions;
pub use ghost_commits::capture_ghost_snapshot_report;
pub use ghost_commits::create_ghost_commit;
pub use ghost_commits::create_ghost_commit_with_report;
pub use ghost_commits::restore_ghost_commit;
pub use ghost_commits::restore_ghost_commit_with_options;
pub use ghost_commits::restore_to_commit;
pub use platform::create_symlink;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

type CommitID = String;

/// Details of a ghost commit created from a repository state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct GhostCommit {
    id: CommitID,
    parent: Option<CommitID>,
    preexisting_untracked_files: Vec<PathBuf>,
    preexisting_untracked_dirs: Vec<PathBuf>,
}

impl GhostCommit {
    /// Create a new ghost commit wrapper from a raw commit ID and optional parent.
    pub fn new(
        id: CommitID,
        parent: Option<CommitID>,
        preexisting_untracked_files: Vec<PathBuf>,
        preexisting_untracked_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            id,
            parent,
            preexisting_untracked_files,
            preexisting_untracked_dirs,
        }
    }

    /// Commit ID for the snapshot.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parent commit ID, if the repository had a `HEAD` at creation time.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// Untracked or ignored files that already existed when the snapshot was captured.
    pub fn preexisting_untracked_files(&self) -> &[PathBuf] {
        &self.preexisting_untracked_files
    }

    /// Untracked or ignored directories that already existed when the snapshot was captured.
    pub fn preexisting_untracked_dirs(&self) -> &[PathBuf] {
        &self.preexisting_untracked_dirs
    }
}

impl fmt::Display for GhostCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}
