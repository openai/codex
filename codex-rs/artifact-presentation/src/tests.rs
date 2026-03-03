use super::presentation_artifact::*;
use base64::Engine;
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

fn zip_entry_names(path: &std::path::Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let archive = zip::ZipArchive::new(file)?;
    Ok(archive.file_names().map(str::to_owned).collect())
}

fn parse_ndjson_lines(ndjson: &str) -> Result<Vec<serde_json::Value>, serde_json::Error> {
    ndjson
        .lines()
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str)
        .collect()
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
fn custom_slide_size_is_written_to_exported_pptx() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({
                "name": "Custom Size",
                "slide_size": { "width": 960, "height": 540 }
            }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    let export_path = temp_dir.path().join("custom-size.pptx");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;

    let presentation_xml = zip_entry_text(
        &temp_dir.path().join("custom-size.pptx"),
        "ppt/presentation.xml",
    )?;
    assert!(presentation_xml.contains(r#"cx="12192000" cy="6858000""#));
    assert!(presentation_xml.contains(r#"p:notesSz cx="6858000" cy="12192000""#));
    Ok(())
}

#[test]
fn exported_images_are_real_pictures_with_media_parts() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let source_path = temp_dir.path().join("source.png");
    image::RgbaImage::from_pixel(24, 16, image::Rgba([0x20, 0x90, 0xD0, 0xFF]))
        .save(&source_path)?;

    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Image Export" }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id.clone()),
            action: "add_image".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "path": "source.png",
                "position": { "left": 36, "top": 48, "width": 144, "height": 96 },
                "rotation": 15,
                "flip_horizontal": true,
                "alt": "Company logo"
            }),
        },
        temp_dir.path(),
    )?;

    let export_path = temp_dir.path().join("images.pptx");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;

    let pptx_path = temp_dir.path().join("images.pptx");
    let slide_xml = zip_entry_text(&pptx_path, "ppt/slides/slide1.xml")?;
    let rels_xml = zip_entry_text(&pptx_path, "ppt/slides/_rels/slide1.xml.rels")?;
    let content_types_xml = zip_entry_text(&pptx_path, "[Content_Types].xml")?;
    let entry_names = zip_entry_names(&pptx_path)?;

    assert!(slide_xml.contains("<p:pic>"));
    assert!(slide_xml.contains(r#"descr="Company logo""#));
    assert!(slide_xml.contains(r#"r:embed="rIdImage1""#));
    assert!(slide_xml.contains(r#"<a:xfrm rot="900000" flipH="1">"#));
    assert!(!slide_xml.contains("Image Placeholder:"));
    assert!(rels_xml.contains("relationships/image"));
    assert!(rels_xml.contains(r#"Target="../media/image1.png""#));
    assert!(content_types_xml.contains(r#"Extension="png" ContentType="image/png""#));
    assert!(entry_names.contains(&"ppt/media/image1.png".to_string()));
    Ok(())
}

#[test]
fn imported_pptx_surfaces_image_elements() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let source_path = temp_dir.path().join("import-source.png");
    image::RgbaImage::from_pixel(20, 20, image::Rgba([0xD0, 0x60, 0x20, 0xFF]))
        .save(&source_path)?;

    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Image Import" }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id.clone()),
            action: "add_image".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "path": "import-source.png",
                "position": { "left": 40, "top": 52, "width": 120, "height": 120 },
                "crop": { "left": 0.1, "top": 0.0, "right": 0.05, "bottom": 0.0 },
                "rotation": -10,
                "flip_horizontal": true,
                "flip_vertical": true,
                "lock_aspect_ratio": true,
                "alt": "Imported logo"
            }),
        },
        temp_dir.path(),
    )?;
    let export_path = temp_dir.path().join("image-import-roundtrip.pptx");
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
            args: serde_json::json!({ "path": "image-import-roundtrip.pptx" }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        imported
            .artifact_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.slides.first())
            .map(|slide| slide.element_types.clone()),
        Some(vec!["image".to_string()])
    );
    let image_anchor = imported
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .map(|id| format!("im/{id}"))
        .expect("image anchor");
    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(imported.artifact_id),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": image_anchor }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("alt"))
            .and_then(serde_json::Value::as_str),
        Some("Imported logo")
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("rotation"))
            .and_then(serde_json::Value::as_i64),
        Some(-10)
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("flipHorizontal"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("flipVertical"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("lockAspectRatio"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("crop"))
            .and_then(|crop| crop.get("left"))
            .and_then(serde_json::Value::as_f64),
        Some(0.1)
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
        rotation_degrees: None,
        flip_horizontal: false,
        flip_vertical: false,
        lock_aspect_ratio: true,
        alt_text: None,
        prompt: None,
        is_placeholder: false,
        placeholder: None,
        z_order: 0,
    };

    let (left, top, width, height, crop) = fit_image(&image);
    assert_eq!((left, top, width, height), (10, 60, 200, 100));
    assert_eq!(crop, None);
}

#[test]
fn preview_image_writer_supports_jpeg_scale_and_svg() -> Result<(), Box<dyn std::error::Error>> {
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

    let svg_source_path = temp_dir.path().join("preview-svg.png");
    image::RgbaImage::from_pixel(32, 16, image::Rgba([0x55, 0xAA, 0x44, 0xFF]))
        .save(&svg_source_path)?;
    let svg_target_path = temp_dir.path().join("preview.svg");
    write_preview_image(
        &svg_source_path,
        &svg_target_path,
        PreviewOutputFormat::Svg,
        0.5,
        90,
        "test",
    )?;
    let svg = std::fs::read_to_string(&svg_target_path)?;
    assert!(svg.contains(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="8""#));
    assert!(svg.contains("data:image/png;base64,"));
    assert!(!svg_source_path.exists());
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
fn image_blobs_can_add_images() -> Result<(), Box<dyn std::error::Error>> {
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        10,
        6,
        image::Rgba([0xAA, 0x55, 0x22, 0xFF]),
    ))
    .write_to(&mut image_bytes, image::ImageFormat::Png)?;
    let blob = base64::engine::general_purpose::STANDARD.encode(image_bytes.into_inner());

    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Blob Images" }),
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
                "blob": blob,
                "position": { "left": 32, "top": 48, "width": 100, "height": 60 }
            }),
        },
        temp_dir.path(),
    )?;
    let image_id = added
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("image id");
    let proto = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "to_proto".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        proto
            .proto_json
            .as_ref()
            .and_then(|proto| proto.get("slides"))
            .and_then(serde_json::Value::as_array)
            .and_then(|slides| slides.first())
            .and_then(|slide| slide.get("elements"))
            .and_then(serde_json::Value::as_array)
            .and_then(|elements| elements.iter().find(|element| {
                element.get("elementId").and_then(serde_json::Value::as_str)
                    == Some(image_id.as_str())
            }))
            .and_then(|record| record.get("payload"))
            .and_then(|payload| payload.get("format"))
            .and_then(serde_json::Value::as_str),
        Some("PNG")
    );
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
            artifact_id: Some(deleted.artifact_id),
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
    assert_eq!(
        created
            .theme
            .as_ref()
            .map(|theme| theme.hex_color_map.clone()),
        Some(
            [
                ("accent1".to_string(), "123456".to_string()),
                ("bg1".to_string(), "FFFFFF".to_string()),
                ("tx1".to_string(), "111111".to_string()),
            ]
            .into_iter()
            .collect()
        )
    );
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
                "layout_id": "Title Slide",
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
            action: "add_layout_placeholder".to_string(),
            args: serde_json::json!({
                "layout_id": "title slide",
                "name": "hero-image",
                "placeholder_type": "picture",
                "text": "Add cover image",
                "position": { "left": 420, "top": 40, "width": 180, "height": 120 }
            }),
        },
        temp_dir.path(),
    )?;

    let added_slide = manager.execute(
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
        Some(3)
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
        Some(3)
    );
    assert_eq!(
        slide_placeholders
            .placeholder_list
            .as_ref()
            .and_then(|placeholders| placeholders
                .iter()
                .find(|placeholder| placeholder.placeholder_type == "picture")
                .map(|placeholder| (
                    placeholder.geometry.clone(),
                    placeholder.text_preview.clone()
                ))),
        Some((
            Some("Image".to_string()),
            Some("Add cover image".to_string())
        ))
    );
    let image_placeholder_id = added_slide
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.last())
        .cloned()
        .expect("image placeholder id");

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
            artifact_id: Some(artifact_id.clone()),
            action: "inspect".to_string(),
            args: serde_json::json!({ "kind": "deck,slide,textbox,image,notes,layoutList" }),
        },
        temp_dir.path(),
    )?;
    let inspect_ndjson = inspect.inspect_ndjson.expect("inspect output");
    assert!(inspect_ndjson.contains("\"kind\":\"layout\""));
    assert!(inspect_ndjson.contains("\"kind\":\"notes\""));
    assert!(inspect_ndjson.contains("\"placeholder\":\"title\""));
    assert!(inspect_ndjson.contains("\"placeholder\":\"picture\""));
    assert!(inspect_ndjson.contains("\"kind\":\"image\""));

    let resolved_image_placeholder = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("im/{image_placeholder_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved_image_placeholder
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("isPlaceholder"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        resolved_image_placeholder
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("placeholder"))
            .and_then(serde_json::Value::as_str),
        Some("picture")
    );

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
            artifact_id: Some(artifact_id.clone()),
            action: "set_slide_layout".to_string(),
            args: serde_json::json!({ "slide_index": 1, "layout_id": "title slide" }),
        },
        temp_dir.path(),
    )?;
    let inherited_placeholders = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "list_slide_placeholders".to_string(),
            args: serde_json::json!({ "slide_index": 1 }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        inherited_placeholders
            .placeholder_list
            .as_ref()
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        inherited_placeholders
            .placeholder_list
            .as_ref()
            .and_then(|placeholders| placeholders
                .iter()
                .find(|placeholder| placeholder.placeholder_type == "title")
                .map(|placeholder| placeholder.name.clone())),
        Some("title".to_string())
    );
    assert_eq!(
        inherited_placeholders
            .placeholder_list
            .as_ref()
            .and_then(|placeholders| placeholders
                .iter()
                .find(|placeholder| placeholder.placeholder_type == "picture")
                .map(|placeholder| placeholder.geometry.clone())),
        Some(Some("Image".to_string()))
    );

    let truncated = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id),
            action: "inspect".to_string(),
            args: serde_json::json!({
                "kind": "deck,slide,textbox,image,notes,layoutList",
                "max_chars": 250
            }),
        },
        temp_dir.path(),
    )?;
    assert!(
        truncated
            .inspect_ndjson
            .expect("truncated inspect")
            .contains("\"noticeType\":\"truncation\"")
    );
    Ok(())
}

