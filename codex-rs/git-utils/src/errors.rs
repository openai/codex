use std::path::PathBuf;
use std::process::ExitStatus;
use std::string::FromUtf8Error;

use thiserror::Error;
use walkdir::Error as WalkdirError;

/// Errors returned while managing git worktree snapshots.
#[derive(Debug, Error)]
pub enum GitToolingError {
    #[error("git command `{command}` failed with status {status}: {stderr}")]
    GitCommand {
        command: String,
        status: ExitStatus,
        stderr: String,
    },
    #[error("git command `{command}` produced non-UTF-8 output")]
    GitOutputUtf8 {
        command: String,
        #[source]
        source: FromUtf8Error,
    },
    #[error("{path:?} is not a git repository")]
    NotAGitRepository { path: PathBuf },
    #[error("path {path:?} must be relative to the repository root")]
    NonRelativePath { path: PathBuf },
    #[error("path {path:?} escapes the repository root")]
    PathEscapesRepository { path: PathBuf },
    #[error("failed to process path inside worktree")]
    PathPrefix(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    Walkdir(#[from] WalkdirError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum GitReadError {
    #[error(
        "no trusted native Git executable is available; script-based and non-native Git wrappers are skipped, so install a native Git binary outside the repository and place its directory on PATH"
    )]
    NoTrustedGit,
    #[error(
        "refusing non-bare linked worktree because its primary worktree cannot be proven from Git metadata: {common_dir}; run the operation from the primary worktree, or use a standard linked-worktree or plain bare-backed layout"
    )]
    UnprovenPrimaryAuthority { common_dir: String },
    #[error("unsafe Git repository metadata at {path:?}: {reason}")]
    UnsafeRepositoryMetadata { path: PathBuf, reason: String },
    #[error("invalid or unsupported Git repository metadata at {path:?}: {reason}")]
    InvalidRepositoryMetadata { path: PathBuf, reason: String },
    #[error("{path:?} is not a Git repository")]
    NotRepository { path: PathBuf },
}

impl GitReadError {
    pub(crate) fn io_kind(&self) -> std::io::ErrorKind {
        match self {
            Self::NoTrustedGit | Self::NotRepository { .. } => std::io::ErrorKind::NotFound,
            Self::UnprovenPrimaryAuthority { .. } | Self::UnsafeRepositoryMetadata { .. } => {
                std::io::ErrorKind::PermissionDenied
            }
            Self::InvalidRepositoryMetadata { .. } => std::io::ErrorKind::InvalidData,
        }
    }

    pub(crate) fn into_io_error(self) -> std::io::Error {
        std::io::Error::new(self.io_kind(), self)
    }
}
