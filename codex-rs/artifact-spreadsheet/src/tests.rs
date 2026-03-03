use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactManager;
use crate::SpreadsheetArtifactRequest;
use crate::SpreadsheetCell;
use crate::SpreadsheetCellFormat;
use crate::SpreadsheetCellFormatSummary;
use crate::SpreadsheetCellValue;
use crate::SpreadsheetFileType;
use crate::SpreadsheetFill;
use crate::SpreadsheetFontFace;
use crate::SpreadsheetNumberFormat;
use crate::SpreadsheetRenderOptions;
use crate::SpreadsheetSheet;
use crate::SpreadsheetSheetReference;
use crate::SpreadsheetTextStyle;

#[test]
fn manager_can_create_edit_recalculate_and_export() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = SpreadsheetArtifactManager::default();

    let created = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Budget" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_sheet".to_string(),
            args: serde_json::json!({ "name": "Sheet1" }),
        },
        temp_dir.path(),
    )?;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_range_values".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "range": "A1:B2",
                "values": [[1, 2], [3, 4]]
            }),
        },
        temp_dir.path(),
    )?;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_cell_formula".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "address": "C1",
                "formula": "=SUM(A1:B2)",
                "recalculate": true
            }),
        },
        temp_dir.path(),
    )?;

    let cell = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_cell".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "address": "C1"
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        cell.cell.and_then(|entry| entry.value),
        Some(SpreadsheetCellValue::Integer(10))
    );

    let export_path = temp_dir.path().join("budget.xlsx");
    let export = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "export_xlsx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(export.exported_paths.len(), 1);
    assert!(export.exported_paths[0].exists());
    Ok(())
}

#[test]
fn spreadsheet_serialization_roundtrip_preserves_cells() -> Result<(), Box<dyn std::error::Error>> {
    let mut artifact = SpreadsheetArtifact::new(Some("Roundtrip".to_string()));
    let sheet = artifact.create_sheet("Sheet1".to_string())?;
    sheet.set_value(
        crate::CellAddress::parse("A1")?,
        Some(SpreadsheetCellValue::String("hello".to_string())),
    )?;
    sheet.set_formula(crate::CellAddress::parse("B1")?, Some("=A1".to_string()))?;
    artifact.recalculate();

    let json = artifact.to_json()?;
    let restored = SpreadsheetArtifact::from_json(json, None)?;
    let restored_sheet = restored.get_sheet(Some("Sheet1"), None).expect("sheet");
    let cell = restored_sheet.get_cell_view(crate::CellAddress::parse("A1")?);
    assert_eq!(
        cell.value,
        Some(SpreadsheetCellValue::String("hello".to_string()))
    );
    Ok(())
}

#[test]
fn xlsx_roundtrip_preserves_merged_ranges_and_style_indices()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("styled.xlsx");

    let mut artifact = SpreadsheetArtifact::new(Some("Styled".to_string()));
    let sheet = artifact.create_sheet("Sheet1".to_string())?;
    sheet.set_value(
        crate::CellAddress::parse("A1")?,
        Some(SpreadsheetCellValue::Integer(42)),
    )?;
    sheet.set_style_index(&crate::CellRange::parse("A1:B1")?, 3)?;
    sheet.merge_cells(&crate::CellRange::parse("A1:B1")?, true)?;
    artifact.export(&path)?;

    let restored = SpreadsheetArtifact::from_source_file(&path, None)?;
    let restored_sheet = restored.get_sheet(Some("Sheet1"), None).expect("sheet");
    assert_eq!(restored_sheet.merged_ranges.len(), 1);
    assert_eq!(
        restored_sheet
            .get_cell_view(crate::CellAddress::parse("A1")?)
            .style_index,
        3
    );
    Ok(())
}

#[test]
fn path_accesses_cover_import_and_export() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = tempfile::tempdir()?;
    let request = crate::SpreadsheetArtifactRequest {
        artifact_id: Some("spreadsheet_1".to_string()),
        action: "export_xlsx".to_string(),
        args: serde_json::json!({ "path": "out/report.xlsx" }),
    };
    let accesses = request.required_path_accesses(cwd.path())?;
    assert_eq!(accesses.len(), 1);
    assert!(accesses[0].path.ends_with("out/report.xlsx"));
    Ok(())
}

