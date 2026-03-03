const CODEX_METADATA_ENTRY: &str = "ppt/codex-document.json";

fn import_codex_metadata_document(path: &Path) -> Result<Option<PresentationDocument>, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut entry = match archive.by_name(CODEX_METADATA_ENTRY) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn build_pptx_bytes(document: &PresentationDocument, action: &str) -> Result<Vec<u8>, String> {
    let bytes = document
        .to_ppt_rs()
        .build()
        .map_err(|error| format!("{action}: {error}"))?;
    patch_pptx_package(bytes, document).map_err(|error| format!("{action}: {error}"))
}

struct SlideImageAsset {
    xml: String,
    relationship_xml: String,
    media_path: String,
    media_bytes: Vec<u8>,
    extension: String,
}

fn normalized_image_extension(format: &str) -> String {
    match format.to_ascii_lowercase().as_str() {
        "jpeg" => "jpg".to_string(),
        other => other.to_string(),
    }
}

fn image_relationship_xml(relationship_id: &str, target: &str) -> String {
    format!(
        r#"<Relationship Id="{relationship_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{}"/>"#,
        ppt_rs::escape_xml(target)
    )
}

fn image_picture_xml(
    image: &ImageElement,
    shape_id: usize,
    relationship_id: &str,
    frame: Rect,
    crop: Option<ImageCrop>,
) -> String {
    let blip_fill = if let Some((crop_left, crop_top, crop_right, crop_bottom)) = crop {
        format!(
            r#"<p:blipFill>
<a:blip r:embed="{relationship_id}"/>
<a:srcRect l="{}" t="{}" r="{}" b="{}"/>
<a:stretch>
<a:fillRect/>
</a:stretch>
</p:blipFill>"#,
            (crop_left * 100_000.0).round() as u32,
            (crop_top * 100_000.0).round() as u32,
            (crop_right * 100_000.0).round() as u32,
            (crop_bottom * 100_000.0).round() as u32,
        )
    } else {
        format!(
            r#"<p:blipFill>
<a:blip r:embed="{relationship_id}"/>
<a:stretch>
<a:fillRect/>
</a:stretch>
</p:blipFill>"#
        )
    };
    let descr = image
        .alt_text
        .as_deref()
        .map(|alt| format!(r#" descr="{}""#, ppt_rs::escape_xml(alt)))
        .unwrap_or_default();
    let no_change_aspect = if image.lock_aspect_ratio { 1 } else { 0 };
    let rotation = image
        .rotation_degrees
        .map(|rotation| format!(r#" rot="{}""#, i64::from(rotation) * 60_000))
        .unwrap_or_default();
    let flip_horizontal = if image.flip_horizontal {
        r#" flipH="1""#
    } else {
        ""
    };
    let flip_vertical = if image.flip_vertical {
        r#" flipV="1""#
    } else {
        ""
    };
    format!(
        r#"<p:pic>
<p:nvPicPr>
<p:cNvPr id="{shape_id}" name="Picture {shape_id}"{descr}/>
<p:cNvPicPr>
<a:picLocks noChangeAspect="{no_change_aspect}"/>
</p:cNvPicPr>
<p:nvPr/>
</p:nvPicPr>
{blip_fill}
<p:spPr>
<a:xfrm{rotation}{flip_horizontal}{flip_vertical}>
<a:off x="{}" y="{}"/>
<a:ext cx="{}" cy="{}"/>
</a:xfrm>
<a:prstGeom prst="rect">
<a:avLst/>
</a:prstGeom>
</p:spPr>
</p:pic>"#,
        points_to_emu(frame.left),
        points_to_emu(frame.top),
        points_to_emu(frame.width),
        points_to_emu(frame.height),
    )
}

fn slide_image_assets(
    slide: &PresentationSlide,
    next_media_index: &mut usize,
) -> Vec<SlideImageAsset> {
    let mut ordered = slide.elements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|element| element.z_order());
    let shape_count = ordered
        .iter()
        .filter(|element| {
            matches!(
                element,
                PresentationElement::Text(_)
                    | PresentationElement::Shape(_)
                    | PresentationElement::Image(ImageElement { payload: None, .. })
            )
        })
        .count()
        + usize::from(slide.background_fill.is_some());
    let mut image_index = 0_usize;
    let mut assets = Vec::new();
    for element in ordered {
        let PresentationElement::Image(image) = element else {
            continue;
        };
        let Some(payload) = &image.payload else {
            continue;
        };
        let (left, top, width, height, fitted_crop) = if image.fit_mode != ImageFitMode::Stretch {
            fit_image(image)
        } else {
            (
                image.frame.left,
                image.frame.top,
                image.frame.width,
                image.frame.height,
                None,
            )
        };
        image_index += 1;
        let relationship_id = format!("rIdImage{image_index}");
        let extension = normalized_image_extension(&payload.format);
        let media_name = format!("image{next_media_index}.{extension}");
        *next_media_index += 1;
        assets.push(SlideImageAsset {
            xml: image_picture_xml(
                image,
                20 + shape_count + image_index - 1,
                &relationship_id,
                Rect {
                    left,
                    top,
                    width,
                    height,
                },
                image.crop.or(fitted_crop),
            ),
            relationship_xml: image_relationship_xml(
                &relationship_id,
                &format!("../media/{media_name}"),
            ),
            media_path: format!("ppt/media/{media_name}"),
            media_bytes: payload.bytes.clone(),
            extension,
        });
    }
    assets
}

fn patch_pptx_package(
    source_bytes: Vec<u8>,
    document: &PresentationDocument,
) -> Result<Vec<u8>, String> {
    let mut archive =
        ZipArchive::new(Cursor::new(source_bytes)).map_err(|error| error.to_string())?;
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let mut next_media_index = 1_usize;
    let mut pending_slide_relationships = HashMap::new();
    let mut pending_slide_images = HashMap::new();
    let mut pending_media = Vec::new();
    let mut image_extensions = BTreeSet::new();
    for (slide_index, slide) in document.slides.iter().enumerate() {
        let slide_number = slide_index + 1;
        let images = slide_image_assets(slide, &mut next_media_index);
        let mut relationships = slide_hyperlink_relationships(slide);
        relationships.extend(images.iter().map(|image| image.relationship_xml.clone()));
        if !relationships.is_empty() {
            pending_slide_relationships.insert(slide_number, relationships);
        }
        if !images.is_empty() {
            image_extensions.extend(images.iter().map(|image| image.extension.clone()));
            pending_media.extend(
                images
                    .iter()
                    .map(|image| (image.media_path.clone(), image.media_bytes.clone())),
            );
            pending_slide_images.insert(slide_number, images);
        }
    }

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        if name == CODEX_METADATA_ENTRY {
            continue;
        }
        let options = file.options();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        writer
            .start_file(&name, options)
            .map_err(|error| error.to_string())?;
        if name == "[Content_Types].xml" {
            writer
                .write_all(update_content_types_xml(bytes, &image_extensions)?.as_bytes())
                .map_err(|error| error.to_string())?;
            continue;
        }
        if name == "ppt/presentation.xml" {
            writer
                .write_all(
                    update_presentation_xml_dimensions(bytes, document.slide_size)?.as_bytes(),
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(slide_number) = parse_slide_xml_path(&name) {
            writer
                .write_all(
                    update_slide_xml(
                        bytes,
                        &document.slides[slide_number - 1],
                        pending_slide_images
                            .get(&slide_number)
                            .map(std::vec::Vec::as_slice)
                            .unwrap_or(&[]),
                    )?
                    .as_bytes(),
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(slide_number) = parse_slide_relationships_path(&name)
            && let Some(relationships) = pending_slide_relationships.remove(&slide_number)
        {
            writer
                .write_all(update_slide_relationships_xml(bytes, &relationships)?.as_bytes())
                .map_err(|error| error.to_string())?;
            continue;
        }
        writer
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
    }

    for (slide_number, relationships) in pending_slide_relationships {
        writer
            .start_file(
                format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
                SimpleFileOptions::default(),
            )
            .map_err(|error| error.to_string())?;
        writer
            .write_all(slide_relationships_xml(&relationships).as_bytes())
            .map_err(|error| error.to_string())?;
    }

    for (path, bytes) in pending_media {
        writer
            .start_file(path, SimpleFileOptions::default())
            .map_err(|error| error.to_string())?;
        writer
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
    }

    writer
        .start_file(CODEX_METADATA_ENTRY, SimpleFileOptions::default())
        .map_err(|error| error.to_string())?;
    writer
        .write_all(
            &serde_json::to_vec(document).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

    writer
        .finish()
        .map_err(|error| error.to_string())
        .map(Cursor::into_inner)
}

fn update_presentation_xml_dimensions(
    existing_bytes: Vec<u8>,
    slide_size: Rect,
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    let updated = replace_self_closing_xml_tag(
        &existing,
        "p:sldSz",
        &format!(
            r#"<p:sldSz cx="{}" cy="{}" type="screen4x3"/>"#,
            points_to_emu(slide_size.width),
            points_to_emu(slide_size.height)
        ),
    )?;
    replace_self_closing_xml_tag(
        &updated,
        "p:notesSz",
        &format!(
            r#"<p:notesSz cx="{}" cy="{}"/>"#,
            points_to_emu(slide_size.height),
            points_to_emu(slide_size.width)
        ),
    )
}

fn replace_self_closing_xml_tag(xml: &str, tag: &str, replacement: &str) -> Result<String, String> {
    let start = xml
        .find(&format!("<{tag} "))
        .ok_or_else(|| format!("presentation xml is missing `<{tag} .../>`"))?;
    let end = xml[start..]
        .find("/>")
        .map(|offset| start + offset + 2)
        .ok_or_else(|| format!("presentation xml tag `{tag}` is not self-closing"))?;
    Ok(format!("{}{replacement}{}", &xml[..start], &xml[end..]))
}

fn slide_hyperlink_relationships(slide: &PresentationSlide) -> Vec<String> {
    let mut ordered = slide.elements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|element| element.z_order());
    let mut hyperlink_index = 1_u32;
    let mut relationships = Vec::new();
    for element in ordered {
        let Some(hyperlink) = (match element {
            PresentationElement::Text(text) => text.hyperlink.as_ref(),
            PresentationElement::Shape(shape) => shape.hyperlink.as_ref(),
            PresentationElement::Connector(_)
            | PresentationElement::Image(_)
            | PresentationElement::Table(_)
            | PresentationElement::Chart(_) => None,
        }) else {
            continue;
        };
        let relationship_id = format!("rIdHyperlink{hyperlink_index}");
        hyperlink_index += 1;
        relationships.push(hyperlink.relationship_xml(&relationship_id));
    }
    relationships
}

fn parse_slide_relationships_path(path: &str) -> Option<usize> {
    path.strip_prefix("ppt/slides/_rels/slide")?
        .strip_suffix(".xml.rels")?
        .parse::<usize>()
        .ok()
}

fn parse_slide_xml_path(path: &str) -> Option<usize> {
    path.strip_prefix("ppt/slides/slide")?
        .strip_suffix(".xml")?
        .parse::<usize>()
        .ok()
}

fn update_slide_relationships_xml(
    existing_bytes: Vec<u8>,
    relationships: &[String],
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    let injected = relationships.join("\n");
    existing
        .contains("</Relationships>")
        .then(|| existing.replace("</Relationships>", &format!("{injected}\n</Relationships>")))
        .ok_or_else(|| {
            "slide relationships xml is missing a closing `</Relationships>`".to_string()
        })
}

fn slide_relationships_xml(relationships: &[String]) -> String {
    let body = relationships.join("\n");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{body}
</Relationships>"#
    )
}

fn update_content_types_xml(
    existing_bytes: Vec<u8>,
    image_extensions: &BTreeSet<String>,
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    if image_extensions.is_empty() {
        return Ok(existing);
    }
    let existing_lower = existing.to_ascii_lowercase();
    let additions = image_extensions
        .iter()
        .filter(|extension| {
            !existing_lower.contains(&format!(
                r#"extension="{}""#,
                extension.to_ascii_lowercase()
            ))
        })
        .map(|extension| generate_image_content_type(extension))
        .collect::<Vec<_>>();
    if additions.is_empty() {
        return Ok(existing);
    }
    existing
        .contains("</Types>")
        .then(|| existing.replace("</Types>", &format!("{}\n</Types>", additions.join("\n"))))
        .ok_or_else(|| "content types xml is missing a closing `</Types>`".to_string())
}

fn update_slide_xml(
    existing_bytes: Vec<u8>,
    slide: &PresentationSlide,
    slide_images: &[SlideImageAsset],
) -> Result<String, String> {
    let existing = String::from_utf8(existing_bytes).map_err(|error| error.to_string())?;
    let existing = replace_image_placeholders(existing, slide_images)?;
    let existing = apply_shape_block_patches(existing, slide)?;
    let table_xml = slide_table_xml(slide);
    if table_xml.is_empty() {
        return Ok(existing);
    }
    existing
        .contains("</p:spTree>")
        .then(|| existing.replace("</p:spTree>", &format!("{table_xml}\n</p:spTree>")))
        .ok_or_else(|| "slide xml is missing a closing `</p:spTree>`".to_string())
}

fn replace_image_placeholders(
    existing: String,
    slide_images: &[SlideImageAsset],
) -> Result<String, String> {
    if slide_images.is_empty() {
        return Ok(existing);
    }
    let mut updated = String::with_capacity(existing.len());
    let mut remaining = existing.as_str();
    for image in slide_images {
        let marker = remaining
            .find("name=\"Image Placeholder: ")
            .ok_or_else(|| {
                "slide xml is missing an image placeholder block for exported images".to_string()
            })?;
        let start = remaining[..marker].rfind("<p:sp>").ok_or_else(|| {
            "slide xml is missing an opening `<p:sp>` for image placeholder".to_string()
        })?;
        let end = remaining[marker..]
            .find("</p:sp>")
            .map(|offset| marker + offset + "</p:sp>".len())
            .ok_or_else(|| {
                "slide xml is missing a closing `</p:sp>` for image placeholder".to_string()
            })?;
        updated.push_str(&remaining[..start]);
        updated.push_str(&image.xml);
        remaining = &remaining[end..];
    }
    updated.push_str(remaining);
    Ok(updated)
}

#[derive(Clone, Copy)]
struct ShapeXmlPatch {
    line_style: Option<LineStyle>,
    flip_horizontal: bool,
    flip_vertical: bool,
}

fn apply_shape_block_patches(
    existing: String,
    slide: &PresentationSlide,
) -> Result<String, String> {
    let mut patches = Vec::new();
    if slide.background_fill.is_some() {
        patches.push(None);
    }
    let mut ordered = slide.elements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|element| element.z_order());
    for element in ordered {
        match element {
            PresentationElement::Text(_) => patches.push(None),
            PresentationElement::Shape(shape) => patches.push(Some(ShapeXmlPatch {
                line_style: shape
                    .stroke
                    .as_ref()
                    .map(|stroke| stroke.style)
                    .filter(|style| *style != LineStyle::Solid),
                flip_horizontal: shape.flip_horizontal,
                flip_vertical: shape.flip_vertical,
            })),
            PresentationElement::Image(ImageElement { payload: None, .. }) => patches.push(None),
            PresentationElement::Connector(_)
            | PresentationElement::Image(_)
            | PresentationElement::Table(_)
            | PresentationElement::Chart(_) => {}
        }
    }
    if patches.iter().all(|patch| {
        patch.is_none_or(|patch| {
            patch.line_style.is_none() && !patch.flip_horizontal && !patch.flip_vertical
        })
    }) {
        return Ok(existing);
    }

    let mut updated = String::with_capacity(existing.len());
    let mut remaining = existing.as_str();
    for patch in patches {
        let Some(start) = remaining.find("<p:sp>") else {
            return Err("slide xml is missing an expected `<p:sp>` block".to_string());
        };
        let end = remaining[start..]
            .find("</p:sp>")
            .map(|offset| start + offset + "</p:sp>".len())
            .ok_or_else(|| "slide xml is missing a closing `</p:sp>` block".to_string())?;
        updated.push_str(&remaining[..start]);
        let block = &remaining[start..end];
        if let Some(patch) = patch {
            updated.push_str(&patch_shape_block(block, patch)?);
        } else {
            updated.push_str(block);
        }
        remaining = &remaining[end..];
    }
    updated.push_str(remaining);
    Ok(updated)
}

fn patch_shape_block(block: &str, patch: ShapeXmlPatch) -> Result<String, String> {
    let block = if let Some(line_style) = patch.line_style {
        patch_shape_block_dash(block, line_style)?
    } else {
        block.to_string()
    };
    if patch.flip_horizontal || patch.flip_vertical {
        patch_shape_block_flip(&block, patch.flip_horizontal, patch.flip_vertical)
    } else {
        Ok(block)
    }
}

fn patch_shape_block_dash(block: &str, line_style: LineStyle) -> Result<String, String> {
    let Some(line_start) = block.find("<a:ln") else {
        return Err("shape block is missing an `<a:ln>` entry for stroke styling".to_string());
    };
    if let Some(dash_start) = block[line_start..].find("<a:prstDash") {
        let dash_start = line_start + dash_start;
        let dash_end = block[dash_start..]
            .find("/>")
            .map(|offset| dash_start + offset + 2)
            .ok_or_else(|| "shape line dash entry is missing a closing `/>`".to_string())?;
        let mut patched = String::with_capacity(block.len() + 32);
        patched.push_str(&block[..dash_start]);
        patched.push_str(&format!(
            r#"<a:prstDash val="{}"/>"#,
            line_style.to_ppt_xml()
        ));
        patched.push_str(&block[dash_end..]);
        return Ok(patched);
    }

    if let Some(line_end) = block[line_start..].find("</a:ln>") {
        let line_end = line_start + line_end;
        let mut patched = String::with_capacity(block.len() + 32);
        patched.push_str(&block[..line_end]);
        patched.push_str(&format!(
            r#"<a:prstDash val="{}"/>"#,
            line_style.to_ppt_xml()
        ));
        patched.push_str(&block[line_end..]);
        return Ok(patched);
    }

    let line_end = block[line_start..]
        .find("/>")
        .map(|offset| line_start + offset + 2)
        .ok_or_else(|| "shape line entry is missing a closing marker".to_string())?;
    let line_tag = &block[line_start..line_end - 2];
    let mut patched = String::with_capacity(block.len() + 48);
    patched.push_str(&block[..line_start]);
    patched.push_str(line_tag);
    patched.push('>');
    patched.push_str(&format!(
        r#"<a:prstDash val="{}"/>"#,
        line_style.to_ppt_xml()
    ));
    patched.push_str("</a:ln>");
    patched.push_str(&block[line_end..]);
    Ok(patched)
}

fn patch_shape_block_flip(
    block: &str,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> Result<String, String> {
    let Some(xfrm_start) = block.find("<a:xfrm") else {
        return Err("shape block is missing an `<a:xfrm>` entry for flip styling".to_string());
    };
    let tag_end = block[xfrm_start..]
        .find('>')
        .map(|offset| xfrm_start + offset)
        .ok_or_else(|| "shape transform entry is missing a closing `>`".to_string())?;
    let tag = &block[xfrm_start..=tag_end];
    let mut patched_tag = tag.to_string();
    patched_tag = upsert_xml_attribute(
        &patched_tag,
        "flipH",
        if flip_horizontal { "1" } else { "0" },
    );
    patched_tag =
        upsert_xml_attribute(&patched_tag, "flipV", if flip_vertical { "1" } else { "0" });
    Ok(format!(
        "{}{}{}",
        &block[..xfrm_start],
        patched_tag,
        &block[tag_end + 1..]
    ))
}

fn upsert_xml_attribute(tag: &str, attribute: &str, value: &str) -> String {
    let needle = format!(r#"{attribute}=""#);
    if let Some(start) = tag.find(&needle) {
        let value_start = start + needle.len();
        if let Some(end_offset) = tag[value_start..].find('"') {
            let end = value_start + end_offset;
            return format!("{}{}{}", &tag[..value_start], value, &tag[end..]);
        }
    }
    let insert_at = tag.len() - 1;
    format!(r#"{} {attribute}="{value}""#, &tag[..insert_at]) + &tag[insert_at..]
}

fn slide_table_xml(slide: &PresentationSlide) -> String {
    let mut ordered = slide.elements.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|element| element.z_order());
    let mut table_index = 0_usize;
    ordered
        .into_iter()
        .filter_map(|element| {
            let PresentationElement::Table(table) = element else {
                return None;
            };
            table_index += 1;
            let rows = table
                .rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(row_index, row)| {
                    let cells = row
                        .into_iter()
                        .enumerate()
                        .map(|(column_index, cell)| {
                            build_table_cell(cell, &table.merges, row_index, column_index)
                        })
                        .collect::<Vec<_>>();
                    let mut table_row = TableRow::new(cells);
                    if let Some(height) = table.row_heights.get(row_index) {
                        table_row = table_row.with_height(points_to_emu(*height));
                    }
                    Some(table_row)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(ppt_rs::generator::table::generate_table_xml(
                &ppt_rs::generator::table::Table::new(
                    rows,
                    table
                        .column_widths
                        .iter()
                        .copied()
                        .map(points_to_emu)
                        .collect(),
                    points_to_emu(table.frame.left),
                    points_to_emu(table.frame.top),
                ),
                300 + table_index,
            ))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_preview_images(
    document: &PresentationDocument,
    output_dir: &Path,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let pptx_path = output_dir.join("preview.pptx");
    let bytes = build_pptx_bytes(document, action).map_err(|message| {
        PresentationArtifactError::ExportFailed {
            path: pptx_path.clone(),
            message,
        }
    })?;
    std::fs::write(&pptx_path, bytes).map_err(|error| PresentationArtifactError::ExportFailed {
        path: pptx_path.clone(),
        message: error.to_string(),
    })?;
    render_pptx_to_pngs(&pptx_path, output_dir, action)
}

fn render_pptx_to_pngs(
    pptx_path: &Path,
    output_dir: &Path,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let soffice_cmd = if cfg!(target_os = "macos")
        && Path::new("/Applications/LibreOffice.app/Contents/MacOS/soffice").exists()
    {
        "/Applications/LibreOffice.app/Contents/MacOS/soffice"
    } else {
        "soffice"
    };
    let conversion = Command::new(soffice_cmd)
        .arg("--headless")
        .arg("--convert-to")
        .arg("pdf")
        .arg(pptx_path)
        .arg("--outdir")
        .arg(output_dir)
        .output()
        .map_err(|error| PresentationArtifactError::ExportFailed {
            path: pptx_path.to_path_buf(),
            message: format!("{action}: failed to execute LibreOffice: {error}"),
        })?;
    if !conversion.status.success() {
        return Err(PresentationArtifactError::ExportFailed {
            path: pptx_path.to_path_buf(),
            message: format!(
                "{action}: LibreOffice conversion failed: {}",
                String::from_utf8_lossy(&conversion.stderr)
            ),
        });
    }

    let pdf_path = output_dir.join(
        pptx_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| format!("{stem}.pdf"))
            .ok_or_else(|| PresentationArtifactError::ExportFailed {
                path: pptx_path.to_path_buf(),
                message: format!("{action}: preview pptx filename is invalid"),
            })?,
    );
    let prefix = output_dir.join("slide");
    let conversion = Command::new("pdftoppm")
        .arg("-png")
        .arg(&pdf_path)
        .arg(&prefix)
        .output()
        .map_err(|error| PresentationArtifactError::ExportFailed {
            path: pdf_path.clone(),
            message: format!("{action}: failed to execute pdftoppm: {error}"),
        })?;
    std::fs::remove_file(&pdf_path).ok();
    if !conversion.status.success() {
        return Err(PresentationArtifactError::ExportFailed {
            path: output_dir.to_path_buf(),
            message: format!(
                "{action}: pdftoppm conversion failed: {}",
                String::from_utf8_lossy(&conversion.stderr)
            ),
        });
    }
    Ok(())
}

pub(crate) fn write_preview_image(
    source_path: &Path,
    target_path: &Path,
    format: PreviewOutputFormat,
    scale: f32,
    quality: u8,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    if matches!(format, PreviewOutputFormat::Png) && scale == 1.0 {
        std::fs::rename(source_path, target_path).map_err(|error| {
            PresentationArtifactError::ExportFailed {
                path: target_path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        return Ok(());
    }
    let mut preview =
        image::open(source_path).map_err(|error| PresentationArtifactError::ExportFailed {
            path: source_path.to_path_buf(),
            message: format!("{action}: {error}"),
        })?;
    if scale != 1.0 {
        let width = (preview.width() as f32 * scale).round().max(1.0) as u32;
        let height = (preview.height() as f32 * scale).round().max(1.0) as u32;
        preview = preview.resize_exact(width, height, FilterType::Lanczos3);
    }
    let file = std::fs::File::create(target_path).map_err(|error| {
        PresentationArtifactError::ExportFailed {
            path: target_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let mut writer = std::io::BufWriter::new(file);
    match format {
        PreviewOutputFormat::Png => {
            preview
                .write_to(&mut writer, ImageFormat::Png)
                .map_err(|error| PresentationArtifactError::ExportFailed {
                    path: target_path.to_path_buf(),
                    message: format!("{action}: {error}"),
                })?
        }
        PreviewOutputFormat::Jpeg => {
            let rgb = preview.to_rgb8();
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
            encoder.encode_image(&rgb).map_err(|error| {
                PresentationArtifactError::ExportFailed {
                    path: target_path.to_path_buf(),
                    message: format!("{action}: {error}"),
                }
            })?;
        }
        PreviewOutputFormat::Svg => {
            let mut png_bytes = Cursor::new(Vec::new());
            preview
                .write_to(&mut png_bytes, ImageFormat::Png)
                .map_err(|error| PresentationArtifactError::ExportFailed {
                    path: target_path.to_path_buf(),
                    message: format!("{action}: {error}"),
                })?;
            let embedded_png = BASE64_STANDARD.encode(png_bytes.into_inner());
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><image href="data:image/png;base64,{embedded_png}" width="{}" height="{}"/></svg>"#,
                preview.width(),
                preview.height(),
                preview.width(),
                preview.height(),
                preview.width(),
                preview.height(),
            );
            writer.write_all(svg.as_bytes()).map_err(|error| {
                PresentationArtifactError::ExportFailed {
                    path: target_path.to_path_buf(),
                    message: format!("{action}: {error}"),
                }
            })?;
        }
    }
    std::fs::remove_file(source_path).ok();
    Ok(())
}

fn collect_pngs(output_dir: &Path) -> Result<Vec<PathBuf>, PresentationArtifactError> {
    let mut files = std::fs::read_dir(output_dir)
        .map_err(|error| PresentationArtifactError::ExportFailed {
            path: output_dir.to_path_buf(),
            message: error.to_string(),
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn parse_preview_output_format(
    format: Option<&str>,
    path: &Path,
    action: &str,
) -> Result<PreviewOutputFormat, PresentationArtifactError> {
    let value = format
        .map(str::to_owned)
        .or_else(|| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "png".to_string());
    match value.to_ascii_lowercase().as_str() {
        "png" => Ok(PreviewOutputFormat::Png),
        "jpg" | "jpeg" => Ok(PreviewOutputFormat::Jpeg),
        "svg" => Ok(PreviewOutputFormat::Svg),
        other => Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("preview format `{other}` is not supported"),
        }),
    }
}

fn normalize_preview_scale(
    scale: Option<f32>,
    action: &str,
) -> Result<f32, PresentationArtifactError> {
    let scale = scale.unwrap_or(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "`scale` must be a positive number".to_string(),
        });
    }
    Ok(scale)
}

fn normalize_preview_quality(
    quality: Option<u8>,
    action: &str,
) -> Result<u8, PresentationArtifactError> {
    let quality = quality.unwrap_or(90);
    if quality == 0 || quality > 100 {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "`quality` must be between 1 and 100".to_string(),
        });
    }
    Ok(quality)
}
