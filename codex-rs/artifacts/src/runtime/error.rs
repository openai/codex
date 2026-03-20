use codex_package_manager::PackageManagerError;
use std::path::PathBuf;
use thiserror::Error;

/// Errors raised while locating, validating, or installing an artifact runtime.
#[derive(Debug, Error)]
pub enum ArtifactRuntimeError {
    #[error(transparent)]
    PackageManager(#[from] PackageManagerError),
    #[error("{context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid package metadata at {path}")]
    InvalidPackageMetadata {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("runtime path `{0}` is invalid")]
    InvalidRuntimePath(String),
    #[error(
        "no compatible JavaScript runtime found for artifact runtime at {root_dir}; install Node or the Codex desktop app"
    )]
    MissingJsRuntime { root_dir: PathBuf },
}