#[test]
fn render_options_write_deterministic_html_previews() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut artifact = SpreadsheetArtifact::new(Some("Preview".to_string()));
    artifact.create_sheet("Sheet 1".to_string())?;
    {
        let sheet = artifact
            .get_sheet_mut(Some("Sheet 1"), None)
            .expect("sheet");
        sheet.set_value(
            CellAddress::parse("A1")?,
            Some(SpreadsheetCellValue::String("Name".to_string())),
        )?;
        sheet.set_value(
            CellAddress::parse("B1")?,
            Some(SpreadsheetCellValue::String("Value".to_string())),
        )?;
        sheet.set_value(
            CellAddress::parse("A2")?,
            Some(SpreadsheetCellValue::String("Alpha".to_string())),
        )?;
        sheet.set_value(
            CellAddress::parse("B2")?,
            Some(SpreadsheetCellValue::Integer(42)),
        )?;
    }

    let rendered = artifact.render_range_preview(
        temp_dir.path(),
        artifact.get_sheet(Some("Sheet 1"), None).expect("sheet"),
        &CellRange::parse("A1:B2")?,
        &SpreadsheetRenderOptions {
            output_path: Some(temp_dir.path().join("range-preview.html")),
            width: Some(320),
            height: Some(200),
            include_headers: true,
            scale: 1.25,
            performance_mode: true,
            ..Default::default()
        },
    )?;
    assert!(rendered.path.exists());
    assert_eq!(std::fs::read_to_string(&rendered.path)?, rendered.html);
    assert!(rendered.html.contains("<!doctype html>"));
    assert!(rendered.html.contains("data-performance-mode=\"true\""));
    assert!(rendered.html.contains(
        "style=\"--scale: 1.25; --headers: 1; width: 320px; height: 200px; overflow: auto\""
    ));
    assert!(rendered.html.contains("<th>A</th>"));
    assert!(rendered.html.contains("data-address=\"B2\""));
    assert!(rendered.html.contains(">42</td>"));

    let workbook = artifact.render_workbook_previews(
        temp_dir.path(),
        &SpreadsheetRenderOptions {
            output_path: Some(temp_dir.path().join("workbook")),
            include_headers: false,
            ..Default::default()
        },
    )?;
    assert_eq!(workbook.len(), 1);
    assert!(workbook[0].path.ends_with("Sheet_1.html"));
    assert!(!workbook[0].html.contains("<th>A</th>"));
    Ok(())
}

#[test]
fn sheet_refs_support_handle_and_field_apis() -> Result<(), Box<dyn std::error::Error>> {
    let mut artifact = SpreadsheetArtifact::new(Some("Handles".to_string()));
    let (range_ref, cell_ref) = {
        let sheet = artifact.create_sheet("Sheet1".to_string())?;
        let range_ref = sheet.range_ref("A1:B2")?;
        range_ref.set_value(sheet, Some(SpreadsheetCellValue::Integer(7)))?;
        let cell_ref = sheet.cell_ref("B2")?;
        cell_ref.set_formula(sheet, Some("=SUM(A1:B2)".to_string()))?;
        (range_ref, cell_ref)
    };
    artifact.recalculate();
    let sheet = artifact.get_sheet(Some("Sheet1"), None).expect("sheet");

    let values = range_ref.get_values(sheet)?;
    assert_eq!(values[0][0], Some(SpreadsheetCellValue::Integer(7)));
    assert_eq!(
        cell_ref.get(sheet)?.value,
        Some(SpreadsheetCellValue::Integer(28))
    );
    assert_eq!(
        sheet.get_cell_field_by_indices(2, 2, "formula")?,
        Some(serde_json::Value::String("=SUM(A1:B2)".to_string()))
    );
    assert_eq!(
        sheet.minimum_range_ref().map(|entry| entry.address),
        Some("A1:B2".to_string())
    );
    assert!(matches!(
        sheet.to_dict()?,
        serde_json::Value::Object(_) | serde_json::Value::Array(_)
    ));
    Ok(())
}

