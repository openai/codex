#[derive(Debug, Default)]
pub struct PresentationArtifactManager {
    documents: HashMap<String, PresentationDocument>,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    artifact_id: String,
    action: String,
    before: Option<PresentationDocument>,
    after: Option<PresentationDocument>,
}

impl PresentationArtifactManager {
    pub fn execute(
        &mut self,
        request: PresentationArtifactRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        match request.action.as_str() {
            "undo" => return self.undo(request),
            "redo" => return self.redo(request),
            "record_patch" => return self.record_patch(request),
            "apply_patch" => return self.apply_patch(request, cwd),
            _ => {}
        }

        let action = request.action.clone();
        let before = if tracks_history(&action) {
            request
                .artifact_id
                .as_ref()
                .and_then(|artifact_id| self.documents.get(artifact_id).cloned())
        } else {
            None
        };
        let response = self.execute_action(request, cwd)?;
        if tracks_history(&action) {
            let after = self.documents.get(&response.artifact_id).cloned();
            self.push_history_entry(HistoryEntry {
                artifact_id: response.artifact_id.clone(),
                action,
                before,
                after,
            });
        }
        Ok(response)
    }

    fn execute_action(
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
            "to_proto" => self.proto_snapshot(request),
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
            "add_style" => self.add_style(request),
            "get_style" => self.get_style(request),
            "describe_styles" => self.describe_styles(request),
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

    fn push_history_entry(&mut self, entry: HistoryEntry) {
        self.undo_stack.push(entry);
        self.redo_stack.clear();
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
                custom_text_styles: document.custom_text_styles.clone(),
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
        let inspect_ndjson = inspect_document(document, &args);
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

    fn proto_snapshot(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            "Generated proto snapshot".to_string(),
            snapshot_for_document(document),
        );
        response.proto_json = Some(document_to_proto(document, "to_proto")?);
        response.theme = Some(document.theme_snapshot());
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn record_patch(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: RecordPatchArgs = parse_args(&request.action, &request.args)?;
        let patch = self.normalize_patch(
            request.artifact_id.as_deref(),
            args.operations,
            &request.action,
        )?;
        let document = self.get_document(&patch.artifact_id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            patch.artifact_id.clone(),
            request.action,
            format!("Recorded patch with {} operations", patch.operations.len()),
            snapshot_for_document(document),
        );
        response.patch = Some(serde_json::to_value(&patch).map_err(|error| {
            PresentationArtifactError::InvalidArgs {
                action: "record_patch".to_string(),
                message: format!("failed to serialize patch: {error}"),
            }
        })?);
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn apply_patch(
        &mut self,
        request: PresentationArtifactRequest,
        _cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ApplyPatchArgs = parse_args(&request.action, &request.args)?;
        let patch = if let Some(patch) = args.patch {
            self.normalize_serialized_patch(request.artifact_id.as_deref(), patch, &request.action)?
        } else {
            let operations =
                args.operations
                    .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                        action: request.action.clone(),
                        message: "provide either `patch` or `operations`".to_string(),
                    })?;
            self.normalize_patch(request.artifact_id.as_deref(), operations, &request.action)?
        };
        let before = self.documents.get(&patch.artifact_id).cloned();
        let Some(before_document) = before else {
            return Err(PresentationArtifactError::UnknownArtifactId {
                action: request.action,
                artifact_id: patch.artifact_id,
            });
        };
        for operation in &patch.operations {
            let nested_request = PresentationArtifactRequest {
                artifact_id: Some(patch.artifact_id.clone()),
                action: operation.action.clone(),
                args: operation.args.clone(),
            };
            if let Err(error) = self.execute_action(nested_request, Path::new(".")) {
                self.documents
                    .insert(patch.artifact_id.clone(), before_document);
                return Err(error);
            }
        }
        let document = self
            .get_document(&patch.artifact_id, &request.action)?
            .clone();
        self.push_history_entry(HistoryEntry {
            artifact_id: patch.artifact_id.clone(),
            action: request.action.clone(),
            before: Some(before_document),
            after: Some(document.clone()),
        });
        let mut response = PresentationArtifactResponse::new(
            patch.artifact_id.clone(),
            request.action,
            format!("Applied patch with {} operations", patch.operations.len()),
            snapshot_for_document(&document),
        );
        response.patch = Some(serde_json::to_value(&patch).map_err(|error| {
            PresentationArtifactError::InvalidArgs {
                action: "apply_patch".to_string(),
                message: format!("failed to serialize patch: {error}"),
            }
        })?);
        response.active_slide_index = document.active_slide_index;
        Ok(response)
    }

    fn undo(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let position = self
            .undo_stack
            .iter()
            .rposition(|entry| {
                request
                    .artifact_id
                    .as_deref()
                    .is_none_or(|artifact_id| artifact_id == entry.artifact_id)
            })
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: request.action.clone(),
                message: "nothing to undo".to_string(),
            })?;
        let entry = self.undo_stack.remove(position);
        match &entry.before {
            Some(document) => {
                self.documents
                    .insert(entry.artifact_id.clone(), document.clone());
            }
            None => {
                self.documents.remove(&entry.artifact_id);
            }
        }
        self.redo_stack.push(entry.clone());
        Ok(response_for_document_state(
            entry.artifact_id,
            request.action,
            format!("Undid `{}`", entry.action),
            entry.before.as_ref(),
        ))
    }

    fn redo(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let position = self
            .redo_stack
            .iter()
            .rposition(|entry| {
                request
                    .artifact_id
                    .as_deref()
                    .is_none_or(|artifact_id| artifact_id == entry.artifact_id)
            })
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: request.action.clone(),
                message: "nothing to redo".to_string(),
            })?;
        let entry = self.redo_stack.remove(position);
        match &entry.after {
            Some(document) => {
                self.documents
                    .insert(entry.artifact_id.clone(), document.clone());
            }
            None => {
                self.documents.remove(&entry.artifact_id);
            }
        }
        self.undo_stack.push(entry.clone());
        Ok(response_for_document_state(
            entry.artifact_id,
            request.action,
            format!("Redid `{}`", entry.action),
            entry.after.as_ref(),
        ))
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
        let parent_layout_id = args
            .parent_layout_id
            .map(|parent_layout_ref| {
                document
                    .get_layout(&parent_layout_ref, &request.action)
                    .map(|layout| layout.layout_id.clone())
            })
            .transpose()?;
        document.layouts.push(LayoutDocument {
            layout_id: layout_id.clone(),
            name: args.name,
            kind,
            parent_layout_id,
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
            rotation: None,
            flip_horizontal: None,
            flip_vertical: None,
        });
        let layout_id = document
            .get_layout(&args.layout_id, &request.action)?
            .layout_id
            .clone();
        let layout = document
            .layouts
            .iter_mut()
            .find(|layout| layout.layout_id == layout_id)
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
        let placeholders =
            resolved_layout_placeholders(document, &layout.layout_id, &request.action)?
                .into_iter()
                .map(|resolved| resolved.definition)
                .collect::<Vec<_>>();
        let mut placeholder_elements = Vec::new();
        for placeholder in placeholders {
            placeholder_elements.push(materialize_placeholder_element(
                document.next_element_id(),
                placeholder,
                placeholder_elements.len(),
            ));
        }
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.elements.retain(|element| match element {
            PresentationElement::Text(text) => text.placeholder.is_none(),
            PresentationElement::Shape(shape) => shape.placeholder.is_none(),
            PresentationElement::Image(image) => image.placeholder.is_none(),
            _ => true,
        });
        slide.layout_id = Some(layout.layout_id.clone());
        slide.elements.extend(placeholder_elements);
        resequence_z_order(slide);
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Applied layout `{}` to slide {}",
                layout.layout_id, args.slide_index
            ),
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

    fn add_style(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let style_name = normalize_style_name(&args.name, &request.action)?;
        let mut style =
            normalize_text_style_with_document(document, &args.styling, &request.action)?;
        style.style_name = Some(style_name.clone());
        let style_record = NamedTextStyle {
            name: style_name.clone(),
            style: style.clone(),
            built_in: false,
        };
        document
            .custom_text_styles
            .insert(style_name.clone(), style);
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Added text style `{style_name}`"),
            snapshot_for_document(document),
        );
        response.resolved_record = Some(named_text_style_to_json(&style_record, "st"));
        Ok(response)
    }

    fn get_style(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: StyleNameArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let normalized_style_name = normalize_style_name(&args.name, &request.action)?;
        let named_style = document
            .named_text_styles()
            .into_iter()
            .find(|style| style.name == normalized_style_name)
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: request.action.clone(),
                message: format!("unknown text style `{}`", args.name),
            })?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Resolved text style `{}`", args.name),
            snapshot_for_document(document),
        );
        response.resolved_record = Some(named_text_style_to_json(&named_style, "st"));
        Ok(response)
    }

    fn describe_styles(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document(&artifact_id, &request.action)?;
        let mut response = PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            "Described text styles".to_string(),
            snapshot_for_document(document),
        );
        response.resolved_record = Some(serde_json::json!({
            "kind": "styleList",
            "styles": document
                .named_text_styles()
                .iter()
                .map(|style| named_text_style_to_json(style, "st"))
                .collect::<Vec<_>>(),
        }));
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
        if args.index.is_some() && args.after_slide_index.is_some() {
            return Err(PresentationArtifactError::InvalidArgs {
                action: request.action,
                message: "provide at most one of `index` or `after_slide_index`".to_string(),
            });
        }
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let index = if let Some(index) = args.index {
            to_index(index)?
        } else if let Some(after_slide_index) = args.after_slide_index {
            after_slide_index as usize + 1
        } else {
            document
                .active_slide_index
                .map(|active_slide_index| active_slide_index + 1)
                .unwrap_or(document.slides.len())
        };
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
                frame: args.position.clone().into(),
                fill,
                stroke,
                text: args.text,
                text_style,
                hyperlink: None,
                placeholder: None,
                rotation_degrees: args.rotation.or(args.position.rotation),
                flip_horizontal: args
                    .flip_horizontal
                    .or(args.position.flip_horizontal)
                    .unwrap_or(false),
                flip_vertical: args
                    .flip_vertical
                    .or(args.position.flip_vertical)
                    .unwrap_or(false),
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
                    style: LineStyle::Solid,
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
            ImageInputSource::Blob(blob) => {
                Some(load_image_payload_from_blob(&blob, &request.action)?)
            }
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
                frame: args.position.clone().into(),
                payload: image_payload,
                fit_mode,
                crop,
                rotation_degrees: args.rotation.or(args.position.rotation),
                flip_horizontal: args
                    .flip_horizontal
                    .or(args.position.flip_horizontal)
                    .unwrap_or(false),
                flip_vertical: args
                    .flip_vertical
                    .or(args.position.flip_vertical)
                    .unwrap_or(false),
                lock_aspect_ratio,
                alt_text: args.alt,
                prompt: args.prompt,
                is_placeholder,
                placeholder: None,
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
        let image_source = match (
            &args.path,
            &args.data_url,
            &args.blob,
            &args.uri,
            &args.prompt,
        ) {
            (Some(path), None, None, None, None) => ImageInputSource::Path(path.clone()),
            (None, Some(data_url), None, None, None) => ImageInputSource::DataUrl(data_url.clone()),
            (None, None, Some(blob), None, None) => ImageInputSource::Blob(blob.clone()),
            (None, None, None, Some(uri), None) => ImageInputSource::Uri(uri.clone()),
            (None, None, None, None, Some(_)) => ImageInputSource::Placeholder,
            _ => {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: request.action,
                    message:
                        "provide exactly one of `path`, `data_url`, `blob`, or `uri`, or provide `prompt` for a placeholder image"
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
            ImageInputSource::Blob(blob) => {
                Some(load_image_payload_from_blob(&blob, "replace_image")?)
            }
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
                let position_rotation = args
                    .position
                    .as_ref()
                    .and_then(|position| position.rotation);
                let position_flip_horizontal = args
                    .position
                    .as_ref()
                    .and_then(|position| position.flip_horizontal);
                let position_flip_vertical = args
                    .position
                    .as_ref()
                    .and_then(|position| position.flip_vertical);
                if let Some(position) = args.position {
                    shape.frame = apply_partial_position(shape.frame, position);
                }
                if let Some(fill) = fill {
                    shape.fill = Some(fill);
                }
                if let Some(stroke) = stroke {
                    shape.stroke = Some(stroke);
                }
                if let Some(rotation) = args.rotation.or(position_rotation) {
                    shape.rotation_degrees = Some(rotation);
                }
                if let Some(flip_horizontal) = args.flip_horizontal.or(position_flip_horizontal) {
                    shape.flip_horizontal = flip_horizontal;
                }
                if let Some(flip_vertical) = args.flip_vertical.or(position_flip_vertical) {
                    shape.flip_vertical = flip_vertical;
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
                let position_rotation = args
                    .position
                    .as_ref()
                    .and_then(|position| position.rotation);
                let position_flip_horizontal = args
                    .position
                    .as_ref()
                    .and_then(|position| position.flip_horizontal);
                let position_flip_vertical = args
                    .position
                    .as_ref()
                    .and_then(|position| position.flip_vertical);
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
                if let Some(rotation) = args.rotation.or(position_rotation) {
                    image.rotation_degrees = Some(rotation);
                }
                if let Some(flip_horizontal) = args.flip_horizontal.or(position_flip_horizontal) {
                    image.flip_horizontal = flip_horizontal;
                }
                if let Some(flip_vertical) = args.flip_vertical.or(position_flip_vertical) {
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
            proto_json: None,
            patch: None,
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

    fn normalize_patch(
        &self,
        request_artifact_id: Option<&str>,
        operations: Vec<PatchOperationInput>,
        action: &str,
    ) -> Result<PresentationPatch, PresentationArtifactError> {
        if operations.is_empty() {
            return Err(PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "`operations` must contain at least one entry".to_string(),
            });
        }
        let mut patch_artifact_id = request_artifact_id.map(str::to_owned);
        let mut normalized_operations = Vec::with_capacity(operations.len());
        for operation in operations {
            let operation_artifact_id = operation
                .artifact_id
                .or_else(|| request_artifact_id.map(str::to_owned))
                .ok_or_else(|| PresentationArtifactError::MissingArtifactId {
                    action: action.to_string(),
                })?;
            if let Some(existing_artifact_id) = &patch_artifact_id {
                if existing_artifact_id != &operation_artifact_id {
                    return Err(PresentationArtifactError::InvalidArgs {
                        action: action.to_string(),
                        message: format!(
                            "patch operations must target a single artifact, found both `{existing_artifact_id}` and `{operation_artifact_id}`"
                        ),
                    });
                }
            } else {
                patch_artifact_id = Some(operation_artifact_id.clone());
            }
            if !patch_operation_supported(&operation.action) {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: action.to_string(),
                    message: format!(
                        "patch operations do not support nested action `{}`",
                        operation.action
                    ),
                });
            }
            normalized_operations.push(PatchOperation {
                action: operation.action,
                args: operation.args,
            });
        }
        let artifact_id =
            patch_artifact_id.ok_or_else(|| PresentationArtifactError::MissingArtifactId {
                action: action.to_string(),
            })?;
        self.get_document(&artifact_id, action)?;
        Ok(PresentationPatch {
            version: 1,
            artifact_id,
            operations: normalized_operations,
        })
    }

    fn normalize_serialized_patch(
        &self,
        request_artifact_id: Option<&str>,
        patch: PresentationPatch,
        action: &str,
    ) -> Result<PresentationPatch, PresentationArtifactError> {
        if patch.version != 1 {
            return Err(PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: format!("unsupported patch version `{}`", patch.version),
            });
        }
        if patch.operations.is_empty() {
            return Err(PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "`patch.operations` must contain at least one entry".to_string(),
            });
        }
        if let Some(request_artifact_id) = request_artifact_id
            && request_artifact_id != patch.artifact_id
        {
            return Err(PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: format!(
                    "request artifact `{request_artifact_id}` does not match patch artifact `{}`",
                    patch.artifact_id
                ),
            });
        }
        for operation in &patch.operations {
            if !patch_operation_supported(&operation.action) {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: action.to_string(),
                    message: format!(
                        "patch operations do not support nested action `{}`",
                        operation.action
                    ),
                });
            }
        }
        self.get_document(&patch.artifact_id, action)?;
        Ok(patch)
    }
}
