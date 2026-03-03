use super::presentation_artifact::*;
use pretty_assertions::assert_eq;
use std::io::Read;

fn zip_entry_text(
    path: &std::path::Path,
    entry_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    Ok(text)
}

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
        payload: Some(ImagePayload {
            bytes: Vec::new(),
            format: "PNG".to_string(),
            width_px: 400,
            height_px: 200,
        }),
        fit_mode: ImageFitMode::Contain,
        crop: None,
        lock_aspect_ratio: true,
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
fn preview_image_writer_supports_jpeg_and_scale() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let source_path = temp_dir.path().join("preview.png");
    image::RgbaImage::from_pixel(80, 40, image::Rgba([0x22, 0x66, 0xAA, 0xFF]))
        .save(&source_path)?;
    let target_path = temp_dir.path().join("preview.jpg");
    write_preview_image(
        &source_path,
        &target_path,
        PreviewOutputFormat::Jpeg,
        0.5,
        82,
        "test",
    )?;
    let rendered = image::open(&target_path)?;
    assert_eq!((rendered.width(), rendered.height()), (40, 20));
    assert_eq!(
        image::ImageFormat::from_path(&target_path)?,
        image::ImageFormat::Jpeg
    );
    assert!(!source_path.exists());
    Ok(())
}

#[test]
fn image_uris_can_add_and_replace_images() -> Result<(), Box<dyn std::error::Error>> {
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        16,
        8,
        image::Rgba([0x11, 0x88, 0xCC, 0xFF]),
    ))
    .write_to(&mut image_bytes, image::ImageFormat::Png)?;
    let png = image_bytes.into_inner();

    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    let port = server.server_addr().to_ip().expect("ip addr").port();
    let server_thread = std::thread::spawn(move || {
        for request in server.incoming_requests().take(2) {
            let response = tiny_http::Response::from_data(png.clone()).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
                    .expect("header"),
            );
            request.respond(response).expect("respond");
        }
    });

    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Remote Images" }),
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
    let remote_uri = format!("http://127.0.0.1:{port}/image.png");
    let added = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_image".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "uri": remote_uri,
                "position": { "left": 32, "top": 48, "width": 120, "height": 60 }
            }),
        },
        temp_dir.path(),
    )?;
    let element_id = added
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.last())
        .cloned()
        .expect("image id");
    assert_eq!(
        added
            .artifact_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.slides.first())
            .map(|slide| slide.element_types.clone()),
        Some(vec!["image".to_string()])
    );
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "replace_image".to_string(),
            args: serde_json::json!({
                "element_id": format!("im/{element_id}"),
                "uri": format!("http://127.0.0.1:{port}/updated.png"),
                "fit": "contain"
            }),
        },
        temp_dir.path(),
    )?;
    let inspect = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "inspect".to_string(),
            args: serde_json::json!({ "kind": "image" }),
        },
        temp_dir.path(),
    )?;
    assert!(
        inspect
            .inspect_ndjson
            .expect("image inspect")
            .contains("\"fit\":\"Contain\"")
    );
    server_thread.join().expect("server thread");
    Ok(())
}

#[test]
fn active_slide_can_be_set_and_tracks_reorders() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Active Slide" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;
    for _ in 0..3 {
        manager.execute(
            PresentationArtifactRequest {
                artifact_id: Some(artifact_id.clone()),
                action: "add_slide".to_string(),
                args: serde_json::json!({}),
            },
            temp_dir.path(),
        )?;
    }
    let set_active = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_active_slide".to_string(),
            args: serde_json::json!({ "slide_index": 2 }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(set_active.active_slide_index, Some(2));
    assert_eq!(
        set_active.slide_list.as_ref().map(|slides| slides
            .iter()
            .map(|slide| slide.is_active)
            .collect::<Vec<_>>()),
        Some(vec![false, false, true])
    );

    let moved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "move_slide".to_string(),
            args: serde_json::json!({ "from_index": 2, "to_index": 0 }),
        },
        temp_dir.path(),
    )?;
    let summary = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_summary".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(summary.active_slide_index, Some(0));
    assert_eq!(
        summary.slide_list.as_ref().map(|slides| slides
            .iter()
            .map(|slide| slide.is_active)
            .collect::<Vec<_>>()),
        Some(vec![true, false, false])
    );

    let inspect = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "inspect".to_string(),
            args: serde_json::json!({ "kind": "deck,slide" }),
        },
        temp_dir.path(),
    )?;
    let inspect_ndjson = inspect.inspect_ndjson.expect("inspect");
    assert!(inspect_ndjson.contains("\"activeSlideIndex\":0"));
    assert!(inspect_ndjson.contains("\"isActive\":true"));

    let active_slide_id = moved
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .map(|slide| slide.slide_id.clone())
        .expect("active slide id");
    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sl/{active_slide_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("isActive"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );

    let deleted = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "delete_slide".to_string(),
            args: serde_json::json!({ "slide_index": 0 }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        deleted
            .artifact_snapshot
            .as_ref()
            .map(|snapshot| snapshot.slide_count),
        Some(2)
    );
    let after_delete = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(deleted.artifact_id.clone()),
            action: "list_slides".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(after_delete.active_slide_index, Some(0));
    assert_eq!(
        after_delete.slide_list.as_ref().map(|slides| slides
            .iter()
            .map(|slide| slide.is_active)
            .collect::<Vec<_>>()),
        Some(vec![true, false])
    );
    Ok(())
}