#[test]
fn named_text_styles_apply_to_text_and_tables() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({
                "name": "Styles",
                "theme": {
                    "color_scheme": {
                        "tx1": "#222222"
                    },
                    "major_font": "Aptos Display",
                    "minor_font": "Aptos"
                }
            }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;
    let described = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "describe_styles".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        described
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("styles"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(5)
    );
    let title_style = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "get_style".to_string(),
            args: serde_json::json!({ "name": "title" }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        title_style.resolved_record,
        Some(serde_json::json!({
            "kind": "textStyle",
            "id": "st/title",
            "name": "title",
            "builtIn": true,
            "style": {
                "styleName": "title",
                "fontSize": 28,
                "fontFamily": "Aptos Display",
                "color": "222222",
                "alignment": "left",
                "bold": true,
                "italic": false,
                "underline": false
            }
        }))
    );
    let custom_style = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_style".to_string(),
            args: serde_json::json!({
                "name": "callout",
                "font_size": 18,
                "color": "#336699",
                "italic": true,
                "underline": true
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        custom_style.resolved_record,
        Some(serde_json::json!({
            "kind": "textStyle",
            "id": "st/callout",
            "name": "callout",
            "builtIn": false,
            "style": {
                "styleName": "callout",
                "fontSize": 18,
                "fontFamily": serde_json::Value::Null,
                "color": "336699",
                "alignment": serde_json::Value::Null,
                "bold": false,
                "italic": true,
                "underline": true
            }
        }))
    );

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    let text_added = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "Styled title",
                "position": { "left": 40, "top": 40, "width": 220, "height": 50 },
                "style": "title",
                "underline": true
            }),
        },
        temp_dir.path(),
    )?;
    let text_id = text_added
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("text id");
    let table_added = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_table".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "position": { "left": 40, "top": 120, "width": 180, "height": 80 },
                "rows": [["A"]],
            }),
        },
        temp_dir.path(),
    )?;
    let table_id = table_added
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.last())
        .cloned()
        .expect("table id");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "update_table_cell".to_string(),
            args: serde_json::json!({
                "element_id": table_id,
                "row": 0,
                "column": 0,
                "value": "Styled cell",
                "styling": {
                    "style": "callout"
                }
            }),
        },
        temp_dir.path(),
    )?;

    let resolved_text = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sh/{text_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved_text
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("textStyle"))
            .cloned(),
        Some(serde_json::json!({
            "styleName": "title",
            "fontSize": 28,
            "fontFamily": "Aptos Display",
            "color": "222222",
            "alignment": "left",
            "bold": true,
            "italic": false,
            "underline": true
        }))
    );
    let resolved_table = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("tb/{table_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved_table
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("cellTextStyles"))
            .cloned(),
        Some(serde_json::json!([
            [
                {
                    "styleName": "callout",
                    "fontSize": 18,
                    "fontFamily": serde_json::Value::Null,
                    "color": "336699",
                    "alignment": serde_json::Value::Null,
                    "bold": false,
                    "italic": true,
                    "underline": true
                }
            ]
        ]))
    );
    Ok(())
}

