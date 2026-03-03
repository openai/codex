use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetCellRangeRef;
use crate::SpreadsheetCellRef;
use crate::SpreadsheetCellValue;
use crate::SpreadsheetCitation;
use crate::SpreadsheetRangeView;
use crate::SpreadsheetSheetSummary;
use crate::SpreadsheetSummary;

#[derive(Debug, Clone, Deserialize)]
pub struct SpreadsheetArtifactRequest {
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

impl SpreadsheetArtifactRequest {
    pub fn required_path_accesses(
        &self,
        cwd: &Path,
    ) -> Result<Vec<PathAccessRequirement>, SpreadsheetArtifactError> {
        let access = match self.action.as_str() {
            "import_xlsx" | "load" | "read" => {
                let args: PathArgs = parse_args(&self.action, &self.args)?;
                vec![PathAccessRequirement {
                    action: self.action.clone(),
                    kind: PathAccessKind::Read,
                    path: resolve_path(cwd, &args.path),
                }]
            }
            "export_xlsx" => {
                let args: PathArgs = parse_args(&self.action, &self.args)?;
                vec![PathAccessRequirement {
                    action: self.action.clone(),
                    kind: PathAccessKind::Write,
                    path: resolve_path(cwd, &args.path),
                }]
            }
            "save" => {
                let args: SaveArgs = parse_args(&self.action, &self.args)?;
                vec![PathAccessRequirement {
                    action: self.action.clone(),
                    kind: PathAccessKind::Write,
                    path: resolve_path(cwd, &args.path),
                }]
            }
            "render_workbook" | "render_sheet" | "render_range" => {
                let args: RenderArgs = parse_args(&self.action, &self.args)?;
                args.output_path
                    .map(|path| {
                        vec![PathAccessRequirement {
                            action: self.action.clone(),
                            kind: PathAccessKind::Write,
                            path: resolve_path(cwd, &path),
                        }]
                    })
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        };
        Ok(access)
    }
}

#[derive(Debug, Default)]
pub struct SpreadsheetArtifactManager {
    documents: HashMap<String, SpreadsheetArtifact>,
}

impl SpreadsheetArtifactManager {
    pub fn execute(
        &mut self,
        request: SpreadsheetArtifactRequest,
        cwd: &Path,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        match request.action.as_str() {
            "create" => self.create(request),
            "import_xlsx" | "load" | "read" => self.import_xlsx(request, cwd),
            "export_xlsx" => self.export_xlsx(request, cwd),
            "save" => self.save(request, cwd),
            "render_workbook" => self.render_workbook(request, cwd),
            "render_sheet" => self.render_sheet(request, cwd),
            "render_range" => self.render_range(request, cwd),
            "get_summary" => self.get_summary(request),
            "list_sheets" => self.list_sheets(request),
            "get_sheet" => self.get_sheet(request),
            "inspect" => self.inspect(request),
            "list_charts" => self.list_charts(request),
            "get_chart" => self.get_chart(request),
            "create_chart" => self.create_chart(request),
            "add_chart_series" => self.add_chart_series(request),
            "set_chart_properties" => self.set_chart_properties(request),
            "delete_chart" => self.delete_chart(request),
            "list_tables" => self.list_tables(request),
            "get_table" => self.get_table(request),
            "create_table" => self.create_table(request),
            "set_table_style" => self.set_table_style(request),
            "clear_table_filters" => self.clear_table_filters(request),
            "reapply_table_filters" => self.reapply_table_filters(request),
            "rename_table_column" => self.rename_table_column(request),
            "set_table_column_totals" => self.set_table_column_totals(request),
            "delete_table" => self.delete_table(request),
            "list_conditional_formats" => self.list_conditional_formats(request),
            "add_conditional_format" => self.add_conditional_format(request),
            "delete_conditional_format" => self.delete_conditional_format(request),
            "list_pivot_tables" => self.list_pivot_tables(request),
            "get_pivot_table" => self.get_pivot_table(request),
            "create_sheet" => self.create_sheet(request),
            "rename_sheet" => self.rename_sheet(request),
            "delete_sheet" => self.delete_sheet(request),
            "set_sheet_properties" => self.set_sheet_properties(request),
            "set_column_widths" => self.set_column_widths(request),
            "set_column_widths_bulk" => self.set_column_widths_bulk(request),
            "set_row_height" => self.set_row_height(request),
            "set_row_heights" => self.set_row_heights(request),
            "set_row_heights_bulk" => self.set_row_heights_bulk(request),
            "get_row_height" => self.get_row_height(request),
            "cleanup_and_validate_sheet" => self.cleanup_and_validate_sheet(request),
            "create_text_style" => self.create_text_style(request),
            "get_text_style" => self.get_text_style(request),
            "create_fill" => self.create_fill(request),
            "get_fill" => self.get_fill(request),
            "create_border" => self.create_border(request),
            "get_border" => self.get_border(request),
            "create_number_format" => self.create_number_format(request),
            "get_number_format" => self.get_number_format(request),
            "create_cell_format" => self.create_cell_format(request),
            "get_cell_format" => self.get_cell_format(request),
            "create_differential_format" => self.create_differential_format(request),
            "get_differential_format" => self.get_differential_format(request),
            "get_cell_format_summary" => self.get_cell_format_summary(request),
            "get_range_format_summary" => self.get_range_format_summary(request),
            "get_reference" => self.get_reference(request),
            "get_cell" => self.get_cell(request),
            "get_cell_by_indices" => self.get_cell_by_indices(request),
            "get_cell_field" => self.get_cell_field(request),
            "get_cell_field_by_indices" => self.get_cell_field_by_indices(request),
            "get_range" => self.get_range(request),
            "set_cell_value" => self.set_cell_value(request),
            "set_range_value" => self.set_range_value(request),
            "set_range_values" => self.set_range_values(request),
            "set_cell_formula" => self.set_cell_formula(request),
            "set_range_formula" => self.set_range_formula(request),
            "set_range_formulas" => self.set_range_formulas(request),
            "set_cell_style" => self.set_cell_style(request),
            "set_range_style" => self.set_range_style(request),
            "clear_range" => self.clear_range(request),
            "merge_range" => self.merge_range(request),
            "unmerge_range" => self.unmerge_range(request),
            "cite_cell" => self.cite_cell(request),
            "cite_range" => self.cite_range(request),
            "calculate" | "recalculate" => self.calculate(request),
            "serialize_dict" => self.serialize_dict(request),
            "serialize_json" => self.serialize_json(request),
            "serialize_bytes" => self.serialize_bytes(request),
            "deserialize_dict" => self.deserialize_dict(request),
            "deserialize_json" => self.deserialize_json(request),
            "deserialize_bytes" => self.deserialize_bytes(request),
            "delete_artifact" => self.delete_artifact(request),
            other => Err(SpreadsheetArtifactError::UnknownAction(other.to_string())),
        }
    }

    fn create(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateArgs = parse_args(&request.action, &request.args)?;
        let mut artifact = SpreadsheetArtifact::new(args.name);
        if let Some(auto_recalculate) = args.auto_recalculate {
            artifact.auto_recalculate = auto_recalculate;
        }
        let artifact_id = artifact.artifact_id.clone();
        let summary = format!("Created spreadsheet artifact `{artifact_id}`");
        let snapshot = snapshot_for_artifact(&artifact);
        self.documents.insert(artifact_id.clone(), artifact);
        Ok(SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            summary,
            snapshot,
        ))
    }

