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
#[non_exhaustive]
#[serde(
    tag = "reason",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GitReadError {
    #[error(
        "no trusted native Git executable is available; script-based and non-native Git wrappers are skipped, so install a native Git binary outside the repository and place its directory on PATH"
    )]
    NoTrustedGit,
    #[error(
        "refusing non-bare linked worktree because its primary worktree cannot be proven from Git metadata: {common_dir}; run the operation from the primary worktree, or use a standard linked-worktree or plain bare-backed layout"
    )]
    UnprovenPrimaryAuthority { common_dir: String },
    #[error("unsafe Git repository metadata at {path:?}: {reason}")]
    UnsafeRepositoryMetadata {
        #[serde(with = "lossy_path")]
        path: PathBuf,
        #[serde(rename = "details")]
        reason: String,
    },
    #[error("invalid or unsupported Git repository metadata at {path:?}: {reason}")]
    InvalidRepositoryMetadata {
        #[serde(with = "lossy_path")]
        path: PathBuf,
        #[serde(rename = "details")]
        reason: String,
    },
    #[error("{path:?} is not a Git repository")]
    NotRepository {
        #[serde(with = "lossy_path")]
        path: PathBuf,
    },
    #[error("Git resolved {reported_root:?} instead of expected root {expected_root:?}")]
    RepositoryRootMismatch {
        #[serde(with = "lossy_path")]
        expected_root: PathBuf,
        #[serde(with = "lossy_path")]
        reported_root: PathBuf,
    },
    #[error("Git operation {operation:?} timed out")]
    CommandTimedOut { operation: String },
    #[error("Git operation {operation:?} failed with exit code {exit_code:?}")]
    CommandFailed {
        operation: String,
        exit_code: Option<i32>,
    },
    #[error("Git operation {operation:?} returned invalid output")]
    InvalidOutput { operation: String },
    #[error("repository authority refused Git operation {operation:?}")]
    AuthorityRefused { operation: String },
    #[error("Git filter attribute selection exceeded its {max_probes}-probe limit")]
    FilterSelectionProbeLimitExceeded { max_probes: usize },
    #[error("executable filter {driver:?} is selected for {path:?}")]
    SelectedExecutableFilter { driver: String, path: String },
    #[error("invalid Git configuration environment: {reason}")]
    InvalidConfigEnvironment {
        #[serde(rename = "details")]
        reason: String,
    },
}

/// Path fields in caller-visible diagnostic metadata use a deliberately lossy
/// UTF-8 wire representation. The Rust API remains `PathBuf`; deserialization
/// reconstructs the serialized path, not any replaced platform-native bytes.
mod lossy_path {
    use std::path::Path;
    use std::path::PathBuf;

    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serializer;

    pub(super) fn serialize<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&path.to_string_lossy())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(PathBuf::from)
    }
}

impl GitReadError {
    pub(crate) fn io_kind(&self) -> std::io::ErrorKind {
        match self {
            Self::NoTrustedGit | Self::NotRepository { .. } => std::io::ErrorKind::NotFound,
            Self::UnprovenPrimaryAuthority { .. }
            | Self::UnsafeRepositoryMetadata { .. }
            | Self::RepositoryRootMismatch { .. }
            | Self::AuthorityRefused { .. }
            | Self::FilterSelectionProbeLimitExceeded { .. }
            | Self::SelectedExecutableFilter { .. } => std::io::ErrorKind::PermissionDenied,
            Self::InvalidRepositoryMetadata { .. }
            | Self::InvalidOutput { .. }
            | Self::InvalidConfigEnvironment { .. } => std::io::ErrorKind::InvalidData,
            Self::CommandTimedOut { .. } => std::io::ErrorKind::TimedOut,
            Self::CommandFailed { .. } => std::io::ErrorKind::Other,
        }
    }

    pub(crate) fn into_io_error(self) -> std::io::Error {
        std::io::Error::new(self.io_kind(), self)
    }
}