#[test]
fn layout_names_resolve_in_slide_actions_and_insert_defaults_after_active_slide()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Layouts by Name" }),
        },
        temp_dir.path(),
    )?;
    let artifact_id = created.artifact_id;

    let layout_created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "create_layout".to_string(),
            args: serde_json::json!({ "name": "Title Slide" }),
        },
        temp_dir.path(),
    )?;
    let layout_id = layout_created
        .layout_list
        .as_ref()
        .and_then(|layouts| layouts.first())
        .map(|layout| layout.layout_id.clone())
        .expect("layout id");

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_layout_placeholder".to_string(),
            args: serde_json::json!({
                "layout_id": layout_id,
                "name": "title",
                "placeholder_type": "title",
                "text": "Placeholder title"
            }),
        },
        temp_dir.path(),
    )?;

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_slide".to_string(),
            args: serde_json::json!({ "layout": "Title Slide" }),
        },
        temp_dir.path(),
    )?;
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
            artifact_id: Some(artifact_id.clone()),
            action: "set_active_slide".to_string(),
            args: serde_json::json!({ "slide_index": 0 }),
        },
        temp_dir.path(),
    )?;

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "insert_slide".to_string(),
            args: serde_json::json!({ "layout": "title slide" }),
        },
        temp_dir.path(),
    )?;
    let inserted = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "list_slides".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        inserted.slide_list.as_ref().map(|slides| slides
            .iter()
            .map(|slide| slide.layout_id.clone())
            .collect::<Vec<_>>()),
        Some(vec![
            Some("layout_1".to_string()),
            Some("layout_1".to_string()),
            None
        ])
    );
    assert_eq!(
        inserted.slide_list.as_ref().map(|slides| slides
            .iter()
            .map(|slide| slide.is_active)
            .collect::<Vec<_>>()),
        Some(vec![true, false, false])
    );

    let placeholders = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "list_layout_placeholders".to_string(),
            args: serde_json::json!({ "layout_id": "TITLE SLIDE" }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        placeholders.placeholder_list.as_ref().map(|entries| entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect::<Vec<_>>()),
        Some(vec!["title".to_string()])
    );

    let child_layout = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(placeholders.artifact_id),
            action: "create_layout".to_string(),
            args: serde_json::json!({
                "name": "Child Layout",
                "parent_layout_id": "title slide"
            }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        child_layout.layout_list.as_ref().map(|layouts| layouts
            .iter()
            .find(|layout| layout.name == "Child Layout")
            .and_then(|layout| layout.parent_layout_id.clone())),
        Some(Some("layout_1".to_string()))
    );
    Ok(())
}