#[test]
fn manager_supports_single_value_formula_and_cite_cell_actions()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = SpreadsheetArtifactManager::default();
    let created = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Actions" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_sheet".to_string(),
            args: serde_json::json!({ "name": "Sheet1" }),
        },
        temp_dir.path(),
    )?;

    let uniform = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_range_value".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "range": "A1:B2",
                "value": 5
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        uniform
            .range_ref
            .as_ref()
            .map(|entry| entry.address.clone()),
        Some("A1:B2".to_string())
    );

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_range_formula".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "range": "C1:C2",
                "formula": "=SUM(A1:B2)",
                "recalculate": true
            }),
        },
        temp_dir.path(),
    )?;

    let cited = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "cite_cell".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "address": "C1",
                "tether_id": "source-1",
                "start_line": 3,
                "end_line": 8
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        cited.cell.as_ref().map(|entry| entry.citations.len()),
        Some(1)
    );

    let by_indices = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_cell_by_indices".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "column_index": 3,
                "row_index": 1
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        by_indices
            .cell
            .as_ref()
            .and_then(|entry| entry.value.clone()),
        Some(SpreadsheetCellValue::Integer(20))
    );

    let field = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "get_cell_field".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "address": "C1",
                "field": "formula"
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        field.cell_field,
        Some(serde_json::Value::String("=SUM(A1:B2)".to_string()))
    );
    Ok(())
}

#[test]
fn artifact_file_type_helpers_and_source_files_work() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut artifact = SpreadsheetArtifact::new(Some("Files".to_string()));
    artifact.artifact_id = "spreadsheet_fixed".to_string();
    artifact.create_sheet("Sheet1".to_string())?.set_value(
        CellAddress::parse("A1")?,
        Some(SpreadsheetCellValue::String("hello".to_string())),
    )?;

    assert_eq!(
        SpreadsheetArtifact::allowed_file_extensions(),
        &["xlsx", "json", "bin"]
    );
    assert_eq!(
        SpreadsheetArtifact::allowed_file_mime_types(),
        &[
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/json",
            "application/octet-stream",
        ]
    );
    assert_eq!(
        SpreadsheetArtifact::allowed_file_types().to_vec(),
        vec![
            SpreadsheetFileType::Xlsx,
            SpreadsheetFileType::Json,
            SpreadsheetFileType::Binary,
        ]
    );
    assert_eq!(
        artifact.get_output_file_name(Some("preview"), SpreadsheetFileType::Json),
        "spreadsheet_fixed_preview.json".to_string()
    );

    let json_path = temp_dir
        .path()
        .join(artifact.get_output_file_name(None, SpreadsheetFileType::Json));
    artifact.save(&json_path, Some("json"))?;
    let restored_json = SpreadsheetArtifact::load(&json_path, None)?;
    assert_eq!(restored_json.to_dict()?, artifact.to_dict()?);

    let bytes_path = temp_dir
        .path()
        .join(artifact.get_output_file_name(Some("bytes"), SpreadsheetFileType::Binary));
    artifact.save(&bytes_path, Some("bin"))?;
    let restored_bytes = SpreadsheetArtifact::read(&bytes_path, None)?;
    assert_eq!(restored_bytes.to_dict()?, artifact.to_dict()?);
    Ok(())
}

#[test]
fn sheet_cleanup_and_row_sizing_helpers_work() -> Result<(), Box<dyn std::error::Error>> {
    let mut sheet = SpreadsheetSheet::new("Sheet1".to_string());
    sheet.default_row_height = Some(15.0);
    sheet.set_column_widths_bulk(&BTreeMap::from([
        ("A".to_string(), 12.0),
        ("C:D".to_string(), 20.0),
    ]))?;
    sheet.set_row_height(2, Some(18.0))?;
    sheet.set_row_heights(3, 4, Some(22.0))?;
    sheet.set_row_heights_bulk(&BTreeMap::from([(4, None), (5, Some(30.0))]))?;

    assert_eq!(sheet.get_column_width("A")?, Some(12.0));
    assert_eq!(sheet.get_column_width("B")?, None);
    assert_eq!(sheet.get_column_width("D")?, Some(20.0));
    assert_eq!(sheet.get_row_height(2), Some(18.0));
    assert_eq!(sheet.get_row_height(3), Some(22.0));
    assert_eq!(sheet.get_row_height(4), Some(15.0));
    assert_eq!(sheet.get_row_height(5), Some(30.0));

    sheet.cells.insert(
        CellAddress::parse("A1")?,
        SpreadsheetCell {
            value: None,
            formula: None,
            style_index: 0,
            citations: Vec::new(),
        },
    );
    let merged = CellRange::parse("B2:C3")?;
    sheet.merged_ranges = vec![merged.clone(), merged.clone()];
    sheet.cleanup_and_validate_sheet()?;

    assert_eq!(sheet.cells, BTreeMap::new());
    assert_eq!(sheet.merged_ranges, vec![merged]);
    Ok(())
}

