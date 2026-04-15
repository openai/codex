use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use thiserror::Error;

pub const APPLY_PATCH_TOOL_INSTRUCTIONS: &str = include_str!("../apply_patch_tool_instructions.md");
pub const CODEX_CORE_APPLY_PATCH_ARG1: &str = "--codex-run-as-apply-patch";

const APPLY_PATCH_UNAVAILABLE: &str = "apply_patch is unavailable on wasm32";

#[derive(Debug, PartialEq, Clone, Error)]
pub enum ParseError {
    #[error("invalid patch: {0}")]
    InvalidPatchError(String),
    #[error("invalid hunk at line {line_number}, {message}")]
    InvalidHunkError { message: String, line_number: usize },
}

#[derive(Debug, PartialEq, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Hunk {
    AddFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    UpdateFile {
        path: PathBuf,
        move_path: Option<PathBuf>,
        chunks: Vec<UpdateFileChunk>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct UpdateFileChunk {
    pub change_context: Option<String>,
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,
    pub is_end_of_file: bool,
}

#[derive(Debug, Error)]
#[error("{context}: {source}")]
pub struct IoError {
    context: String,
    #[source]
    source: std::io::Error,
}

impl PartialEq for IoError {
    fn eq(&self, other: &Self) -> bool {
        self.context == other.context && self.source.to_string() == other.source.to_string()
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ApplyPatchError {
    #[error(transparent)]
    ParseError(#[from] ParseError),
    #[error(transparent)]
    IoError(#[from] IoError),
    #[error("{0}")]
    ComputeReplacements(String),
    #[error(
        "patch detected without explicit call to apply_patch. Rerun as [\"apply_patch\", \"<patch>\"]"
    )]
    ImplicitInvocation,
}

impl From<std::io::Error> for ApplyPatchError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(IoError {
            context: "I/O error".to_string(),
            source: err,
        })
    }
}

impl From<&std::io::Error> for ApplyPatchError {
    fn from(err: &std::io::Error) -> Self {
        Self::IoError(IoError {
            context: "I/O error".to_string(),
            source: std::io::Error::new(err.kind(), err.to_string()),
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct ApplyPatchArgs {
    pub patch: String,
    pub hunks: Vec<Hunk>,
    pub workdir: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum ApplyPatchFileChange {
    Add {
        content: String,
    },
    Delete {
        content: String,
    },
    Update {
        unified_diff: String,
        move_path: Option<PathBuf>,
        new_content: String,
    },
}

#[derive(Debug, PartialEq)]
pub enum ExtractHeredocError {
    Unsupported,
}

impl std::fmt::Display for ExtractHeredocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{APPLY_PATCH_UNAVAILABLE}")
    }
}

impl std::error::Error for ExtractHeredocError {}

#[derive(Debug, PartialEq)]
pub enum MaybeApplyPatchVerified {
    Body(ApplyPatchAction),
    ShellParseError(ExtractHeredocError),
    CorrectnessError(ApplyPatchError),
    NotApplyPatch,
}

#[derive(Debug, PartialEq)]
pub struct ApplyPatchAction {
    changes: HashMap<PathBuf, ApplyPatchFileChange>,
    pub patch: String,
    pub cwd: PathBuf,
}

impl ApplyPatchAction {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn changes(&self) -> &HashMap<PathBuf, ApplyPatchFileChange> {
        &self.changes
    }

    pub fn new_add_for_test(path: &Path, content: String) -> Self {
        let changes = HashMap::from([(path.to_path_buf(), ApplyPatchFileChange::Add { content })]);
        Self {
            changes,
            patch: String::new(),
            cwd: path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ApplyPatchFileUpdate {
    pub unified_diff: String,
    pub content: String,
}

pub fn parse_patch(_patch: &str) -> Result<ApplyPatchArgs, ParseError> {
    Err(ParseError::InvalidPatchError(
        APPLY_PATCH_UNAVAILABLE.to_string(),
    ))
}

pub fn unified_diff_from_chunks(
    _path: &Path,
    _chunks: &[UpdateFileChunk],
) -> Result<ApplyPatchFileUpdate, ApplyPatchError> {
    Err(ApplyPatchError::ComputeReplacements(
        APPLY_PATCH_UNAVAILABLE.to_string(),
    ))
}

pub fn maybe_parse_apply_patch_verified(argv: &[String], _cwd: &Path) -> MaybeApplyPatchVerified {
    match argv.first().map(String::as_str) {
        Some("apply_patch" | "applypatch") => MaybeApplyPatchVerified::CorrectnessError(
            ApplyPatchError::ComputeReplacements(APPLY_PATCH_UNAVAILABLE.to_string()),
        ),
        _ => MaybeApplyPatchVerified::NotApplyPatch,
    }
}

pub fn apply_patch(
    _patch: &str,
    _stdout: &mut impl std::io::Write,
    _stderr: &mut impl std::io::Write,
) -> Result<(), ApplyPatchError> {
    Err(ApplyPatchError::ComputeReplacements(
        APPLY_PATCH_UNAVAILABLE.to_string(),
    ))
}

pub fn main() -> ! {
    panic!("{APPLY_PATCH_UNAVAILABLE}");
}
