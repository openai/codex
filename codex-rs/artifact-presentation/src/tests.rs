use super::presentation_artifact::*;
use pretty_assertions::assert_eq;

#[test]
fn manager_can_create_add_text_and_export() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let create_response = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Demo" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = create_response.artifact_id;

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;

    let add_text = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "hello",
                "position": { "left": 40, "top": 40, "width": 200, "height": 80 }
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        add_text
            .artifact_snapshot
            .as_ref()
            .map(|snapshot| snapshot.slide_count),
        Some(1)
    );

    let export_path = temp_dir.path().join("deck.pptx");
    let export = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(export.exported_paths.len(), 1);
    assert!(export.exported_paths[0].exists());
    Ok(())
}

#[test]
fn manager_can_import_exported_presentation() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Roundtrip" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id.clone();
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "add_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "geometry": "rectangle",
                "position": { "left": 24, "top": 24, "width": 180, "height": 120 },
                "text": "shape"
            }),
        },
        temp_dir.path(),
    )?;
    let export_path = temp_dir.path().join("roundtrip.pptx");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;

    let imported = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "import_pptx".to_string(),
            args: serde_json::json!({ "path": "roundtrip.pptx" }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        imported
            .artifact_snapshot
            .as_ref()
            .map(|snapshot| snapshot.slide_count),
        Some(1)
    );
    Ok(())
}

#[test]
fn image_fit_contain_preserves_aspect_ratio() {
    let image = ImageElement {
        element_id: "element_1".to_string(),
        frame: Rect {
            left: 10,
            top: 10,
            width: 200,
            height: 200,
        },
        payload: ImagePayload {
            bytes: Vec::new(),
            format: "PNG".to_string(),
            width_px: 400,
            height_px: 200,
        },
        fit_mode: ImageFitMode::Contain,
        alt_text: None,
        prompt: None,
        is_placeholder: false,
        z_order: 0,
    };

    let (left, top, width, height, crop) = fit_image(&image);
    assert_eq!((left, top, width, height), (10, 60, 200, 100));
    assert_eq!(crop, None);
}

