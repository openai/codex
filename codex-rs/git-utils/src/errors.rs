use std::path::PathBuf;
use std::process::ExitStatus;
use std::string::FromUtf8Error;

use serde::Deserialize;
use serde::Serialize;
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

/// Stable, caller-visible reasons that a read-only Git operation was refused
/// or could not produce a complete result.
#[derive(Clone, Debug, Deserialize, Error, PartialEq, Eq, Serialize)]
#[serde(
    tag = "reason",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GitReadError {
    #[error("no trusted Git executable is available")]
    NoTrustedGit,
    #[error("{path:?} is not a Git repository")]
    NotRepository { path: PathBuf },
    #[error("Git resolved {reported_root:?} instead of expected root {expected_root:?}")]
    RepositoryRootMismatch {
        expected_root: PathBuf,
        reported_root: PathBuf,
    },
    #[error("no remote base commit is available")]
    NoRemoteBase,
    #[error("Git operation {operation:?} timed out")]
    CommandTimedOut { operation: String },
    #[error("Git operation {operation:?} failed with exit code {exit_code:?}")]
    CommandFailed {
        operation: String,
        exit_code: Option<i32>,
    },
    #[error("Git operation {operation:?} returned invalid output")]
    InvalidOutput { operation: String },
    #[error("executable filter {driver:?} is selected for {path:?}")]
    SelectedExecutableFilter { driver: String, path: String },
    #[error("untracked path {path:?} is an embedded repository or directory")]
    EmbeddedRepository { path: String },
}
