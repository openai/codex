use std::fmt;
use std::path::PathBuf;

mod apply;
mod errors;
mod ghost_commits;
mod operations;
mod platform;

pub use apply::{
    ApplyGitRequest, ApplyGitResult, apply_git_patch, extract_paths_from_patch,
    parse_git_apply_output, stage_paths,
};
pub use errors::GitToolingError;
pub use ghost_commits::{
    CreateGhostCommitOptions, create_ghost_commit, restore_ghost_commit, restore_to_commit,
};
pub use platform::create_symlink;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