#[test]
fn text_replace_and_insert_helpers_update_text_elements() -> Result<(), Box<dyn std::error::Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Text Helpers" }),
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
    let added = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "Revenue up 24%",
                "position": { "left": 24, "top": 24, "width": 240, "height": 80 }
            }),
        },
        temp_dir.path(),
    )?;
    let element_id = added
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("text id");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "replace_text".to_string(),
            args: serde_json::json!({
                "element_id": format!("sh/{element_id}"),
                "search": "24%",
                "replace": "31%"
            }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "insert_text_after".to_string(),
            args: serde_json::json!({
                "element_id": format!("sh/{element_id}"),
                "after": "Revenue",
                "insert": " QoQ"
            }),
        },
        temp_dir.path(),
    )?;
    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sh/{element_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("text"))
            .and_then(serde_json::Value::as_str),
        Some("Revenue QoQ up 31%")
    );
    Ok(())
}

#[test]
fn hyperlinks_are_inspectable_and_exported() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Hyperlinks" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;
    for _ in 0..2 {
        manager.execute(
            PresentationArtifactRequest {
                artifact_id: Some(artifact_id.clone()),
                action: "add_slide".to_string(),
                args: serde_json::json!({}),
            },
            temp_dir.path(),
        )?;
    }

    let text = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "Open roadmap",
                "position": { "left": 24, "top": 24, "width": 220, "height": 60 }
            }),
        },
        temp_dir.path(),
    )?;
    let text_id = text
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("text id");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_hyperlink".to_string(),
            args: serde_json::json!({
                "element_id": format!("sh/{text_id}"),
                "link_type": "url",
                "url": "https://example.com/roadmap",
                "tooltip": "Roadmap",
                "highlight_click": false
            }),
        },
        temp_dir.path(),
    )?;

    let shape = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "geometry": "rounded_rectangle",
                "position": { "left": 24, "top": 120, "width": 220, "height": 72 },
                "text": "Jump to appendix"
            }),
        },
        temp_dir.path(),
    )?;
    let shape_id = shape
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.last())
        .cloned()
        .expect("shape id");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_hyperlink".to_string(),
            args: serde_json::json!({
                "element_id": format!("sh/{shape_id}"),
                "link_type": "slide",
                "slide_index": 1,
                "tooltip": "Appendix"
            }),
        },
        temp_dir.path(),
    )?;

    let inspect = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "inspect".to_string(),
            args: serde_json::json!({ "kind": "textbox,shape" }),
        },
        temp_dir.path(),
    )?;
    let inspect_ndjson = inspect.inspect_ndjson.expect("inspect");
    assert!(inspect_ndjson.contains("\"type\":\"url\""));
    assert!(inspect_ndjson.contains("\"url\":\"https://example.com/roadmap\""));
    assert!(inspect_ndjson.contains("\"type\":\"slide\""));
    assert!(inspect_ndjson.contains("\"slideIndex\":1"));

    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sh/{text_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("hyperlink"))
            .and_then(|hyperlink| hyperlink.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://example.com/roadmap")
    );

    let export_path = temp_dir.path().join("hyperlinks.pptx");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;
    let slide_xml = zip_entry_text(
        &temp_dir.path().join("hyperlinks.pptx"),
        "ppt/slides/slide1.xml",
    )?;
    let rels_xml = zip_entry_text(
        &temp_dir.path().join("hyperlinks.pptx"),
        "ppt/slides/_rels/slide1.xml.rels",
    )?;
    assert!(slide_xml.contains("hlinkClick"));
    assert!(rels_xml.contains("https://example.com/roadmap"));
    assert!(rels_xml.contains("slide2.xml"));

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_hyperlink".to_string(),
            args: serde_json::json!({
                "element_id": format!("sh/{text_id}"),
                "clear": true
            }),
        },
        temp_dir.path(),
    )?;
    let cleared = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sh/{text_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        cleared
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("hyperlink")),
        None
    );
    Ok(())
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
fn image_placeholders_and_anchor_updates_work() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Images" }),
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
    let placeholder = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_image".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "position": { "left": 24, "top": 24, "width": 200, "height": 120 },
                "fit": "contain",
                "prompt": "Generate a hero illustration",
                "alt": "Hero placeholder"
            }),
        },
        temp_dir.path(),
    )?;
    let image_anchor = placeholder
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .map(|id| format!("im/{id}"))
        .expect("image anchor");

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "update_shape_style".to_string(),
            args: serde_json::json!({
                "element_id": image_anchor,
                "fit": "cover",
                "crop": { "left": 0.1, "top": 0.0, "right": 0.1, "bottom": 0.0 },
                "lock_aspect_ratio": true
            }),
        },
        temp_dir.path(),
    )?;

    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": image_anchor }),
        },
        temp_dir.path(),
    )?;
    let record = resolved.resolved_record.expect("resolved image");
    assert_eq!(
        record.get("kind").and_then(serde_json::Value::as_str),
        Some("image")
    );
    assert_eq!(
        record
            .get("isPlaceholder")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        record
            .get("lockAspectRatio")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    Ok(())
}

