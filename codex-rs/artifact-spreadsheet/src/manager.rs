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
            "get_summary" => self.get_summary(request),
            "list_sheets" => self.list_sheets(request),
            "get_sheet" => self.get_sheet(request),
            "inspect" => self.inspect(request),
            "create_sheet" => self.create_sheet(request),
            "rename_sheet" => self.rename_sheet(request),
            "delete_sheet" => self.delete_sheet(request),
            "set_sheet_properties" => self.set_sheet_properties(request),
            "set_column_widths" => self.set_column_widths(request),
            "get_cell" => self.get_cell(request),
            "get_range" => self.get_range(request),
            "set_cell_value" => self.set_cell_value(request),
            "set_range_values" => self.set_range_values(request),
            "set_cell_formula" => self.set_cell_formula(request),
            "set_range_formulas" => self.set_range_formulas(request),
            "set_cell_style" => self.set_cell_style(request),
            "set_range_style" => self.set_range_style(request),
            "clear_range" => self.clear_range(request),
            "merge_range" => self.merge_range(request),
            "unmerge_range" => self.unmerge_range(request),
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
                .map(|sheet| sheet.summary())
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
                .map(|sheet| sheet.summary())
                .collect(),
        );
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
                .map(|sheet| sheet.summary())
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
        response.rendered_text = Some(sheet.to_rendered_text(range.as_ref()));
        response.range = range.as_ref().map(|entry| sheet.get_range_view(entry));
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
                .map(|sheet| sheet.summary())
                .collect(),
        );
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
                .map(|sheet| sheet.summary())
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
                .map(|sheet| sheet.summary())
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
        response.cell = Some(cell);
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
    pub cell: Option<crate::SpreadsheetCellView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<SpreadsheetRangeView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_text: Option<String>,
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
            cell: None,
            range: None,
            rendered_text: None,
            serialized_dict: None,
            serialized_json: None,
            serialized_bytes_base64: None,
        }
    }
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
struct SheetLookupArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    range: Option<String>,
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
struct CellAddressArgs {
    sheet_name: Option<String>,
    sheet_index: Option<u32>,
    address: String,
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
            })
            .collect(),
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
