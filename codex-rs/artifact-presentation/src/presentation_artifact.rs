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
                    | ImageInputSource::Uri(_)
                    | ImageInputSource::Placeholder => Vec::new(),
                }
            }
            "replace_image" => {
                let args: ReplaceImageArgs = parse_args(&self.action, &self.args)?;
                match (&args.path, &args.data_url, &args.uri, &args.prompt) {
                    (Some(path), None, None, None) => vec![PathAccessRequirement {
                        action: self.action.clone(),
                        kind: PathAccessKind::Read,
                        path: resolve_path(cwd, path),
                    }],
                    (None, Some(_), None, None)
                    | (None, None, Some(_), None)
                    | (None, None, None, Some(_)) => Vec::new(),
                    _ => {
                        return Err(PresentationArtifactError::InvalidArgs {
                            action: self.action.clone(),
                            message:
                                "provide exactly one of `path`, `data_url`, or `uri`, or provide `prompt` for a placeholder image"
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

#[derive(Debug, Default)]
pub struct PresentationArtifactManager {
    documents: HashMap<String, PresentationDocument>,
}

impl PresentationArtifactManager {
    pub fn execute(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        match request.action.as_str() {
            "create" => self.create(request),
            "import_pptx" => self.import_pptx(request, cwd),
            "export_pptx" => self.export_pptx(request, cwd),
            "export_preview" => self.export_preview(request, cwd),
            "get_summary" => self.get_summary(request),
            "list_slides" => self.list_slides(request),
            "list_layouts" => self.list_layouts(request),
            "list_layout_placeholders" => self.list_layout_placeholders(request),
            "list_slide_placeholders" => self.list_slide_placeholders(request),
            "inspect" => self.inspect(request),
            "resolve" => self.resolve(request),
            "add_slide" => self.add_slide(request),
            "insert_slide" => self.insert_slide(request),
            "duplicate_slide" => self.duplicate_slide(request),
            "move_slide" => self.move_slide(request),
            "delete_slide" => self.delete_slide(request),
            "create_layout" => self.create_layout(request),
            "add_layout_placeholder" => self.add_layout_placeholder(request),
            "set_slide_layout" => self.set_slide_layout(request),
            "update_placeholder_text" => self.update_placeholder_text(request),
            "set_theme" => self.set_theme(request),
            "set_notes" => self.set_notes(request),
            "append_notes" => self.append_notes(request),
            "clear_notes" => self.clear_notes(request),
            "set_notes_visibility" => self.set_notes_visibility(request),
            "set_active_slide" => self.set_active_slide(request),
            "set_slide_background" => self.set_slide_background(request),
            "add_text_shape" => self.add_text_shape(request),
            "add_shape" => self.add_shape(request),
            "add_connector" => self.add_connector(request),
            "add_image" => self.add_image(request, cwd),
            "replace_image" => self.replace_image(request, cwd),
            "add_table" => self.add_table(request),
            "update_table_cell" => self.update_table_cell(request),
            "merge_table_cells" => self.merge_table_cells(request),
            "add_chart" => self.add_chart(request),
            "update_text" => self.update_text(request),
            "replace_text" => self.replace_text(request),
            "insert_text_after" => self.insert_text_after(request),
            "set_hyperlink" => self.set_hyperlink(request),
            "update_shape_style" => self.update_shape_style(request),
            "bring_to_front" => self.bring_to_front(request),
            "send_to_back" => self.send_to_back(request),
            "delete_element" => self.delete_element(request),
            "delete_artifact" => self.delete_artifact(request),
            other => Err(PresentationArtifactError::UnknownAction(other.to_string())),
        }
    }

    fn create(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: CreateArgs = parse_args(&request.action, &request.args)?;
        let mut document = PresentationDocument::new(args.name);
        if let Some(slide_size) = args.slide_size {
            document.slide_size = parse_slide_size(&slide_size, &request.action)?;
        }
        if let Some(theme) = args.theme {
            document.theme = normalize_theme(theme, &request.action)?;
        }
        let artifact_id = document.artifact_id.clone();
        let summary = format!(
            "Created presentation artifact `{artifact_id}` with {} slides",
            document.slides.len()
        );
        let snapshot = snapshot_for_document(&document);
        let mut response =
            PresentationArtifactResponse::new(artifact_id, request.action, summary, snapshot);
        response.theme = Some(document.theme_snapshot());
        self.documents
            .insert(response.artifact_id.clone(), document);
        Ok(response)
    }

    fn import_pptx(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ImportPptxArgs = parse_args(&request.action, &request.args)?;
        let path = resolve_path(cwd, &args.path);
        let imported = Presentation::from_path(&path).map_err(|error| {
            PresentationArtifactError::ImportFailed {
                path: path.clone(),
                message: error.to_string(),
            }
        })?;
        let mut document = PresentationDocument::from_ppt_rs(imported);
        import_pptx_images(&path, &mut document, &request.action)?;
        let artifact_id = document.artifact_id.clone();
        let slide_count = document.slides.len();
        let snapshot = snapshot_for_document(&document);
        self.documents.insert(artifact_id.clone(), document);
        let summary = format!(
            "Imported `{}` as artifact `{artifact_id}` with {slide_count} slides",
            path.display()
        );
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            summary,
            snapshot,
        ))
    }

    fn export_pptx(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ExportPptxArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let path = resolve_path(cwd, &args.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                PresentationArtifactError::ExportFailed {
                    path: path.clone(),
                    message: error.to_string(),
                }
            })?;
        }

        let bytes = build_pptx_bytes(document, &request.action).map_err(|message| {
            PresentationArtifactError::ExportFailed {
                path: path.clone(),
                message,
            }
        })?;
        std::fs::write(&path, bytes).map_err(|error| PresentationArtifactError::ExportFailed {
            path: path.clone(),
            message: error.to_string(),
        })?;

        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Exported presentation to `{}`", path.display()),
            snapshot_for_document(document),
        );
        response.exported_paths.push(path);
        Ok(response)
    }

    fn export_preview(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ExportPreviewArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let output_path = resolve_path(cwd, &args.path);
        let preview_format =
            parse_preview_output_format(args.format.as_deref(), &output_path, &request.action)?;
        let scale = normalize_preview_scale(args.scale, &request.action)?;
        let quality = normalize_preview_quality(args.quality, &request.action)?;
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                PresentationArtifactError::ExportFailed {
                    path: output_path.clone(),
                    message: error.to_string(),
                }
            })?;
        }
        let temp_dir =
            std::env::temp_dir().join(format!("presentation_preview_{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir).map_err(|error| {
            PresentationArtifactError::ExportFailed {
                path: output_path.clone(),
                message: error.to_string(),
            }
        })?;
        let preview_document = if let Some(slide_index) = args.slide_index {
            let slide = document
                .slides
                .get(slide_index as usize)
                .cloned()
                .ok_or_else(|| {
                    index_out_of_range(&request.action, slide_index as usize, document.slides.len())
                })?;
            PresentationDocument {
                artifact_id: document.artifact_id.clone(),
                name: document.name.clone(),
                slide_size: document.slide_size,
                theme: document.theme.clone(),
                layouts: Vec::new(),
                slides: vec![slide],
                active_slide_index: Some(0),
                next_slide_seq: 1,
                next_element_seq: 1,
                next_layout_seq: 1,
            }
        } else {
            document.clone()
        };
        write_preview_images(&preview_document, &temp_dir, &request.action)?;
        let mut exported_paths = collect_pngs(&temp_dir)?;
        if args.slide_index.is_some() {
            let rendered =
                exported_paths
                    .pop()
                    .ok_or_else(|| PresentationArtifactError::ExportFailed {
                        path: output_path.clone(),
                        message: "preview renderer produced no images".to_string(),
                    })?;
            write_preview_image(
                &rendered,
                &output_path,
                preview_format,
                scale,
                quality,
                &request.action,
            )?;
            exported_paths = vec![output_path];
        } else {
            std::fs::create_dir_all(&output_path).map_err(|error| {
                PresentationArtifactError::ExportFailed {
                    path: output_path.clone(),
                    message: error.to_string(),
                }
            })?;
            let mut relocated = Vec::new();
            for rendered in exported_paths {
                let filename = rendered.file_name().ok_or_else(|| {
                    PresentationArtifactError::ExportFailed {
                        path: output_path.clone(),
                        message: "rendered preview had no filename".to_string(),
                    }
                })?;
                let stem = Path::new(filename)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("preview");
                let target = output_path.join(format!("{stem}.{}", preview_format.extension()));
                write_preview_image(
                    &rendered,
                    &target,
                    preview_format,
                    scale,
                    quality,
                    &request.action,
                )?;
                relocated.push(target);
            }
            exported_paths = relocated;
        }
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            "Exported slide preview".to_string(),
            snapshot_for_document(document),
        );
        response.exported_paths = exported_paths;
        Ok(response)
    }

    fn get_summary(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Presentation `{}` has {} slides, {} elements, {} layouts, and active slide {}",
                document.name.as_deref().unwrap_or("Untitled"),
                document.slides.len(),
                document.total_element_count(),
                document.layouts.len(),
                document
                    .active_slide_index
                    .map(|index| index.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            snapshot_for_document(document),
        );
        response.slide_list = Some(slide_list(document));
        response.layout_list = Some(layout_list(document));
        response.theme = Some(document.theme_snapshot());
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn list_slides(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Listed {} slides", document.slides.len()),
            snapshot_for_document(document),
        );
        response.slide_list = Some(slide_list(document));
        response.theme = Some(document.theme_snapshot());
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn list_layouts(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Listed {} layouts", document.layouts.len()),
            snapshot_for_document(document),
        );
        response.layout_list = Some(layout_list(document));
        response.theme = Some(document.theme_snapshot());
        Ok(response)
    }

    fn list_layout_placeholders(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: LayoutIdArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let placeholders = layout_placeholder_list(document, &args.layout_id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Listed {} placeholders for layout `{}`",
                placeholders.len(),
                args.layout_id
            ),
            snapshot_for_document(document),
        );
        response.placeholder_list = Some(placeholders);
        response.layout_list = Some(layout_list(document));
        Ok(response)
    }

    fn list_slide_placeholders(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SlideIndexArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let slide_index = args.slide_index as usize;
        let slide = document.slides.get(slide_index).ok_or_else(|| {
            index_out_of_range(&request.action, slide_index, document.slides.len())
        })?;
        let placeholders = slide_placeholder_list(slide, slide_index);
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Listed {} placeholders for slide {}",
                placeholders.len(),
                args.slide_index
            ),
            snapshot_for_document(document),
        );
        response.placeholder_list = Some(placeholders);
        response.slide_list = Some(slide_list(document));
        Ok(response)
    }

    fn inspect(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: InspectArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let inspect_ndjson = inspect_document(
            document,
            args.kind.as_deref(),
            args.target_id.as_deref(),
            args.max_chars,
        );
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            "Generated inspection snapshot".to_string(),
            snapshot_for_document(document),
        );
        response.inspect_ndjson = Some(inspect_ndjson);
        response.theme = Some(document.theme_snapshot());
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn resolve(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ResolveArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let resolved_record = resolve_anchor(document, &args.id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Resolved `{}`", args.id),
            snapshot_for_document(document),
        );
        response.resolved_record = Some(resolved_record);
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn create_layout(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: CreateLayoutArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let layout_id = document.next_layout_id();
        let kind = match args.kind.as_deref() {
            Some("master") => LayoutKind::Master,
            Some("layout") | None => LayoutKind::Layout,
            Some(other) => {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: request.action,
                    message: format!("unsupported layout kind `{other}`"),
                });
            }
        };
        document.layouts.push(LayoutDocument {
            layout_id: layout_id.clone(),
            name: args.name,
            kind,
            parent_layout_id: args.parent_layout_id,
            placeholders: Vec::new(),
        });
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created layout `{layout_id}`"),
            snapshot_for_document(document),
        );
        response.layout_list = Some(layout_list(document));
        Ok(response)
    }

    fn add_layout_placeholder(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddLayoutPlaceholderArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let geometry = args
            .geometry
            .as_deref()
            .map(|value| parse_shape_geometry(value, &request.action))
            .transpose()?
            .unwrap_or(ShapeGeometry::Rectangle);
        let frame = args.position.unwrap_or(PositionArgs {
            left: 48,
            top: 72,
            width: 624,
            height: 96,
        });
        let layout = document
            .layouts
            .iter_mut()
            .find(|layout| layout.layout_id == args.layout_id)
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: request.action.clone(),
                message: format!("unknown layout id `{}`", args.layout_id),
            })?;
        layout.placeholders.push(PlaceholderDefinition {
            name: args.name,
            placeholder_type: args.placeholder_type,
            index: args.index,
            text: args.text,
            geometry,
            frame: frame.into(),
        });
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Added placeholder to layout `{}`", layout.layout_id),
            snapshot_for_document(document),
        );
        response.layout_list = Some(layout_list(document));
        Ok(response)
    }

    fn set_slide_layout(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SetSlideLayoutArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let layout = document
            .get_layout(&args.layout_id, &request.action)?
            .clone();
        let mut placeholder_elements = Vec::new();
        for placeholder in layout.placeholders {
            let element_id = document.next_element_id();
            let placeholder_ref = Some(PlaceholderRef {
                name: placeholder.name.clone(),
                placeholder_type: placeholder.placeholder_type.clone(),
                index: placeholder.index,
            });
            if placeholder.geometry == ShapeGeometry::Rectangle {
                placeholder_elements.push(PresentationElement::Text(TextElement {
                    element_id,
                    text: placeholder.text.unwrap_or_default(),
                    frame: placeholder.frame,
                    fill: None,
                    style: TextStyle::default(),
                    hyperlink: None,
                    placeholder: placeholder_ref,
                    z_order: placeholder_elements.len(),
                }));
            } else {
                placeholder_elements.push(PresentationElement::Shape(ShapeElement {
                    element_id,
                    geometry: placeholder.geometry,
                    frame: placeholder.frame,
                    fill: None,
                    stroke: None,
                    text: placeholder.text,
                    text_style: TextStyle::default(),
                    hyperlink: None,
                    placeholder: placeholder_ref,
                    rotation_degrees: None,
                    z_order: placeholder_elements.len(),
                }));
            }
        }
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.elements.retain(|element| match element {
            PresentationElement::Text(text) => text.placeholder.is_none(),
            PresentationElement::Shape(shape) => shape.placeholder.is_none(),
            _ => true,
        });
        slide.layout_id = Some(args.layout_id);
        slide.elements.extend(placeholder_elements);
        resequence_z_order(slide);
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Applied layout to slide {}", args.slide_index),
            snapshot_for_document(document),
        );
        response.slide_list = Some(slide_list(document));
        response.layout_list = Some(layout_list(document));
        Ok(response)
    }

    fn update_placeholder_text(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: UpdatePlaceholderTextArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        let target_name = args.name.to_ascii_lowercase();
        let element = slide
            .elements
            .iter_mut()
            .find(|element| match element {
                PresentationElement::Text(text) => text
                    .placeholder
                    .as_ref()
                    .map(|placeholder| placeholder.name.eq_ignore_ascii_case(&target_name))
                    .unwrap_or(false),
                PresentationElement::Shape(shape) => shape
                    .placeholder
                    .as_ref()
                    .map(|placeholder| placeholder.name.eq_ignore_ascii_case(&target_name))
                    .unwrap_or(false),
                _ => false,
            })
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: request.action.clone(),
                message: format!(
                    "placeholder `{}` was not found on slide {}",
                    args.name, args.slide_index
                ),
            })?;
        match element {
            PresentationElement::Text(text) => text.text = args.text,
            PresentationElement::Shape(shape) => shape.text = Some(args.text),
            PresentationElement::Connector(_)
            | PresentationElement::Image(_)
            | PresentationElement::Table(_)
            | PresentationElement::Chart(_) => {}
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Updated placeholder `{}` on slide {}",
                args.name, args.slide_index
            ),
            snapshot_for_document(document),
        ))
    }

    fn set_theme(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ThemeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        document.theme = normalize_theme(args, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            "Updated theme".to_string(),
            snapshot_for_document(document),
        );
        response.theme = Some(document.theme_snapshot());
        Ok(response)
    }

    fn set_notes(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: NotesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.notes.text = args.text.unwrap_or_default();
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated notes for slide {}", args.slide_index),
            snapshot_for_document(document),
        ))
    }

    fn append_notes(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: NotesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        let text = args.text.unwrap_or_default();
        if slide.notes.text.is_empty() {
            slide.notes.text = text;
        } else {
            slide.notes.text = format!("{}\n{text}", slide.notes.text);
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Appended notes for slide {}", args.slide_index),
            snapshot_for_document(document),
        ))
    }

    fn clear_notes(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: NotesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.notes.text.clear();
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Cleared notes for slide {}", args.slide_index),
            snapshot_for_document(document),
        ))
    }

    fn set_notes_visibility(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: NotesVisibilityArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.notes.visible = args.visible;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated notes visibility for slide {}", args.slide_index),
            snapshot_for_document(document),
        ))
    }

    fn set_active_slide(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SetActiveSlideArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        document.set_active_slide_index(args.slide_index as usize, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Set active slide to {}", args.slide_index),
            snapshot_for_document(document),
        );
        response.slide_list = Some(slide_list(document));
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn add_slide(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddSlideArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let mut slide = document.new_slide(args.notes, args.background_fill, &request.action)?;
        if let Some(layout_id) = args.layout {
            apply_layout_to_slide(document, &mut slide, &layout_id, &request.action)?;
        }
        let index = document.append_slide(slide);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Added slide at index {index}"),
            snapshot_for_document(document),
        ))
    }

    fn insert_slide(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: InsertSlideArgs = parse_args(&request.action, &request.args)?;
        if args.index.is_some() == args.after_slide_index.is_some() {
            return Err(PresentationArtifactError::InvalidArgs {
                action: request.action,
                message: "provide exactly one of `index` or `after_slide_index`".to_string(),
            });
        }
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let index = args.index.map(to_index).transpose()?.unwrap_or_else(|| {
            args.after_slide_index
                .map(|value| value as usize + 1)
                .unwrap_or(0)
        });
        if index > document.slides.len() {
            return Err(index_out_of_range(
                &request.action,
                index,
                document.slides.len(),
            ));
        }
        let mut slide = document.new_slide(args.notes, args.background_fill, &request.action)?;
        if let Some(layout_id) = args.layout {
            apply_layout_to_slide(document, &mut slide, &layout_id, &request.action)?;
        }
        document.adjust_active_slide_for_insert(index);
        document.slides.insert(index, slide);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Inserted slide at index {index}"),
            snapshot_for_document(document),
        ))
    }

    fn duplicate_slide(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SlideIndexArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let source = document
            .slides
            .get(args.slide_index as usize)
            .cloned()
            .ok_or_else(|| {
                index_out_of_range(
                    &request.action,
                    args.slide_index as usize,
                    document.slides.len(),
                )
            })?;
        let duplicated = document.clone_slide(source);
        let insert_at = args.slide_index as usize + 1;
        document.adjust_active_slide_for_insert(insert_at);
        document.slides.insert(insert_at, duplicated);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Duplicated slide {} to index {insert_at}", args.slide_index),
            snapshot_for_document(document),
        ))
    }

    fn move_slide(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: MoveSlideArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let from = args.from_index as usize;
        let to = args.to_index as usize;
        if from >= document.slides.len() {
            return Err(index_out_of_range(
                &request.action,
                from,
                document.slides.len(),
            ));
        }
        if to >= document.slides.len() {
            return Err(index_out_of_range(
                &request.action,
                to,
                document.slides.len(),
            ));
        }
        let slide = document.slides.remove(from);
        document.slides.insert(to, slide);
        document.adjust_active_slide_for_move(from, to);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Moved slide from index {from} to {to}"),
            snapshot_for_document(document),
        ))
    }

    fn delete_slide(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SlideIndexArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let index = args.slide_index as usize;
        if index >= document.slides.len() {
            return Err(index_out_of_range(
                &request.action,
                index,
                document.slides.len(),
            ));
        }
        document.slides.remove(index);
        document.adjust_active_slide_for_delete(index);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Deleted slide at index {index}"),
            snapshot_for_document(document),
        ))
    }

    fn set_slide_background(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SetSlideBackgroundArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let fill = normalize_color_with_document(document, &args.fill, &request.action, "fill")?;
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.background_fill = Some(fill);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated background for slide {}", args.slide_index),
            snapshot_for_document(document),
        ))
    }

    fn add_text_shape(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddTextShapeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let style = normalize_text_style_with_document(document, &args.styling, &request.action)?;
        let fill = args
            .styling
            .fill
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, &request.action, "fill"))
            .transpose()?;
        let element_id = document.next_element_id();
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.elements.push(PresentationElement::Text(TextElement {
            element_id: element_id.clone(),
            text: args.text,
            frame: args.position.into(),
            fill,
            style,
            hyperlink: None,
            placeholder: None,
            z_order: slide.elements.len(),
        }));
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Added text element `{element_id}` to slide {}",
                args.slide_index
            ),
            snapshot_for_document(document),
        ))
    }

    fn add_shape(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddShapeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let text_style =
            normalize_text_style_with_document(document, &args.text_style, &request.action)?;
        let fill = args
            .fill
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, &request.action, "fill"))
            .transpose()?;
        let stroke = parse_stroke(document, args.stroke, &request.action)?;
        let element_id = document.next_element_id();
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide
            .elements
            .push(PresentationElement::Shape(ShapeElement {
                element_id: element_id.clone(),
                geometry: parse_shape_geometry(&args.geometry, &request.action)?,
                frame: args.position.into(),
                fill,
                stroke,
                text: args.text,
                text_style,
                hyperlink: None,
                placeholder: None,
                rotation_degrees: None,
                z_order: slide.elements.len(),
            }));
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Added shape element `{element_id}` to slide {}",
                args.slide_index
            ),
            snapshot_for_document(document),
        ))
    }

    fn add_connector(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddConnectorArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let element_id = document.next_element_id();
        let line = parse_connector_line(document, args.line, &request.action)?;
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide
            .elements
            .push(PresentationElement::Connector(ConnectorElement {
                element_id: element_id.clone(),
                connector_type: parse_connector_kind(&args.connector_type, &request.action)?,
                start: args.start,
                end: args.end,
                line: StrokeStyle {
                    color: line.color,
                    width: line.width,
                },
                line_style: line.style,
                start_arrow: args
                    .start_arrow
                    .as_deref()
                    .map(|value| parse_connector_arrow(value, &request.action))
                    .transpose()?
                    .unwrap_or(ConnectorArrowKind::None),
                end_arrow: args
                    .end_arrow
                    .as_deref()
                    .map(|value| parse_connector_arrow(value, &request.action))
                    .transpose()?
                    .unwrap_or(ConnectorArrowKind::None),
                arrow_size: args
                    .arrow_size
                    .as_deref()
                    .map(|value| parse_connector_arrow_size(value, &request.action))
                    .transpose()?
                    .unwrap_or(ConnectorArrowScale::Medium),
                label: args.label,
                z_order: slide.elements.len(),
            }));
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Added connector element `{element_id}` to slide {}",
                args.slide_index
            ),
            snapshot_for_document(document),
        ))
    }

    fn add_image(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddImageArgs = parse_args(&request.action, &request.args)?;
        let image_source = args.image_source()?;
        let is_placeholder = matches!(image_source, ImageInputSource::Placeholder);
        let image_payload = match image_source {
            ImageInputSource::Path(path) => Some(load_image_payload_from_path(
                &resolve_path(cwd, &path),
                &request.action,
            )?),
            ImageInputSource::DataUrl(data_url) => Some(load_image_payload_from_data_url(
                &data_url,
                &request.action,
            )?),
            ImageInputSource::Uri(uri) => Some(load_image_payload_from_uri(&uri, &request.action)?),
            ImageInputSource::Placeholder => None,
        };
        let fit_mode = args.fit.unwrap_or(ImageFitMode::Stretch);
        let lock_aspect_ratio = args
            .lock_aspect_ratio
            .unwrap_or(fit_mode != ImageFitMode::Stretch);
        let crop = args
            .crop
            .map(|crop| normalize_image_crop(crop, &request.action))
            .transpose()?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let element_id = document.next_element_id();
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide
            .elements
            .push(PresentationElement::Image(ImageElement {
                element_id: element_id.clone(),
                frame: args.position.into(),
                payload: image_payload,
                fit_mode,
                crop,
                rotation_degrees: args.rotation,
                flip_horizontal: args.flip_horizontal.unwrap_or(false),
                flip_vertical: args.flip_vertical.unwrap_or(false),
                lock_aspect_ratio,
                alt_text: args.alt,
                prompt: args.prompt,
                is_placeholder,
                z_order: slide.elements.len(),
            }));
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Added image element `{element_id}` to slide {}",
                args.slide_index
            ),
            snapshot_for_document(document),
        ))
    }

    fn replace_image(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ReplaceImageArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let image_source = match (&args.path, &args.data_url, &args.uri, &args.prompt) {
            (Some(path), None, None, None) => ImageInputSource::Path(path.clone()),
            (None, Some(data_url), None, None) => ImageInputSource::DataUrl(data_url.clone()),
            (None, None, Some(uri), None) => ImageInputSource::Uri(uri.clone()),
            (None, None, None, Some(_)) => ImageInputSource::Placeholder,
            _ => {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: request.action,
                    message:
                        "provide exactly one of `path`, `data_url`, or `uri`, or provide `prompt` for a placeholder image"
                            .to_string(),
                });
            }
        };
        let is_placeholder = matches!(image_source, ImageInputSource::Placeholder);
        let image_payload = match image_source {
            ImageInputSource::Path(path) => Some(load_image_payload_from_path(
                &resolve_path(cwd, &path),
                "replace_image",
            )?),
            ImageInputSource::DataUrl(data_url) => Some(load_image_payload_from_data_url(
                &data_url,
                "replace_image",
            )?),
            ImageInputSource::Uri(uri) => Some(load_image_payload_from_uri(&uri, "replace_image")?),
            ImageInputSource::Placeholder => None,
        };
        let fit_mode = args.fit.unwrap_or(ImageFitMode::Stretch);
        let lock_aspect_ratio = args
            .lock_aspect_ratio
            .unwrap_or(fit_mode != ImageFitMode::Stretch);
        let crop = args
            .crop
            .map(|crop| normalize_image_crop(crop, &request.action))
            .transpose()?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Image(image) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not an image", args.element_id),
            });
        };
        image.payload = image_payload;
        image.fit_mode = fit_mode;
        image.crop = crop;
        if let Some(rotation) = args.rotation {
            image.rotation_degrees = Some(rotation);
        }
        if let Some(flip_horizontal) = args.flip_horizontal {
            image.flip_horizontal = flip_horizontal;
        }
        if let Some(flip_vertical) = args.flip_vertical {
            image.flip_vertical = flip_vertical;
        }
        image.lock_aspect_ratio = lock_aspect_ratio;
        image.alt_text = args.alt;
        image.prompt = args.prompt;
        image.is_placeholder = is_placeholder;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            "replace_image".to_string(),
            format!("Replaced image `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn add_table(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddTableArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let rows = coerce_table_rows(args.rows, &request.action)?;
        let mut frame: Rect = args.position.into();
        let (column_widths, row_heights) = normalize_table_dimensions(
            &rows,
            frame,
            args.column_widths,
            args.row_heights,
            &request.action,
        )?;
        frame.width = column_widths.iter().sum();
        frame.height = row_heights.iter().sum();
        let element_id = document.next_element_id();
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide
            .elements
            .push(PresentationElement::Table(TableElement {
                element_id: element_id.clone(),
                frame,
                rows,
                column_widths,
                row_heights,
                style: args.style,
                merges: Vec::new(),
                z_order: slide.elements.len(),
            }));
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Added table element `{element_id}` to slide {}",
                args.slide_index
            ),
            snapshot_for_document(document),
        ))
    }

    fn update_table_cell(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: UpdateTableCellArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let text_style =
            normalize_text_style_with_document(document, &args.styling, &request.action)?;
        let background_fill = args
            .background_fill
            .as_deref()
            .map(|fill| {
                normalize_color_with_document(document, fill, &request.action, "background_fill")
            })
            .transpose()?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Table(table) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not a table", args.element_id),
            });
        };
        let row = args.row as usize;
        let column = args.column as usize;
        if row >= table.rows.len() || column >= table.rows[row].len() {
            return Err(PresentationArtifactError::InvalidArgs {
                action: request.action,
                message: format!("cell ({row}, {column}) is out of bounds"),
            });
        }
        let cell = &mut table.rows[row][column];
        cell.text = cell_value_to_string(args.value);
        cell.text_style = text_style;
        cell.background_fill = background_fill;
        cell.alignment = args
            .alignment
            .as_deref()
            .map(|value| parse_alignment(value, &request.action))
            .transpose()?;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated table cell ({row}, {column})"),
            snapshot_for_document(document),
        ))
    }

    fn merge_table_cells(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: MergeTableCellsArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Table(table) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not a table", args.element_id),
            });
        };
        let region = TableMergeRegion {
            start_row: args.start_row as usize,
            end_row: args.end_row as usize,
            start_column: args.start_column as usize,
            end_column: args.end_column as usize,
        };
        table.merges.push(region);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Merged table cells in `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn add_chart(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddChartArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let chart_type = parse_chart_type(&args.chart_type, &request.action)?;
        let series = args
            .series
            .into_iter()
            .map(|entry| {
                if entry.values.is_empty() {
                    return Err(PresentationArtifactError::InvalidArgs {
                        action: request.action.clone(),
                        message: format!("series `{}` must contain at least one value", entry.name),
                    });
                }
                Ok(ChartSeriesSpec {
                    name: entry.name,
                    values: entry.values,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let element_id = document.next_element_id();
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide
            .elements
            .push(PresentationElement::Chart(ChartElement {
                element_id: element_id.clone(),
                frame: args.position.into(),
                chart_type,
                categories: args.categories,
                series,
                title: args.title,
                z_order: slide.elements.len(),
            }));
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Added chart element `{element_id}` to slide {}",
                args.slide_index
            ),
            snapshot_for_document(document),
        ))
    }

    fn update_text(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: UpdateTextArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let style = normalize_text_style_with_document(document, &args.styling, &request.action)?;
        let fill = args
            .styling
            .fill
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, &request.action, "fill"))
            .transpose()?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => {
                text.text = args.text;
                if let Some(fill) = fill.clone() {
                    text.fill = Some(fill);
                }
                text.style = style;
            }
            PresentationElement::Shape(shape) => {
                if shape.text.is_none() {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: format!(
                            "element `{}` does not contain editable text",
                            args.element_id
                        ),
                    });
                }
                shape.text = Some(args.text);
                if let Some(fill) = fill {
                    shape.fill = Some(fill);
                }
                shape.text_style = style;
            }
            other => {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: request.action,
                    message: format!(
                        "element `{}` is `{}`; only text-bearing elements support `update_text`",
                        args.element_id,
                        other.kind()
                    ),
                });
            }
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated text for element `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn replace_text(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ReplaceTextArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => {
                if !text.text.contains(&args.search) {
                    return Err(PresentationArtifactError::InvalidArgs {
                        action: request.action,
                        message: format!(
                            "text `{}` was not found in element `{}`",
                            args.search, args.element_id
                        ),
                    });
                }
                text.text = text.text.replace(&args.search, &args.replace);
            }
            PresentationElement::Shape(shape) => {
                let Some(text) = &mut shape.text else {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: format!(
                            "element `{}` does not contain editable text",
                            args.element_id
                        ),
                    });
                };
                if !text.contains(&args.search) {
                    return Err(PresentationArtifactError::InvalidArgs {
                        action: request.action,
                        message: format!(
                            "text `{}` was not found in element `{}`",
                            args.search, args.element_id
                        ),
                    });
                }
                *text = text.replace(&args.search, &args.replace);
            }
            other => {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: request.action,
                    message: format!(
                        "element `{}` is `{}`; only text-bearing elements support `replace_text`",
                        args.element_id,
                        other.kind()
                    ),
                });
            }
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Replaced text in element `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn insert_text_after(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: InsertTextAfterArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => {
                let Some(index) = text.text.find(&args.after) else {
                    return Err(PresentationArtifactError::InvalidArgs {
                        action: request.action,
                        message: format!(
                            "text `{}` was not found in element `{}`",
                            args.after, args.element_id
                        ),
                    });
                };
                let insert_at = index + args.after.len();
                text.text.insert_str(insert_at, &args.insert);
            }
            PresentationElement::Shape(shape) => {
                let Some(text) = &mut shape.text else {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: format!(
                            "element `{}` does not contain editable text",
                            args.element_id
                        ),
                    });
                };
                let Some(index) = text.find(&args.after) else {
                    return Err(PresentationArtifactError::InvalidArgs {
                        action: request.action,
                        message: format!(
                            "text `{}` was not found in element `{}`",
                            args.after, args.element_id
                        ),
                    });
                };
                let insert_at = index + args.after.len();
                text.insert_str(insert_at, &args.insert);
            }
            other => {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: request.action,
                    message: format!(
                        "element `{}` is `{}`; only text-bearing elements support `insert_text_after`",
                        args.element_id,
                        other.kind()
                    ),
                });
            }
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Inserted text in element `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn set_hyperlink(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SetHyperlinkArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let clear = args.clear.unwrap_or(false);
        let hyperlink = if clear {
            None
        } else {
            Some(parse_hyperlink_state(document, &args, &request.action)?)
        };
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => text.hyperlink = hyperlink,
            PresentationElement::Shape(shape) => shape.hyperlink = hyperlink,
            other => {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: request.action,
                    message: format!(
                        "element `{}` is `{}`; only text boxes and shapes support `set_hyperlink`",
                        args.element_id,
                        other.kind()
                    ),
                });
            }
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            "set_hyperlink".to_string(),
            if clear {
                format!("Cleared hyperlink for element `{}`", args.element_id)
            } else {
                format!("Updated hyperlink for element `{}`", args.element_id)
            },
            snapshot_for_document(document),
        ))
    }

    fn update_shape_style(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: UpdateShapeStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let fill = args
            .fill
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, &request.action, "fill"))
            .transpose()?;
        let stroke = args
            .stroke
            .clone()
            .map(|value| parse_required_stroke(document, value, &request.action))
            .transpose()?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => {
                if let Some(position) = args.position {
                    text.frame = apply_partial_position(text.frame, position);
                }
                if let Some(fill) = fill.clone() {
                    text.fill = Some(fill);
                }
                if args.stroke.is_some()
                    || args.rotation.is_some()
                    || args.flip_horizontal.is_some()
                    || args.flip_vertical.is_some()
                {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message:
                            "text elements support only `position`, `z_order`, and `fill` updates"
                                .to_string(),
                    });
                }
            }
            PresentationElement::Shape(shape) => {
                if let Some(position) = args.position {
                    shape.frame = apply_partial_position(shape.frame, position);
                }
                if let Some(fill) = fill {
                    shape.fill = Some(fill);
                }
                if let Some(stroke) = stroke {
                    shape.stroke = Some(stroke);
                }
                if args.flip_horizontal.is_some() || args.flip_vertical.is_some() {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message:
                            "shape elements support `position`, `fill`, `stroke`, `rotation`, and `z_order` updates"
                                .to_string(),
                    });
                }
                if let Some(rotation) = args.rotation {
                    shape.rotation_degrees = Some(rotation);
                }
            }
            PresentationElement::Connector(connector) => {
                if args.fill.is_some()
                    || args.rotation.is_some()
                    || args.flip_horizontal.is_some()
                    || args.flip_vertical.is_some()
                    || args.fit.is_some()
                    || args.crop.is_some()
                    || args.lock_aspect_ratio.is_some()
                {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message:
                            "connector elements support only `position`, `stroke`, and `z_order` updates"
                                .to_string(),
                    });
                }
                if let Some(position) = args.position {
                    let updated = apply_partial_position(
                        Rect {
                            left: connector.start.left,
                            top: connector.start.top,
                            width: connector.end.left.abs_diff(connector.start.left),
                            height: connector.end.top.abs_diff(connector.start.top),
                        },
                        position,
                    );
                    connector.start = PointArgs {
                        left: updated.left,
                        top: updated.top,
                    };
                    connector.end = PointArgs {
                        left: updated.left.saturating_add(updated.width),
                        top: updated.top.saturating_add(updated.height),
                    };
                }
                if let Some(stroke) = stroke {
                    connector.line = stroke;
                }
            }
            PresentationElement::Image(image) => {
                if args.fill.is_some() || args.stroke.is_some() {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message:
                            "image elements support only `position`, `fit`, `crop`, `rotation`, `flip_horizontal`, `flip_vertical`, `lock_aspect_ratio`, and `z_order` updates"
                                .to_string(),
                    });
                }
                if let Some(position) = args.position {
                    image.frame = apply_partial_position_to_image(image, position);
                }
                if let Some(fit) = args.fit {
                    image.fit_mode = fit;
                    if !matches!(fit, ImageFitMode::Stretch) && args.lock_aspect_ratio.is_none() {
                        image.lock_aspect_ratio = true;
                    }
                }
                if let Some(crop) = args.crop {
                    image.crop = Some(normalize_image_crop(crop, &request.action)?);
                }
                if let Some(rotation) = args.rotation {
                    image.rotation_degrees = Some(rotation);
                }
                if let Some(flip_horizontal) = args.flip_horizontal {
                    image.flip_horizontal = flip_horizontal;
                }
                if let Some(flip_vertical) = args.flip_vertical {
                    image.flip_vertical = flip_vertical;
                }
                if let Some(lock_aspect_ratio) = args.lock_aspect_ratio {
                    image.lock_aspect_ratio = lock_aspect_ratio;
                }
            }
            PresentationElement::Table(table) => {
                if args.fill.is_some()
                    || args.stroke.is_some()
                    || args.rotation.is_some()
                    || args.flip_horizontal.is_some()
                    || args.flip_vertical.is_some()
                {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: "table elements support only `position` and `z_order` updates"
                            .to_string(),
                    });
                }
                if let Some(position) = args.position {
                    table.frame = apply_partial_position(table.frame, position);
                }
            }
            PresentationElement::Chart(chart) => {
                if args.fill.is_some()
                    || args.stroke.is_some()
                    || args.rotation.is_some()
                    || args.flip_horizontal.is_some()
                    || args.flip_vertical.is_some()
                {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: "chart elements support only `position` and `z_order` updates"
                            .to_string(),
                    });
                }
                if let Some(position) = args.position {
                    chart.frame = apply_partial_position(chart.frame, position);
                }
            }
        }
        if let Some(z_order) = args.z_order {
            document.set_z_order(&args.element_id, z_order as usize, &request.action)?;
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated style for element `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn delete_element(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ElementIdArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        document.remove_element(&args.element_id, &request.action)?;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Deleted element `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn bring_to_front(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ElementIdArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let target_index = document.total_element_count();
        document.set_z_order(&args.element_id, target_index, &request.action)?;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Brought `{}` to front", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn send_to_back(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ElementIdArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        document.set_z_order(&args.element_id, 0, &request.action)?;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Sent `{}` to back", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn delete_artifact(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let removed = self.documents.remove(&artifact_id).ok_or_else(|| {
            PresentationArtifactError::UnknownArtifactId {
                action: request.action.clone(),
                artifact_id: artifact_id.clone(),
            }
        })?;
        Ok(PresentationArtifactResponse {
            artifact_id,
            action: request.action,
            summary: format!(
                "Deleted in-memory artifact `{}` with {} slides",
                removed.artifact_id,
                removed.slides.len()
            ),
            exported_paths: Vec::new(),
            artifact_snapshot: None,
            slide_list: None,
            layout_list: None,
            placeholder_list: None,
            theme: None,
            inspect_ndjson: None,
            resolved_record: None,
            active_slide_index: None,
        })
    }

    fn get_document(
        &self,
        artifact_id: &str,
        action: &str,
    ) -> Result<&PresentationDocument, PresentationArtifactError> {
        self.documents.get(artifact_id).ok_or_else(|| {
            PresentationArtifactError::UnknownArtifactId {
                action: action.to_string(),
                artifact_id: artifact_id.to_string(),
            }
        })
    }

    fn get_document_mut(
        &mut self,
        artifact_id: &str,
        action: &str,
    ) -> Result<&mut PresentationDocument, PresentationArtifactError> {
        self.documents.get_mut(artifact_id).ok_or_else(|| {
            PresentationArtifactError::UnknownArtifactId {
                action: action.to_string(),
                artifact_id: artifact_id.to_string(),
            }
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PresentationArtifactResponse {
    pub artifact_id: String,
    pub action: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exported_paths: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_snapshot: Option<ArtifactSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide_list: Option<Vec<SlideListEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_list: Option<Vec<LayoutListEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder_list: Option<Vec<PlaceholderListEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspect_ndjson: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_record: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_slide_index: Option<usize>,
}

impl PresentationArtifactResponse {
    fn new(
        artifact_id: String,
        action: String,
        summary: String,
        artifact_snapshot: ArtifactSnapshot,
    ) -> Self {
        Self {
            artifact_id,
            action,
            summary,
            exported_paths: Vec::new(),
            artifact_snapshot: Some(artifact_snapshot),
            slide_list: None,
            layout_list: None,
            placeholder_list: None,
            theme: None,
            inspect_ndjson: None,
            resolved_record: None,
            active_slide_index: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactSnapshot {
    pub slide_count: usize,
    pub slides: Vec<SlideSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlideSnapshot {
    pub slide_id: String,
    pub index: usize,
    pub element_ids: Vec<String>,
    pub element_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlideListEntry {
    pub slide_id: String,
    pub index: usize,
    pub is_active: bool,
    pub notes: Option<String>,
    pub notes_visible: bool,
    pub background_fill: Option<String>,
    pub layout_id: Option<String>,
    pub element_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutListEntry {
    pub layout_id: String,
    pub name: String,
    pub kind: String,
    pub parent_layout_id: Option<String>,
    pub placeholder_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceholderListEntry {
    pub scope: String,
    pub source_layout_id: Option<String>,
    pub slide_index: Option<usize>,
    pub element_id: Option<String>,
    pub name: String,
    pub placeholder_type: String,
    pub index: Option<u32>,
    pub geometry: Option<String>,
    pub text_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeSnapshot {
    pub color_scheme: HashMap<String, String>,
    pub major_font: Option<String>,
    pub minor_font: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ThemeState {
    color_scheme: HashMap<String, String>,
    major_font: Option<String>,
    minor_font: Option<String>,
}

impl ThemeState {
    fn resolve_color(&self, color: &str) -> Option<String> {
        let key = color.trim().to_ascii_lowercase();
        let alias = match key.as_str() {
            "background1" => "bg1",
            "background2" => "bg2",
            "text1" => "tx1",
            "text2" => "tx2",
            "dark1" => "dk1",
            "dark2" => "dk2",
            "light1" => "lt1",
            "light2" => "lt2",
            other => other,
        };
        self.color_scheme
            .get(alias)
            .or_else(|| self.color_scheme.get(&key))
            .cloned()
            .map(|value| value.trim_start_matches('#').to_uppercase())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutKind {
    Layout,
    Master,
}

#[derive(Debug, Clone)]
struct LayoutDocument {
    layout_id: String,
    name: String,
    kind: LayoutKind,
    parent_layout_id: Option<String>,
    placeholders: Vec<PlaceholderDefinition>,
}

#[derive(Debug, Clone)]
struct PlaceholderDefinition {
    name: String,
    placeholder_type: String,
    index: Option<u32>,
    text: Option<String>,
    geometry: ShapeGeometry,
    frame: Rect,
}

#[derive(Debug, Clone)]
struct ResolvedPlaceholder {
    source_layout_id: String,
    definition: PlaceholderDefinition,
}

#[derive(Debug, Clone, Default)]
struct NotesState {
    text: String,
    visible: bool,
}

#[derive(Debug, Clone, Default)]
struct TextStyle {
    font_size: Option<u32>,
    font_family: Option<String>,
    color: Option<String>,
    alignment: Option<TextAlignment>,
    bold: bool,
    italic: bool,
    underline: bool,
}

#[derive(Debug, Clone)]
struct HyperlinkState {
    target: HyperlinkTarget,
    tooltip: Option<String>,
    highlight_click: bool,
}

#[derive(Debug, Clone)]
enum HyperlinkTarget {
    Url(String),
    Slide(u32),
    FirstSlide,
    LastSlide,
    NextSlide,
    PreviousSlide,
    EndShow,
    Email {
        address: String,
        subject: Option<String>,
    },
    File(String),
}

impl HyperlinkTarget {
    fn relationship_target(&self) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::Slide(slide_index) => format!("slide{}.xml", slide_index + 1),
            Self::FirstSlide => "ppaction://hlinkshowjump?jump=firstslide".to_string(),
            Self::LastSlide => "ppaction://hlinkshowjump?jump=lastslide".to_string(),
            Self::NextSlide => "ppaction://hlinkshowjump?jump=nextslide".to_string(),
            Self::PreviousSlide => "ppaction://hlinkshowjump?jump=previousslide".to_string(),
            Self::EndShow => "ppaction://hlinkshowjump?jump=endshow".to_string(),
            Self::Email { address, subject } => {
                let mut mailto = format!("mailto:{address}");
                if let Some(subject) = subject {
                    mailto.push_str(&format!("?subject={subject}"));
                }
                mailto
            }
            Self::File(path) => format!("file:///{}", path.replace('\\', "/")),
        }
    }

    fn is_external(&self) -> bool {
        matches!(self, Self::Url(_) | Self::Email { .. } | Self::File(_))
    }
}

impl HyperlinkState {
    fn to_ppt_rs(&self, relationship_id: &str) -> PptHyperlink {
        let hyperlink = match &self.target {
            HyperlinkTarget::Url(url) => PptHyperlink::new(PptHyperlinkAction::url(url)),
            HyperlinkTarget::Slide(slide_index) => {
                PptHyperlink::new(PptHyperlinkAction::slide(slide_index + 1))
            }
            HyperlinkTarget::FirstSlide => PptHyperlink::new(PptHyperlinkAction::FirstSlide),
            HyperlinkTarget::LastSlide => PptHyperlink::new(PptHyperlinkAction::LastSlide),
            HyperlinkTarget::NextSlide => PptHyperlink::new(PptHyperlinkAction::NextSlide),
            HyperlinkTarget::PreviousSlide => PptHyperlink::new(PptHyperlinkAction::PreviousSlide),
            HyperlinkTarget::EndShow => PptHyperlink::new(PptHyperlinkAction::EndShow),
            HyperlinkTarget::Email { address, subject } => PptHyperlink::new(match subject {
                Some(subject) => PptHyperlinkAction::email_with_subject(address, subject),
                None => PptHyperlinkAction::email(address),
            }),
            HyperlinkTarget::File(path) => PptHyperlink::new(PptHyperlinkAction::file(path)),
        };
        let hyperlink = if let Some(tooltip) = &self.tooltip {
            hyperlink.with_tooltip(tooltip)
        } else {
            hyperlink
        };
        hyperlink
            .with_highlight_click(self.highlight_click)
            .with_r_id(relationship_id)
    }

    fn to_json(&self) -> Value {
        let mut record = match &self.target {
            HyperlinkTarget::Url(url) => serde_json::json!({
                "type": "url",
                "url": url,
            }),
            HyperlinkTarget::Slide(slide_index) => serde_json::json!({
                "type": "slide",
                "slideIndex": slide_index,
            }),
            HyperlinkTarget::FirstSlide => serde_json::json!({
                "type": "firstSlide",
            }),
            HyperlinkTarget::LastSlide => serde_json::json!({
                "type": "lastSlide",
            }),
            HyperlinkTarget::NextSlide => serde_json::json!({
                "type": "nextSlide",
            }),
            HyperlinkTarget::PreviousSlide => serde_json::json!({
                "type": "previousSlide",
            }),
            HyperlinkTarget::EndShow => serde_json::json!({
                "type": "endShow",
            }),
            HyperlinkTarget::Email { address, subject } => serde_json::json!({
                "type": "email",
                "address": address,
                "subject": subject,
            }),
            HyperlinkTarget::File(path) => serde_json::json!({
                "type": "file",
                "path": path,
            }),
        };
        record["tooltip"] = self
            .tooltip
            .as_ref()
            .map(|tooltip| Value::String(tooltip.clone()))
            .unwrap_or(Value::Null);
        record["highlightClick"] = Value::Bool(self.highlight_click);
        record
    }

    fn relationship_xml(&self, relationship_id: &str) -> String {
        let target_mode = if self.target.is_external() {
            r#" TargetMode="External""#
        } else {
            ""
        };
        format!(
            r#"<Relationship Id="{relationship_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}"{target_mode}/>"#,
            ppt_rs::escape_xml(&self.target.relationship_target()),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone)]
struct PlaceholderRef {
    name: String,
    placeholder_type: String,
    index: Option<u32>,
}

#[derive(Debug, Clone)]
struct TableMergeRegion {
    start_row: usize,
    end_row: usize,
    start_column: usize,
    end_column: usize,
}

#[derive(Debug, Clone)]
struct TableCellSpec {
    text: String,
    text_style: TextStyle,
    background_fill: Option<String>,
    alignment: Option<TextAlignment>,
}

#[derive(Debug, Clone)]
struct PresentationDocument {
    artifact_id: String,
    name: Option<String>,
    slide_size: Rect,
    theme: ThemeState,
    layouts: Vec<LayoutDocument>,
    slides: Vec<PresentationSlide>,
    active_slide_index: Option<usize>,
    next_slide_seq: u32,
    next_element_seq: u32,
    next_layout_seq: u32,
}

impl PresentationDocument {
    fn new(name: Option<String>) -> Self {
        Self {
            artifact_id: format!("presentation_{}", Uuid::new_v4().simple()),
            name,
            slide_size: Rect {
                left: 0,
                top: 0,
                width: DEFAULT_SLIDE_WIDTH_POINTS,
                height: DEFAULT_SLIDE_HEIGHT_POINTS,
            },
            theme: ThemeState::default(),
            layouts: Vec::new(),
            slides: Vec::new(),
            active_slide_index: None,
            next_slide_seq: 1,
            next_element_seq: 1,
            next_layout_seq: 1,
        }
    }

    fn from_ppt_rs(presentation: Presentation) -> Self {
        let mut document = Self::new(
            (!presentation.get_title().is_empty()).then(|| presentation.get_title().to_string()),
        );
        for imported_slide in presentation.slides() {
            let mut slide = PresentationSlide {
                slide_id: format!("slide_{}", document.next_slide_seq),
                notes: NotesState {
                    text: imported_slide.notes.clone().unwrap_or_default(),
                    visible: true,
                },
                background_fill: None,
                layout_id: None,
                elements: Vec::new(),
            };
            document.next_slide_seq += 1;

            if !imported_slide.title.is_empty() {
                slide.elements.push(PresentationElement::Text(TextElement {
                    element_id: document.next_element_id(),
                    text: imported_slide.title.clone(),
                    frame: Rect {
                        left: DEFAULT_IMPORTED_TITLE_LEFT,
                        top: DEFAULT_IMPORTED_TITLE_TOP,
                        width: DEFAULT_IMPORTED_TITLE_WIDTH,
                        height: DEFAULT_IMPORTED_TITLE_HEIGHT,
                    },
                    fill: None,
                    style: TextStyle::default(),
                    hyperlink: None,
                    placeholder: None,
                    z_order: slide.elements.len(),
                }));
            }

            if !imported_slide.content.is_empty() {
                slide.elements.push(PresentationElement::Text(TextElement {
                    element_id: document.next_element_id(),
                    text: imported_slide.content.join("\n"),
                    frame: Rect {
                        left: DEFAULT_IMPORTED_CONTENT_LEFT,
                        top: DEFAULT_IMPORTED_CONTENT_TOP,
                        width: DEFAULT_IMPORTED_CONTENT_WIDTH,
                        height: DEFAULT_IMPORTED_CONTENT_HEIGHT,
                    },
                    fill: None,
                    style: TextStyle::default(),
                    hyperlink: None,
                    placeholder: None,
                    z_order: slide.elements.len(),
                }));
            }

            for imported_shape in &imported_slide.shapes {
                slide
                    .elements
                    .push(PresentationElement::Shape(ShapeElement {
                        element_id: document.next_element_id(),
                        geometry: ShapeGeometry::from_shape_type(imported_shape.shape_type),
                        frame: Rect::from_emu(
                            imported_shape.x,
                            imported_shape.y,
                            imported_shape.width,
                            imported_shape.height,
                        ),
                        fill: imported_shape.fill.as_ref().map(|fill| fill.color.clone()),
                        stroke: imported_shape.line.as_ref().map(|line| StrokeStyle {
                            color: line.color.clone(),
                            width: emu_to_points(line.width),
                        }),
                        text: imported_shape.text.clone(),
                        text_style: TextStyle::default(),
                        hyperlink: None,
                        placeholder: None,
                        rotation_degrees: imported_shape.rotation,
                        z_order: slide.elements.len(),
                    }));
            }

            if let Some(imported_table) = &imported_slide.table {
                slide
                    .elements
                    .push(PresentationElement::Table(TableElement {
                        element_id: document.next_element_id(),
                        frame: Rect::from_emu(
                            imported_table.x,
                            imported_table.y,
                            imported_table.width(),
                            imported_table.height(),
                        ),
                        rows: imported_table
                            .rows
                            .iter()
                            .map(|row| {
                                row.cells
                                    .iter()
                                    .map(|text| TableCellSpec {
                                        text: text.text.clone(),
                                        text_style: TextStyle::default(),
                                        background_fill: None,
                                        alignment: None,
                                    })
                                    .collect()
                            })
                            .collect(),
                        column_widths: imported_table
                            .column_widths
                            .iter()
                            .copied()
                            .map(emu_to_points)
                            .collect(),
                        row_heights: imported_table
                            .rows
                            .iter()
                            .map(|row| row.height.map_or(400_000, |height| height))
                            .map(emu_to_points)
                            .collect(),
                        style: None,
                        merges: Vec::new(),
                        z_order: slide.elements.len(),
                    }));
            }

            document.slides.push(slide);
        }
        document.active_slide_index = (!document.slides.is_empty()).then_some(0);
        document
    }

    fn new_slide(
        &mut self,
        notes: Option<String>,
        background_fill: Option<String>,
        action: &str,
    ) -> Result<PresentationSlide, PresentationArtifactError> {
        let normalized_fill = background_fill
            .map(|value| {
                normalize_color_with_palette(Some(&self.theme), &value, action, "background_fill")
            })
            .transpose()?;
        let slide = PresentationSlide {
            slide_id: format!("slide_{}", self.next_slide_seq),
            notes: NotesState {
                text: notes.unwrap_or_default(),
                visible: true,
            },
            background_fill: normalized_fill,
            layout_id: None,
            elements: Vec::new(),
        };
        self.next_slide_seq += 1;
        Ok(slide)
    }

    fn append_slide(&mut self, slide: PresentationSlide) -> usize {
        let index = self.slides.len();
        self.slides.push(slide);
        if self.active_slide_index.is_none() {
            self.active_slide_index = Some(index);
        }
        index
    }

    fn clone_slide(&mut self, slide: PresentationSlide) -> PresentationSlide {
        let mut clone = slide;
        clone.slide_id = format!("slide_{}", self.next_slide_seq);
        self.next_slide_seq += 1;
        for element in &mut clone.elements {
            element.set_element_id(self.next_element_id());
        }
        clone
    }

    fn next_element_id(&mut self) -> String {
        let element_id = format!("element_{}", self.next_element_seq);
        self.next_element_seq += 1;
        element_id
    }

    fn total_element_count(&self) -> usize {
        self.slides.iter().map(|slide| slide.elements.len()).sum()
    }

    fn set_active_slide_index(
        &mut self,
        slide_index: usize,
        action: &str,
    ) -> Result<(), PresentationArtifactError> {
        if slide_index >= self.slides.len() {
            return Err(index_out_of_range(action, slide_index, self.slides.len()));
        }
        self.active_slide_index = Some(slide_index);
        Ok(())
    }

    fn adjust_active_slide_for_insert(&mut self, inserted_index: usize) {
        match self.active_slide_index {
            None => self.active_slide_index = Some(inserted_index),
            Some(active_index) if inserted_index <= active_index => {
                self.active_slide_index = Some(active_index + 1);
            }
            Some(_) => {}
        }
    }

    fn adjust_active_slide_for_move(&mut self, from_index: usize, to_index: usize) {
        if let Some(active_index) = self.active_slide_index {
            self.active_slide_index = Some(if active_index == from_index {
                to_index
            } else if from_index < active_index && active_index <= to_index {
                active_index - 1
            } else if to_index <= active_index && active_index < from_index {
                active_index + 1
            } else {
                active_index
            });
        }
    }

    fn adjust_active_slide_for_delete(&mut self, deleted_index: usize) {
        self.active_slide_index = match self.active_slide_index {
            None => None,
            Some(_) if self.slides.is_empty() => None,
            Some(active_index) if active_index == deleted_index => {
                Some(deleted_index.min(self.slides.len() - 1))
            }
            Some(active_index) if deleted_index < active_index => Some(active_index - 1),
            Some(active_index) => Some(active_index),
        };
    }

    fn next_layout_id(&mut self) -> String {
        let layout_id = format!("layout_{}", self.next_layout_seq);
        self.next_layout_seq += 1;
        layout_id
    }

    fn get_layout(
        &self,
        layout_id: &str,
        action: &str,
    ) -> Result<&LayoutDocument, PresentationArtifactError> {
        self.layouts
            .iter()
            .find(|layout| layout.layout_id == layout_id)
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("unknown layout id `{layout_id}`"),
            })
    }

    fn theme_snapshot(&self) -> ThemeSnapshot {
        ThemeSnapshot {
            color_scheme: self.theme.color_scheme.clone(),
            major_font: self.theme.major_font.clone(),
            minor_font: self.theme.minor_font.clone(),
        }
    }

    fn find_element_mut(
        &mut self,
        element_id: &str,
        action: &str,
    ) -> Result<&mut PresentationElement, PresentationArtifactError> {
        let element_id = normalize_element_lookup_id(element_id);
        for slide in &mut self.slides {
            if let Some(element) = slide
                .elements
                .iter_mut()
                .find(|element| element.element_id() == element_id)
            {
                return Ok(element);
            }
        }
        Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("unknown element id `{element_id}`"),
        })
    }

    fn get_slide_mut(
        &mut self,
        slide_index: u32,
        action: &str,
    ) -> Result<&mut PresentationSlide, PresentationArtifactError> {
        let index = slide_index as usize;
        if index >= self.slides.len() {
            return Err(index_out_of_range(action, index, self.slides.len()));
        }
        Ok(&mut self.slides[index])
    }

    fn remove_element(
        &mut self,
        element_id: &str,
        action: &str,
    ) -> Result<(), PresentationArtifactError> {
        let element_id = normalize_element_lookup_id(element_id);
        for slide in &mut self.slides {
            if let Some(index) = slide
                .elements
                .iter()
                .position(|element| element.element_id() == element_id)
            {
                slide.elements.remove(index);
                resequence_z_order(slide);
                return Ok(());
            }
        }
        Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("unknown element id `{element_id}`"),
        })
    }

    fn set_z_order(
        &mut self,
        element_id: &str,
        target_index: usize,
        action: &str,
    ) -> Result<(), PresentationArtifactError> {
        let element_id = normalize_element_lookup_id(element_id);
        for slide in &mut self.slides {
            if let Some(current_index) = slide
                .elements
                .iter()
                .position(|element| element.element_id() == element_id)
            {
                let destination = target_index.min(slide.elements.len().saturating_sub(1));
                let element = slide.elements.remove(current_index);
                slide.elements.insert(destination, element);
                resequence_z_order(slide);
                return Ok(());
            }
        }
        Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("unknown element id `{element_id}`"),
        })
    }

    fn to_ppt_rs(&self) -> Presentation {
        let mut presentation = self
            .name
            .as_deref()
            .map(Presentation::with_title)
            .unwrap_or_default();
        for slide in &self.slides {
            presentation = presentation.add_slide(slide.to_ppt_rs(self.slide_size));
        }
        presentation
    }
}

#[derive(Debug, Clone)]
struct PresentationSlide {
    slide_id: String,
    notes: NotesState,
    background_fill: Option<String>,
    layout_id: Option<String>,
    elements: Vec<PresentationElement>,
}

struct ImportedPicture {
    relationship_id: String,
    frame: Rect,
    crop: Option<ImageCrop>,
    alt_text: Option<String>,
    rotation_degrees: Option<i32>,
    flip_horizontal: bool,
    flip_vertical: bool,
    lock_aspect_ratio: bool,
}

fn import_pptx_images(
    path: &Path,
    document: &mut PresentationDocument,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let file =
        std::fs::File::open(path).map_err(|error| PresentationArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| PresentationArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    for slide_index in 0..document.slides.len() {
        let slide_number = slide_index + 1;
        let slide_xml_path = format!("ppt/slides/slide{slide_number}.xml");
        let Some(slide_xml) =
            zip_entry_string_if_exists(&mut archive, &slide_xml_path).map_err(|message| {
                PresentationArtifactError::ImportFailed {
                    path: path.to_path_buf(),
                    message,
                }
            })?
        else {
            continue;
        };
        let pictures = parse_imported_pictures(&slide_xml);
        if pictures.is_empty() {
            continue;
        }
        let relationships = zip_entry_string_if_exists(
            &mut archive,
            &format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
        )
        .map_err(|message| PresentationArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message,
        })?
        .map(|xml| parse_slide_image_relationship_targets(&xml))
        .unwrap_or_default();
        let mut imported_images = Vec::new();
        for picture in pictures {
            let Some(target) = relationships.get(&picture.relationship_id) else {
                continue;
            };
            let media_path = resolve_zip_relative_path(&slide_xml_path, target);
            let Some(bytes) =
                zip_entry_bytes_if_exists(&mut archive, &media_path).map_err(|message| {
                    PresentationArtifactError::ImportFailed {
                        path: path.to_path_buf(),
                        message,
                    }
                })?
            else {
                continue;
            };
            let Some(filename) = Path::new(&media_path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
            else {
                continue;
            };
            let Ok(payload) = build_image_payload(bytes, filename, action) else {
                continue;
            };
            imported_images.push(ImageElement {
                element_id: document.next_element_id(),
                frame: picture.frame,
                payload: Some(payload),
                fit_mode: ImageFitMode::Stretch,
                crop: picture.crop,
                rotation_degrees: picture.rotation_degrees,
                flip_horizontal: picture.flip_horizontal,
                flip_vertical: picture.flip_vertical,
                lock_aspect_ratio: picture.lock_aspect_ratio,
                alt_text: picture.alt_text,
                prompt: None,
                is_placeholder: false,
                z_order: 0,
            });
        }
        let slide = &mut document.slides[slide_index];
        for mut image in imported_images {
            image.z_order = slide.elements.len();
            slide.elements.push(PresentationElement::Image(image));
        }
    }
    Ok(())
}

fn zip_entry_string_if_exists<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Option<String>, String> {
    let Some(bytes) = zip_entry_bytes_if_exists(archive, path)? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| format!("zip entry `{path}` is not valid UTF-8: {error}"))
}

fn zip_entry_bytes_if_exists<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Option<Vec<u8>>, String> {
    match archive.by_name(path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|error| format!("failed to read zip entry `{path}`: {error}"))?;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(format!("failed to open zip entry `{path}`: {error}")),
    }
}

fn parse_imported_pictures(slide_xml: &str) -> Vec<ImportedPicture> {
    let mut pictures = Vec::new();
    let mut remaining = slide_xml;
    while let Some(start) = remaining.find("<p:pic>") {
        let block_start = start;
        let Some(block_end_offset) = remaining[block_start..].find("</p:pic>") else {
            break;
        };
        let block_end = block_start + block_end_offset + "</p:pic>".len();
        let block = &remaining[block_start..block_end];
        remaining = &remaining[block_end..];

        let Some(relationship_id) = xml_tag_attribute(block, "<a:blip", "r:embed") else {
            continue;
        };
        let Some(x) = xml_tag_attribute(block, "<a:off", "x").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let Some(y) = xml_tag_attribute(block, "<a:off", "y").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let Some(width) =
            xml_tag_attribute(block, "<a:ext", "cx").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let Some(height) =
            xml_tag_attribute(block, "<a:ext", "cy").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        pictures.push(ImportedPicture {
            relationship_id,
            frame: Rect::from_emu(x, y, width, height),
            crop: xml_tag_attribute(block, "<a:srcRect", "l").map(|left| {
                (
                    left.parse::<f64>().unwrap_or(0.0) / 100_000.0,
                    xml_tag_attribute(block, "<a:srcRect", "t")
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / 100_000.0,
                    xml_tag_attribute(block, "<a:srcRect", "r")
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / 100_000.0,
                    xml_tag_attribute(block, "<a:srcRect", "b")
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / 100_000.0,
                )
            }),
            alt_text: xml_tag_attribute(block, "<p:cNvPr", "descr"),
            rotation_degrees: xml_tag_attribute(block, "<a:xfrm", "rot")
                .and_then(|value| value.parse::<i64>().ok())
                .map(|rotation| (rotation as f64 / 60_000.0).round() as i32),
            flip_horizontal: xml_tag_attribute(block, "<a:xfrm", "flipH").as_deref() == Some("1"),
            flip_vertical: xml_tag_attribute(block, "<a:xfrm", "flipV").as_deref() == Some("1"),
            lock_aspect_ratio: xml_tag_attribute(block, "<a:picLocks", "noChangeAspect").as_deref()
                != Some("0"),
        });
    }
    pictures
}

fn parse_slide_image_relationship_targets(rels_xml: &str) -> HashMap<String, String> {
    let mut relationships = HashMap::new();
    let mut remaining = rels_xml;
    while let Some(start) = remaining.find("<Relationship ") {
        let tag_start = start;
        let Some(tag_end_offset) = remaining[tag_start..].find("/>") else {
            break;
        };
        let tag_end = tag_start + tag_end_offset + 2;
        let tag = &remaining[tag_start..tag_end];
        remaining = &remaining[tag_end..];
        if xml_attribute(tag, "Type").as_deref()
            != Some("http://schemas.openxmlformats.org/officeDocument/2006/relationships/image")
        {
            continue;
        }
        let (Some(id), Some(target)) = (xml_attribute(tag, "Id"), xml_attribute(tag, "Target"))
        else {
            continue;
        };
        relationships.insert(id, target);
    }
    relationships
}

fn resolve_zip_relative_path(base_path: &str, target: &str) -> String {
    let mut components = Path::new(base_path)
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            std::path::Component::CurDir => None,
            std::path::Component::ParentDir => None,
            std::path::Component::RootDir | std::path::Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>();
    for component in Path::new(target).components() {
        match component {
            std::path::Component::Normal(value) => {
                components.push(value.to_string_lossy().to_string())
            }
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                components.clear();
            }
        }
    }
    components.join("/")
}

fn xml_tag_attribute(xml: &str, tag_start: &str, attribute: &str) -> Option<String> {
    let start = xml.find(tag_start)?;
    let tag = &xml[start..start + xml[start..].find('>')?];
    xml_attribute(tag, attribute)
}

fn xml_attribute(tag: &str, attribute: &str) -> Option<String> {
    let pattern = format!(r#"{attribute}=""#);
    let start = tag.find(&pattern)? + pattern.len();
    let end = start + tag[start..].find('"')?;
    Some(
        tag[start..end]
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

impl PresentationSlide {
    fn to_ppt_rs(&self, slide_size: Rect) -> SlideContent {
        let mut content = SlideContent::new("").layout(SlideLayout::Blank);
        if self.notes.visible && !self.notes.text.is_empty() {
            content = content.notes(&self.notes.text);
        }

        if let Some(background_fill) = &self.background_fill {
            content = content.add_shape(
                Shape::new(
                    ShapeType::Rectangle,
                    0,
                    0,
                    points_to_emu(slide_size.width),
                    points_to_emu(slide_size.height),
                )
                .with_fill(ShapeFill::new(background_fill)),
            );
        }

        let mut ordered = self.elements.clone();
        ordered.sort_by_key(PresentationElement::z_order);
        let mut hyperlink_seq = 1_u32;
        for element in ordered {
            match element {
                PresentationElement::Text(text) => {
                    let mut shape = Shape::new(
                        ShapeType::Rectangle,
                        points_to_emu(text.frame.left),
                        points_to_emu(text.frame.top),
                        points_to_emu(text.frame.width),
                        points_to_emu(text.frame.height),
                    )
                    .with_text(&text.text);
                    if let Some(fill) = text.fill {
                        shape = shape.with_fill(ShapeFill::new(&fill));
                    }
                    if let Some(hyperlink) = &text.hyperlink {
                        let relationship_id = format!("rIdHyperlink{hyperlink_seq}");
                        hyperlink_seq += 1;
                        shape = shape.with_hyperlink(hyperlink.to_ppt_rs(&relationship_id));
                    }
                    content = content.add_shape(shape);
                }
                PresentationElement::Shape(shape) => {
                    let mut ppt_shape = Shape::new(
                        shape.geometry.to_ppt_rs(),
                        points_to_emu(shape.frame.left),
                        points_to_emu(shape.frame.top),
                        points_to_emu(shape.frame.width),
                        points_to_emu(shape.frame.height),
                    );
                    if let Some(text) = shape.text {
                        ppt_shape = ppt_shape.with_text(&text);
                    }
                    if let Some(fill) = shape.fill {
                        ppt_shape = ppt_shape.with_fill(ShapeFill::new(&fill));
                    }
                    if let Some(stroke) = shape.stroke {
                        ppt_shape = ppt_shape
                            .with_line(ShapeLine::new(&stroke.color, points_to_emu(stroke.width)));
                    }
                    if let Some(rotation) = shape.rotation_degrees {
                        ppt_shape = ppt_shape.with_rotation(rotation);
                    }
                    if let Some(hyperlink) = &shape.hyperlink {
                        let relationship_id = format!("rIdHyperlink{hyperlink_seq}");
                        hyperlink_seq += 1;
                        ppt_shape = ppt_shape.with_hyperlink(hyperlink.to_ppt_rs(&relationship_id));
                    }
                    content = content.add_shape(ppt_shape);
                }
                PresentationElement::Connector(connector) => {
                    let mut ppt_connector = Connector::new(
                        connector.connector_type.to_ppt_rs(),
                        points_to_emu(connector.start.left),
                        points_to_emu(connector.start.top),
                        points_to_emu(connector.end.left),
                        points_to_emu(connector.end.top),
                    )
                    .with_line(
                        ConnectorLine::new(
                            &connector.line.color,
                            points_to_emu(connector.line.width),
                        )
                        .with_dash(connector.line_style.to_ppt_rs()),
                    )
                    .with_arrow_size(connector.arrow_size.to_ppt_rs())
                    .with_start_arrow(connector.start_arrow.to_ppt_rs())
                    .with_end_arrow(connector.end_arrow.to_ppt_rs());
                    if let Some(label) = connector.label {
                        ppt_connector = ppt_connector.with_label(&label);
                    }
                    content = content.add_connector(ppt_connector);
                }
                PresentationElement::Image(image) => {
                    if let Some(ref payload) = image.payload {
                        let mut ppt_image = Image::from_bytes(
                            payload.bytes.clone(),
                            points_to_emu(image.frame.width),
                            points_to_emu(image.frame.height),
                            &payload.format,
                        )
                        .position(
                            points_to_emu(image.frame.left),
                            points_to_emu(image.frame.top),
                        );
                        if image.fit_mode != ImageFitMode::Stretch {
                            let (x, y, width, height, crop) = fit_image(&image);
                            ppt_image = Image::from_bytes(
                                payload.bytes.clone(),
                                points_to_emu(width),
                                points_to_emu(height),
                                &payload.format,
                            )
                            .position(points_to_emu(x), points_to_emu(y));
                            if let Some((left, top, right, bottom)) = crop {
                                ppt_image = ppt_image.with_crop(left, top, right, bottom);
                            }
                        }
                        if let Some((left, top, right, bottom)) = image.crop {
                            ppt_image = ppt_image.with_crop(left, top, right, bottom);
                        }
                        content = content.add_image(ppt_image);
                    } else {
                        let mut placeholder = Shape::new(
                            ShapeType::Rectangle,
                            points_to_emu(image.frame.left),
                            points_to_emu(image.frame.top),
                            points_to_emu(image.frame.width),
                            points_to_emu(image.frame.height),
                        )
                        .with_text(image.prompt.as_deref().unwrap_or("Image placeholder"));
                        if let Some(rotation) = image.rotation_degrees {
                            placeholder = placeholder.with_rotation(rotation);
                        }
                        content = content.add_shape(placeholder);
                    }
                }
                PresentationElement::Table(table) => {
                    let mut builder = TableBuilder::new(
                        table
                            .column_widths
                            .iter()
                            .copied()
                            .map(points_to_emu)
                            .collect(),
                    )
                    .position(
                        points_to_emu(table.frame.left),
                        points_to_emu(table.frame.top),
                    );
                    for (row_index, row) in table.rows.into_iter().enumerate() {
                        let cells = row
                            .into_iter()
                            .enumerate()
                            .map(|(column_index, cell)| {
                                build_table_cell(cell, &table.merges, row_index, column_index)
                            })
                            .collect::<Vec<_>>();
                        let mut table_row = TableRow::new(cells);
                        if let Some(height) = table.row_heights.get(row_index) {
                            table_row = table_row.with_height(points_to_emu(*height));
                        }
                        builder = builder.add_row(table_row);
                    }
                    content = content.table(builder.build());
                }
                PresentationElement::Chart(chart) => {
                    let mut ppt_chart = Chart::new(
                        chart.title.as_deref().unwrap_or("Chart"),
                        chart.chart_type.to_ppt_rs(),
                        chart.categories,
                        points_to_emu(chart.frame.left),
                        points_to_emu(chart.frame.top),
                        points_to_emu(chart.frame.width),
                        points_to_emu(chart.frame.height),
                    );
                    for series in chart.series {
                        ppt_chart =
                            ppt_chart.add_series(ChartSeries::new(&series.name, series.values));
                    }
                    content = content.add_chart(ppt_chart);
                }
            }
        }
        content
    }
}

#[derive(Debug, Clone)]
enum PresentationElement {
    Text(TextElement),
    Shape(ShapeElement),
    Connector(ConnectorElement),
    Image(ImageElement),
    Table(TableElement),
    Chart(ChartElement),
}

impl PresentationElement {
    fn element_id(&self) -> &str {
        match self {
            Self::Text(element) => &element.element_id,
            Self::Shape(element) => &element.element_id,
            Self::Connector(element) => &element.element_id,
            Self::Image(element) => &element.element_id,
            Self::Table(element) => &element.element_id,
            Self::Chart(element) => &element.element_id,
        }
    }

    fn set_element_id(&mut self, new_id: String) {
        match self {
            Self::Text(element) => element.element_id = new_id,
            Self::Shape(element) => element.element_id = new_id,
            Self::Connector(element) => element.element_id = new_id,
            Self::Image(element) => element.element_id = new_id,
            Self::Table(element) => element.element_id = new_id,
            Self::Chart(element) => element.element_id = new_id,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Shape(_) => "shape",
            Self::Connector(_) => "connector",
            Self::Image(_) => "image",
            Self::Table(_) => "table",
            Self::Chart(_) => "chart",
        }
    }

    fn z_order(&self) -> usize {
        match self {
            Self::Text(element) => element.z_order,
            Self::Shape(element) => element.z_order,
            Self::Connector(element) => element.z_order,
            Self::Image(element) => element.z_order,
            Self::Table(element) => element.z_order,
            Self::Chart(element) => element.z_order,
        }
    }

    fn set_z_order(&mut self, z_order: usize) {
        match self {
            Self::Text(element) => element.z_order = z_order,
            Self::Shape(element) => element.z_order = z_order,
            Self::Connector(element) => element.z_order = z_order,
            Self::Image(element) => element.z_order = z_order,
            Self::Table(element) => element.z_order = z_order,
            Self::Chart(element) => element.z_order = z_order,
        }
    }
}

#[derive(Debug, Clone)]
struct TextElement {
    element_id: String,
    text: String,
    frame: Rect,
    fill: Option<String>,
    style: TextStyle,
    hyperlink: Option<HyperlinkState>,
    placeholder: Option<PlaceholderRef>,
    z_order: usize,
}

#[derive(Debug, Clone)]
struct ShapeElement {
    element_id: String,
    geometry: ShapeGeometry,
    frame: Rect,
    fill: Option<String>,
    stroke: Option<StrokeStyle>,
    text: Option<String>,
    text_style: TextStyle,
    hyperlink: Option<HyperlinkState>,
    placeholder: Option<PlaceholderRef>,
    rotation_degrees: Option<i32>,
    z_order: usize,
}

#[derive(Debug, Clone)]
struct ConnectorElement {
    element_id: String,
    connector_type: ConnectorKind,
    start: PointArgs,
    end: PointArgs,
    line: StrokeStyle,
    line_style: LineStyle,
    start_arrow: ConnectorArrowKind,
    end_arrow: ConnectorArrowKind,
    arrow_size: ConnectorArrowScale,
    label: Option<String>,
    z_order: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ImageElement {
    pub(crate) element_id: String,
    pub(crate) frame: Rect,
    pub(crate) payload: Option<ImagePayload>,
    pub(crate) fit_mode: ImageFitMode,
    pub(crate) crop: Option<ImageCrop>,
    pub(crate) rotation_degrees: Option<i32>,
    pub(crate) flip_horizontal: bool,
    pub(crate) flip_vertical: bool,
    pub(crate) lock_aspect_ratio: bool,
    pub(crate) alt_text: Option<String>,
    pub(crate) prompt: Option<String>,
    pub(crate) is_placeholder: bool,
    pub(crate) z_order: usize,
}

#[derive(Debug, Clone)]
struct TableElement {
    element_id: String,
    frame: Rect,
    rows: Vec<Vec<TableCellSpec>>,
    column_widths: Vec<u32>,
    row_heights: Vec<u32>,
    style: Option<String>,
    merges: Vec<TableMergeRegion>,
    z_order: usize,
}

#[derive(Debug, Clone)]
struct ChartElement {
    element_id: String,
    frame: Rect,
    chart_type: ChartTypeSpec,
    categories: Vec<String>,
    series: Vec<ChartSeriesSpec>,
    title: Option<String>,
    z_order: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ImagePayload {
    pub(crate) bytes: Vec<u8>,
    pub(crate) format: String,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

#[derive(Debug, Clone)]
struct ChartSeriesSpec {
    name: String,
    values: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeGeometry {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    Octagon,
    Star4,
    Star5,
    Star6,
    Star8,
    RightArrow,
    LeftArrow,
    UpArrow,
    DownArrow,
    LeftRightArrow,
    UpDownArrow,
    Chevron,
    Heart,
    Cloud,
    Wave,
    FlowChartProcess,
    FlowChartDecision,
    FlowChartConnector,
    Parallelogram,
    Trapezoid,
}

impl ShapeGeometry {
    fn from_shape_type(shape_type: ShapeType) -> Self {
        match shape_type {
            ShapeType::RoundedRectangle => Self::RoundedRectangle,
            ShapeType::Ellipse | ShapeType::Circle => Self::Ellipse,
            ShapeType::Triangle => Self::Triangle,
            ShapeType::RightTriangle => Self::RightTriangle,
            ShapeType::Diamond => Self::Diamond,
            ShapeType::Pentagon => Self::Pentagon,
            ShapeType::Hexagon => Self::Hexagon,
            ShapeType::Octagon => Self::Octagon,
            ShapeType::Star4 => Self::Star4,
            ShapeType::Star5 => Self::Star5,
            ShapeType::Star6 => Self::Star6,
            ShapeType::Star8 => Self::Star8,
            ShapeType::RightArrow => Self::RightArrow,
            ShapeType::LeftArrow => Self::LeftArrow,
            ShapeType::UpArrow => Self::UpArrow,
            ShapeType::DownArrow => Self::DownArrow,
            ShapeType::LeftRightArrow => Self::LeftRightArrow,
            ShapeType::UpDownArrow => Self::UpDownArrow,
            ShapeType::ChevronArrow => Self::Chevron,
            ShapeType::Heart => Self::Heart,
            ShapeType::Cloud => Self::Cloud,
            ShapeType::Wave => Self::Wave,
            ShapeType::FlowChartProcess => Self::FlowChartProcess,
            ShapeType::FlowChartDecision => Self::FlowChartDecision,
            ShapeType::FlowChartConnector => Self::FlowChartConnector,
            ShapeType::Parallelogram => Self::Parallelogram,
            ShapeType::Trapezoid => Self::Trapezoid,
            _ => Self::Rectangle,
        }
    }

    fn to_ppt_rs(self) -> ShapeType {
        match self {
            Self::Rectangle => ShapeType::Rectangle,
            Self::RoundedRectangle => ShapeType::RoundedRectangle,
            Self::Ellipse => ShapeType::Ellipse,
            Self::Triangle => ShapeType::Triangle,
            Self::RightTriangle => ShapeType::RightTriangle,
            Self::Diamond => ShapeType::Diamond,
            Self::Pentagon => ShapeType::Pentagon,
            Self::Hexagon => ShapeType::Hexagon,
            Self::Octagon => ShapeType::Octagon,
            Self::Star4 => ShapeType::Star4,
            Self::Star5 => ShapeType::Star5,
            Self::Star6 => ShapeType::Star6,
            Self::Star8 => ShapeType::Star8,
            Self::RightArrow => ShapeType::RightArrow,
            Self::LeftArrow => ShapeType::LeftArrow,
            Self::UpArrow => ShapeType::UpArrow,
            Self::DownArrow => ShapeType::DownArrow,
            Self::LeftRightArrow => ShapeType::LeftRightArrow,
            Self::UpDownArrow => ShapeType::UpDownArrow,
            Self::Chevron => ShapeType::ChevronArrow,
            Self::Heart => ShapeType::Heart,
            Self::Cloud => ShapeType::Cloud,
            Self::Wave => ShapeType::Wave,
            Self::FlowChartProcess => ShapeType::FlowChartProcess,
            Self::FlowChartDecision => ShapeType::FlowChartDecision,
            Self::FlowChartConnector => ShapeType::FlowChartConnector,
            Self::Parallelogram => ShapeType::Parallelogram,
            Self::Trapezoid => ShapeType::Trapezoid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChartTypeSpec {
    Bar,
    BarHorizontal,
    BarStacked,
    BarStacked100,
    Line,
    LineMarkers,
    LineStacked,
    Pie,
    Doughnut,
    Area,
    AreaStacked,
    AreaStacked100,
    Scatter,
    ScatterLines,
    ScatterSmooth,
    Bubble,
    Radar,
    RadarFilled,
    StockHlc,
    StockOhlc,
    Combo,
}

impl ChartTypeSpec {
    fn to_ppt_rs(self) -> ChartType {
        match self {
            Self::Bar => ChartType::Bar,
            Self::BarHorizontal => ChartType::BarHorizontal,
            Self::BarStacked => ChartType::BarStacked,
            Self::BarStacked100 => ChartType::BarStacked100,
            Self::Line => ChartType::Line,
            Self::LineMarkers => ChartType::LineMarkers,
            Self::LineStacked => ChartType::LineStacked,
            Self::Pie => ChartType::Pie,
            Self::Doughnut => ChartType::Doughnut,
            Self::Area => ChartType::Area,
            Self::AreaStacked => ChartType::AreaStacked,
            Self::AreaStacked100 => ChartType::AreaStacked100,
            Self::Scatter => ChartType::Scatter,
            Self::ScatterLines => ChartType::ScatterLines,
            Self::ScatterSmooth => ChartType::ScatterSmooth,
            Self::Bubble => ChartType::Bubble,
            Self::Radar => ChartType::Radar,
            Self::RadarFilled => ChartType::RadarFilled,
            Self::StockHlc => ChartType::StockHLC,
            Self::StockOhlc => ChartType::StockOHLC,
            Self::Combo => ChartType::Combo,
        }
    }
}

impl ConnectorKind {
    fn to_ppt_rs(self) -> ConnectorType {
        match self {
            Self::Straight => ConnectorType::Straight,
            Self::Elbow => ConnectorType::Elbow,
            Self::Curved => ConnectorType::Curved,
        }
    }
}

impl ConnectorArrowKind {
    fn to_ppt_rs(self) -> ArrowType {
        match self {
            Self::None => ArrowType::None,
            Self::Triangle => ArrowType::Triangle,
            Self::Stealth => ArrowType::Stealth,
            Self::Diamond => ArrowType::Diamond,
            Self::Oval => ArrowType::Oval,
            Self::Open => ArrowType::Open,
        }
    }
}

impl ConnectorArrowScale {
    fn to_ppt_rs(self) -> ArrowSize {
        match self {
            Self::Small => ArrowSize::Small,
            Self::Medium => ArrowSize::Medium,
            Self::Large => ArrowSize::Large,
        }
    }
}

impl LineStyle {
    fn to_ppt_rs(self) -> LineDash {
        match self {
            Self::Solid => LineDash::Solid,
            Self::Dashed => LineDash::Dash,
            Self::Dotted => LineDash::Dot,
            Self::DashDot => LineDash::DashDot,
            Self::DashDotDot => LineDash::DashDotDot,
            Self::LongDash => LineDash::LongDash,
            Self::LongDashDot => LineDash::LongDashDot,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ImageFitMode {
    Stretch,
    Contain,
    Cover,
}

#[derive(Debug, Clone)]
struct StrokeStyle {
    color: String,
    width: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectorKind {
    Straight,
    Elbow,
    Curved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectorArrowKind {
    None,
    Triangle,
    Stealth,
    Diamond,
    Oval,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectorArrowScale {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    DashDot,
    DashDotDot,
    LongDash,
    LongDashDot,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Rect {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Rect {
    fn from_emu(left: u32, top: u32, width: u32, height: u32) -> Self {
        Self {
            left: emu_to_points(left),
            top: emu_to_points(top),
            width: emu_to_points(width),
            height: emu_to_points(height),
        }
    }
}

impl From<PositionArgs> for Rect {
    fn from(value: PositionArgs) -> Self {
        Self {
            left: value.left,
            top: value.top,
            width: value.width,
            height: value.height,
        }
    }
}

fn apply_partial_position(rect: Rect, position: PartialPositionArgs) -> Rect {
    Rect {
        left: position.left.unwrap_or(rect.left),
        top: position.top.unwrap_or(rect.top),
        width: position.width.unwrap_or(rect.width),
        height: position.height.unwrap_or(rect.height),
    }
}

fn apply_partial_position_to_image(image: &ImageElement, position: PartialPositionArgs) -> Rect {
    let mut frame = apply_partial_position(image.frame, position.clone());
    if image.lock_aspect_ratio {
        let base_ratio = image
            .payload
            .as_ref()
            .map(|payload| payload.width_px as f64 / payload.height_px as f64)
            .unwrap_or_else(|| image.frame.width as f64 / image.frame.height as f64);
        if let Some(width) = position.width
            && position.height.is_none()
        {
            frame.height = (width as f64 / base_ratio).round() as u32;
        } else if let Some(height) = position.height
            && position.width.is_none()
        {
            frame.width = (height as f64 * base_ratio).round() as u32;
        }
    }
    frame
}

#[derive(Debug, Deserialize)]
struct CreateArgs {
    name: Option<String>,
    slide_size: Option<Value>,
    theme: Option<ThemeArgs>,
}

#[derive(Debug, Deserialize)]
struct ImportPptxArgs {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ExportPptxArgs {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ExportPreviewArgs {
    path: PathBuf,
    slide_index: Option<u32>,
    format: Option<String>,
    scale: Option<f32>,
    quality: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
struct AddSlideArgs {
    layout: Option<String>,
    notes: Option<String>,
    background_fill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateLayoutArgs {
    name: String,
    kind: Option<String>,
    parent_layout_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewOutputFormat {
    Png,
    Jpeg,
}

impl PreviewOutputFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddLayoutPlaceholderArgs {
    layout_id: String,
    name: String,
    placeholder_type: String,
    index: Option<u32>,
    text: Option<String>,
    geometry: Option<String>,
    position: Option<PositionArgs>,
}

#[derive(Debug, Deserialize)]
struct LayoutIdArgs {
    layout_id: String,
}

#[derive(Debug, Deserialize)]
struct SetSlideLayoutArgs {
    slide_index: u32,
    layout_id: String,
}

#[derive(Debug, Deserialize)]
struct UpdatePlaceholderTextArgs {
    slide_index: u32,
    name: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct NotesArgs {
    slide_index: u32,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NotesVisibilityArgs {
    slide_index: u32,
    visible: bool,
}

#[derive(Debug, Deserialize)]
struct ThemeArgs {
    color_scheme: HashMap<String, String>,
    major_font: Option<String>,
    minor_font: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InspectArgs {
    kind: Option<String>,
    target_id: Option<String>,
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ResolveArgs {
    id: String,
}

#[derive(Debug, Default, Deserialize)]
struct InsertSlideArgs {
    index: Option<u32>,
    after_slide_index: Option<u32>,
    layout: Option<String>,
    notes: Option<String>,
    background_fill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlideIndexArgs {
    slide_index: u32,
}

#[derive(Debug, Deserialize)]
struct MoveSlideArgs {
    from_index: u32,
    to_index: u32,
}

#[derive(Debug, Deserialize)]
struct SetActiveSlideArgs {
    slide_index: u32,
}

#[derive(Debug, Deserialize)]
struct SetSlideBackgroundArgs {
    slide_index: u32,
    fill: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PositionArgs {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PartialPositionArgs {
    left: Option<u32>,
    top: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct TextStylingArgs {
    font_size: Option<u32>,
    font_family: Option<String>,
    color: Option<String>,
    fill: Option<String>,
    alignment: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AddTextShapeArgs {
    slide_index: u32,
    text: String,
    position: PositionArgs,
    #[serde(flatten)]
    styling: TextStylingArgs,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct StrokeArgs {
    color: String,
    width: u32,
}

#[derive(Debug, Deserialize)]
struct AddShapeArgs {
    slide_index: u32,
    geometry: String,
    position: PositionArgs,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
    text: Option<String>,
    #[serde(default)]
    text_style: TextStylingArgs,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConnectorLineArgs {
    color: Option<String>,
    width: Option<u32>,
    style: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PointArgs {
    left: u32,
    top: u32,
}

#[derive(Debug, Deserialize)]
struct AddConnectorArgs {
    slide_index: u32,
    connector_type: String,
    start: PointArgs,
    end: PointArgs,
    line: Option<ConnectorLineArgs>,
    start_arrow: Option<String>,
    end_arrow: Option<String>,
    arrow_size: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddImageArgs {
    slide_index: u32,
    path: Option<PathBuf>,
    data_url: Option<String>,
    uri: Option<String>,
    position: PositionArgs,
    fit: Option<ImageFitMode>,
    crop: Option<ImageCropArgs>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    lock_aspect_ratio: Option<bool>,
    alt: Option<String>,
    prompt: Option<String>,
}

impl AddImageArgs {
    fn image_source(&self) -> Result<ImageInputSource, PresentationArtifactError> {
        match (&self.path, &self.data_url, &self.uri) {
            (Some(path), None, None) => Ok(ImageInputSource::Path(path.clone())),
            (None, Some(data_url), None) => Ok(ImageInputSource::DataUrl(data_url.clone())),
            (None, None, Some(uri)) => Ok(ImageInputSource::Uri(uri.clone())),
            (None, None, None) if self.prompt.is_some() => Ok(ImageInputSource::Placeholder),
            _ => Err(PresentationArtifactError::InvalidArgs {
                action: "add_image".to_string(),
                message:
                    "provide exactly one of `path`, `data_url`, or `uri`, or provide `prompt` for a placeholder image"
                        .to_string(),
            }),
        }
    }
}

enum ImageInputSource {
    Path(PathBuf),
    DataUrl(String),
    Uri(String),
    Placeholder,
}

#[derive(Debug, Clone, Deserialize)]
struct ImageCropArgs {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

#[derive(Debug, Deserialize)]
struct AddTableArgs {
    slide_index: u32,
    position: PositionArgs,
    rows: Vec<Vec<Value>>,
    column_widths: Option<Vec<u32>>,
    row_heights: Option<Vec<u32>>,
    style: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddChartArgs {
    slide_index: u32,
    position: PositionArgs,
    chart_type: String,
    categories: Vec<String>,
    series: Vec<ChartSeriesArgs>,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChartSeriesArgs {
    name: String,
    values: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct UpdateTextArgs {
    element_id: String,
    text: String,
    #[serde(default)]
    styling: TextStylingArgs,
}

#[derive(Debug, Deserialize)]
struct ReplaceTextArgs {
    element_id: String,
    search: String,
    replace: String,
}

#[derive(Debug, Deserialize)]
struct InsertTextAfterArgs {
    element_id: String,
    after: String,
    insert: String,
}

#[derive(Debug, Deserialize)]
struct SetHyperlinkArgs {
    element_id: String,
    link_type: Option<String>,
    url: Option<String>,
    slide_index: Option<u32>,
    address: Option<String>,
    subject: Option<String>,
    path: Option<String>,
    tooltip: Option<String>,
    highlight_click: Option<bool>,
    clear: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateShapeStyleArgs {
    element_id: String,
    position: Option<PartialPositionArgs>,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    fit: Option<ImageFitMode>,
    crop: Option<ImageCropArgs>,
    lock_aspect_ratio: Option<bool>,
    z_order: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ElementIdArgs {
    element_id: String,
}

#[derive(Debug, Deserialize)]
struct ReplaceImageArgs {
    element_id: String,
    path: Option<PathBuf>,
    data_url: Option<String>,
    uri: Option<String>,
    fit: Option<ImageFitMode>,
    crop: Option<ImageCropArgs>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    lock_aspect_ratio: Option<bool>,
    alt: Option<String>,
    prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTableCellArgs {
    element_id: String,
    row: u32,
    column: u32,
    value: Value,
    #[serde(default)]
    styling: TextStylingArgs,
    background_fill: Option<String>,
    alignment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MergeTableCellsArgs {
    element_id: String,
    start_row: u32,
    end_row: u32,
    start_column: u32,
    end_column: u32,
}

fn parse_args<T>(action: &str, value: &Value) -> Result<T, PresentationArtifactError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(value.clone()).map_err(|error| PresentationArtifactError::InvalidArgs {
        action: action.to_string(),
        message: error.to_string(),
    })
}

fn required_artifact_id(
    request: &PresentationArtifactRequest,
) -> Result<String, PresentationArtifactError> {
    request
        .artifact_id
        .clone()
        .ok_or_else(|| PresentationArtifactError::MissingArtifactId {
            action: request.action.clone(),
        })
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn normalize_color(
    color: &str,
    action: &str,
    field: &str,
) -> Result<String, PresentationArtifactError> {
    normalize_color_with_palette(None, color, action, field)
}

fn normalize_color_with_document(
    document: &PresentationDocument,
    color: &str,
    action: &str,
    field: &str,
) -> Result<String, PresentationArtifactError> {
    normalize_color_with_palette(Some(&document.theme), color, action, field)
}

fn normalize_color_with_palette(
    theme: Option<&ThemeState>,
    color: &str,
    action: &str,
    field: &str,
) -> Result<String, PresentationArtifactError> {
    let trimmed = color.trim();
    let normalized = theme
        .and_then(|palette| palette.resolve_color(trimmed))
        .unwrap_or_else(|| trimmed.trim_start_matches('#').to_uppercase());
    if normalized.len() != 6
        || !normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("field `{field}` must be a 6-digit RGB hex color"),
        });
    }
    Ok(normalized)
}

fn parse_shape_geometry(
    geometry: &str,
    action: &str,
) -> Result<ShapeGeometry, PresentationArtifactError> {
    match geometry {
        "rectangle" | "rect" => Ok(ShapeGeometry::Rectangle),
        "rounded_rectangle" | "roundedRect" => Ok(ShapeGeometry::RoundedRectangle),
        "ellipse" | "circle" => Ok(ShapeGeometry::Ellipse),
        "triangle" => Ok(ShapeGeometry::Triangle),
        "right_triangle" => Ok(ShapeGeometry::RightTriangle),
        "diamond" => Ok(ShapeGeometry::Diamond),
        "pentagon" => Ok(ShapeGeometry::Pentagon),
        "hexagon" => Ok(ShapeGeometry::Hexagon),
        "octagon" => Ok(ShapeGeometry::Octagon),
        "star4" => Ok(ShapeGeometry::Star4),
        "star" | "star5" => Ok(ShapeGeometry::Star5),
        "star6" => Ok(ShapeGeometry::Star6),
        "star8" => Ok(ShapeGeometry::Star8),
        "right_arrow" => Ok(ShapeGeometry::RightArrow),
        "left_arrow" => Ok(ShapeGeometry::LeftArrow),
        "up_arrow" => Ok(ShapeGeometry::UpArrow),
        "down_arrow" => Ok(ShapeGeometry::DownArrow),
        "left_right_arrow" | "leftRightArrow" => Ok(ShapeGeometry::LeftRightArrow),
        "up_down_arrow" | "upDownArrow" => Ok(ShapeGeometry::UpDownArrow),
        "chevron" => Ok(ShapeGeometry::Chevron),
        "heart" => Ok(ShapeGeometry::Heart),
        "cloud" => Ok(ShapeGeometry::Cloud),
        "wave" => Ok(ShapeGeometry::Wave),
        "flowChartProcess" | "flow_chart_process" => Ok(ShapeGeometry::FlowChartProcess),
        "flowChartDecision" | "flow_chart_decision" => Ok(ShapeGeometry::FlowChartDecision),
        "flowChartConnector" | "flow_chart_connector" => Ok(ShapeGeometry::FlowChartConnector),
        "parallelogram" => Ok(ShapeGeometry::Parallelogram),
        "trapezoid" => Ok(ShapeGeometry::Trapezoid),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("geometry `{geometry}` is not supported"),
        }),
    }
}

fn parse_chart_type(
    chart_type: &str,
    action: &str,
) -> Result<ChartTypeSpec, PresentationArtifactError> {
    match chart_type {
        "bar" => Ok(ChartTypeSpec::Bar),
        "bar_horizontal" => Ok(ChartTypeSpec::BarHorizontal),
        "bar_stacked" => Ok(ChartTypeSpec::BarStacked),
        "bar_stacked_100" => Ok(ChartTypeSpec::BarStacked100),
        "line" => Ok(ChartTypeSpec::Line),
        "line_markers" => Ok(ChartTypeSpec::LineMarkers),
        "line_stacked" => Ok(ChartTypeSpec::LineStacked),
        "pie" => Ok(ChartTypeSpec::Pie),
        "doughnut" => Ok(ChartTypeSpec::Doughnut),
        "area" => Ok(ChartTypeSpec::Area),
        "area_stacked" => Ok(ChartTypeSpec::AreaStacked),
        "area_stacked_100" => Ok(ChartTypeSpec::AreaStacked100),
        "scatter" => Ok(ChartTypeSpec::Scatter),
        "scatter_lines" => Ok(ChartTypeSpec::ScatterLines),
        "scatter_smooth" => Ok(ChartTypeSpec::ScatterSmooth),
        "bubble" => Ok(ChartTypeSpec::Bubble),
        "radar" => Ok(ChartTypeSpec::Radar),
        "radar_filled" => Ok(ChartTypeSpec::RadarFilled),
        "stock_hlc" => Ok(ChartTypeSpec::StockHlc),
        "stock_ohlc" => Ok(ChartTypeSpec::StockOhlc),
        "combo" => Ok(ChartTypeSpec::Combo),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("chart_type `{chart_type}` is not supported"),
        }),
    }
}

fn parse_stroke(
    document: &PresentationDocument,
    stroke: Option<StrokeArgs>,
    action: &str,
) -> Result<Option<StrokeStyle>, PresentationArtifactError> {
    stroke
        .map(|value| parse_required_stroke(document, value, action))
        .transpose()
}

fn parse_required_stroke(
    document: &PresentationDocument,
    stroke: StrokeArgs,
    action: &str,
) -> Result<StrokeStyle, PresentationArtifactError> {
    Ok(StrokeStyle {
        color: normalize_color_with_document(document, &stroke.color, action, "stroke.color")?,
        width: stroke.width,
    })
}

fn parse_connector_kind(
    connector_type: &str,
    action: &str,
) -> Result<ConnectorKind, PresentationArtifactError> {
    match connector_type {
        "straight" => Ok(ConnectorKind::Straight),
        "elbow" => Ok(ConnectorKind::Elbow),
        "curved" => Ok(ConnectorKind::Curved),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("connector_type `{connector_type}` is not supported"),
        }),
    }
}

fn parse_connector_arrow(
    value: &str,
    action: &str,
) -> Result<ConnectorArrowKind, PresentationArtifactError> {
    match value {
        "none" => Ok(ConnectorArrowKind::None),
        "triangle" => Ok(ConnectorArrowKind::Triangle),
        "stealth" => Ok(ConnectorArrowKind::Stealth),
        "diamond" => Ok(ConnectorArrowKind::Diamond),
        "oval" => Ok(ConnectorArrowKind::Oval),
        "open" => Ok(ConnectorArrowKind::Open),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("connector arrow `{value}` is not supported"),
        }),
    }
}

fn parse_connector_arrow_size(
    value: &str,
    action: &str,
) -> Result<ConnectorArrowScale, PresentationArtifactError> {
    match value {
        "small" => Ok(ConnectorArrowScale::Small),
        "medium" => Ok(ConnectorArrowScale::Medium),
        "large" => Ok(ConnectorArrowScale::Large),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("connector arrow_size `{value}` is not supported"),
        }),
    }
}

fn parse_line_style(value: &str, action: &str) -> Result<LineStyle, PresentationArtifactError> {
    match value {
        "solid" => Ok(LineStyle::Solid),
        "dashed" => Ok(LineStyle::Dashed),
        "dotted" => Ok(LineStyle::Dotted),
        "dash-dot" | "dash_dot" => Ok(LineStyle::DashDot),
        "dash-dot-dot" | "dash_dot_dot" => Ok(LineStyle::DashDotDot),
        "long-dash" | "long_dash" => Ok(LineStyle::LongDash),
        "long-dash-dot" | "long_dash_dot" => Ok(LineStyle::LongDashDot),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("line style `{value}` is not supported"),
        }),
    }
}

fn parse_connector_line(
    document: &PresentationDocument,
    line: Option<ConnectorLineArgs>,
    action: &str,
) -> Result<ParsedConnectorLine, PresentationArtifactError> {
    let line = line.unwrap_or_default();
    Ok(ParsedConnectorLine {
        color: line
            .color
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, action, "line.color"))
            .transpose()?
            .unwrap_or_else(|| "000000".to_string()),
        width: line.width.unwrap_or(1),
        style: line
            .style
            .as_deref()
            .map(|value| parse_line_style(value, action))
            .transpose()?
            .unwrap_or(LineStyle::Solid),
    })
}

struct ParsedConnectorLine {
    color: String,
    width: u32,
    style: LineStyle,
}

fn normalize_text_style_with_document(
    document: &PresentationDocument,
    styling: &TextStylingArgs,
    action: &str,
) -> Result<TextStyle, PresentationArtifactError> {
    normalize_text_style_with_palette(Some(&document.theme), styling, action)
}

fn normalize_text_style_with_palette(
    theme: Option<&ThemeState>,
    styling: &TextStylingArgs,
    action: &str,
) -> Result<TextStyle, PresentationArtifactError> {
    Ok(TextStyle {
        font_size: styling.font_size,
        font_family: styling.font_family.clone(),
        color: styling
            .color
            .as_deref()
            .map(|value| normalize_color_with_palette(theme, value, action, "color"))
            .transpose()?,
        alignment: styling
            .alignment
            .as_deref()
            .map(|value| parse_alignment(value, action))
            .transpose()?,
        bold: styling.bold.unwrap_or(false),
        italic: styling.italic.unwrap_or(false),
        underline: false,
    })
}

fn parse_hyperlink_state(
    document: &PresentationDocument,
    args: &SetHyperlinkArgs,
    action: &str,
) -> Result<HyperlinkState, PresentationArtifactError> {
    let link_type =
        args.link_type
            .as_deref()
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "`link_type` is required unless `clear` is true".to_string(),
            })?;
    let target = match link_type {
        "url" => HyperlinkTarget::Url(required_hyperlink_field(&args.url, action, "url")?.clone()),
        "slide" => {
            let slide_index =
                args.slide_index
                    .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                        action: action.to_string(),
                        message: "`slide_index` is required for slide hyperlinks".to_string(),
                    })?;
            if slide_index as usize >= document.slides.len() {
                return Err(index_out_of_range(
                    action,
                    slide_index as usize,
                    document.slides.len(),
                ));
            }
            HyperlinkTarget::Slide(slide_index)
        }
        "first_slide" => HyperlinkTarget::FirstSlide,
        "last_slide" => HyperlinkTarget::LastSlide,
        "next_slide" => HyperlinkTarget::NextSlide,
        "previous_slide" => HyperlinkTarget::PreviousSlide,
        "end_show" => HyperlinkTarget::EndShow,
        "email" => HyperlinkTarget::Email {
            address: required_hyperlink_field(&args.address, action, "address")?.clone(),
            subject: args.subject.clone(),
        },
        "file" => {
            HyperlinkTarget::File(required_hyperlink_field(&args.path, action, "path")?.clone())
        }
        other => {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("hyperlink type `{other}` is not supported"),
            });
        }
    };
    Ok(HyperlinkState {
        target,
        tooltip: args.tooltip.clone(),
        highlight_click: args.highlight_click.unwrap_or(true),
    })
}

fn required_hyperlink_field<'a>(
    value: &'a Option<String>,
    action: &str,
    field: &str,
) -> Result<&'a String, PresentationArtifactError> {
    value
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("`{field}` is required for this hyperlink type"),
        })
}

fn coerce_table_rows(
    rows: Vec<Vec<Value>>,
    action: &str,
) -> Result<Vec<Vec<TableCellSpec>>, PresentationArtifactError> {
    if rows.is_empty() {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "`rows` must contain at least one row".to_string(),
        });
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| TableCellSpec {
                    text: cell_value_to_string(value),
                    text_style: TextStyle::default(),
                    background_fill: None,
                    alignment: None,
                })
                .collect()
        })
        .collect())
}

fn normalize_table_dimensions(
    rows: &[Vec<TableCellSpec>],
    frame: Rect,
    column_widths: Option<Vec<u32>>,
    row_heights: Option<Vec<u32>>,
    action: &str,
) -> Result<(Vec<u32>, Vec<u32>), PresentationArtifactError> {
    let column_count = rows.iter().map(std::vec::Vec::len).max().unwrap_or(1);
    let normalized_column_widths = match column_widths {
        Some(widths) => {
            if widths.len() != column_count {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!(
                        "`column_widths` must contain {column_count} entries for this table"
                    ),
                });
            }
            widths
        }
        None => split_points(frame.width, column_count),
    };
    let normalized_row_heights = match row_heights {
        Some(heights) => {
            if heights.len() != rows.len() {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!(
                        "`row_heights` must contain {} entries for this table",
                        rows.len()
                    ),
                });
            }
            heights
        }
        None => split_points(frame.height, rows.len()),
    };
    Ok((normalized_column_widths, normalized_row_heights))
}

fn split_points(total: u32, count: usize) -> Vec<u32> {
    if count == 0 {
        return Vec::new();
    }
    let base = total / count as u32;
    let remainder = total % count as u32;
    (0..count)
        .map(|index| base + u32::from(index < remainder as usize))
        .collect()
}

fn parse_alignment(value: &str, action: &str) -> Result<TextAlignment, PresentationArtifactError> {
    match value {
        "left" => Ok(TextAlignment::Left),
        "center" | "middle" => Ok(TextAlignment::Center),
        "right" => Ok(TextAlignment::Right),
        "justify" => Ok(TextAlignment::Justify),
        _ => Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("unsupported alignment `{value}`"),
        }),
    }
}

fn normalize_theme(args: ThemeArgs, action: &str) -> Result<ThemeState, PresentationArtifactError> {
    let color_scheme = args
        .color_scheme
        .into_iter()
        .map(|(key, value)| {
            normalize_color(&value, action, &key)
                .map(|normalized| (key.to_ascii_lowercase(), normalized))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(ThemeState {
        color_scheme,
        major_font: args.major_font,
        minor_font: args.minor_font,
    })
}

fn parse_slide_size(value: &Value, action: &str) -> Result<Rect, PresentationArtifactError> {
    #[derive(Deserialize)]
    struct SlideSizeArgs {
        width: u32,
        height: u32,
    }

    let slide_size: SlideSizeArgs = serde_json::from_value(value.clone()).map_err(|error| {
        PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("invalid slide_size: {error}"),
        }
    })?;
    Ok(Rect {
        left: 0,
        top: 0,
        width: slide_size.width,
        height: slide_size.height,
    })
}

fn apply_layout_to_slide(
    document: &mut PresentationDocument,
    slide: &mut PresentationSlide,
    layout_id: &str,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let placeholders = resolved_layout_placeholders(document, layout_id, action)?;
    slide.layout_id = Some(layout_id.to_string());
    for resolved in placeholders {
        let placeholder = resolved.definition;
        let placeholder_ref = Some(PlaceholderRef {
            name: placeholder.name,
            placeholder_type: placeholder.placeholder_type,
            index: placeholder.index,
        });
        let element_id = document.next_element_id();
        if placeholder.geometry == ShapeGeometry::Rectangle {
            slide.elements.push(PresentationElement::Text(TextElement {
                element_id,
                text: placeholder.text.unwrap_or_default(),
                frame: placeholder.frame,
                fill: None,
                style: TextStyle::default(),
                hyperlink: None,
                placeholder: placeholder_ref,
                z_order: slide.elements.len(),
            }));
        } else {
            slide
                .elements
                .push(PresentationElement::Shape(ShapeElement {
                    element_id,
                    geometry: placeholder.geometry,
                    frame: placeholder.frame,
                    fill: None,
                    stroke: None,
                    text: placeholder.text,
                    text_style: TextStyle::default(),
                    hyperlink: None,
                    placeholder: placeholder_ref,
                    rotation_degrees: None,
                    z_order: slide.elements.len(),
                }));
        }
    }
    Ok(())
}

fn resolved_layout_placeholders(
    document: &PresentationDocument,
    layout_id: &str,
    action: &str,
) -> Result<Vec<ResolvedPlaceholder>, PresentationArtifactError> {
    let mut lineage = Vec::new();
    collect_layout_lineage(
        document,
        layout_id,
        action,
        &mut HashSet::new(),
        &mut lineage,
    )?;
    let mut resolved: Vec<ResolvedPlaceholder> = Vec::new();
    for layout in lineage {
        for placeholder in &layout.placeholders {
            if let Some(index) = resolved.iter().position(|entry| {
                placeholder_key(&entry.definition) == placeholder_key(placeholder)
            }) {
                resolved[index] = ResolvedPlaceholder {
                    source_layout_id: layout.layout_id.clone(),
                    definition: placeholder.clone(),
                };
            } else {
                resolved.push(ResolvedPlaceholder {
                    source_layout_id: layout.layout_id.clone(),
                    definition: placeholder.clone(),
                });
            }
        }
    }
    Ok(resolved)
}

fn collect_layout_lineage<'a>(
    document: &'a PresentationDocument,
    layout_id: &str,
    action: &str,
    seen: &mut HashSet<String>,
    lineage: &mut Vec<&'a LayoutDocument>,
) -> Result<(), PresentationArtifactError> {
    if !seen.insert(layout_id.to_string()) {
        return Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("layout inheritance cycle detected at `{layout_id}`"),
        });
    }
    let layout = document.get_layout(layout_id, action)?;
    if let Some(parent_layout_id) = &layout.parent_layout_id {
        collect_layout_lineage(document, parent_layout_id, action, seen, lineage)?;
    }
    lineage.push(layout);
    Ok(())
}

fn placeholder_key(placeholder: &PlaceholderDefinition) -> (String, String, Option<u32>) {
    (
        placeholder.name.to_ascii_lowercase(),
        placeholder.placeholder_type.to_ascii_lowercase(),
        placeholder.index,
    )
}

fn layout_placeholder_list(
    document: &PresentationDocument,
    layout_id: &str,
    action: &str,
) -> Result<Vec<PlaceholderListEntry>, PresentationArtifactError> {
    resolved_layout_placeholders(document, layout_id, action).map(|placeholders| {
        placeholders
            .into_iter()
            .map(|placeholder| PlaceholderListEntry {
                scope: "layout".to_string(),
                source_layout_id: Some(placeholder.source_layout_id),
                slide_index: None,
                element_id: None,
                name: placeholder.definition.name,
                placeholder_type: placeholder.definition.placeholder_type,
                index: placeholder.definition.index,
                geometry: Some(format!("{:?}", placeholder.definition.geometry)),
                text_preview: placeholder.definition.text,
            })
            .collect()
    })
}

fn slide_placeholder_list(
    slide: &PresentationSlide,
    slide_index: usize,
) -> Vec<PlaceholderListEntry> {
    slide
        .elements
        .iter()
        .filter_map(|element| match element {
            PresentationElement::Text(text) => {
                text.placeholder
                    .as_ref()
                    .map(|placeholder| PlaceholderListEntry {
                        scope: "slide".to_string(),
                        source_layout_id: slide.layout_id.clone(),
                        slide_index: Some(slide_index),
                        element_id: Some(text.element_id.clone()),
                        name: placeholder.name.clone(),
                        placeholder_type: placeholder.placeholder_type.clone(),
                        index: placeholder.index,
                        geometry: Some("Rectangle".to_string()),
                        text_preview: Some(text.text.clone()),
                    })
            }
            PresentationElement::Shape(shape) => {
                shape
                    .placeholder
                    .as_ref()
                    .map(|placeholder| PlaceholderListEntry {
                        scope: "slide".to_string(),
                        source_layout_id: slide.layout_id.clone(),
                        slide_index: Some(slide_index),
                        element_id: Some(shape.element_id.clone()),
                        name: placeholder.name.clone(),
                        placeholder_type: placeholder.placeholder_type.clone(),
                        index: placeholder.index,
                        geometry: Some(format!("{:?}", shape.geometry)),
                        text_preview: shape.text.clone(),
                    })
            }
            PresentationElement::Connector(_)
            | PresentationElement::Image(_)
            | PresentationElement::Table(_)
            | PresentationElement::Chart(_) => None,
        })
        .collect()
}

fn build_table_cell(
    cell: TableCellSpec,
    merges: &[TableMergeRegion],
    row_index: usize,
    column_index: usize,
) -> TableCell {
    let mut table_cell = TableCell::new(&cell.text);
    if cell.text_style.bold {
        table_cell = table_cell.bold();
    }
    if cell.text_style.italic {
        table_cell = table_cell.italic();
    }
    if cell.text_style.underline {
        table_cell = table_cell.underline();
    }
    if let Some(color) = cell.text_style.color {
        table_cell = table_cell.text_color(&color);
    }
    if let Some(fill) = cell.background_fill {
        table_cell = table_cell.background_color(&fill);
    }
    if let Some(size) = cell.text_style.font_size {
        table_cell = table_cell.font_size(size);
    }
    if let Some(font_family) = cell.text_style.font_family {
        table_cell = table_cell.font_family(&font_family);
    }
    if let Some(alignment) = cell.alignment.or(cell.text_style.alignment) {
        table_cell = match alignment {
            TextAlignment::Left => table_cell.align_left(),
            TextAlignment::Center => table_cell.align_center(),
            TextAlignment::Right => table_cell.align_right(),
            TextAlignment::Justify => table_cell.align(CellAlign::Justify),
        };
    }
    for merge in merges {
        if row_index == merge.start_row && column_index == merge.start_column {
            table_cell = table_cell
                .grid_span((merge.end_column - merge.start_column + 1) as u32)
                .row_span((merge.end_row - merge.start_row + 1) as u32);
        } else if row_index >= merge.start_row
            && row_index <= merge.end_row
            && column_index >= merge.start_column
            && column_index <= merge.end_column
        {
            if row_index == merge.start_row {
                table_cell = table_cell.h_merge();
            } else {
                table_cell = table_cell.v_merge();
            }
        }
    }
    table_cell
}

fn inspect_document(
    document: &PresentationDocument,
    kind: Option<&str>,
    target_id: Option<&str>,
    max_chars: Option<usize>,
) -> String {
    let kinds =
        kind.unwrap_or("deck,slide,textbox,shape,connector,table,chart,image,notes,layoutList");
    let include = |name: &str| kinds.split(',').map(str::trim).any(|entry| entry == name);
    let mut lines = Vec::new();
    if include("deck") {
        let record = serde_json::json!({
            "kind": "deck",
            "id": format!("pr/{}", document.artifact_id),
            "name": document.name,
            "slides": document.slides.len(),
            "activeSlideIndex": document.active_slide_index,
            "activeSlideId": document.active_slide_index.and_then(|index| document.slides.get(index)).map(|slide| format!("sl/{}", slide.slide_id)),
        });
        if target_matches(target_id, &record) {
            lines.push(record);
        }
    }
    if include("layoutList") {
        for layout in &document.layouts {
            let placeholders = resolved_layout_placeholders(document, &layout.layout_id, "inspect")
                .unwrap_or_default()
                .into_iter()
                .map(|placeholder| {
                    serde_json::json!({
                        "name": placeholder.definition.name,
                        "type": placeholder.definition.placeholder_type,
                        "sourceLayoutId": placeholder.source_layout_id,
                        "textPreview": placeholder.definition.text,
                    })
                })
                .collect::<Vec<_>>();
            let record = serde_json::json!({
                "kind": "layout",
                "id": format!("ly/{}", layout.layout_id),
                "layoutId": layout.layout_id,
                "name": layout.name,
                "type": match layout.kind { LayoutKind::Layout => "layout", LayoutKind::Master => "master" },
                "parentLayoutId": layout.parent_layout_id,
                "placeholders": placeholders,
            });
            if target_matches(target_id, &record) {
                lines.push(record);
            }
        }
    }
    for (index, slide) in document.slides.iter().enumerate() {
        let slide_id = format!("sl/{}", slide.slide_id);
        if include("slide") {
            let record = serde_json::json!({
                "kind": "slide",
                "id": slide_id,
                "slide": index + 1,
                "slideIndex": index,
                "isActive": document.active_slide_index == Some(index),
                "layoutId": slide.layout_id,
                "elements": slide.elements.len(),
            });
            if target_matches(target_id, &record) {
                lines.push(record);
            }
        }
        if include("notes") && !slide.notes.text.is_empty() {
            let record = serde_json::json!({
                "kind": "notes",
                "id": format!("nt/{}", slide.slide_id),
                "slide": index + 1,
                "visible": slide.notes.visible,
                "text": slide.notes.text,
            });
            if target_matches(target_id, &record) || target_id == Some(slide_id.as_str()) {
                lines.push(record);
            }
        }
        for element in &slide.elements {
            let mut record = match element {
                PresentationElement::Text(text) => {
                    if !include("textbox") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "textbox",
                        "id": format!("sh/{}", text.element_id),
                        "slide": index + 1,
                        "text": text.text,
                        "textPreview": text.text.replace('\n', " | "),
                        "textChars": text.text.chars().count(),
                        "textLines": text.text.lines().count(),
                        "bbox": [text.frame.left, text.frame.top, text.frame.width, text.frame.height],
                        "bboxUnit": "points",
                    })
                }
                PresentationElement::Shape(shape) => {
                    if !(include("shape") || include("textbox") && shape.text.is_some()) {
                        continue;
                    }
                    let kind = if shape.text.is_some() && include("textbox") {
                        "textbox"
                    } else {
                        "shape"
                    };
                    serde_json::json!({
                        "kind": kind,
                        "id": format!("sh/{}", shape.element_id),
                        "slide": index + 1,
                        "geometry": format!("{:?}", shape.geometry),
                        "text": shape.text,
                        "bbox": [shape.frame.left, shape.frame.top, shape.frame.width, shape.frame.height],
                        "bboxUnit": "points",
                    })
                }
                PresentationElement::Connector(connector) => {
                    if !include("shape") && !include("connector") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "connector",
                        "id": format!("cn/{}", connector.element_id),
                        "slide": index + 1,
                        "connectorType": format!("{:?}", connector.connector_type),
                        "start": [connector.start.left, connector.start.top],
                        "end": [connector.end.left, connector.end.top],
                        "lineStyle": format!("{:?}", connector.line_style),
                        "label": connector.label,
                    })
                }
                PresentationElement::Table(table) => {
                    if !include("table") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "table",
                        "id": format!("tb/{}", table.element_id),
                        "slide": index + 1,
                        "rows": table.rows.len(),
                        "cols": table.rows.iter().map(std::vec::Vec::len).max().unwrap_or(0),
                        "columnWidths": table.column_widths,
                        "rowHeights": table.row_heights,
                        "preview": table.rows.first().map(|row| row.iter().map(|cell| cell.text.clone()).collect::<Vec<_>>().join(" | ")),
                        "style": table.style,
                        "bbox": [table.frame.left, table.frame.top, table.frame.width, table.frame.height],
                        "bboxUnit": "points",
                    })
                }
                PresentationElement::Chart(chart) => {
                    if !include("chart") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "chart",
                        "id": format!("ch/{}", chart.element_id),
                        "slide": index + 1,
                        "chartType": format!("{:?}", chart.chart_type),
                        "title": chart.title,
                        "bbox": [chart.frame.left, chart.frame.top, chart.frame.width, chart.frame.height],
                        "bboxUnit": "points",
                    })
                }
                PresentationElement::Image(image) => {
                    if !include("image") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "image",
                        "id": format!("im/{}", image.element_id),
                        "slide": index + 1,
                        "alt": image.alt_text,
                        "prompt": image.prompt,
                        "fit": format!("{:?}", image.fit_mode),
                        "rotation": image.rotation_degrees,
                        "flipHorizontal": image.flip_horizontal,
                        "flipVertical": image.flip_vertical,
                        "crop": image.crop.map(|(left, top, right, bottom)| serde_json::json!({
                            "left": left,
                            "top": top,
                            "right": right,
                            "bottom": bottom,
                        })),
                        "isPlaceholder": image.is_placeholder,
                        "lockAspectRatio": image.lock_aspect_ratio,
                        "bbox": [image.frame.left, image.frame.top, image.frame.width, image.frame.height],
                        "bboxUnit": "points",
                    })
                }
            };
            if !target_matches(target_id, &record) && target_id != Some(slide_id.as_str()) {
                continue;
            }
            if let Some(placeholder) = match element {
                PresentationElement::Text(text) => text.placeholder.as_ref(),
                PresentationElement::Shape(shape) => shape.placeholder.as_ref(),
                PresentationElement::Connector(_)
                | PresentationElement::Image(_)
                | PresentationElement::Table(_)
                | PresentationElement::Chart(_) => None,
            } {
                record["placeholder"] =
                    serde_json::Value::String(placeholder.placeholder_type.clone());
                record["placeholderName"] = serde_json::Value::String(placeholder.name.clone());
                record["placeholderIndex"] = placeholder
                    .index
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null);
            }
            if let Some(hyperlink) = match element {
                PresentationElement::Text(text) => text.hyperlink.as_ref(),
                PresentationElement::Shape(shape) => shape.hyperlink.as_ref(),
                PresentationElement::Connector(_)
                | PresentationElement::Image(_)
                | PresentationElement::Table(_)
                | PresentationElement::Chart(_) => None,
            } {
                record["hyperlink"] = hyperlink.to_json();
            }
            lines.push(record);
        }
    }
    let mut ndjson = lines
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(max_chars) = max_chars
        && ndjson.len() > max_chars
    {
        let omitted_lines = ndjson[max_chars..].lines().count();
        ndjson.truncate(max_chars);
        ndjson.push('\n');
        ndjson.push_str(
            &serde_json::json!({
                "kind": "notice",
                "message": format!(
                    "Truncated: omitted {omitted_lines} lines. Increase maxChars or narrow target."
                ),
            })
            .to_string(),
        );
    }
    ndjson
}