#[test]
fn image_partial_resize_respects_lock_aspect_ratio() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Image Resize" }),
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
    let added = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_image".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "position": { "left": 10, "top": 10, "width": 200, "height": 100 },
                "prompt": "Placeholder image",
                "lock_aspect_ratio": true
            }),
        },
        temp_dir.path(),
    )?;
    let image_anchor = added
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .map(|id| format!("im/{id}"))
        .expect("image anchor");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "update_shape_style".to_string(),
            args: serde_json::json!({
                "element_id": image_anchor,
                "position": { "width": 120 }
            }),
        },
        temp_dir.path(),
    )?;
    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": image_anchor }),
        },
        temp_dir.path(),
    )?;
    let bbox = resolved
        .resolved_record
        .as_ref()
        .and_then(|record| record.get("bbox"))
        .and_then(serde_json::Value::as_array)
        .expect("bbox");
    assert_eq!(bbox[2].as_u64(), Some(120));
    assert_eq!(bbox[3].as_u64(), Some(60));
    Ok(())
}

#[test]
fn connectors_support_arrows_and_inspect() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Connectors" }),
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
    let added = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_connector".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "connector_type": "elbow",
                "start": { "left": 20, "top": 20 },
                "end": { "left": 180, "top": 160 },
                "line": { "color": "#ff0000", "width": 2, "style": "dash-dot" },
                "start_arrow": "none",
                "end_arrow": "triangle",
                "arrow_size": "large",
                "label": "flow"
            }),
        },
        temp_dir.path(),
    )?;
    let connector_id = added
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .map(|id| format!("cn/{id}"))
        .expect("connector id");
    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": connector_id }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("kind"))
            .and_then(serde_json::Value::as_str),
        Some("connector")
    );
    let inspect = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "inspect".to_string(),
            args: serde_json::json!({ "kind": "connector" }),
        },
        temp_dir.path(),
    )?;
    assert!(
        inspect
            .inspect_ndjson
            .expect("connector inspect")
            .contains("\"kind\":\"connector\"")
    );
    Ok(())
}

#[test]
fn z_order_helpers_resequence_elements() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Z Order" }),
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
    let first = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "A",
                "position": { "left": 10, "top": 10, "width": 100, "height": 40 }
            }),
        },
        temp_dir.path(),
    )?;
    let second = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "B",
                "position": { "left": 20, "top": 20, "width": 100, "height": 40 }
            }),
        },
        temp_dir.path(),
    )?;
    let first_id = first
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("first id");
    let second_id = second
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.last())
        .cloned()
        .expect("second id");
    let sent_back = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "send_to_back".to_string(),
            args: serde_json::json!({ "element_id": format!("sh/{second_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        sent_back
            .artifact_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.slides.first())
            .map(|slide| slide.element_ids.clone()),
        Some(vec![second_id.clone(), first_id.clone()])
    );
    let brought_front = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "bring_to_front".to_string(),
            args: serde_json::json!({ "element_id": format!("sh/{second_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        brought_front
            .artifact_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.slides.first())
            .map(|slide| slide.element_ids.clone()),
        Some(vec![first_id, second_id])
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