#[test]
fn xlsx_roundtrip_preserves_row_and_column_sizes() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("sizing.xlsx");

    let mut artifact = SpreadsheetArtifact::new(Some("Sizing".to_string()));
    let (expected_column_widths, expected_row_heights, expected_show_grid_lines) = {
        let sheet = artifact.create_sheet("Sheet1".to_string())?;
        sheet.show_grid_lines = false;
        sheet.set_value(
            CellAddress::parse("A1")?,
            Some(SpreadsheetCellValue::Integer(42)),
        )?;
        sheet.set_column_widths_bulk(&BTreeMap::from([
            ("A:B".to_string(), 12.5),
            ("D".to_string(), 18.0),
        ]))?;
        sheet.set_row_heights_bulk(&BTreeMap::from([(2, Some(24.0)), (6, Some(19.5))]))?;
        (
            sheet.column_widths.clone(),
            sheet.row_heights.clone(),
            sheet.show_grid_lines,
        )
    };
    artifact.export(&path)?;

    let restored = SpreadsheetArtifact::from_source_file(&path, None)?;
    let restored_sheet = restored.get_sheet(Some("Sheet1"), None).expect("sheet");
    assert_eq!(restored_sheet.column_widths, expected_column_widths);
    assert_eq!(restored_sheet.row_heights, expected_row_heights);
    assert_eq!(restored_sheet.show_grid_lines, expected_show_grid_lines);
    Ok(())
}

#[test]
fn manager_supports_bulk_sizes_and_row_heights() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = SpreadsheetArtifactManager::default();
    let created = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Sizing" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_sheet".to_string(),
            args: serde_json::json!({ "name": "Sheet1" }),
        },
        temp_dir.path(),
    )?;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_column_widths_bulk".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "widths": {
                    "A:B": 12.0,
                    "D": 20.0
                }
            }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_row_height".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "row_index": 2,
                "height": 18.0
            }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_row_heights".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "start_row_index": 3,
                "end_row_index": 4,
                "height": 21.0
            }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_row_heights_bulk".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "heights": {
                    "4": null,
                    "5": 25.0
                }
            }),
        },
        temp_dir.path(),
    )?;

    let row_height = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_row_height".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "row_index": 5
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(row_height.row_height, Some(25.0));

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "cleanup_and_validate_sheet".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1"
            }),
        },
        temp_dir.path(),
    )?;

    let sheet = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "get_sheet".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1"
            }),
        },
        temp_dir.path(),
    )?;
    let restored: SpreadsheetSheet =
        serde_json::from_value(sheet.serialized_dict.expect("sheet dict"))?;
    assert_eq!(
        restored.column_widths,
        BTreeMap::from([(1, 12.0), (2, 12.0), (4, 20.0)])
    );
    assert_eq!(
        restored.row_heights,
        BTreeMap::from([(2, 18.0), (3, 21.0), (5, 25.0)])
    );
    Ok(())
}

