use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use image::GenericImageView;
use image::ImageFormat;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use ppt_rs::Chart;
use ppt_rs::ChartSeries;
use ppt_rs::ChartType;
use ppt_rs::Hyperlink as PptHyperlink;
use ppt_rs::HyperlinkAction as PptHyperlinkAction;
use ppt_rs::Image;
use ppt_rs::Presentation;
use ppt_rs::Shape;
use ppt_rs::ShapeFill;
use ppt_rs::ShapeLine;
use ppt_rs::ShapeType;
use ppt_rs::SlideContent;
use ppt_rs::SlideLayout;
use ppt_rs::TableBuilder;
use ppt_rs::TableCell;
use ppt_rs::TableRow;
use ppt_rs::generator::ArrowSize;
use ppt_rs::generator::ArrowType;
use ppt_rs::generator::CellAlign;
use ppt_rs::generator::Connector;
use ppt_rs::generator::ConnectorLine;
use ppt_rs::generator::ConnectorType;
use ppt_rs::generator::LineDash;
use ppt_rs::generator::generate_image_content_type;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;
use uuid::Uuid;
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const POINT_TO_EMU: u32 = 12_700;
const DEFAULT_SLIDE_WIDTH_POINTS: u32 = 720;
const DEFAULT_SLIDE_HEIGHT_POINTS: u32 = 540;
const DEFAULT_IMPORTED_TITLE_LEFT: u32 = 36;
const DEFAULT_IMPORTED_TITLE_TOP: u32 = 24;
const DEFAULT_IMPORTED_TITLE_WIDTH: u32 = 648;
const DEFAULT_IMPORTED_TITLE_HEIGHT: u32 = 48;
const DEFAULT_IMPORTED_CONTENT_LEFT: u32 = 48;
const DEFAULT_IMPORTED_CONTENT_TOP: u32 = 96;
const DEFAULT_IMPORTED_CONTENT_WIDTH: u32 = 624;
const DEFAULT_IMPORTED_CONTENT_HEIGHT: u32 = 324;

#[derive(Debug, Error)]
pub enum PresentationArtifactError {
    #[error("missing `artifact_id` for action `{action}`")]
    MissingArtifactId { action: String },
    #[error("unknown artifact id `{artifact_id}` for action `{action}`")]
    UnknownArtifactId { action: String, artifact_id: String },
    #[error("unknown action `{0}`")]
    UnknownAction(String),
    #[error("invalid args for action `{action}`: {message}")]
    InvalidArgs { action: String, message: String },
    #[error("unsupported feature for action `{action}`: {message}")]
    UnsupportedFeature { action: String, message: String },
    #[error("failed to import PPTX `{path}`: {message}")]
    ImportFailed { path: PathBuf, message: String },
    #[error("failed to export PPTX `{path}`: {message}")]
    ExportFailed { path: PathBuf, message: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct PresentationArtifactRequest {
    pub artifact_id: Option<String>,
    pub action: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationArtifactToolRequest {
    pub artifact_id: Option<String>,
    pub actions: Vec<PresentationArtifactToolAction>,
}

#[derive(Debug, Clone)]
pub struct PresentationArtifactExecutionRequest {
    pub artifact_id: Option<String>,
    pub requests: Vec<PresentationArtifactRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationArtifactToolAction {
    pub action: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathAccessKind {
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathAccessRequirement {
    pub action: String,
    pub kind: PathAccessKind,
    pub path: PathBuf,
}

impl PresentationArtifactRequest {
    pub fn is_mutating(&self) -> bool {
        !is_read_only_action(&self.action)
    }

    pub fn required_path_accesses(
        &self,
        cwd: &Path,
    ) -> Result<Vec<PathAccessRequirement>, PresentationArtifactError> {
        let access = match self.action.as_str() {
            "import_pptx" => {
                let args: ImportPptxArgs = parse_args(&self.action, &self.args)?;
                vec![PathAccessRequirement {
                    action: self.action.clone(),
                    kind: PathAccessKind::Read,
                    path: resolve_path(cwd, &args.path),
                }]
            }
            "export_pptx" => {
                let args: ExportPptxArgs = parse_args(&self.action, &self.args)?;
                vec![PathAccessRequirement {
                    action: self.action.clone(),
                    kind: PathAccessKind::Write,
                    path: resolve_path(cwd, &args.path),
                }]
            }
            "export_preview" => {
                let args: ExportPreviewArgs = parse_args(&self.action, &self.args)?;
                vec![PathAccessRequirement {
                    action: self.action.clone(),
                    kind: PathAccessKind::Write,
                    path: resolve_path(cwd, &args.path),
                }]
            }
            "add_image" => {
                let args: AddImageArgs = parse_args(&self.action, &self.args)?;
                match args.image_source()? {
                    ImageInputSource::Path(path) => vec![PathAccessRequirement {
                        action: self.action.clone(),
                        kind: PathAccessKind::Read,
                        path: resolve_path(cwd, &path),
                    }],
                    ImageInputSource::DataUrl(_)
                    | ImageInputSource::Blob(_)
                    | ImageInputSource::Uri(_)
                    | ImageInputSource::Placeholder => Vec::new(),
                }
            }
            "replace_image" => {
                let args: ReplaceImageArgs = parse_args(&self.action, &self.args)?;
                match (
                    &args.path,
                    &args.data_url,
                    &args.blob,
                    &args.uri,
                    &args.prompt,
                ) {
                    (Some(path), None, None, None, None) => vec![PathAccessRequirement {
                        action: self.action.clone(),
                        kind: PathAccessKind::Read,
                        path: resolve_path(cwd, path),
                    }],
                    (None, Some(_), None, None, None)
                    | (None, None, Some(_), None, None)
                    | (None, None, None, Some(_), None)
                    | (None, None, None, None, Some(_)) => Vec::new(),
                    _ => {
                        return Err(PresentationArtifactError::InvalidArgs {
                            action: self.action.clone(),
                            message:
                                "provide exactly one of `path`, `data_url`, `blob`, or `uri`, or provide `prompt` for a placeholder image"
                                    .to_string(),
                        });
                    }
                }
            }
            _ => Vec::new(),
        };
        Ok(access)
    }
}

impl PresentationArtifactToolRequest {
    pub fn is_mutating(&self) -> Result<bool, PresentationArtifactError> {
        Ok(self.actions.iter().any(|request| !is_read_only_action(&request.action)))
    }

    pub fn into_execution_request(
        self,
    ) -> Result<PresentationArtifactExecutionRequest, PresentationArtifactError> {
        if self.actions.is_empty() {
            return Err(PresentationArtifactError::InvalidArgs {
                action: "presentation_artifact".to_string(),
                message: "`actions` must contain at least one item".to_string(),
            });
        }
        Ok(PresentationArtifactExecutionRequest {
            artifact_id: self.artifact_id,
            requests: self
                .actions
                .into_iter()
                .map(|request| PresentationArtifactRequest {
                    artifact_id: None,
                    action: request.action,
                    args: request.args,
                })
                .collect(),
        })
    }

    pub fn required_path_accesses(
        &self,
        cwd: &Path,
    ) -> Result<Vec<PathAccessRequirement>, PresentationArtifactError> {
        let mut accesses = Vec::new();
        for request in &self.actions {
            accesses.extend(
                PresentationArtifactRequest {
                    artifact_id: None,
                    action: request.action.clone(),
                    args: request.args.clone(),
                }
                .required_path_accesses(cwd)?,
            );
        }
        Ok(accesses)
    }
}
