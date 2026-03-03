use pretty_assertions::assert_eq;

use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactManager;
use crate::SpreadsheetArtifactRequest;
use crate::SpreadsheetCellValue;

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