#[test]
fn manager_style_registry_and_format_summaries_work() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = SpreadsheetArtifactManager::default();
    let created = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Styles" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_sheet".to_string(),
            args: serde_json::json!({ "name": "Sheet1" }),
        },
        temp_dir.path(),
    )?;

    let text_style = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_text_style".to_string(),
            args: serde_json::json!({
                "style": {
                    "bold": true,
                    "italic": true,
                    "underline": true,
                    "font_size": 14.0,
                    "font_color": "#112233",
                    "text_alignment": "center",
                    "anchor": "middle",
                    "vertical_text_orientation": "stacked",
                    "text_rotation": 90,
                    "paragraph_spacing": true,
                    "bottom_inset": 1.0,
                    "left_inset": 2.0,
                    "right_inset": 3.0,
                    "top_inset": 4.0,
                    "font_family": "IBM Plex Sans",
                    "font_scheme": "minor",
                    "typeface": "IBM Plex Sans",
                    "font_face": {
                        "font_family": "IBM Plex Sans",
                        "font_scheme": "minor",
                        "typeface": "IBM Plex Sans"
                    }
                }
            }),
        },
        temp_dir.path(),
    )?;
    let text_style_id = text_style.style_id.expect("text style id");

    let fill = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_fill".to_string(),
            args: serde_json::json!({
                "fill": {
                    "solid_fill_color": "#ffeeaa",
                    "pattern_type": "solid",
                    "pattern_foreground_color": "#ffeeaa",
                    "pattern_background_color": "#221100",
                    "color_transforms": ["tint:0.2"],
                    "gradient_fill_type": "linear",
                    "gradient_stops": [
                        { "position": 0.0, "color": "#ffeeaa" },
                        { "position": 1.0, "color": "#aa5500" }
                    ],
                    "gradient_kind": "linear",
                    "angle": 45.0,
                    "scaled": true,
                    "path_type": "rect",
                    "fill_rectangle": {
                        "left": 0.0,
                        "right": 1.0,
                        "top": 0.0,
                        "bottom": 1.0
                    },
                    "image_reference": "image://fill"
                }
            }),
        },
        temp_dir.path(),
    )?;
    let fill_id = fill.style_id.expect("fill id");

    let border = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_border".to_string(),
            args: serde_json::json!({
                "border": {
                    "top": { "style": "solid", "color": "#111111" },
                    "right": { "style": "dashed", "color": "#222222" },
                    "bottom": { "style": "double", "color": "#333333" },
                    "left": { "style": "solid", "color": "#444444" }
                }
            }),
        },
        temp_dir.path(),
    )?;
    let border_id = border.style_id.expect("border id");

    let number_format = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_number_format".to_string(),
            args: serde_json::json!({
                "number_format": {
                    "format_id": 4
                }
            }),
        },
        temp_dir.path(),
    )?;
    let number_format_id = number_format.style_id.expect("number format id");

    let base_format = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_cell_format".to_string(),
            args: serde_json::json!({
                "format": {
                    "text_style_id": text_style_id,
                    "number_format_id": number_format_id,
                    "alignment": {
                        "horizontal": "center",
                        "vertical": "middle"
                    }
                }
            }),
        },
        temp_dir.path(),
    )?;
    let base_format_id = base_format.style_id.expect("base format id");

    let derived_format = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_cell_format".to_string(),
            args: serde_json::json!({
                "format": {
                    "fill_id": fill_id,
                    "border_id": border_id,
                    "wrap_text": true,
                    "base_cell_style_format_id": base_format_id
                }
            }),
        },
        temp_dir.path(),
    )?;
    let derived_format_id = derived_format.style_id.expect("derived format id");

    let merged_format = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_cell_format".to_string(),
            args: serde_json::json!({
                "source_format_id": derived_format_id,
                "merge_with_existing_components": true,
                "format": {
                    "alignment": {
                        "vertical": "bottom"
                    }
                }
            }),
        },
        temp_dir.path(),
    )?;
    let merged_format_id = merged_format.style_id.expect("merged format id");

    let differential_format = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_differential_format".to_string(),
            args: serde_json::json!({
                "format": {
                    "fill_id": fill_id,
                    "wrap_text": true
                }
            }),
        },
        temp_dir.path(),
    )?;
    let differential_format_id = differential_format.style_id.expect("dxf id");

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_cell_style".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "address": "A1",
                "style_index": merged_format_id
            }),
        },
        temp_dir.path(),
    )?;

    let summary = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_cell_format_summary".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "address": "A1"
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        summary.cell_format_summary,
        Some(SpreadsheetCellFormatSummary {
            style_index: merged_format_id,
            text_style: Some(SpreadsheetTextStyle {
                bold: Some(true),
                italic: Some(true),
                underline: Some(true),
                font_size: Some(14.0),
                font_color: Some("#112233".to_string()),
                text_alignment: Some("center".to_string()),
                anchor: Some("middle".to_string()),
                vertical_text_orientation: Some("stacked".to_string()),
                text_rotation: Some(90),
                paragraph_spacing: Some(true),
                bottom_inset: Some(1.0),
                left_inset: Some(2.0),
                right_inset: Some(3.0),
                top_inset: Some(4.0),
                font_family: Some("IBM Plex Sans".to_string()),
                font_scheme: Some("minor".to_string()),
                typeface: Some("IBM Plex Sans".to_string()),
                font_face: Some(SpreadsheetFontFace {
                    font_family: Some("IBM Plex Sans".to_string()),
                    font_scheme: Some("minor".to_string()),
                    typeface: Some("IBM Plex Sans".to_string()),
                }),
            }),
            fill: Some(SpreadsheetFill {
                solid_fill_color: Some("#ffeeaa".to_string()),
                pattern_type: Some("solid".to_string()),
                pattern_foreground_color: Some("#ffeeaa".to_string()),
                pattern_background_color: Some("#221100".to_string()),
                color_transforms: vec!["tint:0.2".to_string()],
                gradient_fill_type: Some("linear".to_string()),
                gradient_stops: vec![
                    crate::SpreadsheetGradientStop {
                        position: 0.0,
                        color: "#ffeeaa".to_string(),
                    },
                    crate::SpreadsheetGradientStop {
                        position: 1.0,
                        color: "#aa5500".to_string(),
                    },
                ],
                gradient_kind: Some("linear".to_string()),
                angle: Some(45.0),
                scaled: Some(true),
                path_type: Some("rect".to_string()),
                fill_rectangle: Some(crate::SpreadsheetFillRectangle {
                    left: 0.0,
                    right: 1.0,
                    top: 0.0,
                    bottom: 1.0,
                }),
                image_reference: Some("image://fill".to_string()),
            }),
            border: Some(crate::SpreadsheetBorder {
                top: Some(crate::SpreadsheetBorderLine {
                    style: Some("solid".to_string()),
                    color: Some("#111111".to_string()),
                }),
                right: Some(crate::SpreadsheetBorderLine {
                    style: Some("dashed".to_string()),
                    color: Some("#222222".to_string()),
                }),
                bottom: Some(crate::SpreadsheetBorderLine {
                    style: Some("double".to_string()),
                    color: Some("#333333".to_string()),
                }),
                left: Some(crate::SpreadsheetBorderLine {
                    style: Some("solid".to_string()),
                    color: Some("#444444".to_string()),
                }),
            }),
            alignment: Some(crate::SpreadsheetAlignment {
                horizontal: Some("center".to_string()),
                vertical: Some("bottom".to_string()),
            }),
            number_format: Some(SpreadsheetNumberFormat {
                format_id: Some(4),
                format_code: Some("#,##0.00".to_string()),
            }),
            wrap_text: Some(true),
        })
    );

    let retrieved_format = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_cell_format".to_string(),
            args: serde_json::json!({ "id": merged_format_id }),
        },
        temp_dir.path(),
    )?;
    let retrieved_format: SpreadsheetCellFormat =
        serde_json::from_value(retrieved_format.serialized_dict.expect("cell format"))?;
    assert_eq!(
        retrieved_format,
        SpreadsheetCellFormat {
            text_style_id: None,
            fill_id: Some(fill_id),
            border_id: Some(border_id),
            alignment: Some(crate::SpreadsheetAlignment {
                horizontal: None,
                vertical: Some("bottom".to_string()),
            }),
            number_format_id: None,
            wrap_text: Some(true),
            base_cell_style_format_id: Some(base_format_id),
        }
    );

    let retrieved_number_format = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_number_format".to_string(),
            args: serde_json::json!({ "id": number_format_id }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        serde_json::from_value::<SpreadsheetNumberFormat>(
            retrieved_number_format
                .serialized_dict
                .expect("number format")
        )?,
        SpreadsheetNumberFormat {
            format_id: Some(4),
            format_code: Some("#,##0.00".to_string()),
        }
    );

    let retrieved_text_style = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_text_style".to_string(),
            args: serde_json::json!({ "id": text_style_id }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        serde_json::from_value::<SpreadsheetTextStyle>(
            retrieved_text_style.serialized_dict.expect("text style")
        )?
        .bold,
        Some(true)
    );

    let range_summary = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_range_format_summary".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "range": "A1:B2"
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(range_summary.top_left_style_index, Some(merged_format_id));
    assert_eq!(
        range_summary
            .range_format
            .as_ref()
            .map(|format| format.range.clone()),
        Some("A1:B2".to_string())
    );

    let retrieved_dxf = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "get_differential_format".to_string(),
            args: serde_json::json!({ "id": differential_format_id }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        serde_json::from_value::<crate::SpreadsheetDifferentialFormat>(
            retrieved_dxf.serialized_dict.expect("differential format")
        )?
        .wrap_text,
        Some(true)
    );
    Ok(())
}