    fn import_xlsx(
        &mut self,
        request: SpreadsheetArtifactRequest,
        cwd: &Path,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: PathArgs = parse_args(&request.action, &request.args)?;
        let path = resolve_path(cwd, &args.path);
        let artifact = SpreadsheetArtifact::from_source_file(&path, None)?;
        let artifact_id = artifact.artifact_id.clone();
        let snapshot = snapshot_for_artifact(&artifact);
        let summary = format!(
            "Imported `{}` as spreadsheet artifact `{artifact_id}` with {} sheets",
            path.display(),
            artifact.sheets.len()
        );
        self.documents.insert(artifact_id.clone(), artifact);
        Ok(SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            summary,
            snapshot,
        ))
    }

    fn export_xlsx(
        &mut self,
        request: SpreadsheetArtifactRequest,
        cwd: &Path,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: PathArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let path = resolve_path(cwd, &args.path);
        let exported = artifact.export(&path)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Exported spreadsheet to `{}`", exported.display()),
            snapshot_for_artifact(artifact),
        );
        response.exported_paths.push(exported);
        response.workbook_summary = Some(artifact.summary());
        response.sheet_list = Some(
            artifact
                .sheets
                .iter()
                .map(super::model::SpreadsheetSheet::summary)
                .collect(),
        );
        Ok(response)
    }

    fn save(
        &mut self,
        request: SpreadsheetArtifactRequest,
        cwd: &Path,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SaveArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let path = resolve_path(cwd, &args.path);
        let exported = artifact.save(&path, args.file_type.as_deref())?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Saved spreadsheet to `{}`", exported.display()),
            snapshot_for_artifact(artifact),
        );
        response.exported_paths.push(exported);
        response.workbook_summary = Some(artifact.summary());
        Ok(response)
    }

    fn get_summary(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Spreadsheet has {} sheets and {} bytes of serialized state",
                artifact.sheets.len(),
                artifact.summary().size_bytes
            ),
            snapshot_for_artifact(artifact),
        );
        response.workbook_summary = Some(artifact.summary());
        response.sheet_list = Some(
            artifact
                .sheets
                .iter()
                .map(super::model::SpreadsheetSheet::summary)
                .collect(),
        );
        Ok(response)
    }

    fn render_workbook(
        &mut self,
        request: SpreadsheetArtifactRequest,
        cwd: &Path,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: RenderArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let rendered = artifact.render_workbook_previews(cwd, &render_options_from_args(args)?)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Rendered workbook to {} preview files", rendered.len()),
            snapshot_for_artifact(artifact),
        );
        response.exported_paths = rendered.into_iter().map(|output| output.path).collect();
        Ok(response)
    }

    fn render_sheet(
        &mut self,
        request: SpreadsheetArtifactRequest,
        cwd: &Path,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: RenderArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let rendered =
            artifact.render_sheet_preview(cwd, sheet, &render_options_from_args(args)?)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Rendered sheet `{}`", sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.exported_paths.push(rendered.path);
        response.rendered_html = Some(rendered.html);
        response.rendered_text = Some(sheet.to_rendered_text(None));
        Ok(response)
    }

    fn render_range(
        &mut self,
        request: SpreadsheetArtifactRequest,
        cwd: &Path,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: RenderArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range_text =
            args.range
                .clone()
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: request.action.clone(),
                    message: "range is required".to_string(),
                })?;
        let range = CellRange::parse(&range_text)?;
        let rendered =
            artifact.render_range_preview(cwd, sheet, &range, &render_options_from_args(args)?)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Rendered range `{range_text}` from `{}`", sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        response.exported_paths.push(rendered.path);
        response.rendered_html = Some(rendered.html);
        response.rendered_text = Some(sheet.to_rendered_text(Some(&range)));
        Ok(response)
    }

    fn list_sheets(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Listed {} sheets", artifact.sheets.len()),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(
            artifact
                .sheets
                .iter()
                .map(super::model::SpreadsheetSheet::summary)
                .collect(),
        );
        Ok(response)
    }

    fn get_sheet(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SheetLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range = if let Some(range) = args.range.as_deref() {
            Some(CellRange::parse(range)?)
        } else {
            sheet.minimum_range()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved sheet `{}`", sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet.summary()]);
        response.sheet_ref = Some(sheet_reference(sheet));
        response.range_ref = range
            .as_ref()
            .map(|entry| SpreadsheetCellRangeRef::new(sheet.name.clone(), entry));
        response.rendered_text = Some(sheet.to_rendered_text(range.as_ref()));
        response.range = range.as_ref().map(|entry| sheet.get_range_view(entry));
        response.top_left_style_index = range
            .as_ref()
            .map(|entry| sheet.top_left_style_index(entry));
        response.range_format = range.as_ref().map(|entry| sheet.range_format(entry));
        response.cell_format_summary = response
            .top_left_style_index
            .and_then(|style_index| artifact.cell_format_summary(style_index));
        response.serialized_dict = Some(sheet.to_dict()?);
        Ok(response)
    }

    fn inspect(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SheetLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let inspected = if args.sheet_name.is_none() && args.sheet_index.is_none() {
            artifact.to_dict()?
        } else {
            let sheet = artifact.sheet_lookup(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            serde_json::to_value(sheet).map_err(|error| {
                SpreadsheetArtifactError::Serialization {
                    message: error.to_string(),
                }
            })?
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Generated inspection snapshot".to_string(),
            snapshot_for_artifact(artifact),
        );
        response.serialized_dict = Some(inspected);
        Ok(response)
    }

    fn create_sheet(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateSheetArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let name = args.name.clone();
        artifact.create_sheet(args.name)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created sheet `{name}`"),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(
            artifact
                .sheets
                .iter()
                .map(super::model::SpreadsheetSheet::summary)
                .collect(),
        );
        Ok(response)
    }

    fn list_charts(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SheetLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range = args.range.as_deref().map(CellRange::parse).transpose()?;
        let charts = sheet.list_charts(range.as_ref())?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Listed {} charts on `{}`", charts.len(), sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.range_ref = range
            .as_ref()
            .map(|entry| SpreadsheetCellRangeRef::new(sheet.name.clone(), entry));
        response.chart_list = Some(charts);
        Ok(response)
    }

    fn get_chart(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: ChartLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let chart = sheet
            .get_chart(&request.action, chart_lookup_from_args(&args))?
            .clone();
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved chart `{}` from `{}`", chart.id, sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.chart_list = Some(vec![chart]);
        Ok(response)
    }

    fn create_chart(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateChartArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        if let Some(source_sheet_name) = args.source_sheet_name.as_deref() {
            artifact.sheet_lookup(&request.action, Some(source_sheet_name), None)?;
        }
        let source_range = CellRange::parse(&args.source_range)?;
        let chart = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let chart_id = sheet.create_chart(
                &request.action,
                args.chart_type,
                args.source_sheet_name.or_else(|| Some(sheet.name.clone())),
                &source_range,
                args.options.unwrap_or_default(),
            )?;
            sheet
                .get_chart(
                    &request.action,
                    crate::SpreadsheetChartLookup {
                        id: Some(chart_id),
                        index: None,
                    },
                )?
                .clone()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created chart `{}`", chart.id),
            snapshot_for_artifact(artifact),
        );
        response.chart_list = Some(vec![chart]);
        Ok(response)
    }

    fn add_chart_series(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: AddChartSeriesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let chart = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let series_id = sheet.add_chart_series(
                &request.action,
                crate::SpreadsheetChartLookup {
                    id: args.chart_id,
                    index: args.chart_index.map(|value| value as usize),
                },
                args.series,
            )?;
            let chart = sheet.get_chart(
                &request.action,
                crate::SpreadsheetChartLookup {
                    id: args.chart_id.or(Some(series_id).and(None)),
                    index: args.chart_index.map(|value| value as usize),
                },
            );
            match chart {
                Ok(chart) => chart.clone(),
                Err(_) => sheet
                    .get_chart(
                        &request.action,
                        crate::SpreadsheetChartLookup {
                            id: None,
                            index: args.chart_index.map(|value| value as usize),
                        },
                    )?
                    .clone(),
            }
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Added chart series to chart `{}`", chart.id),
            snapshot_for_artifact(artifact),
        );
        response.chart_list = Some(vec![chart]);
        Ok(response)
    }

    fn set_chart_properties(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetChartPropertiesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let chart = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.lookup.sheet_name.as_deref(),
                args.lookup.sheet_index.map(|value| value as usize),
            )?;
            let lookup = chart_lookup_from_args(&args.lookup);
            sheet.set_chart_properties(&request.action, lookup.clone(), args.properties)?;
            sheet.get_chart(&request.action, lookup)?.clone()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated chart `{}`", chart.id),
            snapshot_for_artifact(artifact),
        );
        response.chart_list = Some(vec![chart]);
        Ok(response)
    }

    fn delete_chart(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: ChartLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_name = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let name = sheet.name.clone();
            sheet.delete_chart(&request.action, chart_lookup_from_args(&args))?;
            name
        };
        let action = request.action.clone();
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Deleted chart from `{sheet_name}`"),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![
            artifact
                .sheet_lookup(&action, Some(&sheet_name), None)?
                .summary(),
        ]);
        Ok(response)
    }

    fn list_tables(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SheetLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range = args.range.as_deref().map(CellRange::parse).transpose()?;
        let tables = sheet.list_tables(range.as_ref())?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Listed {} tables on `{}`", tables.len(), sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.range_ref = range
            .as_ref()
            .map(|entry| SpreadsheetCellRangeRef::new(sheet.name.clone(), entry));
        response.table_list = Some(tables);
        Ok(response)
    }

    fn get_table(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: TableLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let table = sheet.get_table_view(&request.action, table_lookup_from_args(&args))?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved table `{}` from `{}`", table.name, sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.table_list = Some(vec![table]);
        Ok(response)
    }

    fn create_table(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateTableArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        let table = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let table_id = sheet.create_table(&request.action, &range, args.options)?;
            sheet.get_table_view(
                &request.action,
                crate::SpreadsheetTableLookup {
                    name: None,
                    display_name: None,
                    id: Some(table_id),
                },
            )?
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created table `{}`", table.name),
            snapshot_for_artifact(artifact),
        );
        response.table_list = Some(vec![table]);
        Ok(response)
    }

    fn set_table_style(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetTableStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let table = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.lookup.sheet_name.as_deref(),
                args.lookup.sheet_index.map(|value| value as usize),
            )?;
            let lookup = table_lookup_from_args(&args.lookup);
            sheet.set_table_style(&request.action, lookup.clone(), args.options)?;
            sheet.get_table_view(&request.action, lookup)?
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated table style for `{}`", table.name),
            snapshot_for_artifact(artifact),
        );
        response.table_list = Some(vec![table]);
        Ok(response)
    }

    fn clear_table_filters(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: TableLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let table = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let lookup = table_lookup_from_args(&args);
            sheet.clear_table_filters(&request.action, lookup.clone())?;
            sheet.get_table_view(&request.action, lookup)?
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Cleared table filters for `{}`", table.name),
            snapshot_for_artifact(artifact),
        );
        response.table_list = Some(vec![table]);
        Ok(response)
    }

    fn reapply_table_filters(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: TableLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let table = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let lookup = table_lookup_from_args(&args);
            sheet.reapply_table_filters(&request.action, lookup.clone())?;
            sheet.get_table_view(&request.action, lookup)?
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Reapplied table filters for `{}`", table.name),
            snapshot_for_artifact(artifact),
        );
        response.table_list = Some(vec![table]);
        Ok(response)
    }

    fn rename_table_column(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: RenameTableColumnArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let column = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.lookup.sheet_name.as_deref(),
                args.lookup.sheet_index.map(|value| value as usize),
            )?;
            sheet.rename_table_column(
                &request.action,
                table_lookup_from_args(&args.lookup),
                args.column_id,
                args.column_name.as_deref(),
                args.new_name,
            )?
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Renamed table column to `{}`", column.name),
            snapshot_for_artifact(artifact),
        );
        response.serialized_dict = Some(to_serialized_value(column)?);
        Ok(response)
    }

    fn set_table_column_totals(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetTableColumnTotalsArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let column = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.lookup.sheet_name.as_deref(),
                args.lookup.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_table_column_totals(
                &request.action,
                table_lookup_from_args(&args.lookup),
                args.column_id,
                args.column_name.as_deref(),
                args.totals_row_label,
                args.totals_row_function,
            )?
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated totals metadata for column `{}`", column.name),
            snapshot_for_artifact(artifact),
        );
        response.serialized_dict = Some(to_serialized_value(column)?);
        Ok(response)
    }

    fn delete_table(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: TableLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_name = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let name = sheet.name.clone();
            sheet.delete_table(&request.action, table_lookup_from_args(&args))?;
            name
        };
        let action = request.action.clone();
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Deleted table from `{sheet_name}`"),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![
            artifact
                .sheet_lookup(&action, Some(&sheet_name), None)?
                .summary(),
        ]);
        Ok(response)
    }

    fn list_conditional_formats(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SheetLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range = args.range.as_deref().map(CellRange::parse).transpose()?;
        let formats = sheet.list_conditional_formats(range.as_ref());
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Listed {} conditional formats on `{}`",
                formats.len(),
                sheet.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.range_ref = range
            .as_ref()
            .map(|entry| SpreadsheetCellRangeRef::new(sheet.name.clone(), entry));
        response.conditional_format_list = Some(formats);
        Ok(response)
    }

    fn add_conditional_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: AddConditionalFormatArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_name = artifact
            .sheet_lookup(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?
            .name
            .clone();
        let format_id =
            artifact.add_conditional_format(&request.action, &sheet_name, args.format)?;
        let format = artifact
            .sheet_lookup(&request.action, Some(&sheet_name), None)?
            .conditional_formats
            .iter()
            .find(|entry| entry.id == format_id)
            .cloned()
            .ok_or_else(|| SpreadsheetArtifactError::Serialization {
                message: format!("created conditional format `{format_id}` was not available"),
            })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created conditional format `{format_id}`"),
            snapshot_for_artifact(artifact),
        );
        response.conditional_format_list = Some(vec![format]);
        Ok(response)
    }

    fn delete_conditional_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: DeleteConditionalFormatArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_name = artifact
            .sheet_lookup(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?
            .name
            .clone();
        artifact.delete_conditional_format(&request.action, &sheet_name, args.id)?;
        let action = request.action.clone();
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Deleted conditional format `{}`", args.id),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![
            artifact
                .sheet_lookup(&action, Some(&sheet_name), None)?
                .summary(),
        ]);
        Ok(response)
    }

    fn list_pivot_tables(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SheetLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range = args.range.as_deref().map(CellRange::parse).transpose()?;
        let pivots = sheet.list_pivot_tables(range.as_ref())?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Listed {} pivot tables on `{}`", pivots.len(), sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.range_ref = range
            .as_ref()
            .map(|entry| SpreadsheetCellRangeRef::new(sheet.name.clone(), entry));
        response.pivot_table_list = Some(pivots);
        Ok(response)
    }

    fn get_pivot_table(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: PivotTableLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let pivot = sheet
            .get_pivot_table(&request.action, pivot_lookup_from_args(&args))?
            .clone();
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Retrieved pivot table `{}` from `{}`",
                pivot.name, sheet.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.pivot_table_list = Some(vec![pivot]);
        Ok(response)
    }

    fn rename_sheet(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: RenameSheetArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let new_name = args.new_name.clone();
        artifact.rename_sheet(
            args.new_name,
            args.old_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Renamed sheet to `{new_name}`"),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(
            artifact
                .sheets
                .iter()
                .map(super::model::SpreadsheetSheet::summary)
                .collect(),
        );
        Ok(response)
    }

    fn delete_sheet(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: DeleteSheetArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        artifact.delete_sheet(
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Deleted sheet".to_string(),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(
            artifact
                .sheets
                .iter()
                .map(super::model::SpreadsheetSheet::summary)
                .collect(),
        );
        Ok(response)
    }

    fn set_sheet_properties(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetSheetPropertiesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_summary = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            if let Some(default_row_height) = args.default_row_height {
                sheet.default_row_height = Some(default_row_height);
            }
            if let Some(default_column_width) = args.default_column_width {
                sheet.default_column_width = Some(default_column_width);
            }
            if let Some(show_grid_lines) = args.show_grid_lines {
                sheet.show_grid_lines = show_grid_lines;
            }
            sheet.summary()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Updated sheet `{}` properties", sheet_summary.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet_summary]);
        Ok(response)
    }

    fn set_column_widths(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetColumnWidthsArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_summary = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_column_widths(&args.reference, args.width)?;
            sheet.summary()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Updated column widths `{}` on `{}`",
                args.reference, sheet_summary.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet_summary]);
        Ok(response)
    }

    fn set_column_widths_bulk(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetColumnWidthsBulkArgs = parse_args(&request.action, &request.args)?;
        let width_count = args.widths.len();
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_summary = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_column_widths_bulk(&args.widths)?;
            sheet.summary()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Updated {width_count} column width references on `{}`",
                sheet_summary.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet_summary]);
        Ok(response)
    }

    fn set_row_height(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetRowHeightArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let (sheet_summary, row_height) = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_row_height(args.row_index, args.height)?;
            (sheet.summary(), sheet.get_row_height(args.row_index))
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Updated row height for row {} on `{}`",
                args.row_index, sheet_summary.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet_summary]);
        response.row_height = row_height;
        Ok(response)
    }

    fn set_row_heights(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetRowHeightsArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let start = args.start_row_index.min(args.end_row_index);
        let end = args.start_row_index.max(args.end_row_index);
        let sheet_summary = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_row_heights(args.start_row_index, args.end_row_index, args.height)?;
            sheet.summary()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Updated row heights {start}:{end} on `{}`",
                sheet_summary.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet_summary]);
        Ok(response)
    }

    fn set_row_heights_bulk(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetRowHeightsBulkArgs = parse_args(&request.action, &request.args)?;
        let height_count = args.heights.len();
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_summary = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_row_heights_bulk(&args.heights)?;
            sheet.summary()
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Updated {height_count} row height entries on `{}`",
                sheet_summary.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet_summary]);
        Ok(response)
    }

    fn get_row_height(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: GetRowHeightArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Retrieved row height for row {} from `{}`",
                args.row_index, sheet.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.sheet_ref = Some(sheet_reference(sheet));
        response.sheet_list = Some(vec![sheet.summary()]);
        response.row_height = sheet.get_row_height(args.row_index);
        Ok(response)
    }

    fn cleanup_and_validate_sheet(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SheetLookupArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let sheet_summary = {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.cleanup_and_validate_sheet()?;
            sheet.summary()
        };
        artifact.validate_conditional_formats(&request.action, &sheet_summary.name)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Cleaned and validated `{}`", sheet_summary.name),
            snapshot_for_artifact(artifact),
        );
        response.sheet_list = Some(vec![sheet_summary]);
        Ok(response)
    }

    fn create_text_style(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateTextStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let style_id = artifact.create_text_style(
            args.style,
            args.source_style_id,
            args.merge_with_existing_components.unwrap_or(false),
        )?;
        let style = artifact.get_text_style(style_id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::Serialization {
                message: format!("created text style `{style_id}` was not available"),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created text style `{style_id}`"),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(style_id);
        response.serialized_dict = Some(to_serialized_value(style)?);
        Ok(response)
    }

    fn get_text_style(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: GetStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let style = artifact.get_text_style(args.id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("text style `{}` was not found", args.id),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved text style `{}`", args.id),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(args.id);
        response.serialized_dict = Some(to_serialized_value(style)?);
        Ok(response)
    }

    fn create_fill(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateFillArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let style_id = artifact.create_fill(
            args.fill,
            args.source_fill_id,
            args.merge_with_existing_components.unwrap_or(false),
        )?;
        let fill = artifact.get_fill(style_id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::Serialization {
                message: format!("created fill `{style_id}` was not available"),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created fill `{style_id}`"),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(style_id);
        response.serialized_dict = Some(to_serialized_value(fill)?);
        Ok(response)
    }

    fn get_fill(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: GetStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let fill = artifact.get_fill(args.id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("fill `{}` was not found", args.id),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved fill `{}`", args.id),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(args.id);
        response.serialized_dict = Some(to_serialized_value(fill)?);
        Ok(response)
    }

    fn create_border(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateBorderArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let style_id = artifact.create_border(
            args.border,
            args.source_border_id,
            args.merge_with_existing_components.unwrap_or(false),
        )?;
        let border = artifact.get_border(style_id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::Serialization {
                message: format!("created border `{style_id}` was not available"),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created border `{style_id}`"),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(style_id);
        response.serialized_dict = Some(to_serialized_value(border)?);
        Ok(response)
    }

    fn get_border(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: GetStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let border = artifact.get_border(args.id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("border `{}` was not found", args.id),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved border `{}`", args.id),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(args.id);
        response.serialized_dict = Some(to_serialized_value(border)?);
        Ok(response)
    }

    fn create_number_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateNumberFormatArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let style_id = artifact.create_number_format(
            args.number_format,
            args.source_number_format_id,
            args.merge_with_existing_components.unwrap_or(false),
        )?;
        let number_format = artifact
            .get_number_format(style_id)
            .cloned()
            .ok_or_else(|| SpreadsheetArtifactError::Serialization {
                message: format!("created number format `{style_id}` was not available"),
            })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created number format `{style_id}`"),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(style_id);
        response.serialized_dict = Some(to_serialized_value(number_format)?);
        Ok(response)
    }

    fn get_number_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: GetStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let number_format = artifact
            .get_number_format(args.id)
            .cloned()
            .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("number format `{}` was not found", args.id),
            })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved number format `{}`", args.id),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(args.id);
        response.serialized_dict = Some(to_serialized_value(number_format)?);
        Ok(response)
    }

    fn create_cell_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateCellFormatArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let style_id = artifact.create_cell_format(
            args.format,
            args.source_format_id,
            args.merge_with_existing_components.unwrap_or(false),
        )?;
        let format = artifact.get_cell_format(style_id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::Serialization {
                message: format!("created cell format `{style_id}` was not available"),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created cell format `{style_id}`"),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(style_id);
        response.serialized_dict = Some(to_serialized_value(format)?);
        response.cell_format_summary = artifact.cell_format_summary(style_id);
        Ok(response)
    }

    fn get_cell_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: GetStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let format = artifact.get_cell_format(args.id).cloned().ok_or_else(|| {
            SpreadsheetArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("cell format `{}` was not found", args.id),
            }
        })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved cell format `{}`", args.id),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(args.id);
        response.serialized_dict = Some(to_serialized_value(format)?);
        response.cell_format_summary = artifact.cell_format_summary(args.id);
        Ok(response)
    }

    fn create_differential_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CreateDifferentialFormatArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let style_id = artifact.create_differential_format(args.format);
        let format = artifact
            .get_differential_format(style_id)
            .cloned()
            .ok_or_else(|| SpreadsheetArtifactError::Serialization {
                message: format!("created differential format `{style_id}` was not available"),
            })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Created differential format `{style_id}`"),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(style_id);
        response.serialized_dict = Some(to_serialized_value(format)?);
        Ok(response)
    }

    fn get_differential_format(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: GetStyleArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let format = artifact
            .get_differential_format(args.id)
            .cloned()
            .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                action: request.action.clone(),
                message: format!("differential format `{}` was not found", args.id),
            })?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved differential format `{}`", args.id),
            snapshot_for_artifact(artifact),
        );
        response.style_id = Some(args.id);
        response.serialized_dict = Some(to_serialized_value(format)?);
        Ok(response)
    }

    fn get_cell_format_summary(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CellAddressArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let cell = sheet.get_cell_view(CellAddress::parse(&args.address)?);
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved cell format summary for `{}`", args.address),
            snapshot_for_artifact(artifact),
        );
        response.cell_ref = Some(sheet.cell_ref(&args.address)?);
        response.cell = Some(cell.clone());
        response.cell_format_summary = artifact.cell_format_summary(cell.style_index);
        Ok(response)
    }

    fn get_range_format_summary(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: RangeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range = CellRange::parse(&args.range)?;
        let top_left_style_index = sheet.top_left_style_index(&range);
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved range format summary for `{}`", args.range),
            snapshot_for_artifact(artifact),
        );
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        response.range_format = Some(sheet.range_format(&range));
        response.range = Some(sheet.get_range_view(&range));
        response.top_left_style_index = Some(top_left_style_index);
        response.cell_format_summary = artifact.cell_format_summary(top_left_style_index);
        Ok(response)
    }

    fn get_reference(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: ReferenceArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Resolved reference `{}` on `{}`",
                args.reference, sheet.name
            ),
            snapshot_for_artifact(artifact),
        );
        match sheet.reference(&args.reference)? {
            crate::SpreadsheetSheetReference::Cell { cell_ref } => {
                let address = cell_ref.cell_address()?;
                let cell = sheet.get_cell_view(address);
                response.cell_format_summary = artifact.cell_format_summary(cell.style_index);
                response.cell = Some(cell);
                response.raw_cell = sheet.get_raw_cell(address);
                response.cell_ref = Some(cell_ref);
            }
            crate::SpreadsheetSheetReference::Range { range_ref } => {
                let range = range_ref.range()?;
                let top_left_style_index = sheet.top_left_style_index(&range);
                response.range = Some(sheet.get_range_view(&range));
                response.range_ref = Some(range_ref);
                response.range_format = Some(sheet.range_format(&range));
                response.top_left_style_index = Some(top_left_style_index);
                response.cell_format_summary = artifact.cell_format_summary(top_left_style_index);
                response.rendered_text = Some(sheet.to_rendered_text(Some(&range)));
            }
        }
        Ok(response)
    }

    fn get_cell(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CellAddressArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let cell = sheet.get_cell_view(CellAddress::parse(&args.address)?);
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved cell `{}` from `{}`", args.address, sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.cell_format_summary = artifact.cell_format_summary(cell.style_index);
        response.cell = Some(cell);
        response.raw_cell = sheet.get_raw_cell(CellAddress::parse(&args.address)?);
        response.cell_ref = Some(sheet.cell_ref(&args.address)?);
        Ok(response)
    }

    fn get_cell_by_indices(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CellIndicesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let address = CellAddress {
            column: args.column_index,
            row: args.row_index,
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Retrieved cell by indices ({}, {}) from `{}`",
                args.column_index, args.row_index, sheet.name
            ),
            snapshot_for_artifact(artifact),
        );
        let cell = sheet.get_cell_view_by_indices(args.column_index, args.row_index);
        response.cell_format_summary = artifact.cell_format_summary(cell.style_index);
        response.cell = Some(cell);
        response.raw_cell = sheet.get_raw_cell(address);
        response.cell_ref = Some(sheet.cell_ref(address.to_a1())?);
        Ok(response)
    }

    fn get_cell_field(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CellFieldArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let value = sheet.get_cell_field(CellAddress::parse(&args.address)?, &args.field)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Retrieved field `{}` from `{}` on `{}`",
                args.field, args.address, sheet.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.cell_field = value;
        response.cell_ref = Some(sheet.cell_ref(&args.address)?);
        Ok(response)
    }

    fn get_cell_field_by_indices(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: CellFieldByIndicesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let value =
            sheet.get_cell_field_by_indices(args.column_index, args.row_index, &args.field)?;
        let address = CellAddress {
            column: args.column_index,
            row: args.row_index,
        };
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!(
                "Retrieved field `{}` from indices ({}, {}) on `{}`",
                args.field, args.column_index, args.row_index, sheet.name
            ),
            snapshot_for_artifact(artifact),
        );
        response.cell_field = value;
        response.cell_ref = Some(sheet.cell_ref(address.to_a1())?);
        Ok(response)
    }

    fn get_range(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: RangeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let sheet = artifact.sheet_lookup(
            &request.action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        let range = CellRange::parse(&args.range)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            format!("Retrieved range `{}` from `{}`", args.range, sheet.name),
            snapshot_for_artifact(artifact),
        );
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        response.range_format = Some(sheet.range_format(&range));
        response.top_left_style_index = Some(sheet.top_left_style_index(&range));
        response.cell_format_summary =
            artifact.cell_format_summary(sheet.top_left_style_index(&range));
        response.rendered_text = Some(sheet.to_rendered_text(Some(&range)));
        Ok(response)
    }

    fn set_cell_value(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: SetCellValueArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let address = CellAddress::parse(&args.address)?;
        let recalculate = args.recalculate.unwrap_or(artifact.auto_recalculate);
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let value = normalize_optional_cell_value(args.value)?;
            sheet.set_value(address, value)?;
        }
        if recalculate {
            artifact.recalculate();
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Updated cell `{}`", args.address),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.cell = Some(sheet.get_cell_view(address));
        response.cell_ref = Some(sheet.cell_ref(&args.address)?);
        Ok(response)
    }

    fn set_range_value(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: SetRangeValueArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        let recalculate = args.recalculate.unwrap_or(artifact.auto_recalculate);
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_range_to_value(&range, normalize_optional_cell_value(args.value)?)?;
        }
        if recalculate {
            artifact.recalculate();
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Updated range `{}` to a single value", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        Ok(response)
    }

    fn set_range_values(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: SetRangeValuesArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        let recalculate = args.recalculate.unwrap_or(artifact.auto_recalculate);
        let values = normalize_value_matrix(args.values, &request.action)?;
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_values_matrix(&range, &values)?;
        }
        if recalculate {
            artifact.recalculate();
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Updated range `{}`", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        Ok(response)
    }

    fn set_cell_formula(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: SetCellFormulaArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let address = CellAddress::parse(&args.address)?;
        let recalculate = args.recalculate.unwrap_or(artifact.auto_recalculate);
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_formula(address, Some(normalize_formula(args.formula)))?;
        }
        if recalculate {
            artifact.recalculate();
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Updated formula in `{}`", args.address),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.cell = Some(sheet.get_cell_view(address));
        response.cell_ref = Some(sheet.cell_ref(&args.address)?);
        Ok(response)
    }

    fn set_range_formula(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: SetRangeFormulaArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        let recalculate = args.recalculate.unwrap_or(artifact.auto_recalculate);
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_range_to_formula(&range, Some(normalize_formula(args.formula)))?;
        }
        if recalculate {
            artifact.recalculate();
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Updated range `{}` to a single formula", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        Ok(response)
    }

    fn set_range_formulas(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: SetRangeFormulasArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        let recalculate = args.recalculate.unwrap_or(artifact.auto_recalculate);
        let formulas = args
            .formulas
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|value| value.map(normalize_formula))
                    .collect()
            })
            .collect::<Vec<Vec<Option<String>>>>();
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.set_formulas_matrix(&range, &formulas)?;
        }
        if recalculate {
            artifact.recalculate();
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Updated formulas in `{}`", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        Ok(response)
    }

    fn set_cell_style(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetCellStyleArgs = parse_args(&request.action, &request.args)?;
        let range = CellRange::parse(&args.address)?;
        self.set_style_impl(
            request,
            args.sheet_name,
            args.sheet_index,
            range,
            args.style_index,
        )
    }

    fn set_range_style(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: SetRangeStyleArgs = parse_args(&request.action, &request.args)?;
        let range = CellRange::parse(&args.range)?;
        self.set_style_impl(
            request,
            args.sheet_name,
            args.sheet_index,
            range,
            args.style_index,
        )
    }

    fn set_style_impl(
        &mut self,
        request: SpreadsheetArtifactRequest,
        sheet_name: Option<String>,
        sheet_index: Option<u32>,
        range: CellRange,
        style_index: u32,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                sheet_name.as_deref(),
                sheet_index.map(|value| value as usize),
            )?;
            sheet.set_style_index(&range, style_index)?;
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Updated style index {} on `{}`", style_index, range.to_a1()),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            sheet_name.as_deref(),
            sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        Ok(response)
    }

    fn clear_range(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: ClearRangeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        let recalculate = args.recalculate.unwrap_or(artifact.auto_recalculate);
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.clear_range(&range, args.fields.as_deref())?;
        }
        if recalculate {
            artifact.recalculate();
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Cleared range `{}`", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        Ok(response)
    }

    fn merge_range(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: MergeRangeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.merge_cells(&range, args.raise_on_conflict.unwrap_or(false))?;
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Merged `{}`", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        response.range_ref = Some(SpreadsheetCellRangeRef::new(sheet.name.clone(), &range));
        Ok(response)
    }

    fn cite_cell(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: CiteCellArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let address = CellAddress::parse(&args.address)?;
        let citation = SpreadsheetCitation {
            tether_id: args.tether_id,
            start_line: args.start_line,
            end_line: args.end_line,
            content_reference_type: args.content_reference_type,
            source_type: args.source_type,
        };
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            let cell_ref = sheet.cell_ref(&args.address)?;
            cell_ref.cite(sheet, citation)?;
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Attached citation to cell `{}`", args.address),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.cell = Some(sheet.get_cell_view(address));
        response.cell_ref = Some(sheet.cell_ref(&args.address)?);
        Ok(response)
    }

    fn unmerge_range(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: RangeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.unmerge_cells(&range);
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Unmerged `{}`", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        Ok(response)
    }

    fn cite_range(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let action = request.action.clone();
        let args: CiteRangeArgs = parse_args(&request.action, &request.args)?;
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        let range = CellRange::parse(&args.range)?;
        let citation = SpreadsheetCitation {
            tether_id: args.tether_id,
            start_line: args.start_line,
            end_line: args.end_line,
            content_reference_type: args.content_reference_type,
            source_type: args.source_type,
        };
        {
            let sheet = artifact.sheet_lookup_mut(
                &request.action,
                args.sheet_name.as_deref(),
                args.sheet_index.map(|value| value as usize),
            )?;
            sheet.cite_range(&range, citation)?;
        }
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            action.clone(),
            format!("Attached citation to `{}`", args.range),
            snapshot_for_artifact(artifact),
        );
        let sheet = artifact.sheet_lookup(
            &action,
            args.sheet_name.as_deref(),
            args.sheet_index.map(|value| value as usize),
        )?;
        response.range = Some(sheet.get_range_view(&range));
        Ok(response)
    }

    fn calculate(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact_mut(&artifact_id, &request.action)?;
        artifact.recalculate();
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Recalculated workbook".to_string(),
            snapshot_for_artifact(artifact),
        );
        response.workbook_summary = Some(artifact.summary());
        Ok(response)
    }

    fn serialize_dict(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Serialized workbook to dict".to_string(),
            snapshot_for_artifact(artifact),
        );
        response.serialized_dict = Some(artifact.to_dict()?);
        Ok(response)
    }

    fn serialize_json(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Serialized workbook to JSON".to_string(),
            snapshot_for_artifact(artifact),
        );
        response.serialized_json = Some(artifact.to_json()?);
        Ok(response)
    }

    fn serialize_bytes(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.get_artifact(&artifact_id, &request.action)?;
        let mut response = SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Serialized workbook to bytes".to_string(),
            snapshot_for_artifact(artifact),
        );
        response.serialized_bytes_base64 = Some(artifact.to_bytes_base64());
        Ok(response)
    }

    fn deserialize_dict(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: DeserializeDictArgs = parse_args(&request.action, &request.args)?;
        let artifact = SpreadsheetArtifact::from_dict(args.data, args.artifact_id)?;
        let artifact_id = artifact.artifact_id.clone();
        let snapshot = snapshot_for_artifact(&artifact);
        self.documents.insert(artifact_id.clone(), artifact);
        Ok(SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Deserialized workbook from dict".to_string(),
            snapshot,
        ))
    }

    fn deserialize_json(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: DeserializeJsonArgs = parse_args(&request.action, &request.args)?;
        let artifact = SpreadsheetArtifact::from_json(args.json, args.artifact_id)?;
        let artifact_id = artifact.artifact_id.clone();
        let snapshot = snapshot_for_artifact(&artifact);
        self.documents.insert(artifact_id.clone(), artifact);
        Ok(SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Deserialized workbook from JSON".to_string(),
            snapshot,
        ))
    }

    fn deserialize_bytes(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let args: DeserializeBytesArgs = parse_args(&request.action, &request.args)?;
        let bytes = BASE64_STANDARD.decode(args.bytes_base64).map_err(|error| {
            SpreadsheetArtifactError::Serialization {
                message: error.to_string(),
            }
        })?;
        let artifact = SpreadsheetArtifact::from_bytes(&bytes, args.artifact_id)?;
        let artifact_id = artifact.artifact_id.clone();
        let snapshot = snapshot_for_artifact(&artifact);
        self.documents.insert(artifact_id.clone(), artifact);
        Ok(SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Deserialized workbook from bytes".to_string(),
            snapshot,
        ))
    }

    fn delete_artifact(
        &mut self,
        request: SpreadsheetArtifactRequest,
    ) -> Result<SpreadsheetArtifactResponse, SpreadsheetArtifactError> {
        let artifact_id = required_artifact_id(&request)?;
        let artifact = self.documents.remove(&artifact_id).ok_or_else(|| {
            SpreadsheetArtifactError::UnknownArtifactId {
                action: request.action.clone(),
                artifact_id: artifact_id.clone(),
            }
        })?;
        let snapshot = snapshot_for_artifact(&artifact);
        Ok(SpreadsheetArtifactResponse::new(
            artifact_id,
            request.action,
            "Deleted spreadsheet artifact".to_string(),
            snapshot,
        ))
    }

    fn get_artifact(
        &self,
        artifact_id: &str,
        action: &str,
    ) -> Result<&SpreadsheetArtifact, SpreadsheetArtifactError> {
        self.documents
            .get(artifact_id)
            .ok_or_else(|| SpreadsheetArtifactError::UnknownArtifactId {
                action: action.to_string(),
                artifact_id: artifact_id.to_string(),
            })
    }

    fn get_artifact_mut(
        &mut self,
        artifact_id: &str,
        action: &str,
    ) -> Result<&mut SpreadsheetArtifact, SpreadsheetArtifactError> {
        self.documents.get_mut(artifact_id).ok_or_else(|| {
            SpreadsheetArtifactError::UnknownArtifactId {
                action: action.to_string(),
                artifact_id: artifact_id.to_string(),
            }
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetArtifactResponse {
    pub artifact_id: String,
    pub action: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exported_paths: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_snapshot: Option<SpreadsheetArtifactSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workbook_summary: Option<SpreadsheetSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_list: Option<Vec<SpreadsheetSheetSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_list: Option<Vec<crate::SpreadsheetChart>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_list: Option<Vec<crate::SpreadsheetTableView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditional_format_list: Option<Vec<crate::SpreadsheetConditionalFormat>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pivot_table_list: Option<Vec<crate::SpreadsheetPivotTable>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_ref: Option<SpreadsheetSheetRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_ref: Option<SpreadsheetCellRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_ref: Option<SpreadsheetCellRangeRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_format: Option<crate::SpreadsheetRangeFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<crate::SpreadsheetCellView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_cell: Option<crate::SpreadsheetCell>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_field: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<SpreadsheetRangeView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_left_style_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_format_summary: Option<crate::SpreadsheetCellFormatSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialized_dict: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialized_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialized_bytes_base64: Option<String>,
}

impl SpreadsheetArtifactResponse {
    fn new(
        artifact_id: String,
        action: String,
        summary: String,
        artifact_snapshot: SpreadsheetArtifactSnapshot,
    ) -> Self {
        Self {
            artifact_id,
            action,
            summary,
            exported_paths: Vec::new(),
            artifact_snapshot: Some(artifact_snapshot),
            workbook_summary: None,
            sheet_list: None,
            chart_list: None,
            table_list: None,
            conditional_format_list: None,
            pivot_table_list: None,
            sheet_ref: None,
            cell_ref: None,
            range_ref: None,
            range_format: None,
            cell: None,
            raw_cell: None,
            style_id: None,
            cell_field: None,
            range: None,
            top_left_style_index: None,
            cell_format_summary: None,
            rendered_text: None,
            rendered_html: None,
            row_height: None,
            serialized_dict: None,
            serialized_json: None,
            serialized_bytes_base64: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetSheetRef {
    pub sheet_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetArtifactSnapshot {
    pub sheet_count: usize,
    pub sheet_names: Vec<String>,
    pub sheets: Vec<SpreadsheetSheetSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadsheetSheetSnapshot {
    pub sheet_id: String,
    pub name: String,
    pub filled_rows: usize,
    pub filled_columns: usize,
    pub minimum_range_filled: String,
    pub merged_range_count: usize,
    pub chart_count: usize,
    pub table_count: usize,
    pub conditional_format_count: usize,
    pub pivot_table_count: usize,
}

#[derive(Debug, Deserialize)]
struct CreateArgs {
    name: Option<String>,
    auto_recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PathArgs {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SaveArgs {
    path: PathBuf,
    file_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenderArgs {
    output_path: Option<PathBuf>,
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: Option<String>,
    center_address: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    include_headers: Option<bool>,
    scale: Option<f64>,
    performance_mode: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SheetLookupArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChartLookupArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    chart_id: Option<u32>,
    chart_index: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CreateChartArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    chart_type: crate::SpreadsheetChartType,
    source_sheet_name: Option<String>,
    source_range: String,
    options: Option<crate::SpreadsheetChartCreateOptions>,
}

#[derive(Debug, Deserialize)]
struct AddChartSeriesArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    chart_id: Option<u32>,
    chart_index: Option<u32>,
    series: crate::SpreadsheetChartSeries,
}

#[derive(Debug, Deserialize)]
struct SetChartPropertiesArgs {
    #[serde(flatten)]
    lookup: ChartLookupArgs,
    properties: crate::SpreadsheetChartProperties,
}

#[derive(Debug, Deserialize)]
struct TableLookupArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    name: Option<String>,
    display_name: Option<String>,
    id: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CreateTableArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    options: crate::SpreadsheetCreateTableOptions,
}

#[derive(Debug, Deserialize)]
struct SetTableStyleArgs {
    #[serde(flatten)]
    lookup: TableLookupArgs,
    options: crate::SpreadsheetTableStyleOptions,
}

#[derive(Debug, Deserialize)]
struct RenameTableColumnArgs {
    #[serde(flatten)]
    lookup: TableLookupArgs,
    column_id: Option<u32>,
    column_name: Option<String>,
    new_name: String,
}

#[derive(Debug, Deserialize)]
struct SetTableColumnTotalsArgs {
    #[serde(flatten)]
    lookup: TableLookupArgs,
    column_id: Option<u32>,
    column_name: Option<String>,
    totals_row_label: Option<String>,
    totals_row_function: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddConditionalFormatArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    format: crate::SpreadsheetConditionalFormat,
}

#[derive(Debug, Deserialize)]
struct DeleteConditionalFormatArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    id: u32,
}

#[derive(Debug, Deserialize)]
struct PivotTableLookupArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    name: Option<String>,
    index: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CreateSheetArgs {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RenameSheetArgs {
    old_name: Option<String>,
    sheet_index: Option<u32>,
    new_name: String,
}

#[derive(Debug, Deserialize)]
struct DeleteSheetArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SetSheetPropertiesArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    default_row_height: Option<f64>,
    default_column_width: Option<f64>,
    show_grid_lines: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetColumnWidthsArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    reference: String,
    width: f64,
}

#[derive(Debug, Deserialize)]
struct SetColumnWidthsBulkArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    widths: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
struct SetRowHeightArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    row_index: u32,
    height: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SetRowHeightsArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    start_row_index: u32,
    end_row_index: u32,
    height: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SetRowHeightsBulkArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    heights: BTreeMap<u32, Option<f64>>,
}

#[derive(Debug, Deserialize)]
struct GetRowHeightArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    row_index: u32,
}

#[derive(Debug, Deserialize)]
struct CreateTextStyleArgs {
    style: crate::SpreadsheetTextStyle,
    source_style_id: Option<u32>,
    merge_with_existing_components: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateFillArgs {
    fill: crate::SpreadsheetFill,
    source_fill_id: Option<u32>,
    merge_with_existing_components: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateBorderArgs {
    border: crate::SpreadsheetBorder,
    source_border_id: Option<u32>,
    merge_with_existing_components: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateNumberFormatArgs {
    number_format: crate::SpreadsheetNumberFormat,
    source_number_format_id: Option<u32>,
    merge_with_existing_components: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateCellFormatArgs {
    format: crate::SpreadsheetCellFormat,
    source_format_id: Option<u32>,
    merge_with_existing_components: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateDifferentialFormatArgs {
    format: crate::SpreadsheetDifferentialFormat,
}

#[derive(Debug, Deserialize)]
struct GetStyleArgs {
    id: u32,
}

#[derive(Debug, Deserialize)]
struct CellAddressArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    address: String,
}

#[derive(Debug, Deserialize)]
struct ReferenceArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    reference: String,
}

#[derive(Debug, Deserialize)]
struct CellIndicesArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    column_index: u32,
    row_index: u32,
}

#[derive(Debug, Deserialize)]
struct CellFieldArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    address: String,
    field: String,
}

#[derive(Debug, Deserialize)]
struct CellFieldByIndicesArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    column_index: u32,
    row_index: u32,
    field: String,
}

#[derive(Debug, Deserialize)]
struct RangeArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
}

#[derive(Debug, Deserialize)]
struct SetCellValueArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    address: String,
    value: Value,
    recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetRangeValuesArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    values: Vec<Vec<Value>>,
    recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetRangeValueArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    value: Value,
    recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetCellFormulaArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    address: String,
    formula: String,
    recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetRangeFormulasArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    formulas: Vec<Vec<Option<String>>>,
    recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetRangeFormulaArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    formula: String,
    recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetCellStyleArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    address: String,
    style_index: u32,
}

#[derive(Debug, Deserialize)]
struct SetRangeStyleArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    style_index: u32,
}

#[derive(Debug, Deserialize)]
struct ClearRangeArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    fields: Option<Vec<String>>,
    recalculate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct MergeRangeArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    raise_on_conflict: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CiteRangeArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: String,
    tether_id: String,
    start_line: Option<u32>,
    end_line: Option<u32>,
    content_reference_type: Option<String>,
    source_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CiteCellArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    address: String,
    tether_id: String,
    start_line: Option<u32>,
    end_line: Option<u32>,
    content_reference_type: Option<String>,
    source_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeserializeDictArgs {
    data: Value,
    artifact_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeserializeJsonArgs {
    json: String,
    artifact_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeserializeBytesArgs {
    bytes_base64: String,
    artifact_id: Option<String>,
}

fn snapshot_for_artifact(artifact: &SpreadsheetArtifact) -> SpreadsheetArtifactSnapshot {
    SpreadsheetArtifactSnapshot {
        sheet_count: artifact.sheets.len(),
        sheet_names: artifact.list_sheet_names(),
        sheets: artifact
            .sheets
            .iter()
            .map(|sheet| SpreadsheetSheetSnapshot {
                sheet_id: sheet.sheet_id.clone(),
                name: sheet.name.clone(),
                filled_rows: sheet.filled_rows(),
                filled_columns: sheet.filled_columns(),
                minimum_range_filled: sheet.minimum_range_filled(),
                merged_range_count: sheet.merged_ranges.len(),
                chart_count: sheet.charts.len(),
                table_count: sheet.tables.len(),
                conditional_format_count: sheet.conditional_formats.len(),
                pivot_table_count: sheet.pivot_tables.len(),
            })
            .collect(),
    }
}

fn chart_lookup_from_args(args: &ChartLookupArgs) -> crate::SpreadsheetChartLookup {
    crate::SpreadsheetChartLookup {
        id: args.chart_id,
        index: args.chart_index.map(|value| value as usize),
    }
}

fn table_lookup_from_args(args: &TableLookupArgs) -> crate::SpreadsheetTableLookup<'_> {
    crate::SpreadsheetTableLookup {
        name: args.name.as_deref(),
        display_name: args.display_name.as_deref(),
        id: args.id,
    }
}

fn pivot_lookup_from_args(args: &PivotTableLookupArgs) -> crate::SpreadsheetPivotTableLookup<'_> {
    crate::SpreadsheetPivotTableLookup {
        name: args.name.as_deref(),
        index: args.index.map(|value| value as usize),
    }
}

fn sheet_reference(sheet: &crate::SpreadsheetSheet) -> SpreadsheetSheetRef {
    SpreadsheetSheetRef {
        sheet_name: sheet.name.clone(),
    }
}

fn normalize_optional_cell_value(
    value: Value,
) -> Result<Option<SpreadsheetCellValue>, SpreadsheetArtifactError> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(SpreadsheetCellValue::try_from(value)?))
    }
}