fn target_matches(target_id: Option<&str>, record: &Value) -> bool {
    match target_id {
        None => true,
        Some(target_id) => record.get("id").and_then(Value::as_str) == Some(target_id),
    }
}

fn normalize_element_lookup_id(element_id: &str) -> &str {
    element_id
        .split_once('/')
        .map(|(_, normalized)| normalized)
        .unwrap_or(element_id)
}

fn resolve_anchor(
    document: &PresentationDocument,
    id: &str,
    action: &str,
) -> Result<Value, PresentationArtifactError> {
    if id == format!("pr/{}", document.artifact_id) {
        return Ok(serde_json::json!({
            "kind": "deck",
            "id": id,
            "artifactId": document.artifact_id,
            "name": document.name,
            "slideCount": document.slides.len(),
            "activeSlideIndex": document.active_slide_index,
            "activeSlideId": document.active_slide_index.and_then(|index| document.slides.get(index)).map(|slide| format!("sl/{}", slide.slide_id)),
        }));
    }

    for (slide_index, slide) in document.slides.iter().enumerate() {
        let slide_id = format!("sl/{}", slide.slide_id);
        if id == slide_id {
            return Ok(serde_json::json!({
                "kind": "slide",
                "id": slide_id,
                "slide": slide_index + 1,
                "slideIndex": slide_index,
                "isActive": document.active_slide_index == Some(slide_index),
                "layoutId": slide.layout_id,
                "notesId": (!slide.notes.text.is_empty()).then(|| format!("nt/{}", slide.slide_id)),
                "elementIds": slide.elements.iter().map(|element| {
                    let prefix = match element {
                        PresentationElement::Text(_) | PresentationElement::Shape(_) => "sh",
                        PresentationElement::Connector(_) => "cn",
                        PresentationElement::Image(_) => "im",
                        PresentationElement::Table(_) => "tb",
                        PresentationElement::Chart(_) => "ch",
                    };
                    format!("{prefix}/{}", element.element_id())
                }).collect::<Vec<_>>(),
            }));
        }
        let notes_id = format!("nt/{}", slide.slide_id);
        if id == notes_id {
            return Ok(serde_json::json!({
                "kind": "notes",
                "id": notes_id,
                "slide": slide_index + 1,
                "slideIndex": slide_index,
                "visible": slide.notes.visible,
                "text": slide.notes.text,
            }));
        }
        for element in &slide.elements {
            let mut record = match element {
                PresentationElement::Text(text) => serde_json::json!({
                    "kind": "textbox",
                    "id": format!("sh/{}", text.element_id),
                    "elementId": text.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "text": text.text,
                    "bbox": [text.frame.left, text.frame.top, text.frame.width, text.frame.height],
                    "bboxUnit": "points",
                }),
                PresentationElement::Shape(shape) => serde_json::json!({
                    "kind": if shape.text.is_some() { "textbox" } else { "shape" },
                    "id": format!("sh/{}", shape.element_id),
                    "elementId": shape.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "geometry": format!("{:?}", shape.geometry),
                    "text": shape.text,
                    "bbox": [shape.frame.left, shape.frame.top, shape.frame.width, shape.frame.height],
                    "bboxUnit": "points",
                }),
                PresentationElement::Connector(connector) => serde_json::json!({
                    "kind": "connector",
                    "id": format!("cn/{}", connector.element_id),
                    "elementId": connector.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "connectorType": format!("{:?}", connector.connector_type),
                    "start": [connector.start.left, connector.start.top],
                    "end": [connector.end.left, connector.end.top],
                    "lineStyle": format!("{:?}", connector.line_style),
                    "label": connector.label,
                }),
                PresentationElement::Image(image) => serde_json::json!({
                    "kind": "image",
                    "id": format!("im/{}", image.element_id),
                    "elementId": image.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "alt": image.alt_text,
                    "prompt": image.prompt,
                    "fit": format!("{:?}", image.fit_mode),
                    "rotation": image.rotation_degrees,
                    "flipHorizontal": image.flip_horizontal,
                    "flipVertical": image.flip_vertical,
                    "crop": image.crop.map(|(left, top, right, bottom)| serde_json::json!({
                        "left": left,
                        "top": top,
                        "right": right,
                        "bottom": bottom,
                    })),
                    "isPlaceholder": image.is_placeholder,
                    "lockAspectRatio": image.lock_aspect_ratio,
                    "bbox": [image.frame.left, image.frame.top, image.frame.width, image.frame.height],
                    "bboxUnit": "points",
                }),
                PresentationElement::Table(table) => serde_json::json!({
                    "kind": "table",
                    "id": format!("tb/{}", table.element_id),
                    "elementId": table.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "rows": table.rows.len(),
                    "cols": table.rows.iter().map(std::vec::Vec::len).max().unwrap_or(0),
                    "columnWidths": table.column_widths,
                    "rowHeights": table.row_heights,
                    "bbox": [table.frame.left, table.frame.top, table.frame.width, table.frame.height],
                    "bboxUnit": "points",
                }),
                PresentationElement::Chart(chart) => serde_json::json!({
                    "kind": "chart",
                    "id": format!("ch/{}", chart.element_id),
                    "elementId": chart.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "chartType": format!("{:?}", chart.chart_type),
                    "title": chart.title,
                    "bbox": [chart.frame.left, chart.frame.top, chart.frame.width, chart.frame.height],
                    "bboxUnit": "points",
                }),
            };
            if let Some(hyperlink) = match element {
                PresentationElement::Text(text) => text.hyperlink.as_ref(),
                PresentationElement::Shape(shape) => shape.hyperlink.as_ref(),
                PresentationElement::Connector(_)
                | PresentationElement::Image(_)
                | PresentationElement::Table(_)
                | PresentationElement::Chart(_) => None,
            } {
                record["hyperlink"] = hyperlink.to_json();
            }
            if record.get("id").and_then(Value::as_str) == Some(id) {
                return Ok(record);
            }
        }
    }

    for layout in &document.layouts {
        let layout_id = format!("ly/{}", layout.layout_id);
        if id == layout_id {
            return Ok(serde_json::json!({
                "kind": "layout",
                "id": layout_id,
                "layoutId": layout.layout_id,
                "name": layout.name,
                "type": match layout.kind {
                    LayoutKind::Layout => "layout",
                    LayoutKind::Master => "master",
                },
                "parentLayoutId": layout.parent_layout_id,
                "placeholders": layout_placeholder_list(document, &layout.layout_id, action)?,
            }));
        }
    }

    Err(PresentationArtifactError::UnsupportedFeature {
        action: action.to_string(),
        message: format!("unknown resolve id `{id}`"),
    })
}