#[test]
fn manager_supports_layout_theme_notes_and_inspect() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({
                "name": "Deck",
                "theme": {
                    "color_scheme": {
                        "accent1": "#123456",
                        "bg1": "#FFFFFF",
                        "tx1": "#111111"
                    },
                    "major_font": "Aptos"
                }
            }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id.clone();

    let master_layouts = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_layout".to_string(),
            args: serde_json::json!({ "name": "Brand Master", "kind": "master" }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(master_layouts.layout_list.as_ref().map(Vec::len), Some(1));
    let master_id = master_layouts.layout_list.unwrap()[0].layout_id.clone();

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_layout_placeholder".to_string(),
            args: serde_json::json!({
                "layout_id": master_id,
                "name": "title",
                "placeholder_type": "title",
                "text": "Placeholder title",
                "position": { "left": 48, "top": 48, "width": 500, "height": 60 }
            }),
        },
        temp_dir.path(),
    )?;

    let child_layouts = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_layout".to_string(),
            args: serde_json::json!({
                "name": "Title Slide",
                "kind": "layout",
                "parent_layout_id": master_id
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(child_layouts.layout_list.as_ref().map(Vec::len), Some(2));
    let layout_id = child_layouts
        .layout_list
        .as_ref()
        .and_then(|layouts| layouts.iter().find(|layout| layout.kind == "layout"))
        .map(|layout| layout.layout_id.clone())
        .expect("child layout id");

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_layout_placeholder".to_string(),
            args: serde_json::json!({
                "layout_id": layout_id,
                "name": "subtitle",
                "placeholder_type": "subtitle",
                "text": "Placeholder subtitle",
                "position": { "left": 48, "top": 128, "width": 500, "height": 48 }
            }),
        },
        temp_dir.path(),
    )?;

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({ "layout": layout_id }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_notes".to_string(),
            args: serde_json::json!({ "slide_index": 0, "text": "Speaker notes" }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "append_notes".to_string(),
            args: serde_json::json!({ "slide_index": 0, "text": "More context" }),
        },
        temp_dir.path(),
    )?;
    let layout_placeholders = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "list_layout_placeholders".to_string(),
            args: serde_json::json!({ "layout_id": layout_id }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        layout_placeholders.placeholder_list.as_ref().map(Vec::len),
        Some(2)
    );

    let slide_placeholders = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "list_slide_placeholders".to_string(),
            args: serde_json::json!({ "slide_index": 0 }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        slide_placeholders.placeholder_list.as_ref().map(Vec::len),
        Some(2)
    );

    let resolved_layout = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("ly/{layout_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved_layout
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("kind"))
            .and_then(serde_json::Value::as_str),
        Some("layout")
    );

    let inspect = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "inspect".to_string(),
            args: serde_json::json!({ "kind": "deck,slide,textbox,notes,layoutList" }),
        },
        temp_dir.path(),
    )?;
    let inspect_ndjson = inspect.inspect_ndjson.expect("inspect output");
    assert!(inspect_ndjson.contains("\"kind\":\"layout\""));
    assert!(inspect_ndjson.contains("\"kind\":\"notes\""));
    assert!(inspect_ndjson.contains("\"placeholder\":\"title\""));

    let truncated = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id),
            action: "inspect".to_string(),
            args: serde_json::json!({
                "kind": "deck,slide,textbox,notes,layoutList",
                "max_chars": 250
            }),
        },
        temp_dir.path(),
    )?;
    assert!(
        truncated
            .inspect_ndjson
            .expect("truncated inspect")
            .contains("\"kind\":\"notice\"")
    );
    Ok(())
}

#[test]
fn notes_visibility_controls_exported_notes() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Notes" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({ "notes": "Hidden notes" }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_notes_visibility".to_string(),
            args: serde_json::json!({ "slide_index": 0, "visible": false }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": "notes-hidden.pptx" }),
        },
        temp_dir.path(),
    )?;

    let imported = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "import_pptx".to_string(),
            args: serde_json::json!({ "path": "notes-hidden.pptx" }),
        },
        temp_dir.path(),
    )?;
    let summary = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(imported.artifact_id),
            action: "list_slides".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        summary
            .slide_list
            .as_ref()
            .and_then(|slides| slides.first())
            .and_then(|slide| slide.notes.clone()),
        None
    );
    Ok(())
}

#[test]
fn manager_supports_table_cell_updates_and_merges() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Tables" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    let table = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_table".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "position": { "left": 24, "top": 24, "width": 240, "height": 120 },
                "rows": [["A", "B"], ["C", "D"]],
                "style": "TableStyleMedium9"
            }),
        },
        temp_dir.path(),
    )?;
    let table_id = table
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("table id");

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "update_table_cell".to_string(),
            args: serde_json::json!({
                "element_id": table_id,
                "row": 0,
                "column": 1,
                "value": "Updated",
                "background_fill": "#eeeeee",
                "alignment": "right",
                "styling": { "bold": true }
            }),
        },
        temp_dir.path(),
    )?;
    let inspect = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "inspect".to_string(),
            args: serde_json::json!({ "kind": "table" }),
        },
        temp_dir.path(),
    )?;
    assert!(
        inspect
            .inspect_ndjson
            .expect("inspect")
            .contains("\"kind\":\"table\"")
    );

    let merged = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "merge_table_cells".to_string(),
            args: serde_json::json!({
                "element_id": table_id,
                "start_row": 0,
                "end_row": 0,
                "start_column": 0,
                "end_column": 1
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        merged
            .artifact_snapshot
            .as_ref()
            .map(|snapshot| snapshot.slide_count),
        Some(1)
    );
    Ok(())
}
