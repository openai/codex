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
    pub fn execute_requests(
        &mut self,
        request: PresentationArtifactExecutionRequest,
        cwd: &Path,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let PresentationArtifactExecutionRequest {
            artifact_id,
            requests,
        } = request;
        let request_count = requests.len();
        let mut current_artifact_id = artifact_id;
        let mut executed_actions = Vec::with_capacity(request_count);
        let mut exported_paths = Vec::new();
        let mut last_response = None;

        for mut request in requests {
            if request.artifact_id.is_none() {
                request.artifact_id = current_artifact_id.clone();
            }
            let response = self.execute(request, cwd)?;
            current_artifact_id = Some(response.artifact_id.clone());
            exported_paths.extend(response.exported_paths.iter().cloned());
            executed_actions.push(response.action.clone());
            last_response = Some(response);
        }

        let mut response = last_response.ok_or_else(|| PresentationArtifactError::InvalidArgs {
            action: "presentation_artifact".to_string(),
            message: "request sequence must contain at least one action".to_string(),
        })?;
        if request_count > 1 {
            let final_summary = response.summary.clone();
            response.action = "batch".to_string();
            response.summary =
                format!("Executed {request_count} actions sequentially. {final_summary}");
            response.executed_actions = Some(executed_actions);
            response.exported_paths = exported_paths;
        }
        Ok(response)
    }

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
            "set_notes_rich_text" => self.set_notes_rich_text(request),
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
            "update_table_style" => self.update_table_style(request),
            "style_table_block" => self.style_table_block(request),
            "update_table_cell" => self.update_table_cell(request),
            "merge_table_cells" => self.merge_table_cells(request),
            "add_chart" => self.add_chart(request),
            "update_chart" => self.update_chart(request),
            "add_chart_series" => self.add_chart_series(request),
            "update_text" => self.update_text(request),
            "set_rich_text" => self.set_rich_text(request),
            "format_text_range" => self.format_text_range(request),
            "replace_text" => self.replace_text(request),
            "insert_text_after" => self.insert_text_after(request),
            "set_hyperlink" => self.set_hyperlink(request),
            "set_comment_author" => self.set_comment_author(request),
            "add_comment_thread" => self.add_comment_thread(request),
            "add_comment_reply" => self.add_comment_reply(request),
            "toggle_comment_reaction" => self.toggle_comment_reaction(request),
            "resolve_comment_thread" => self.resolve_comment_thread(request),
            "reopen_comment_thread" => self.reopen_comment_thread(request),
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
        let document = if let Some(document) = import_codex_metadata_document(&path)
            .map_err(|message| PresentationArtifactError::ImportFailed {
                path: path.clone(),
                message,
            })?
        {
            document
        } else {
            let imported = Presentation::from_path(&path).map_err(|error| {
                PresentationArtifactError::ImportFailed {
                    path: path.clone(),
                    message: error.to_string(),
                }
            })?;
            let mut document = PresentationDocument::from_ppt_rs(imported);
            import_pptx_images(&path, &mut document, &request.action)?;
            document
        };
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
            let slide_id = slide.slide_id.clone();
            PresentationDocument {
                artifact_id: document.artifact_id.clone(),
                name: document.name.clone(),
                slide_size: document.slide_size,
                theme: document.theme.clone(),
                custom_text_styles: document.custom_text_styles.clone(),
                layouts: Vec::new(),
                slides: vec![slide],
                active_slide_index: Some(0),
                comment_self: document.comment_self.clone(),
                comment_threads: document
                    .comment_threads
                    .iter()
                    .filter(|thread| match &thread.target {
                        CommentTarget::Slide { slide_id: target_slide_id }
                        | CommentTarget::Element { slide_id: target_slide_id, .. }
                        | CommentTarget::TextRange { slide_id: target_slide_id, .. } => {
                            target_slide_id == &slide_id
                        }
                    })
                    .cloned()
                    .collect(),
                next_slide_seq: 1,
                next_element_seq: 1,
                next_layout_seq: 1,
                next_text_range_seq: document.next_text_range_seq,
                next_comment_thread_seq: document.next_comment_thread_seq,
                next_comment_message_seq: document.next_comment_message_seq,
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
        slide.notes.rich_text = RichTextState::default();
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated notes for slide {}", args.slide_index),
            snapshot_for_document(document),
        ))
    }

    fn set_notes_rich_text(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SetRichTextArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let slide_index = args.slide_index.ok_or_else(|| PresentationArtifactError::InvalidArgs {
            action: request.action.clone(),
            message: "`slide_index` is required for notes rich text".to_string(),
        })?;
        let (text, mut rich_text) =
            normalize_rich_text_input(document, args.text, &request.action)?;
        rich_text.layout = normalize_text_layout(&args.text_layout, &request.action)?;
        let slide = document.get_slide_mut(slide_index, &request.action)?;
        slide.notes.text = text;
        slide.notes.rich_text = rich_text;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated rich notes for slide {slide_index}"),
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
        slide.notes.rich_text = RichTextState::default();
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
        slide.notes.rich_text = RichTextState::default();
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
        let text_layout = normalize_text_layout(&args.text_layout, &request.action)?;
        let element_id = document.next_element_id();
        let slide = document.get_slide_mut(args.slide_index, &request.action)?;
        slide.elements.push(PresentationElement::Text(TextElement {
            element_id: element_id.clone(),
            text: args.text,
            frame: args.position.into(),
            fill,
            style,
            hyperlink: None,
            rich_text: RichTextState {
                ranges: Vec::new(),
                layout: text_layout,
            },
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
        let text_layout = normalize_text_layout(&args.text_layout, &request.action)?;
        let rich_text = args.text.as_ref().map(|_| RichTextState {
            ranges: Vec::new(),
            layout: text_layout,
        });
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
                rich_text,
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
        let borders = parse_table_borders(document, args.borders, &request.action)?;
        let style_options = parse_table_style_options(args.style_options);
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
                style_options,
                borders,
                right_to_left: args.right_to_left.unwrap_or(false),
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
        cell.rich_text = RichTextState::default();
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

    fn update_table_style(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: UpdateTableStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let borders = parse_table_borders(document, args.borders, &request.action)?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Table(table) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not a table", args.element_id),
            });
        };
        table.style = args.style;
        if let Some(borders) = borders {
            if let Some(existing) = table.borders.as_mut() {
                if borders.outside.is_some() {
                    existing.outside = borders.outside;
                }
                if borders.inside.is_some() {
                    existing.inside = borders.inside;
                }
                if borders.top.is_some() {
                    existing.top = borders.top;
                }
                if borders.bottom.is_some() {
                    existing.bottom = borders.bottom;
                }
                if borders.left.is_some() {
                    existing.left = borders.left;
                }
                if borders.right.is_some() {
                    existing.right = borders.right;
                }
            } else {
                table.borders = Some(borders);
            }
        }
        if let Some(style_options) = args.style_options {
            if let Some(value) = style_options.header_row {
                table.style_options.header_row = value;
            }
            if let Some(value) = style_options.banded_rows {
                table.style_options.banded_rows = value;
            }
            if let Some(value) = style_options.banded_columns {
                table.style_options.banded_columns = value;
            }
            if let Some(value) = style_options.first_column {
                table.style_options.first_column = value;
            }
            if let Some(value) = style_options.last_column {
                table.style_options.last_column = value;
            }
            if let Some(value) = style_options.total_row {
                table.style_options.total_row = value;
            }
        }
        if let Some(right_to_left) = args.right_to_left {
            table.right_to_left = right_to_left;
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated table style for `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn style_table_block(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: StyleTableBlockArgs = parse_args(&request.action, &request.args)?;
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
        let borders = parse_table_borders(document, args.borders, &request.action)?;
        let alignment = args
            .alignment
            .as_deref()
            .map(|value| parse_alignment(value, &request.action))
            .transpose()?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Table(table) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not a table", args.element_id),
            });
        };
        let end_row = (args.row + args.row_count) as usize;
        let end_column = (args.column + args.column_count) as usize;
        for row_index in args.row as usize..end_row {
            if row_index >= table.rows.len() {
                break;
            }
            for column_index in args.column as usize..end_column {
                if column_index >= table.rows[row_index].len() {
                    break;
                }
                let cell = &mut table.rows[row_index][column_index];
                cell.text_style = text_style.clone();
                if let Some(fill) = background_fill.clone() {
                    cell.background_fill = Some(fill);
                }
                if let Some(alignment) = alignment {
                    cell.alignment = Some(alignment);
                }
                if let Some(borders) = borders.clone() {
                    cell.borders = Some(borders);
                }
            }
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Styled table block for `{}`", args.element_id),
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
        let series = parse_chart_series(document, args.series, &request.action)?;
        let legend_text_style =
            normalize_text_style_with_document(document, &args.legend_text_style, &request.action)?;
        let data_labels = parse_chart_data_labels(document, args.data_labels, &request.action)?;
        let chart_fill = args
            .chart_fill
            .as_deref()
            .map(|value| {
                normalize_color_with_document(document, value, &request.action, "chart_fill")
            })
            .transpose()?;
        let plot_area_fill = args
            .plot_area_fill
            .as_deref()
            .map(|value| {
                normalize_color_with_document(document, value, &request.action, "plot_area_fill")
            })
            .transpose()?;
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
                style_index: args.style_index,
                has_legend: args.has_legend.unwrap_or(false),
                legend: Some(ChartLegend {
                    position: args.legend_position,
                    text_style: legend_text_style,
                }),
                x_axis: Some(ChartAxisSpec {
                    title: args.x_axis_title,
                }),
                y_axis: Some(ChartAxisSpec {
                    title: args.y_axis_title,
                }),
                data_labels,
                chart_fill,
                plot_area_fill,
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

    fn update_chart(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: UpdateChartArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let legend_text_style =
            normalize_text_style_with_document(document, &args.legend_text_style, &request.action)?;
        let data_labels = parse_chart_data_labels(document, args.data_labels, &request.action)?;
        let chart_fill = args
            .chart_fill
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, &request.action, "chart_fill"))
            .transpose()?;
        let plot_area_fill = args
            .plot_area_fill
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, &request.action, "plot_area_fill"))
            .transpose()?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Chart(chart) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not a chart", args.element_id),
            });
        };
        if let Some(title) = args.title {
            chart.title = Some(title);
        }
        if let Some(categories) = args.categories {
            chart.categories = categories;
        }
        if let Some(style_index) = args.style_index {
            chart.style_index = Some(style_index);
        }
        if let Some(has_legend) = args.has_legend {
            chart.has_legend = has_legend;
        }
        if args.legend_position.is_some() || !text_style_is_empty(&legend_text_style) {
            chart.legend = Some(ChartLegend {
                position: args.legend_position,
                text_style: legend_text_style,
            });
        }
        if args.x_axis_title.is_some() {
            chart.x_axis = Some(ChartAxisSpec {
                title: args.x_axis_title,
            });
        }
        if args.y_axis_title.is_some() {
            chart.y_axis = Some(ChartAxisSpec {
                title: args.y_axis_title,
            });
        }
        if data_labels.is_some() {
            chart.data_labels = data_labels;
        }
        if chart_fill.is_some() {
            chart.chart_fill = chart_fill;
        }
        if plot_area_fill.is_some() {
            chart.plot_area_fill = plot_area_fill;
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated chart `{}`", args.element_id),
            snapshot_for_document(document),
        ))
    }

    fn add_chart_series(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddChartSeriesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let series = parse_chart_series(
            document,
            vec![ChartSeriesArgs {
                name: args.name,
                values: args.values,
                categories: args.categories,
                x_values: args.x_values,
                fill: args.fill,
                stroke: args.stroke,
                marker: args.marker,
                data_label_overrides: None,
            }],
            &request.action,
        )?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        let PresentationElement::Chart(chart) = element else {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: request.action,
                message: format!("element `{}` is not a chart", args.element_id),
            });
        };
        chart.series.extend(series);
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Added chart series to `{}`", args.element_id),
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
        let text_layout = normalize_text_layout(&args.text_layout, &request.action)?;
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => {
                text.text = args.text;
                if let Some(fill) = fill.clone() {
                    text.fill = Some(fill);
                }
                text.style = style;
                text.rich_text = RichTextState {
                    ranges: Vec::new(),
                    layout: text_layout,
                };
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
                shape.rich_text = Some(RichTextState {
                    ranges: Vec::new(),
                    layout: text_layout,
                });
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

    fn set_rich_text(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SetRichTextArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let (text, mut rich_text) =
            normalize_rich_text_input(document, args.text, &request.action)?;
        rich_text.layout = normalize_text_layout(&args.text_layout, &request.action)?;
        let style = normalize_text_style_with_document(document, &args.styling, &request.action)?;
        if args.notes.unwrap_or(false) {
            let slide_index = args.slide_index.ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: "`slide_index` is required for notes rich text".to_string(),
            })?;
            let slide = document.get_slide_mut(slide_index, &request.action)?;
            slide.notes.text = text;
            slide.notes.rich_text = rich_text;
            return Ok(PresentationArtifactResponse::new(
                artifact_id,
                request.action,
                format!("Updated rich text for notes on slide {slide_index}"),
                snapshot_for_document(document),
            ));
        }
        if let Some(element_id) = args.element_id {
            if let (Some(row), Some(column)) = (args.row, args.column) {
                let element = document.find_element_mut(&element_id, &request.action)?;
                let PresentationElement::Table(table) = element else {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: format!("element `{element_id}` is not a table"),
                    });
                };
                let row = row as usize;
                let column = column as usize;
                if row >= table.rows.len() || column >= table.rows[row].len() {
                    return Err(PresentationArtifactError::InvalidArgs {
                        action: request.action,
                        message: format!("cell ({row}, {column}) is out of bounds"),
                    });
                }
                let cell = &mut table.rows[row][column];
                cell.text = text;
                cell.text_style = style;
                cell.rich_text = rich_text;
                return Ok(PresentationArtifactResponse::new(
                    artifact_id,
                    request.action,
                    format!("Updated rich text for table cell ({row}, {column})"),
                    snapshot_for_document(document),
                ));
            }
            let element = document.find_element_mut(&element_id, &request.action)?;
            match element {
                PresentationElement::Text(text_element) => {
                    text_element.text = text;
                    text_element.style = style;
                    text_element.rich_text = rich_text;
                }
                PresentationElement::Shape(shape) => {
                    if shape.text.is_none() {
                        return Err(PresentationArtifactError::UnsupportedFeature {
                            action: request.action,
                            message: format!(
                                "element `{element_id}` does not contain editable text"
                            ),
                        });
                    }
                    shape.text = Some(text);
                    shape.text_style = style;
                    shape.rich_text = Some(rich_text);
                }
                other => {
                    return Err(PresentationArtifactError::UnsupportedFeature {
                        action: request.action,
                        message: format!(
                            "element `{element_id}` is `{}`; only text-bearing elements support `set_rich_text`",
                            other.kind()
                        ),
                    });
                }
            }
            return Ok(PresentationArtifactResponse::new(
                artifact_id,
                request.action,
                format!("Updated rich text for element `{element_id}`"),
                snapshot_for_document(document),
            ));
        }
        Err(PresentationArtifactError::InvalidArgs {
            action: request.action,
            message: "provide `element_id` or `slide_index` with `notes: true`".to_string(),
        })
    }

    fn format_text_range(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: FormatTextRangeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let style = normalize_text_style_with_document(document, &args.styling, &request.action)?;
        let hyperlink = args
            .link
            .as_ref()
            .map(|link| parse_rich_text_link(link, &request.action))
            .transpose()?;
        let layout = normalize_text_layout(&args.text_layout, &request.action)?;
        let apply_annotation =
            |range_id: String,
             text: &str,
             rich_text: &mut RichTextState|
             -> Result<(), PresentationArtifactError> {
                let (start_cp, length, _) = resolve_text_range_selector(
                    text,
                    args.query.as_deref(),
                    args.occurrence,
                    args.start_cp,
                    args.length,
                    &request.action,
                )?;
                rich_text.ranges.push(TextRangeAnnotation {
                    range_id,
                    start_cp,
                    length,
                    style: style.clone(),
                    hyperlink: hyperlink.clone(),
                    spacing_before: args.spacing_before.map(|value| value * 100),
                    spacing_after: args.spacing_after.map(|value| value * 100),
                    line_spacing: args.line_spacing,
                });
                if layout.insets.is_some()
                    || layout.wrap.is_some()
                    || layout.auto_fit.is_some()
                    || layout.vertical_alignment.is_some()
                {
                    rich_text.layout = layout.clone();
                }
                Ok(())
            };
        if args.notes.unwrap_or(false) {
            let slide_index = args.slide_index.ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: "`slide_index` is required for notes text ranges".to_string(),
            })?;
            let range_id = document.next_text_range_id();
            let slide = document.get_slide_mut(slide_index, &request.action)?;
            apply_annotation(range_id, &slide.notes.text, &mut slide.notes.rich_text)?;
            return Ok(PresentationArtifactResponse::new(
                artifact_id,
                request.action,
                format!("Formatted notes text range on slide {slide_index}"),
                snapshot_for_document(document),
            ));
        }
        let element_id = args.element_id.clone().ok_or_else(|| PresentationArtifactError::InvalidArgs {
            action: request.action.clone(),
            message: "`element_id` is required unless formatting notes".to_string(),
        })?;
        if let (Some(row), Some(column)) = (args.row, args.column) {
            let range_id = document.next_text_range_id();
            let element = document.find_element_mut(&element_id, &request.action)?;
            let PresentationElement::Table(table) = element else {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: request.action,
                    message: format!("element `{element_id}` is not a table"),
                });
            };
            let row = row as usize;
            let column = column as usize;
            if row >= table.rows.len() || column >= table.rows[row].len() {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: request.action,
                    message: format!("cell ({row}, {column}) is out of bounds"),
                });
            }
            let cell = &mut table.rows[row][column];
            apply_annotation(range_id, &cell.text, &mut cell.rich_text)?;
            return Ok(PresentationArtifactResponse::new(
                artifact_id,
                request.action,
                format!("Formatted table cell text range ({row}, {column})"),
                snapshot_for_document(document),
            ));
        }
        let range_id = document.next_text_range_id();
        let element = document.find_element_mut(&element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => {
                apply_annotation(range_id, &text.text, &mut text.rich_text)?;
            }
            PresentationElement::Shape(shape) => {
                let text_value = shape.text.as_ref().ok_or_else(|| {
                    PresentationArtifactError::UnsupportedFeature {
                        action: request.action.clone(),
                        message: format!("element `{element_id}` does not contain editable text"),
                    }
                })?;
                let rich_text = shape.rich_text.get_or_insert_with(RichTextState::default);
                apply_annotation(range_id, text_value, rich_text)?;
            }
            other => {
                return Err(PresentationArtifactError::UnsupportedFeature {
                    action: request.action,
                    message: format!(
                        "element `{element_id}` is `{}`; only text-bearing elements support `format_text_range`",
                        other.kind()
                    ),
                });
            }
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Formatted text range on element `{element_id}`"),
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

    fn set_comment_author(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: SetCommentAuthorArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        document.comment_self = Some(CommentAuthorProfile {
            display_name: args.display_name,
            initials: args.initials,
            email: args.email,
        });
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            "Updated comment author".to_string(),
            snapshot_for_document(document),
        ))
    }

    fn add_comment_thread(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddCommentThreadArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let author = document.comment_self.clone().ok_or_else(|| {
            PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: "set a comment author first with `set_comment_author`".to_string(),
            }
        })?;
        let target = if let Some(slide_index) = args.slide_index {
            let slide = document.get_slide_mut(slide_index, &request.action)?;
            if let Some(element_id) = args.element_id {
                if args.query.is_some() || args.start_cp.is_some() {
                    let (text, _rich_text) = lookup_text_target(
                        slide,
                        &element_id,
                        None,
                        None,
                        &request.action,
                    )?;
                    let (start_cp, length, context) = resolve_text_range_selector(
                        text,
                        args.query.as_deref(),
                        args.occurrence,
                        args.start_cp,
                        args.length,
                        &request.action,
                    )?;
                    CommentTarget::TextRange {
                        slide_id: slide.slide_id.clone(),
                        element_id: normalize_element_lookup_id(&element_id).to_string(),
                        start_cp,
                        length,
                        context,
                    }
                } else {
                    CommentTarget::Element {
                        slide_id: slide.slide_id.clone(),
                        element_id: normalize_element_lookup_id(&element_id).to_string(),
                    }
                }
            } else {
                CommentTarget::Slide {
                    slide_id: slide.slide_id.clone(),
                }
            }
        } else {
            return Err(PresentationArtifactError::InvalidArgs {
                action: request.action,
                message: "`slide_index` is required for comment threads".to_string(),
            });
        };
        let thread_id = document.next_comment_thread_id();
        let message_id = document.next_comment_message_id();
        document.comment_threads.push(CommentThread {
            thread_id: thread_id.clone(),
            target,
            position: args.position.map(|position| CommentPosition {
                x: position.x,
                y: position.y,
            }),
            status: CommentThreadStatus::Active,
            messages: vec![CommentMessage {
                message_id,
                author,
                text: args.text,
                created_at: "2026-03-03T00:00:00Z".to_string(),
                reactions: Vec::new(),
            }],
        });
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Added comment thread `{thread_id}`"),
            snapshot_for_document(document),
        ))
    }

    fn add_comment_reply(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: AddCommentReplyArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let author = document.comment_self.clone().ok_or_else(|| {
            PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: "set a comment author first with `set_comment_author`".to_string(),
            }
        })?;
        let message_id = document.next_comment_message_id();
        let thread = document
            .comment_threads
            .iter_mut()
            .find(|thread| thread.thread_id == args.thread_id)
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("unknown comment thread `{}`", args.thread_id),
            })?;
        thread.messages.push(CommentMessage {
            message_id,
            author,
            text: args.text,
            created_at: "2026-03-03T00:00:00Z".to_string(),
            reactions: Vec::new(),
        });
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Added reply to `{}`", args.thread_id),
            snapshot_for_document(document),
        ))
    }

    fn toggle_comment_reaction(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: ToggleCommentReactionArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let thread = document
            .comment_threads
            .iter_mut()
            .find(|thread| thread.thread_id == args.thread_id)
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("unknown comment thread `{}`", args.thread_id),
            })?;
        let target_message_id = args
            .message_id
            .clone()
            .or_else(|| thread.messages.last().map(|message| message.message_id.clone()))
            .unwrap_or_default();
        let message = thread
            .messages
            .iter_mut()
            .find(|message| message.message_id == target_message_id)
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("unknown comment message `{target_message_id}`"),
            })?;
        if let Some(index) = message.reactions.iter().position(|emoji| emoji == &args.emoji) {
            message.reactions.remove(index);
        } else {
            message.reactions.push(args.emoji.clone());
        }
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Toggled reaction on `{}`", args.thread_id),
            snapshot_for_document(document),
        ))
    }

    fn resolve_comment_thread(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: CommentThreadIdArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let thread = document
            .comment_threads
            .iter_mut()
            .find(|thread| thread.thread_id == args.thread_id)
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("unknown comment thread `{}`", args.thread_id),
            })?;
        thread.status = CommentThreadStatus::Resolved;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Resolved `{}`", args.thread_id),
            snapshot_for_document(document),
        ))
    }

    fn reopen_comment_thread(
        &mut self,
        request: PresentationArtifactRequest,
    ) -> Result<PresentationArtifactResponse, PresentationArtifactError> {
        let args: CommentThreadIdArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let document = self.get_document_mut(&artifact_id, &request.action)?;
        let thread = document
            .comment_threads
            .iter_mut()
            .find(|thread| thread.thread_id == args.thread_id)
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("unknown comment thread `{}`", args.thread_id),
            })?;
        thread.status = CommentThreadStatus::Active;
        Ok(PresentationArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Reopened `{}`", args.thread_id),
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
        let text_layout = normalize_text_layout(&args.text_layout, &request.action)?;
        let has_text_layout = text_layout.insets.is_some()
            || text_layout.wrap.is_some()
            || text_layout.auto_fit.is_some()
            || text_layout.vertical_alignment.is_some();
        let element = document.find_element_mut(&args.element_id, &request.action)?;
        match element {
            PresentationElement::Text(text) => {
                if let Some(position) = args.position {
                    text.frame = apply_partial_position(text.frame, position);
                }
                if let Some(fill) = fill.clone() {
                    text.fill = Some(fill);
                }
                if has_text_layout {
                    text.rich_text.layout = text_layout;
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
                if shape.text.is_some() && has_text_layout {
                    shape.rich_text = Some(RichTextState {
                        ranges: shape.rich_text.take().unwrap_or_default().ranges,
                        layout: text_layout,
                    });
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
            executed_actions: None,
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

fn lookup_text_target<'a>(
    slide: &'a PresentationSlide,
    element_id: &str,
    _row: Option<u32>,
    _column: Option<u32>,
    action: &str,
) -> Result<(&'a str, Option<&'a RichTextState>), PresentationArtifactError> {
    let normalized_element_id = normalize_element_lookup_id(element_id);
    let element = slide
        .elements
        .iter()
        .find(|element| element.element_id() == normalized_element_id)
        .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("unknown element `{element_id}` on slide `{}`", slide.slide_id),
        })?;
    match element {
        PresentationElement::Text(text) => Ok((&text.text, Some(&text.rich_text))),
        PresentationElement::Shape(shape) => shape
            .text
            .as_deref()
            .map(|text| (text, shape.rich_text.as_ref()))
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("element `{element_id}` does not contain editable text"),
            }),
        PresentationElement::Connector(_)
        | PresentationElement::Image(_)
        | PresentationElement::Table(_)
        | PresentationElement::Chart(_) => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!(
                "element `{element_id}` does not support text-range comments"
            ),
        }),
    }
}