fn build_pptx_bytes(document: &PresentationDocument, action: &str) -> Result<Vec<u8>, String> {
    let bytes = document
        .to_ppt_rs()
        .build()
        .map_err(|error| format!("{action}: {error}"))?;
    patch_pptx_package(bytes, document).map_err(|error| format!("{action}: {error}"))
}

struct SlideImageAsset {
    xml: String,
    relationship_xml: String,
    media_path: String,
    media_bytes: Vec<u8>,
    extension: String,
}

fn normalized_image_extension(format: &str) -> String {
    match format.to_ascii_lowercase().as_str() {
        "jpeg" => "jpg".to_string(),
        other => other.to_string(),
    }
}

fn image_relationship_xml(relationship_id: &str, target: &str) -> String {
    format!(
        r#"<Relationship Id="{relationship_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{}"/>"#,
        ppt_rs::escape_xml(target)
    )
}

fn image_picture_xml(
    image: &ImageElement,
    shape_id: usize,
    relationship_id: &str,
    frame: Rect,
    crop: Option<ImageCrop>,
) -> String {
    let blip_fill = if let Some((crop_left, crop_top, crop_right, crop_bottom)) = crop {
        format!(
            r#"<p:blipFill>
<a:blip r:embed="{relationship_id}"/>
<a:srcRect l="{}" t="{}" r="{}" b="{}"/>
<a:stretch>
<a:fillRect/>
</a:stretch>
</p:blipFill>"#,
            (crop_left * 100_000.0).round() as u32,
            (crop_top * 100_000.0).round() as u32,
            (crop_right * 100_000.0).round() as u32,
            (crop_bottom * 100_000.0).round() as u32,
        )
    } else {
        format!(
            r#"<p:blipFill>
<a:blip r:embed="{relationship_id}"/>
<a:stretch>
<a:fillRect/>
</a:stretch>
</p:blipFill>"#
        )
    };
    let descr = image
        .alt_text
        .as_deref()
        .map(|alt| format!(r#" descr="{}""#, ppt_rs::escape_xml(alt)))
        .unwrap_or_default();
    let no_change_aspect = if image.lock_aspect_ratio { 1 } else { 0 };
    let rotation = image
        .rotation_degrees
        .map(|rotation| format!(r#" rot="{}""#, i64::from(rotation) * 60_000))
        .unwrap_or_default();
    let flip_horizontal = if image.flip_horizontal {
        r#" flipH="1""#
    } else {
        ""
    };
    let flip_vertical = if image.flip_vertical {
        r#" flipV="1""#
    } else {
        ""
    };
    format!(
        r#"<p:pic>
<p:nvPicPr>
<p:cNvPr id="{shape_id}" name="Picture {shape_id}"{descr}/>
<p:cNvPicPr>
<a:picLocks noChangeAspect="{no_change_aspect}"/>
</p:cNvPicPr>
<p:nvPr/>
</p:nvPicPr>
{blip_fill}
<p:spPr>
<a:xfrm{rotation}{flip_horizontal}{flip_vertical}>
<a:off x="{}" y="{}"/>
<a:ext cx="{}" cy="{}"/>
</a:xfrm>
<a:prstGeom prst="rect">
<a:avLst/>
</a:prstGeom>
</p:spPr>
</p:pic>"#,
        points_to_emu(frame.left),
        points_to_emu(frame.top),
        points_to_emu(frame.width),
        points_to_emu(frame.height),
    )
}

fn slide_image_assets(
    slide: &PresentationSlide,
    next_media_index: &mut usize,
) -> Vec<SlideImageAsset> {
    let mut ordered = slide.elements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|element| element.z_order());
    let shape_count = ordered
        .iter()
        .filter(|element| {
            matches!(
                element,
                PresentationElement::Text(_)
                    | PresentationElement::Shape(_)
                    | PresentationElement::Image(ImageElement { payload: None, .. })
            )
        })
        .count()
        + usize::from(slide.background_fill.is_some());
    let mut image_index = 0_usize;
    let mut assets = Vec::new();
    for element in ordered {
        let PresentationElement::Image(image) = element else {
            continue;
        };
        let Some(payload) = &image.payload else {
            continue;
        };
        let (left, top, width, height, fitted_crop) = if image.fit_mode != ImageFitMode::Stretch {
            fit_image(image)
        } else {
            (
                image.frame.left,
                image.frame.top,
                image.frame.width,
                image.frame.height,
                None,
            )
        };
        image_index += 1;
        let relationship_id = format!("rIdImage{image_index}");
        let extension = normalized_image_extension(&payload.format);
        let media_name = format!("image{next_media_index}.{extension}");
        *next_media_index += 1;
        assets.push(SlideImageAsset {
            xml: image_picture_xml(
                image,
                20 + shape_count + image_index - 1,
                &relationship_id,
                Rect {
                    left,
                    top,
                    width,
                    height,
                },
                image.crop.or(fitted_crop),
            ),
            relationship_xml: image_relationship_xml(
                &relationship_id,
                &format!("../media/{media_name}"),
            ),
            media_path: format!("ppt/media/{media_name}"),
            media_bytes: payload.bytes.clone(),
            extension,
        });
    }
    assets
}

fn patch_pptx_package(
    source_bytes: Vec<u8>,
    document: &PresentationDocument,
) -> Result<Vec<u8>, String> {
    let mut archive =
        ZipArchive::new(Cursor::new(source_bytes)).map_err(|error| error.to_string())?;
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let mut next_media_index = 1_usize;
    let mut pending_slide_relationships = HashMap::new();
    let mut pending_slide_images = HashMap::new();
    let mut pending_media = Vec::new();
    let mut image_extensions = BTreeSet::new();
    for (slide_index, slide) in document.slides.iter().enumerate() {
        let slide_number = slide_index + 1;
        let images = slide_image_assets(slide, &mut next_media_index);
        let mut relationships = slide_hyperlink_relationships(slide);
        relationships.extend(images.iter().map(|image| image.relationship_xml.clone()));
        if !relationships.is_empty() {
            pending_slide_relationships.insert(slide_number, relationships);
        }
        if !images.is_empty() {
            image_extensions.extend(images.iter().map(|image| image.extension.clone()));
            pending_media.extend(
                images
                    .iter()
                    .map(|image| (image.media_path.clone(), image.media_bytes.clone())),
            );
            pending_slide_images.insert(slide_number, images);
        }
    }

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        let options = file.options();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        writer
            .start_file(&name, options)
            .map_err(|error| error.to_string())?;
        if name == "[Content_Types].xml" {
            writer
                .write_all(update_content_types_xml(bytes, &image_extensions)?.as_bytes())
                .map_err(|error| error.to_string())?;
            continue;
        }
        if name == "ppt/presentation.xml" {
            writer
                .write_all(
                    update_presentation_xml_dimensions(bytes, document.slide_size)?.as_bytes(),
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(slide_number) = parse_slide_xml_path(&name) {
            writer
                .write_all(
                    update_slide_xml(
                        bytes,
                        &document.slides[slide_number - 1],
                        pending_slide_images
                            .get(&slide_number)
                            .map(std::vec::Vec::as_slice)
                            .unwrap_or(&[]),
                    )?
                    .as_bytes(),
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(slide_number) = parse_slide_relationships_path(&name)
            && let Some(relationships) = pending_slide_relationships.remove(&slide_number)
        {
            writer
                .write_all(update_slide_relationships_xml(bytes, &relationships)?.as_bytes())
                .map_err(|error| error.to_string())?;
            continue;
        }
        writer
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
    }

    for (slide_number, relationships) in pending_slide_relationships {
        writer
            .start_file(
                format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
                SimpleFileOptions::default(),
            )
            .map_err(|error| error.to_string())?;
        writer
            .write_all(slide_relationships_xml(&relationships).as_bytes())
            .map_err(|error| error.to_string())?;
    }

    for (path, bytes) in pending_media {
        writer
            .start_file(path, SimpleFileOptions::default())
            .map_err(|error| error.to_string())?;
        writer
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
    }

    writer
        .finish()
        .map_err(|error| error.to_string())
        .map(Cursor::into_inner)
}

fn update_presentation_xml_dimensions(
    existing_bytes: Vec<u8>,
    slide_size: Rect,
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    let updated = replace_self_closing_xml_tag(
        &existing,
        "p:sldSz",
        &format!(
            r#"<p:sldSz cx="{}" cy="{}" type="screen4x3"/>"#,
            points_to_emu(slide_size.width),
            points_to_emu(slide_size.height)
        ),
    )?;
    replace_self_closing_xml_tag(
        &updated,
        "p:notesSz",
        &format!(
            r#"<p:notesSz cx="{}" cy="{}"/>"#,
            points_to_emu(slide_size.height),
            points_to_emu(slide_size.width)
        ),
    )
}

fn replace_self_closing_xml_tag(xml: &str, tag: &str, replacement: &str) -> Result<String, String> {
    let start = xml
        .find(&format!("<{tag} "))
        .ok_or_else(|| format!("presentation xml is missing `<{tag} .../>`"))?;
    let end = xml[start..]
        .find("/>")
        .map(|offset| start + offset + 2)
        .ok_or_else(|| format!("presentation xml tag `{tag}` is not self-closing"))?;
    Ok(format!("{}{replacement}{}", &xml[..start], &xml[end..]))
}

fn slide_hyperlink_relationships(slide: &PresentationSlide) -> Vec<String> {
    let mut ordered = slide.elements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|element| element.z_order());
    let mut hyperlink_index = 1_u32;
    let mut relationships = Vec::new();
    for element in ordered {
        let Some(hyperlink) = (match element {
            PresentationElement::Text(text) => text.hyperlink.as_ref(),
            PresentationElement::Shape(shape) => shape.hyperlink.as_ref(),
            PresentationElement::Connector(_)
            | PresentationElement::Image(_)
            | PresentationElement::Table(_)
            | PresentationElement::Chart(_) => None,
        }) else {
            continue;
        };
        let relationship_id = format!("rIdHyperlink{hyperlink_index}");
        hyperlink_index += 1;
        relationships.push(hyperlink.relationship_xml(&relationship_id));
    }
    relationships
}

fn parse_slide_relationships_path(path: &str) -> Option<usize> {
    path.strip_prefix("ppt/slides/_rels/slide")?
        .strip_suffix(".xml.rels")?
        .parse::<usize>()
        .ok()
}

fn parse_slide_xml_path(path: &str) -> Option<usize> {
    path.strip_prefix("ppt/slides/slide")?
        .strip_suffix(".xml")?
        .parse::<usize>()
        .ok()
}

fn update_slide_relationships_xml(
    existing_bytes: Vec<u8>,
    relationships: &[String],
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    let injected = relationships.join("\n");
    existing
        .contains("</Relationships>")
        .then(|| existing.replace("</Relationships>", &format!("{injected}\n</Relationships>")))
        .ok_or_else(|| {
            "slide relationships xml is missing a closing `</Relationships>`".to_string()
        })
}

fn slide_relationships_xml(relationships: &[String]) -> String {
    let body = relationships.join("\n");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{body}
</Relationships>"#
    )
}

fn update_content_types_xml(
    existing_bytes: Vec<u8>,
    image_extensions: &BTreeSet<String>,
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    if image_extensions.is_empty() {
        return Ok(existing);
    }
    let existing_lower = existing.to_ascii_lowercase();
    let additions = image_extensions
        .iter()
        .filter(|extension| {
            !existing_lower.contains(&format!(
                r#"extension="{}""#,
                extension.to_ascii_lowercase()
            ))
        })
        .map(|extension| generate_image_content_type(extension))
        .collect::<Vec<_>>();
    if additions.is_empty() {
        return Ok(existing);
    }
    existing
        .contains("</Types>")
        .then(|| existing.replace("</Types>", &format!("{}\n</Types>", additions.join("\n"))))
        .ok_or_else(|| "content types xml is missing a closing `</Types>`".to_string())
}

fn update_slide_xml(
    existing_bytes: Vec<u8>,
    slide: &PresentationSlide,
    slide_images: &[SlideImageAsset],
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    let existing = replace_image_placeholders(existing, slide_images)?;
    let table_xml = slide_table_xml(slide);
    if table_xml.is_empty() {
        return Ok(existing);
    }
    existing
        .contains("</p:spTree>")
        .then(|| existing.replace("</p:spTree>", &format!("{table_xml}\n</p:spTree>")))
        .ok_or_else(|| "slide xml is missing a closing `</p:spTree>`".to_string())
}

fn replace_image_placeholders(
    existing: String,
    slide_images: &[SlideImageAsset],
) -> Result<String, String> {
    if slide_images.is_empty() {
        return Ok(existing);
    }
    let mut updated = String::with_capacity(existing.len());
    let mut remaining = existing.as_str();
    for image in slide_images {
        let marker = remaining
            .find("name=\"Image Placeholder: ")
            .ok_or_else(|| {
                "slide xml is missing an image placeholder block for exported images".to_string()
            })?;
        let start = remaining[..marker].rfind("<p:sp>").ok_or_else(|| {
            "slide xml is missing an opening `<p:sp>` for image placeholder".to_string()
        })?;
        let end = remaining[marker..]
            .find("</p:sp>")
            .map(|offset| marker + offset + "</p:sp>".len())
            .ok_or_else(|| {
                "slide xml is missing a closing `</p:sp>` for image placeholder".to_string()
            })?;
        updated.push_str(&remaining[..start]);
        updated.push_str(&image.xml);
        remaining = &remaining[end..];
    }
    updated.push_str(remaining);
    Ok(updated)
}

fn slide_table_xml(slide: &PresentationSlide) -> String {
    let mut ordered = slide.elements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|element| element.z_order());
    let mut table_index = 0_usize;
    ordered
        .into_iter()
        .filter_map(|element| {
            let PresentationElement::Table(table) = element else {
                return None;
            };
            table_index += 1;
            let rows = table
                .rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(row_index, row)| {
                    let cells = row
                        .into_iter()
                        .enumerate()
                        .map(|(column_index, cell)| {
                            build_table_cell(cell, &table.merges, row_index, column_index)
                        })
                        .collect::<Vec<_>>();
                    let mut table_row = TableRow::new(cells);
                    if let Some(height) = table.row_heights.get(row_index) {
                        table_row = table_row.with_height(points_to_emu(*height));
                    }
                    Some(table_row)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(ppt_rs::generator::table::generate_table_xml(
                &ppt_rs::generator::table::Table::new(
                    rows,
                    table
                        .column_widths
                        .iter()
                        .copied()
                        .map(points_to_emu)
                        .collect(),
                    points_to_emu(table.frame.left),
                    points_to_emu(table.frame.top),
                ),
                300 + table_index,
            ))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_preview_images(
    document: &PresentationDocument,
    output_dir: &Path,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let pptx_path = output_dir.join("preview.pptx");
    let bytes = build_pptx_bytes(document, action).map_err(|message| {
        PresentationArtifactError::ExportFailed {
            path: pptx_path.clone(),
            message,
        }
    })?;
    std::fs::write(&pptx_path, bytes).map_err(|error| PresentationArtifactError::ExportFailed {
        path: pptx_path.clone(),
        message: error.to_string(),
    })?;
    render_pptx_to_pngs(&pptx_path, output_dir, action)
}

fn render_pptx_to_pngs(
    pptx_path: &Path,
    output_dir: &Path,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let soffice_cmd = if cfg!(target_os = "macos")
        && Path::new("/Applications/LibreOffice.app/Contents/MacOS/soffice").exists()
    {
        "/Applications/LibreOffice.app/Contents/MacOS/soffice"
    } else {
        "soffice"
    };
    let conversion = Command::new(soffice_cmd)
        .arg("--headless")
        .arg("--convert-to")
        .arg("pdf")
        .arg(pptx_path)
        .arg("--outdir")
        .arg(output_dir)
        .output()
        .map_err(|error| PresentationArtifactError::ExportFailed {
            path: pptx_path.to_path_buf(),
            message: format!("{action}: failed to execute LibreOffice: {error}"),
        })?;
    if !conversion.status.success() {
        return Err(PresentationArtifactError::ExportFailed {
            path: pptx_path.to_path_buf(),
            message: format!(
                "{action}: LibreOffice conversion failed: {}",
                String::from_utf8_lossy(&conversion.stderr)
            ),
        });
    }

    let pdf_path = output_dir.join(
        pptx_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| format!("{stem}.pdf"))
            .ok_or_else(|| PresentationArtifactError::ExportFailed {
                path: pptx_path.to_path_buf(),
                message: format!("{action}: preview pptx filename is invalid"),
            })?,
    );
    let prefix = output_dir.join("slide");
    let conversion = Command::new("pdftoppm")
        .arg("-png")
        .arg(&pdf_path)
        .arg(&prefix)
        .output()
        .map_err(|error| PresentationArtifactError::ExportFailed {
            path: pdf_path.clone(),
            message: format!("{action}: failed to execute pdftoppm: {error}"),
        })?;
    std::fs::remove_file(&pdf_path).ok();
    if !conversion.status.success() {
        return Err(PresentationArtifactError::ExportFailed {
            path: output_dir.to_path_buf(),
            message: format!(
                "{action}: pdftoppm conversion failed: {}",
                String::from_utf8_lossy(&conversion.stderr)
            ),
        });
    }
    Ok(())
}

pub(crate) fn write_preview_image(
    source_path: &Path,
    target_path: &Path,
    format: PreviewOutputFormat,
    scale: f32,
    quality: u8,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    if matches!(format, PreviewOutputFormat::Png) && scale == 1.0 {
        std::fs::rename(source_path, target_path).map_err(|error| {
            PresentationArtifactError::ExportFailed {
                path: target_path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        return Ok(());
    }
    let mut preview =
        image::open(source_path).map_err(|error| PresentationArtifactError::ExportFailed {
            path: source_path.to_path_buf(),
            message: format!("{action}: {error}"),
        })?;
    if scale != 1.0 {
        let width = (preview.width() as f32 * scale).round().max(1.0) as u32;
        let height = (preview.height() as f32 * scale).round().max(1.0) as u32;
        preview = preview.resize_exact(width, height, FilterType::Lanczos3);
    }
    let file = std::fs::File::create(target_path).map_err(|error| {
        PresentationArtifactError::ExportFailed {
            path: target_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let mut writer = std::io::BufWriter::new(file);
    match format {
        PreviewOutputFormat::Png => {
            preview
                .write_to(&mut writer, ImageFormat::Png)
                .map_err(|error| PresentationArtifactError::ExportFailed {
                    path: target_path.to_path_buf(),
                    message: format!("{action}: {error}"),
                })?
        }
        PreviewOutputFormat::Jpeg => {
            let rgb = preview.to_rgb8();
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
            encoder.encode_image(&rgb).map_err(|error| {
                PresentationArtifactError::ExportFailed {
                    path: target_path.to_path_buf(),
                    message: format!("{action}: {error}"),
                }
            })?;
        }
    }
    std::fs::remove_file(source_path).ok();
    Ok(())
}

fn collect_pngs(output_dir: &Path) -> Result<Vec<PathBuf>, PresentationArtifactError> {
    let mut files = std::fs::read_dir(output_dir)
        .map_err(|error| PresentationArtifactError::ExportFailed {
            path: output_dir.to_path_buf(),
            message: error.to_string(),
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn parse_preview_output_format(
    format: Option<&str>,
    path: &Path,
    action: &str,
) -> Result<PreviewOutputFormat, PresentationArtifactError> {
    let value = format
        .map(str::to_owned)
        .or_else(|| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "png".to_string());
    match value.to_ascii_lowercase().as_str() {
        "png" => Ok(PreviewOutputFormat::Png),
        "jpg" | "jpeg" => Ok(PreviewOutputFormat::Jpeg),
        "svg" => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: "preview format `svg` is not supported".to_string(),
        }),
        other => Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("preview format `{other}` is not supported"),
        }),
    }
}

fn normalize_preview_scale(
    scale: Option<f32>,
    action: &str,
) -> Result<f32, PresentationArtifactError> {
    let scale = scale.unwrap_or(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "`scale` must be a positive number".to_string(),
        });
    }
    Ok(scale)
}

fn normalize_preview_quality(
    quality: Option<u8>,
    action: &str,
) -> Result<u8, PresentationArtifactError> {
    let quality = quality.unwrap_or(90);
    if quality == 0 || quality > 100 {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "`quality` must be between 1 and 100".to_string(),
        });
    }
    Ok(quality)
}

fn cell_value_to_string(value: Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text,
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

fn snapshot_for_document(document: &PresentationDocument) -> ArtifactSnapshot {
    ArtifactSnapshot {
        slide_count: document.slides.len(),
        slides: document
            .slides
            .iter()
            .enumerate()
            .map(|(index, slide)| SlideSnapshot {
                slide_id: slide.slide_id.clone(),
                index,
                element_ids: slide
                    .elements
                    .iter()
                    .map(|element| element.element_id().to_string())
                    .collect(),
                element_types: slide
                    .elements
                    .iter()
                    .map(|element| element.kind().to_string())
                    .collect(),
            })
            .collect(),
    }
}

fn slide_list(document: &PresentationDocument) -> Vec<SlideListEntry> {
    document
        .slides
        .iter()
        .enumerate()
        .map(|(index, slide)| SlideListEntry {
            slide_id: slide.slide_id.clone(),
            index,
            is_active: document.active_slide_index == Some(index),
            notes: (!slide.notes.text.is_empty()).then(|| slide.notes.text.clone()),
            notes_visible: slide.notes.visible,
            background_fill: slide.background_fill.clone(),
            layout_id: slide.layout_id.clone(),
            element_count: slide.elements.len(),
        })
        .collect()
}

fn layout_list(document: &PresentationDocument) -> Vec<LayoutListEntry> {
    document
        .layouts
        .iter()
        .map(|layout| LayoutListEntry {
            layout_id: layout.layout_id.clone(),
            name: layout.name.clone(),
            kind: match layout.kind {
                LayoutKind::Layout => "layout".to_string(),
                LayoutKind::Master => "master".to_string(),
            },
            parent_layout_id: layout.parent_layout_id.clone(),
            placeholder_count: layout.placeholders.len(),
        })
        .collect()
}

fn points_to_emu(points: u32) -> u32 {
    points.saturating_mul(POINT_TO_EMU)
}

fn emu_to_points(emu: u32) -> u32 {
    emu / POINT_TO_EMU
}

type ImageCrop = (f64, f64, f64, f64);
type FittedImage = (u32, u32, u32, u32, Option<ImageCrop>);

pub(crate) fn fit_image(image: &ImageElement) -> FittedImage {
    let Some(payload) = image.payload.as_ref() else {
        return (
            image.frame.left,
            image.frame.top,
            image.frame.width,
            image.frame.height,
            None,
        );
    };
    let frame = image.frame;
    let source_width = payload.width_px as f64;
    let source_height = payload.height_px as f64;
    let target_width = frame.width as f64;
    let target_height = frame.height as f64;
    let source_ratio = source_width / source_height;
    let target_ratio = target_width / target_height;

    match image.fit_mode {
        ImageFitMode::Stretch => (frame.left, frame.top, frame.width, frame.height, None),
        ImageFitMode::Contain => {
            let scale = if source_ratio > target_ratio {
                target_width / source_width
            } else {
                target_height / source_height
            };
            let width = (source_width * scale).round() as u32;
            let height = (source_height * scale).round() as u32;
            let left = frame.left + frame.width.saturating_sub(width) / 2;
            let top = frame.top + frame.height.saturating_sub(height) / 2;
            (left, top, width, height, None)
        }
        ImageFitMode::Cover => {
            let scale = if source_ratio > target_ratio {
                target_height / source_height
            } else {
                target_width / source_width
            };
            let width = source_width * scale;
            let height = source_height * scale;
            let crop_x = ((width - target_width).max(0.0) / width) / 2.0;
            let crop_y = ((height - target_height).max(0.0) / height) / 2.0;
            (
                frame.left,
                frame.top,
                frame.width,
                frame.height,
                Some((crop_x, crop_y, crop_x, crop_y)),
            )
        }
    }
}

fn normalize_image_crop(
    crop: ImageCropArgs,
    action: &str,
) -> Result<ImageCrop, PresentationArtifactError> {
    for (name, value) in [
        ("left", crop.left),
        ("top", crop.top),
        ("right", crop.right),
        ("bottom", crop.bottom),
    ] {
        if !(0.0..=1.0).contains(&value) {
            return Err(PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: format!("image crop `{name}` must be between 0.0 and 1.0"),
            });
        }
    }
    Ok((crop.left, crop.top, crop.right, crop.bottom))
}

fn load_image_payload_from_path(
    path: &Path,
    action: &str,
) -> Result<ImagePayload, PresentationArtifactError> {
    let bytes = std::fs::read(path).map_err(|error| PresentationArtifactError::InvalidArgs {
        action: action.to_string(),
        message: format!("failed to read image `{}`: {error}", path.display()),
    })?;
    build_image_payload(
        bytes,
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image")
            .to_string(),
        action,
    )
}

fn load_image_payload_from_data_url(
    data_url: &str,
    action: &str,
) -> Result<ImagePayload, PresentationArtifactError> {
    let (header, payload) =
        data_url
            .split_once(',')
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "data_url must include a MIME header and base64 payload".to_string(),
            })?;
    let mime = header
        .strip_prefix("data:")
        .and_then(|prefix| prefix.strip_suffix(";base64"))
        .ok_or_else(|| PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "data_url must be base64-encoded".to_string(),
        })?;
    let bytes = BASE64_STANDARD.decode(payload).map_err(|error| {
        PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to decode image data_url: {error}"),
        }
    })?;
    build_image_payload(
        bytes,
        format!("image.{}", image_extension_from_mime(mime)),
        action,
    )
}

fn load_image_payload_from_uri(
    uri: &str,
    action: &str,
) -> Result<ImagePayload, PresentationArtifactError> {
    let response =
        reqwest::blocking::get(uri).map_err(|error| PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to fetch image `{uri}`: {error}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to fetch image `{uri}`: HTTP {status}"),
        });
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string());
    let bytes = response
        .bytes()
        .map_err(|error| PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to read image `{uri}`: {error}"),
        })?;
    build_image_payload(
        bytes.to_vec(),
        infer_remote_image_filename(uri, content_type.as_deref()),
        action,
    )
}