#[test]
fn sheet_references_resolve_cells_and_ranges() -> Result<(), Box<dyn std::error::Error>> {
    let sheet = SpreadsheetSheet::new("Sheet1".to_string());
    assert_eq!(
        sheet.reference("A1")?,
        SpreadsheetSheetReference::Cell {
            cell_ref: sheet.cell_ref("A1")?,
        }
    );
    assert_eq!(
        sheet.reference("A1:B2")?,
        SpreadsheetSheetReference::Range {
            range_ref: sheet.range_ref("A1:B2")?,
        }
    );
    Ok(())
}

#[test]
fn manager_get_reference_and_xlsx_import_preserve_workbook_name()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("named.xlsx");

    let mut artifact = SpreadsheetArtifact::new(Some("Named Workbook".to_string()));
    artifact.create_sheet("Sheet1".to_string())?.set_value(
        CellAddress::parse("A1")?,
        Some(SpreadsheetCellValue::Integer(9)),
    )?;
    artifact.export(&path)?;

    let restored = SpreadsheetArtifact::from_source_file(&path, None)?;
    assert_eq!(restored.name, Some("Named Workbook".to_string()));

    let mut manager = SpreadsheetArtifactManager::default();
    let imported = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: None,
            action: "read".to_string(),
            args: serde_json::json!({ "path": path }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = imported.artifact_id;

    let cell_reference = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_reference".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "reference": "A1"
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        cell_reference
            .raw_cell
            .as_ref()
            .and_then(|cell| cell.value.clone()),
        Some(SpreadsheetCellValue::Integer(9))
    );

    let range_reference = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "get_reference".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "reference": "A1:B2"
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        range_reference
            .range_ref
            .as_ref()
            .map(|range_ref| range_ref.address.clone()),
        Some("A1:B2".to_string())
    );
    Ok(())
}

