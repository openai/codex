use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use image::GenericImageView;
use ppt_rs::Chart;
use ppt_rs::ChartSeries;
use ppt_rs::ChartType;
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
use ppt_rs::generator::CellAlign;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

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
                    ImageInputSource::DataUrl(_) => Vec::new(),
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
            "set_slide_background" => self.set_slide_background(request),
            "add_text_shape" => self.add_text_shape(request),
            "add_shape" => self.add_shape(request),
            "add_image" => self.add_image(request, cwd),
            "replace_image" => self.replace_image(request, cwd),
            "add_table" => self.add_table(request),
            "update_table_cell" => self.update_table_cell(request),
            "merge_table_cells" => self.merge_table_cells(request),
            "add_chart" => self.add_chart(request),
            "update_text" => self.update_text(request),
            "update_shape_style" => self.update_shape_style(request),
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
        let document = PresentationDocument::from_ppt_rs(imported);
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

        document.to_ppt_rs().save(&path).map_err(|error| {
            PresentationArtifactError::ExportFailed {
                path: path.clone(),
                message: error.to_string(),
            }
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
            std::fs::rename(&rendered, &output_path).map_err(|error| {
                PresentationArtifactError::ExportFailed {
                    path: output_path.clone(),
                    message: error.to_string(),
                }
            })?;
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
                let target = output_path.join(filename);
                std::fs::rename(&rendered, &target).map_err(|error| {
                    PresentationArtifactError::ExportFailed {
                        path: target.clone(),
                        message: error.to_string(),
                    }
                })?;
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
                "Presentation `{}` has {} slides, {} elements, and {} layouts",
                document.name.as_deref().unwrap_or("Untitled"),
                document.slides.len(),
                document.total_element_count(),
                document.layouts.len()
            ),
            snapshot_for_document(document),
        );
        response.slide_list = Some(slide_list(document));
        response.layout_list = Some(layout_list(document));
        response.theme = Some(document.theme_snapshot());
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
            PresentationElement::Image(_)
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

    fn add_image(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddImageArgs = parse_args(&request.action, &request.args)?;
        let image_source = args.image_source()?;
        let image_payload = match image_source {
            ImageInputSource::Path(path) => {
                load_image_payload_from_path(&resolve_path(cwd, &path), &request.action)?
            }
            ImageInputSource::DataUrl(data_url) => {
                load_image_payload_from_data_url(&data_url, &request.action)?
            }
        };
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
                fit_mode: args.fit.unwrap_or(ImageFitMode::Stretch),
                alt_text: args.alt,
                prompt: args.prompt,
                is_placeholder: args.path.is_none() && args.data_url.is_none(),
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
        let image_source = match (&args.path, &args.data_url) {
            (Some(path), None) => ImageInputSource::Path(path.clone()),
            (None, Some(data_url)) => ImageInputSource::DataUrl(data_url.clone()),
            _ => {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: request.action,
                    message: "provide exactly one of `path` or `data_url`".to_string(),
                });
            }
        };
        let image_payload = match image_source {
            ImageInputSource::Path(path) => {
                load_image_payload_from_path(&resolve_path(cwd, &path), "replace_image")?
            }
            ImageInputSource::DataUrl(data_url) => {
                load_image_payload_from_data_url(&data_url, "replace_image")?
            }
        };
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Image(image) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not an image", args.element_id),
            });
        };
        image.payload = image_payload;
        image.alt_text = args.alt;
        image.prompt = args.prompt;
        image.is_placeholder = false;
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
        let element_id = document.next_element_id();
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide
            .elements
            .push(PresentationElement::Table(TableElement {
                element_id: element_id.clone(),
                frame: args.position.into(),
                rows: coerce_table_rows(args.rows, &request.action)?,
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
                    text.frame = position.into();
                }
                if let Some(fill) = fill.clone() {
                    text.fill = Some(fill);
                }
                if args.stroke.is_some() || args.rotation.is_some() {
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
                    shape.frame = position.into();
                }
                if let Some(fill) = fill {
                    shape.fill = Some(fill);
                }
                if let Some(stroke) = stroke {
                    shape.stroke = Some(stroke);
                }
                if let Some(rotation) = args.rotation {
                    shape.rotation_degrees = Some(rotation);
                }
            }
            PresentationElement::Image(image) => {
                if args.fill.is_some() || args.stroke.is_some() || args.rotation.is_some() {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: "image elements support only `position` and `z_order` updates"
                            .to_string(),
                    });
                }
                if let Some(position) = args.position {
                    image.frame = position.into();
                }
            }
            PresentationElement::Table(table) => {
                if args.fill.is_some() || args.stroke.is_some() || args.rotation.is_some() {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: "table elements support only `position` and `z_order` updates"
                            .to_string(),
                    });
                }
                if let Some(position) = args.position {
                    table.frame = position.into();
                }
            }
            PresentationElement::Chart(chart) => {
                if args.fill.is_some() || args.stroke.is_some() || args.rotation.is_some() {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: "chart elements support only `position` and `z_order` updates"
                            .to_string(),
                    });
                }
                if let Some(position) = args.position {
                    chart.frame = position.into();
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
                        style: None,
                        merges: Vec::new(),
                        z_order: slide.elements.len(),
                    }));
            }

            document.slides.push(slide);
        }
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
                    content = content.add_shape(ppt_shape);
                }
                PresentationElement::Image(image) => {
                    let mut ppt_image = Image::from_bytes(
                        image.payload.bytes.clone(),
                        points_to_emu(image.frame.width),
                        points_to_emu(image.frame.height),
                        &image.payload.format,
                    )
                    .position(
                        points_to_emu(image.frame.left),
                        points_to_emu(image.frame.top),
                    );
                    if image.fit_mode != ImageFitMode::Stretch {
                        let (x, y, width, height, crop) = fit_image(&image);
                        ppt_image = Image::from_bytes(
                            image.payload.bytes.clone(),
                            points_to_emu(width),
                            points_to_emu(height),
                            &image.payload.format,
                        )
                        .position(points_to_emu(x), points_to_emu(y));
                        if let Some((left, top, right, bottom)) = crop {
                            ppt_image = ppt_image.with_crop(left, top, right, bottom);
                        }
                    }
                    content = content.add_image(ppt_image);
                }
                PresentationElement::Table(table) => {
                    let row_count = table.rows.len().max(1) as u32;
                    let column_count =
                        table.rows.iter().map(std::vec::Vec::len).max().unwrap_or(1) as u32;
                    let column_width = points_to_emu(table.frame.width / column_count.max(1));
                    let mut builder = TableBuilder::new(vec![column_width; column_count as usize])
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
                        builder = builder.add_row(TableRow::new(cells));
                    }
                    let _ = row_count;
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
    Image(ImageElement),
    Table(TableElement),
    Chart(ChartElement),
}

