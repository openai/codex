use std::fmt;

mod errors;
mod ghost_commits;
mod operations;
mod platform;

pub use errors::GitToolingError;
pub use ghost_commits::CreateGhostCommitOptions;
pub use ghost_commits::create_ghost_commit;
pub use ghost_commits::restore_ghost_commit;
pub use ghost_commits::restore_to_commit;
pub use platform::create_symlink;

/// Details of a ghost commit created from a repository state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostCommit {
    id: String,
    parent: Option<String>,
}

impl GhostCommit {
    /// Create a new ghost commit wrapper from a raw commit ID and optional parent.
    pub fn new(id: String, parent: Option<String>) -> Self {
        Self { id, parent }
    }

    /// Commit ID for the snapshot.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parent commit ID, if the repository had a `HEAD` at creation time.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }
}

impl fmt::Display for GhostCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}
