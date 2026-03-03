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
            is_active: document.active_slide_index == Some(index),
            notes: (slide.notes.visible && !slide.notes.text.is_empty())
                .then(|| slide.notes.text.clone()),
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
    let Some(payload) = image.payload.as_ref() else {
        return (
            image.frame.left,
            image.frame.top,
            image.frame.width,
            image.frame.height,
            None,
        );
    };
    let frame = image.frame;
    let source_width = payload.width_px as f64;
    let source_height = payload.height_px as f64;
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

fn normalize_image_crop(
    crop: ImageCropArgs,
    action: &str,
) -> Result<ImageCrop, PresentationArtifactError> {
    for (name, value) in [
        ("left", crop.left),
        ("top", crop.top),
        ("right", crop.right),
        ("bottom", crop.bottom),
    ] {
        if !(0.0..=1.0).contains(&value) {
            return Err(PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: format!("image crop `{name}` must be between 0.0 and 1.0"),
            });
        }
    }
    Ok((crop.left, crop.top, crop.right, crop.bottom))
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

fn load_image_payload_from_blob(
    blob: &str,
    action: &str,
) -> Result<ImagePayload, PresentationArtifactError> {
    let bytes = BASE64_STANDARD.decode(blob.trim()).map_err(|error| {
        PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to decode image blob: {error}"),
        }
    })?;
    let extension = image::guess_format(&bytes)
        .ok()
        .map(image_extension_from_format)
        .unwrap_or("png");
    build_image_payload(bytes, format!("image.{extension}"), action)
}

fn load_image_payload_from_uri(
    uri: &str,
    action: &str,
) -> Result<ImagePayload, PresentationArtifactError> {
    let response =
        reqwest::blocking::get(uri).map_err(|error| PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to fetch image `{uri}`: {error}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to fetch image `{uri}`: HTTP {status}"),
        });
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string());
    let bytes = response
        .bytes()
        .map_err(|error| PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("failed to read image `{uri}`: {error}"),
        })?;
    build_image_payload(
        bytes.to_vec(),
        infer_remote_image_filename(uri, content_type.as_deref()),
        action,
    )
}

fn infer_remote_image_filename(uri: &str, content_type: Option<&str>) -> String {
    let path_name = reqwest::Url::parse(uri)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(Iterator::last)
                .map(str::to_owned)
        })
        .filter(|segment| !segment.is_empty());
    match (path_name, content_type) {
        (Some(path_name), _) if Path::new(&path_name).extension().is_some() => path_name,
        (Some(path_name), Some(content_type)) => {
            format!("{path_name}.{}", image_extension_from_mime(content_type))
        }
        (Some(path_name), None) => path_name,
        (None, Some(content_type)) => format!("image.{}", image_extension_from_mime(content_type)),
        (None, None) => "image.png".to_string(),
    }
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

fn image_extension_from_format(format: image::ImageFormat) -> &'static str {
    match format {
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::Gif => "gif",
        image::ImageFormat::WebP => "webp",
        image::ImageFormat::Bmp => "bmp",
        image::ImageFormat::Tiff => "tiff",
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