impl PresentationElement {
    fn element_id(&self) -> &str {
        match self {
            Self::Text(element) => &element.element_id,
            Self::Shape(element) => &element.element_id,
            Self::Image(element) => &element.element_id,
            Self::Table(element) => &element.element_id,
            Self::Chart(element) => &element.element_id,
        }
    }

    fn set_element_id(&mut self, new_id: String) {
        match self {
            Self::Text(element) => element.element_id = new_id,
            Self::Shape(element) => element.element_id = new_id,
            Self::Image(element) => element.element_id = new_id,
            Self::Table(element) => element.element_id = new_id,
            Self::Chart(element) => element.element_id = new_id,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Shape(_) => "shape",
            Self::Image(_) => "image",
            Self::Table(_) => "table",
            Self::Chart(_) => "chart",
        }
    }

    fn z_order(&self) -> usize {
        match self {
            Self::Text(element) => element.z_order,
            Self::Shape(element) => element.z_order,
            Self::Image(element) => element.z_order,
            Self::Table(element) => element.z_order,
            Self::Chart(element) => element.z_order,
        }
    }

    fn set_z_order(&mut self, z_order: usize) {
        match self {
            Self::Text(element) => element.z_order = z_order,
            Self::Shape(element) => element.z_order = z_order,
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
    placeholder: Option<PlaceholderRef>,
    rotation_degrees: Option<i32>,
    z_order: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ImageElement {
    pub(crate) element_id: String,
    pub(crate) frame: Rect,
    pub(crate) payload: ImagePayload,
    pub(crate) fit_mode: ImageFitMode,
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

#[derive(Debug, Deserialize)]
struct AddImageArgs {
    slide_index: u32,
    path: Option<PathBuf>,
    data_url: Option<String>,
    position: PositionArgs,
    fit: Option<ImageFitMode>,
    alt: Option<String>,
    prompt: Option<String>,
}

impl AddImageArgs {
    fn image_source(&self) -> Result<ImageInputSource, PresentationArtifactError> {
        match (&self.path, &self.data_url) {
            (Some(path), None) => Ok(ImageInputSource::Path(path.clone())),
            (None, Some(data_url)) => Ok(ImageInputSource::DataUrl(data_url.clone())),
            _ => Err(PresentationArtifactError::InvalidArgs {
                action: "add_image".to_string(),
                message: "provide exactly one of `path` or `data_url`".to_string(),
            }),
        }
    }
}

enum ImageInputSource {
    Path(PathBuf),
    DataUrl(String),
}

#[derive(Debug, Deserialize)]
struct AddTableArgs {
    slide_index: u32,
    position: PositionArgs,
    rows: Vec<Vec<Value>>,
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
struct UpdateShapeStyleArgs {
    element_id: String,
    position: Option<PositionArgs>,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
    rotation: Option<i32>,
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
    let position: PositionArgs = serde_json::from_value(value.clone()).map_err(|error| {
        PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("invalid slide_size: {error}"),
        }
    })?;
    Ok(position.into())
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
            PresentationElement::Image(_)
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
    let kinds = kind.unwrap_or("deck,slide,textbox,shape,table,chart,image,notes,layoutList");
    let include = |name: &str| kinds.split(',').map(str::trim).any(|entry| entry == name);
    let mut lines = Vec::new();
    if include("deck") {
        let record = serde_json::json!({
            "kind": "deck",
            "id": format!("pr/{}", document.artifact_id),
            "name": document.name,
            "slides": document.slides.len(),
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
                PresentationElement::Image(_)
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
                "layoutId": slide.layout_id,
                "notesId": (!slide.notes.text.is_empty()).then(|| format!("nt/{}", slide.slide_id)),
                "elementIds": slide.elements.iter().map(|element| {
                    let prefix = match element {
                        PresentationElement::Text(_) | PresentationElement::Shape(_) => "sh",
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
            let record = match element {
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
                PresentationElement::Image(image) => serde_json::json!({
                    "kind": "image",
                    "id": format!("im/{}", image.element_id),
                    "elementId": image.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "alt": image.alt_text,
                    "prompt": image.prompt,
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

fn write_preview_images(
    document: &PresentationDocument,
    output_dir: &Path,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let pptx_path = output_dir.join("preview.pptx");
    document.to_ppt_rs().save(&pptx_path).map_err(|error| {
        PresentationArtifactError::ExportFailed {
            path: pptx_path.clone(),
            message: error.to_string(),
        }
    })?;
    document
        .to_ppt_rs()
        .save_as_png(output_dir)
        .map_err(|error| PresentationArtifactError::ExportFailed {
            path: output_dir.to_path_buf(),
            message: format!("{action}: {error}"),
        })
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
    let frame = image.frame;
    let source_width = image.payload.width_px as f64;
    let source_height = image.payload.height_px as f64;
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
