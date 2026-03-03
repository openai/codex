use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpreadsheetArtifactError {
    #[error("missing `artifact_id` for action `{action}`")]
    MissingArtifactId { action: String },
    #[error("unknown artifact id `{artifact_id}` for action `{action}`")]
    UnknownArtifactId { action: String, artifact_id: String },
    #[error("unknown action `{0}`")]
    UnknownAction(String),
    #[error("invalid args for action `{action}`: {message}")]
    InvalidArgs { action: String, message: String },
    #[error("invalid address `{address}`: {message}")]
    InvalidAddress { address: String, message: String },
    #[error("sheet lookup failed for action `{action}`: {message}")]
    SheetLookup { action: String, message: String },
    #[error("index `{index}` is out of range for action `{action}`; len={len}")]
    IndexOutOfRange {
        action: String,
        index: usize,
        len: usize,
    },
    #[error("merge conflict for action `{action}` on range `{range}` with `{conflict}`")]
    MergeConflict {
        action: String,
        range: String,
        conflict: String,
    },
    #[error("formula error at `{location}`: {message}")]
    Formula { location: String, message: String },
    #[error("serialization failed: {message}")]
    Serialization { message: String },
    #[error("failed to import XLSX `{path}`: {message}")]
    ImportFailed { path: PathBuf, message: String },
    #[error("failed to export XLSX `{path}`: {message}")]
    ExportFailed { path: PathBuf, message: String },
}