fn normalize_value_matrix(
    values: Vec<Vec<Value>>,
    action: &str,
) -> Result<Vec<Vec<Option<SpreadsheetCellValue>>>, SpreadsheetArtifactError> {
    if values.is_empty() {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "values matrix cannot be empty".to_string(),
        });
    }
    let width = values.first().map(Vec::len).unwrap_or(0);
    if width == 0 || values.iter().any(|row| row.len() != width) {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "values matrix must be rectangular".to_string(),
        });
    }
    values
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(normalize_optional_cell_value)
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
}

fn normalize_formula(formula: String) -> String {
    let trimmed = formula.trim();
    if trimmed.starts_with('=') {
        trimmed.to_string()
    } else {
        format!("={trimmed}")
    }
}

fn render_options_from_args(
    args: RenderArgs,
) -> Result<crate::SpreadsheetRenderOptions, SpreadsheetArtifactError> {
    let scale = args.scale.unwrap_or(1.0);
    if scale <= 0.0 {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: "render".to_string(),
            message: "render scale must be positive".to_string(),
        });
    }
    Ok(crate::SpreadsheetRenderOptions {
        output_path: args.output_path,
        center_address: args.center_address,
        width: args.width,
        height: args.height,
        include_headers: args.include_headers.unwrap_or(true),
        scale,
        performance_mode: args.performance_mode.unwrap_or(false),
    })
}

fn required_artifact_id(
    request: &SpreadsheetArtifactRequest,
) -> Result<String, SpreadsheetArtifactError> {
    request
        .artifact_id
        .clone()
        .ok_or_else(|| SpreadsheetArtifactError::MissingArtifactId {
            action: request.action.clone(),
        })
}

fn parse_args<T: for<'de> Deserialize<'de>>(
    action: &str,
    value: &Value,
) -> Result<T, SpreadsheetArtifactError> {
    serde_json::from_value(value.clone()).map_err(|error| SpreadsheetArtifactError::InvalidArgs {
        action: action.to_string(),
        message: error.to_string(),
    })
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn to_serialized_value<T: Serialize>(value: T) -> Result<Value, SpreadsheetArtifactError> {
    serde_json::to_value(value).map_err(|error| SpreadsheetArtifactError::Serialization {
        message: error.to_string(),
    })
}