#[test]
fn inspect_supports_filters_target_windows_and_shape_text_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Inspect Filters" }),
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

    let first_text = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "First KPI",
                "position": { "left": 10, "top": 10, "width": 120, "height": 40 }
            }),
        },
        temp_dir.path(),
    )?;
    let second_text = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "Second KPI",
                "position": { "left": 10, "top": 60, "width": 120, "height": 40 }
            }),
        },
        temp_dir.path(),
    )?;
    let third_text = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_text_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "Third KPI",
                "position": { "left": 10, "top": 110, "width": 120, "height": 40 }
            }),
        },
        temp_dir.path(),
    )?;
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "set_notes".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "text": "Speaker note"
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
                "geometry": "rectangle",
                "position": { "left": 180, "top": 10, "width": 180, "height": 90 },
                "text": "Detailed\nShape KPI"
            }),
        },
        temp_dir.path(),
    )?;

    let middle_text_id = second_text
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.get(1))
        .cloned()
        .expect("middle text id");
    let first_text_id = first_text
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("first text id");
    let third_text_id = third_text
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.get(2))
        .cloned()
        .expect("third text id");
    let shape_id = shape
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.get(3))
        .cloned()
        .expect("shape id");
    let slide_id = shape
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .map(|slide| slide.slide_id.clone())
        .expect("slide id");

    let filtered = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "inspect".to_string(),
            args: serde_json::json!({
                "include": "shape,notes",
                "exclude": "notes",
                "search": "Detailed"
            }),
        },
        temp_dir.path(),
    )?;
    let filtered_records = parse_ndjson_lines(
        filtered
            .inspect_ndjson
            .as_deref()
            .expect("filtered inspect"),
    )?;
    assert_eq!(filtered_records.len(), 1);
    assert_eq!(
        filtered_records[0],
        serde_json::json!({
            "kind": "shape",
            "id": format!("sh/{shape_id}"),
            "slide": 1,
            "geometry": "Rectangle",
            "text": "Detailed\nShape KPI",
            "textStyle": {
                "styleName": serde_json::Value::Null,
                "fontSize": serde_json::Value::Null,
                "fontFamily": serde_json::Value::Null,
                "color": serde_json::Value::Null,
                "alignment": serde_json::Value::Null,
                "bold": false,
                "italic": false,
                "underline": false
            },
            "rotation": serde_json::Value::Null,
            "flipHorizontal": false,
            "flipVertical": false,
            "bbox": [180, 10, 180, 90],
            "bboxUnit": "points",
            "textPreview": "Detailed | Shape KPI",
            "textChars": 18,
            "textLines": 2
        })
    );

    let targeted = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "inspect".to_string(),
            args: serde_json::json!({
                "include": "textbox",
                "target": {
                    "id": format!("sh/{middle_text_id}"),
                    "before_lines": 1,
                    "after_lines": 1
                }
            }),
        },
        temp_dir.path(),
    )?;
    let targeted_records = parse_ndjson_lines(
        targeted
            .inspect_ndjson
            .as_deref()
            .expect("targeted inspect"),
    )?;
    assert_eq!(
        targeted_records
            .iter()
            .filter_map(|record| record.get("id").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        vec![
            format!("sh/{first_text_id}"),
            format!("sh/{middle_text_id}"),
            format!("sh/{third_text_id}")
        ]
    );

    let missing_target = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "inspect".to_string(),
            args: serde_json::json!({
                "include": "textbox",
                "target": {
                    "id": "sh/missing",
                    "before_lines": 1,
                    "after_lines": 1
                }
            }),
        },
        temp_dir.path(),
    )?;
    let missing_target_records = parse_ndjson_lines(
        missing_target
            .inspect_ndjson
            .as_deref()
            .expect("missing target inspect"),
    )?;
    assert_eq!(
        missing_target_records,
        vec![serde_json::json!({
            "kind": "notice",
            "noticeType": "targetNotFound",
            "target": {
                "id": "sh/missing",
                "beforeLines": 1,
                "afterLines": 1
            },
            "message": "No inspect records matched target `sh/missing`."
        })]
    );

    let resolved_shape = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(missing_target.artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sh/{shape_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved_shape
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("textPreview"))
            .and_then(serde_json::Value::as_str),
        Some("Detailed | Shape KPI")
    );
    assert_eq!(
        resolved_shape
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("textChars"))
            .and_then(serde_json::Value::as_u64),
        Some(18)
    );
    assert_eq!(
        resolved_shape
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("textLines"))
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );

    let resolved_notes = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(missing_target.artifact_id),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("nt/{slide_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved_notes
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("textPreview"))
            .and_then(serde_json::Value::as_str),
        Some("Speaker note")
    );
    assert_eq!(
        resolved_notes
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("textChars"))
            .and_then(serde_json::Value::as_u64),
        Some(12)
    );
    assert_eq!(
        resolved_notes
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("textLines"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
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
                "rotation": 12,
                "flip_horizontal": true,
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
    assert_eq!(
        record.get("rotation").and_then(serde_json::Value::as_i64),
        Some(12)
    );
    assert_eq!(
        record
            .get("flipHorizontal")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        record
            .get("flipVertical")
            .and_then(serde_json::Value::as_bool),
        Some(false)
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
fn shapes_support_stroke_dash_styles() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Shape Strokes" }),
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
    let added_shape = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "add_shape".to_string(),
            args: serde_json::json!({
                "slide_index": 0,
                "geometry": "rectangle",
                "position": {
                    "left": 24,
                    "top": 24,
                    "width": 180,
                    "height": 120,
                    "rotation": 15,
                    "flip_horizontal": true,
                    "flip_vertical": true
                },
                "stroke": { "color": "#ff0000", "width": 2, "style": "dash-dot" }
            }),
        },
        temp_dir.path(),
    )?;
    let shape_id = added_shape
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .and_then(|slide| slide.element_ids.first())
        .cloned()
        .expect("shape id");

    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sh/{shape_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("stroke"))
            .and_then(|stroke| stroke.get("style"))
            .and_then(serde_json::Value::as_str),
        Some("dash-dot")
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("rotation"))
            .and_then(serde_json::Value::as_i64),
        Some(15)
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("flipHorizontal"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("flipVertical"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );

    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "update_shape_style".to_string(),
            args: serde_json::json!({
                "element_id": format!("sh/{shape_id}"),
                "position": {
                    "rotation": 30,
                    "flip_horizontal": false,
                    "flip_vertical": true
                }
            }),
        },
        temp_dir.path(),
    )?;
    let updated = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("sh/{shape_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        updated
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("rotation"))
            .and_then(serde_json::Value::as_i64),
        Some(30)
    );
    assert_eq!(
        updated
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("flipHorizontal"))
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    let export_path = temp_dir.path().join("shape-strokes.pptx");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;

    let slide_xml = zip_entry_text(
        &temp_dir.path().join("shape-strokes.pptx"),
        "ppt/slides/slide1.xml",
    )?;
    assert!(slide_xml.contains(r#"<a:prstDash val="dashDot"/>"#));
    assert!(slide_xml.contains(r#"<a:xfrm rot="1800000" flipH="0" flipV="1">"#));
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
                "column_widths": [90, 150],
                "row_heights": [40, 80],
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
    let resolved = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "resolve".to_string(),
            args: serde_json::json!({ "id": format!("tb/{table_id}") }),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("columnWidths"))
            .and_then(serde_json::Value::as_array)
            .map(|widths| widths
                .iter()
                .filter_map(serde_json::Value::as_u64)
                .collect::<Vec<_>>()),
        Some(vec![90, 150])
    );
    assert_eq!(
        resolved
            .resolved_record
            .as_ref()
            .and_then(|record| record.get("rowHeights"))
            .and_then(serde_json::Value::as_array)
            .map(|heights| heights
                .iter()
                .filter_map(serde_json::Value::as_u64)
                .collect::<Vec<_>>()),
        Some(vec![40, 80])
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
    let export_path = temp_dir.path().join("tables.pptx");
    manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(merged.artifact_id),
            action: "export_pptx".to_string(),
            args: serde_json::json!({ "path": export_path }),
        },
        temp_dir.path(),
    )?;
    let slide_xml = zip_entry_text(
        &temp_dir.path().join("tables.pptx"),
        "ppt/slides/slide1.xml",
    )?;
    assert!(
        slide_xml.contains(r#"<a:gridCol w="1143000"/>"#),
        "{slide_xml}"
    );
    assert!(slide_xml.contains(r#"<a:gridCol w="1905000"/>"#));
    assert!(slide_xml.contains(r#"<a:tr h="508000">"#));
    assert!(slide_xml.contains(r#"<a:tr h="1016000">"#));
    Ok(())
}

#[test]
fn history_can_undo_and_redo_created_artifact() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "History" }),
        },
        temp_dir.path(),
    )?;

    let undone = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id.clone()),
            action: "undo".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(undone.artifact_id, created.artifact_id);
    assert!(undone.artifact_snapshot.is_none());
    assert!(
        manager
            .execute(
                PresentationArtifactRequest {
                    artifact_id: Some(created.artifact_id.clone()),
                    action: "get_summary".to_string(),
                    args: serde_json::json!({}),
                },
                temp_dir.path(),
            )
            .is_err()
    );

    let redone = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(created.artifact_id.clone()),
            action: "redo".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(redone.artifact_id, created.artifact_id);
    assert_eq!(
        redone
            .artifact_snapshot
            .as_ref()
            .map(|snapshot| snapshot.slide_count),
        Some(0)
    );
    Ok(())
}

#[test]
fn proto_and_patch_actions_work_and_patch_history_is_atomic()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut manager = PresentationArtifactManager::default();
    let created = manager.execute(
        PresentationArtifactRequest {
            artifact_id: None,
            action: "create".to_string(),
            args: serde_json::json!({ "name": "Proto Patch" }),
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

    let patch_ops = serde_json::json!([
        {
            "action": "add_text_shape",
            "args": {
                "slide_index": 0,
                "text": "Patch text",
                "position": { "left": 40, "top": 60, "width": 180, "height": 50 }
            }
        },
        {
            "action": "set_slide_background",
            "args": {
                "slide_index": 0,
                "fill": "#ffeecc"
            }
        }
    ]);
    let recorded = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "record_patch".to_string(),
            args: serde_json::json!({ "operations": patch_ops }),
        },
        temp_dir.path(),
    )?;
    let expected_patch = serde_json::json!({
        "version": 1,
        "artifactId": artifact_id,
        "operations": [
            {
                "action": "add_text_shape",
                "args": {
                    "slide_index": 0,
                    "text": "Patch text",
                    "position": { "left": 40, "top": 60, "width": 180, "height": 50 }
                }
            },
            {
                "action": "set_slide_background",
                "args": {
                    "slide_index": 0,
                    "fill": "#ffeecc"
                }
            }
        ]
    });
    assert_eq!(recorded.patch.as_ref(), Some(&expected_patch));

    let applied = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "apply_patch".to_string(),
            args: serde_json::json!({ "patch": expected_patch }),
        },
        temp_dir.path(),
    )?;
    let slide_snapshot = applied
        .artifact_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.slides.first())
        .cloned()
        .expect("slide snapshot");
    let slide_id = slide_snapshot.slide_id.clone();
    let element_id = slide_snapshot
        .element_ids
        .first()
        .cloned()
        .expect("element id");

    let proto = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "to_proto".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        proto.proto_json,
        Some(serde_json::json!({
            "kind": "presentation",
            "artifactId": artifact_id,
            "anchor": format!("pr/{artifact_id}"),
            "name": "Proto Patch",
            "slideSize": {
                "left": 0,
                "top": 0,
                "width": 720,
                "height": 540,
                "unit": "points"
            },
            "activeSlideIndex": 0,
            "activeSlideId": slide_id,
            "theme": {
                "colorScheme": {},
                "hexColorMap": {},
                "majorFont": serde_json::Value::Null,
                "minorFont": serde_json::Value::Null
            },
            "styles": [
                {
                    "kind": "textStyle",
                    "id": "st/body",
                    "name": "body",
                    "builtIn": true,
                    "style": {
                        "styleName": "body",
                        "fontSize": 14,
                        "fontFamily": serde_json::Value::Null,
                        "color": serde_json::Value::Null,
                        "alignment": "left",
                        "bold": false,
                        "italic": false,
                        "underline": false
                    }
                },
                {
                    "kind": "textStyle",
                    "id": "st/heading1",
                    "name": "heading1",
                    "builtIn": true,
                    "style": {
                        "styleName": "heading1",
                        "fontSize": 22,
                        "fontFamily": serde_json::Value::Null,
                        "color": serde_json::Value::Null,
                        "alignment": "left",
                        "bold": true,
                        "italic": false,
                        "underline": false
                    }
                },
                {
                    "kind": "textStyle",
                    "id": "st/list",
                    "name": "list",
                    "builtIn": true,
                    "style": {
                        "styleName": "list",
                        "fontSize": 14,
                        "fontFamily": serde_json::Value::Null,
                        "color": serde_json::Value::Null,
                        "alignment": "left",
                        "bold": false,
                        "italic": false,
                        "underline": false
                    }
                },
                {
                    "kind": "textStyle",
                    "id": "st/numberedlist",
                    "name": "numberedlist",
                    "builtIn": true,
                    "style": {
                        "styleName": "numberedlist",
                        "fontSize": 14,
                        "fontFamily": serde_json::Value::Null,
                        "color": serde_json::Value::Null,
                        "alignment": "left",
                        "bold": false,
                        "italic": false,
                        "underline": false
                    }
                },
                {
                    "kind": "textStyle",
                    "id": "st/title",
                    "name": "title",
                    "builtIn": true,
                    "style": {
                        "styleName": "title",
                        "fontSize": 28,
                        "fontFamily": serde_json::Value::Null,
                        "color": serde_json::Value::Null,
                        "alignment": "left",
                        "bold": true,
                        "italic": false,
                        "underline": false
                    }
                }
            ],
            "masters": [],
            "layouts": [],
            "slides": [
                {
                    "slideId": slide_id,
                    "anchor": format!("sl/{slide_id}"),
                    "index": 0,
                    "layoutId": serde_json::Value::Null,
                    "backgroundFill": "FFEECC",
                    "notes": {
                        "anchor": format!("nt/{slide_id}"),
                        "text": "",
                        "visible": true,
                        "textPreview": "",
                        "textChars": 0,
                        "textLines": 0
                    },
                    "elements": [
                        {
                            "kind": "text",
                            "elementId": element_id,
                            "anchor": format!("sh/{element_id}"),
                            "frame": {
                                "left": 40,
                                "top": 60,
                                "width": 180,
                                "height": 50,
                                "unit": "points"
                            },
                            "text": "Patch text",
                            "textPreview": "Patch text",
                            "textChars": 10,
                            "textLines": 1,
                            "fill": serde_json::Value::Null,
                            "style": {
                                "styleName": serde_json::Value::Null,
                                "fontSize": serde_json::Value::Null,
                                "fontFamily": serde_json::Value::Null,
                                "color": serde_json::Value::Null,
                                "alignment": serde_json::Value::Null,
                                "bold": false,
                                "italic": false,
                                "underline": false
                            },
                            "zOrder": 0
                        }
                    ]
                }
            ]
        }))
    );

    let undone = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "undo".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        undone
            .artifact_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.slides.first())
            .map(|slide| slide.element_ids.clone()),
        Some(Vec::new())
    );
    let undone_proto = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "to_proto".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(
        undone_proto
            .proto_json
            .as_ref()
            .and_then(|proto| proto.get("slides"))
            .and_then(serde_json::Value::as_array)
            .and_then(|slides| slides.first())
            .and_then(|slide| slide.get("backgroundFill")),
        Some(&serde_json::Value::Null)
    );
    assert_eq!(
        undone_proto
            .proto_json
            .as_ref()
            .and_then(|proto| proto.get("slides"))
            .and_then(serde_json::Value::as_array)
            .and_then(|slides| slides.first())
            .and_then(|slide| slide.get("elements"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let redone = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id.clone()),
            action: "redo".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(redone.patch, None);
    let redone_proto = manager.execute(
        PresentationArtifactRequest {
            artifact_id: Some(artifact_id),
            action: "to_proto".to_string(),
            args: serde_json::json!({}),
        },
        temp_dir.path(),
    )?;
    assert_eq!(redone_proto.proto_json, proto.proto_json);
    Ok(())
}