fn infer_remote_image_filename(uri: &str, content_type: Option<&str>) -> String {
    let path_name = reqwest::Url::parse(uri)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(Iterator::last)
                .map(str::to_owned)
        })
        .filter(|segment| !segment.is_empty());
    match (path_name, content_type) {
        (Some(path_name), _) if Path::new(&path_name).extension().is_some() => path_name,
        (Some(path_name), Some(content_type)) => {
            format!("{path_name}.{}", image_extension_from_mime(content_type))
        }
        (Some(path_name), None) => path_name,
        (None, Some(content_type)) => format!("image.{}", image_extension_from_mime(content_type)),
        (None, None) => "image.png".to_string(),
    }
}

fn build_image_payload(
    bytes: Vec<u8>,
    filename: String,
    action: &str,
) -> Result<ImagePayload, PresentationArtifactError> {
    let image = image::load_from_memory(&bytes).map_err(|error| {
        PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to decode image bytes: {error}"),
        }
    })?;
    let (width_px, height_px) = image.dimensions();
    let format = Path::new(&filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("png")
        .to_uppercase();
    Ok(ImagePayload {
        bytes,
        format,
        width_px,
        height_px,
    })
}

fn image_extension_from_mime(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    }
}

fn index_out_of_range(action: &str, index: usize, len: usize) -> PresentationArtifactError {
    PresentationArtifactError::InvalidArgs {
        action: action.to_string(),
        message: format!("slide index {index} is out of range for {len} slides"),
    }
}

fn to_index(value: u32) -> Result<usize, PresentationArtifactError> {
    usize::try_from(value).map_err(|_| PresentationArtifactError::InvalidArgs {
        action: "insert_slide".to_string(),
        message: "index does not fit in usize".to_string(),
    })
}

fn resequence_z_order(slide: &mut PresentationSlide) {
    for (index, element) in slide.elements.iter_mut().enumerate() {
        element.set_z_order(index);
    }
}