#[test]
fn manager_render_actions_support_workbook_sheet_and_range()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = SpreadsheetArtifactManager::default();
    let created = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Render" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;

    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_sheet".to_string(),
            args: serde_json::json!({ "name": "Sheet1" }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_range_values".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "range": "A1:C4",
                "values": [
                    ["h1", "h2", "h3"],
                    ["a", 1, 2],
                    ["b", 3, 4],
                    ["c", 5, 6]
                ]
            }),
        },
        temp_dir.path(),
    )?;

    let workbook = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "render_workbook".to_string(),
            args: serde_json::json!({
                "output_path": temp_dir.path().join("workbook-previews"),
                "include_headers": false
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(workbook.exported_paths.len(), 1);
    assert!(workbook.exported_paths[0].exists());

    let sheet = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "render_sheet".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "output_path": temp_dir.path().join("sheet-preview.html"),
                "center_address": "B3",
                "width": 220,
                "height": 90,
                "scale": 1.5,
                "performance_mode": true
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(sheet.exported_paths.len(), 1);
    assert!(sheet.exported_paths[0].exists());
    assert!(
        sheet
            .rendered_html
            .as_ref()
            .is_some_and(|html| html.contains("data-performance-mode=\"true\""))
    );

    let range = manager.execute(
        SpreadsheetArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "render_range".to_string(),
            args: serde_json::json!({
                "sheet_name": "Sheet1",
                "range": "A2:C4",
                "output_path": temp_dir.path().join("range-preview.html"),
                "include_headers": true
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(range.exported_paths.len(), 1);
    assert_eq!(
        range
            .range_ref
            .as_ref()
            .map(|range_ref| range_ref.address.clone()),
        Some("A2:C4".to_string())
    );
    assert!(
        range
            .rendered_html
            .as_ref()
            .is_some_and(|html| html.contains("<th>A</th>"))
    );
    Ok(())
}
